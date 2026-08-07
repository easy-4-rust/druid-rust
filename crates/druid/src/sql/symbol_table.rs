//! Druid lexer 字符串符号表。

use crate::core::RdbcString;
use std::sync::{Arc, LazyLock, Mutex};

/// 进程级默认符号表。
///
/// 对应 Java `SymbolTable.global = new SymbolTable(32768)`。Java 对全局对象的
/// 写入没有同步保护；Rust 使用 Mutex 消除数据竞争，但不改变 bucket/碰撞语义。
pub static GLOBAL_SYMBOL_TABLE: LazyLock<Mutex<SymbolTable>> =
    LazyLock::new(|| Mutex::new(SymbolTable::new(32_768)));

#[derive(Debug, Clone)]
struct SymbolTableEntry {
    hash: i64,
    #[allow(dead_code)]
    len: i32,
    value: Arc<RdbcString>,
}

/// Lexer 使用的单槽 bucket 符号表。
///
/// 对应 Java：`com.alibaba.druid.sql.parser.SymbolTable`。每个 bucket 只缓存首个
/// symbol：hash 相等直接返回缓存对象，不比较文本；hash 冲突则返回新字符串且
/// 不覆盖缓存。
#[derive(Debug)]
pub struct SymbolTable {
    entries: Vec<Option<SymbolTableEntry>>,
    index_mask: i32,
}

impl SymbolTable {
    /// 创建指定 bucket 数量的符号表。
    ///
    /// Java 使用 `tableSize - 1` 作为 bit mask，不校验 2 的幂；Rust保持该规则。
    #[must_use]
    pub fn new(table_size: usize) -> Self {
        Self {
            index_mask: table_size.wrapping_sub(1) as i32,
            entries: vec![None; table_size],
        }
    }

    /// 从 Java String 的 UTF-16 子区间添加 symbol。
    ///
    /// offset/len 单位为 Java char；非法边界和 Java 一样在调用点失败。
    pub fn add_symbol(
        &mut self,
        buffer: &RdbcString,
        offset: i32,
        len: i32,
        hash: i64,
    ) -> Arc<RdbcString> {
        let bucket = self.bucket(hash);
        if let Some(entry) = &self.entries[bucket] {
            if hash == entry.hash {
                return Arc::clone(&entry.value);
            }
            return Arc::new(Self::substring(buffer, offset, len));
        }

        let value = Arc::new(Self::substring(buffer, offset, len));
        self.entries[bucket] = Some(SymbolTableEntry {
            hash,
            len,
            value: Arc::clone(&value),
        });
        value
    }

    /// 从 UTF-8 byte 子区间添加 symbol。
    ///
    /// Java `new String(bytes, UTF_8)` 对非法序列使用 replacement character，
    /// 因此这里使用 `from_utf8_lossy` 而不是返回 UTF-8 错误。
    pub fn add_symbol_bytes(
        &mut self,
        buffer: &[u8],
        offset: i32,
        len: i32,
        hash: i64,
    ) -> Arc<RdbcString> {
        let bucket = self.bucket(hash);
        if let Some(entry) = &self.entries[bucket] {
            if hash == entry.hash {
                return Arc::clone(&entry.value);
            }
            return Arc::new(Self::substring_bytes(buffer, offset, len));
        }

        let value = Arc::new(Self::substring_bytes(buffer, offset, len));
        self.entries[bucket] = Some(SymbolTableEntry {
            hash,
            len,
            value: Arc::clone(&value),
        });
        value
    }

    /// 添加已经构造的完整 symbol。
    ///
    /// 未命中 bucket 时缓存并返回输入的同一 Arc；发生不同 hash 的 bucket 冲突
    /// 时仍返回输入 Arc，但不覆盖旧 entry。
    pub fn add_symbol_value(&mut self, symbol: Arc<RdbcString>, hash: i64) -> Arc<RdbcString> {
        let bucket = self.bucket(hash);
        if let Some(entry) = &self.entries[bucket] {
            if hash == entry.hash {
                return Arc::clone(&entry.value);
            }
            return symbol;
        }

        self.entries[bucket] = Some(SymbolTableEntry {
            hash,
            len: i32::try_from(symbol.len()).unwrap_or(i32::MAX),
            value: Arc::clone(&symbol),
        });
        symbol
    }

    /// 按 hash 查找缓存 symbol。
    #[must_use]
    pub fn find_symbol(&self, hash: i64) -> Option<Arc<RdbcString>> {
        self.entries[self.bucket(hash)]
            .as_ref()
            .filter(|entry| entry.hash == hash)
            .map(|entry| Arc::clone(&entry.value))
    }

    fn bucket(&self, hash: i64) -> usize {
        ((hash as i32) & self.index_mask) as usize
    }

    fn substring(buffer: &RdbcString, offset: i32, len: i32) -> RdbcString {
        let offset = usize::try_from(offset).expect("negative Java String offset");
        let len = usize::try_from(len).expect("negative Java String length");
        let end = offset.checked_add(len).expect("Java String range overflow");
        RdbcString::from_utf16(buffer.as_utf16()[offset..end].to_vec())
    }

    fn substring_bytes(buffer: &[u8], offset: i32, len: i32) -> RdbcString {
        let offset = usize::try_from(offset).expect("negative Java byte array offset");
        let len = usize::try_from(len).expect("negative Java byte array length");
        let end = offset
            .checked_add(len)
            .expect("Java byte array range overflow");
        RdbcString::from_rust_str(String::from_utf8_lossy(&buffer[offset..end]).as_ref())
    }
}
