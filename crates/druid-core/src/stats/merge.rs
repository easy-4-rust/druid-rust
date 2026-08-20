//! 对应 Java 类：com.alibaba.druid.stat.RdbcSqlStat + `SqlMerger`
//!
//! SQL 合并统计：把参数化后的 SQL 模板作为 key，聚合执行统计。

use indexmap::IndexMap;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use super::RdbcSqlStat;

/// 旧内部名称，保留源码兼容；canonical 对象为 [`RdbcSqlStat`]。
pub type MergedSqlStat = RdbcSqlStat;

/// SQL 指纹（xxh3 哈希）。
pub fn fingerprint(sql_template: &str) -> u64 {
    xxhash_rust::xxh3::xxh3_64(sql_template.as_bytes())
}

/// 参数化 SQL 结果。
#[derive(Debug, Clone)]
pub struct ParameterizedSql {
    pub template: String,
    pub fingerprint: u64,
}

/// SQL 词法参数化：只把字符串和数值字面量替换为 `?`。
///
/// 对应 Java `SQLUtils#parameterize` 的统计合并职责。标识符、已有 placeholder、
/// quoted identifier 与注释保持不变；字符串转义和科学计数法作为一个字面量
/// 消费，避免旧实现把 `table1` 错误改成 `table?`。
pub fn parameterize(sql: &str) -> ParameterizedSql {
    let mut template = String::with_capacity(sql.len());
    let bytes = sql.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\'' => {
                if index > 0
                    && matches!(
                        bytes[index - 1],
                        b'n' | b'N' | b'e' | b'E' | b'x' | b'X' | b'b' | b'B'
                    )
                    && is_token_start(bytes, index - 1)
                {
                    template.pop();
                }
                index = consume_quoted(bytes, index, b'\'');
                template.push('?');
            }
            b'"' | b'`' => {
                let end = consume_quoted(bytes, index, bytes[index]);
                template.push_str(&sql[index..end]);
                index = end;
            }
            b'[' => {
                let end = consume_bracket_identifier(bytes, index);
                template.push_str(&sql[index..end]);
                index = end;
            }
            b'-' if bytes.get(index + 1) == Some(&b'-') => {
                let end = consume_line_comment(bytes, index);
                template.push_str(&sql[index..end]);
                index = end;
            }
            b'#' => {
                let end = consume_line_comment(bytes, index);
                template.push_str(&sql[index..end]);
                index = end;
            }
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                let end = consume_block_comment(bytes, index);
                template.push_str(&sql[index..end]);
                index = end;
            }
            byte if byte.is_ascii_digit() && is_token_start(bytes, index) => {
                index = consume_number(bytes, index);
                template.push('?');
            }
            b'.' if bytes.get(index + 1).is_some_and(u8::is_ascii_digit)
                && is_token_start(bytes, index) =>
            {
                index = consume_number(bytes, index);
                template.push('?');
            }
            _ => {
                let ch = sql[index..]
                    .chars()
                    .next()
                    .expect("index is on a UTF-8 boundary");
                template.push(ch);
                index += ch.len_utf8();
            }
        }
    }

    let fp = fingerprint(&template);
    ParameterizedSql {
        template,
        fingerprint: fp,
    }
}

fn consume_quoted(bytes: &[u8], mut index: usize, quote: u8) -> usize {
    index += 1;
    while index < bytes.len() {
        if bytes[index] == quote {
            if bytes.get(index + 1) == Some(&quote) {
                index += 2;
                continue;
            }
            return index + 1;
        }
        if bytes[index] == b'\\' && index + 1 < bytes.len() {
            index += 2;
        } else {
            index += 1;
        }
    }
    bytes.len()
}

fn consume_bracket_identifier(bytes: &[u8], mut index: usize) -> usize {
    index += 1;
    while index < bytes.len() {
        if bytes[index] == b']' {
            if bytes.get(index + 1) == Some(&b']') {
                index += 2;
                continue;
            }
            return index + 1;
        }
        index += 1;
    }
    bytes.len()
}

fn consume_line_comment(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && !matches!(bytes[index], b'\n' | b'\r') {
        index += 1;
    }
    index
}

fn consume_block_comment(bytes: &[u8], mut index: usize) -> usize {
    index += 2;
    while index + 1 < bytes.len() {
        if bytes[index] == b'*' && bytes[index + 1] == b'/' {
            return index + 2;
        }
        index += 1;
    }
    bytes.len()
}

fn is_token_start(bytes: &[u8], index: usize) -> bool {
    index == 0
        || bytes[index - 1].is_ascii()
            && !bytes[index - 1].is_ascii_alphanumeric()
            && !matches!(bytes[index - 1], b'_' | b'$')
}

fn consume_number(bytes: &[u8], mut index: usize) -> usize {
    if bytes[index] == b'.' {
        index += 1;
    }
    if bytes.get(index) == Some(&b'0')
        && bytes
            .get(index + 1)
            .is_some_and(|byte| matches!(byte, b'x' | b'X' | b'b' | b'B'))
    {
        index += 2;
        while bytes
            .get(index)
            .is_some_and(|byte| byte.is_ascii_hexdigit() || *byte == b'_')
        {
            index += 1;
        }
        return index;
    }
    while bytes
        .get(index)
        .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'_')
    {
        index += 1;
    }
    if bytes.get(index) == Some(&b'.') {
        index += 1;
        while bytes
            .get(index)
            .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'_')
        {
            index += 1;
        }
    }
    if bytes
        .get(index)
        .is_some_and(|byte| matches!(byte, b'e' | b'E'))
    {
        let exponent = index;
        index += 1;
        if bytes
            .get(index)
            .is_some_and(|byte| matches!(byte, b'+' | b'-'))
        {
            index += 1;
        }
        let digits = index;
        while bytes.get(index).is_some_and(u8::is_ascii_digit) {
            index += 1;
        }
        if digits == index {
            return exponent;
        }
    }
    index
}

/// SQL 合并器。
///
/// 对应 Druid Java 的 `DruidStatService` 中的 SQL 合并逻辑。
pub struct SqlMerger {
    cache: RwLock<IndexMap<u64, Arc<RdbcSqlStat>>>,
    active_sql_stats: RwLock<HashMap<String, u64>>,
    max_sql_size: AtomicI32,
    skip_sql_count: AtomicU64,
}

impl SqlMerger {
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(IndexMap::new()),
            active_sql_stats: RwLock::new(HashMap::new()),
            max_sql_size: AtomicI32::new(1_000),
            skip_sql_count: AtomicU64::new(0),
        }
    }

    /// 记录一条 SQL 执行。
    pub fn record(&self, sql: &str, elapsed: Duration, ok: bool) {
        self.record_with_merge(sql, elapsed, ok, true);
    }

    /// 按 `StatFilter.mergeSql` 配置记录 SQL。
    ///
    /// `merge_sql=false` 时使用原始 SQL 文本及其哈希作为统计键；开启时才执行
    /// Druid 参数化合并。对应 Java：
    /// `StatFilter#createSqlStat(StatementProxy, String)`。
    pub fn record_with_merge(&self, sql: &str, elapsed: Duration, ok: bool, merge_sql: bool) {
        self.record_with_merge_stat(sql, elapsed, ok, merge_sql);
    }

    /// 按配置记录 SQL 并返回本次命中的统计对象。
    ///
    /// 返回对象供 `StatFilter` 在同一次执行后继续累加 update/fetch 行数，
    /// 避免重新计算 SQL key 或在容量淘汰期间命中不同对象。
    pub fn record_with_merge_stat(
        &self,
        sql: &str,
        elapsed: Duration,
        ok: bool,
        merge_sql: bool,
    ) -> Arc<RdbcSqlStat> {
        let stat = self.prepare(sql, merge_sql);
        stat.record(elapsed, ok);
        stat
    }

    /// 在执行前创建或取得 SQL 统计对象，但不增加完成计数。
    pub fn prepare(&self, sql: &str, merge_sql: bool) -> Arc<RdbcSqlStat> {
        let param = sql_key(sql, merge_sql);
        let fingerprint = param.fingerprint;
        let stat = {
            let mut cache = self.cache.write();
            if let Some(stat) = cache.get(&param.fingerprint) {
                Arc::clone(stat)
            } else {
                let stat = Arc::new(RdbcSqlStat::new(param.template, param.fingerprint));
                cache.insert(param.fingerprint, Arc::clone(&stat));
                let max_sql_size = self.max_sql_size.load(Ordering::Acquire);
                if i64::try_from(cache.len()).unwrap_or(i64::MAX) > i64::from(max_sql_size) {
                    if let Some((_, eldest)) = cache.shift_remove_index(0) {
                        let value = eldest.stat_value();
                        if value.running_count > 0 || value.execute_count > 0 {
                            self.skip_sql_count.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
                stat
            }
        };
        // Java StatementProxy 直接持有本次 RdbcSqlStat。Rust 的物理 Statement
        // 与 Filter 上下文解耦，因此保留原始 SQL 到本次统计对象的关联，供
        // ResultSet/CallableStatement 在后续打开 LOB 时更新同一个对象。
        self.active_sql_stats
            .write()
            .insert(sql.to_owned(), fingerprint);
        stat
    }

    /// 获取所有 SQL 统计。
    pub fn all_stats(&self) -> Vec<Arc<RdbcSqlStat>> {
        self.cache.read().values().cloned().collect()
    }

    /// 获取指定指纹的统计。
    pub fn get_stat(&self, fingerprint: u64) -> Option<Arc<RdbcSqlStat>> {
        self.cache.read().get(&fingerprint).cloned()
    }

    /// 返回最近一次为原始 SQL 绑定的统计对象。
    ///
    /// 对应 Java `StatementProxy#getSqlStat()` 的关联语义。若统计对象已因容量
    /// 限制淘汰，则返回 `None`，不会为一次 LOB 读取重新创建 SQL 条目。
    pub fn active_stat_for_sql(&self, sql: &str) -> Option<Arc<RdbcSqlStat>> {
        let fingerprint = self.active_sql_stats.read().get(sql).copied()?;
        self.get_stat(fingerprint)
    }

    /// SQL 模板数量。
    pub fn len(&self) -> usize {
        self.cache.read().len()
    }
    pub fn is_empty(&self) -> bool {
        self.cache.read().is_empty()
    }

    /// 返回 Java `maxSqlSize`。
    pub fn max_sql_size(&self) -> i32 {
        self.max_sql_size.load(Ordering::Acquire)
    }

    /// 设置最大 SQL 条目数，并按 Java 插入顺序删除现有最旧条目。
    pub fn set_max_sql_size(&self, value: i32) {
        let old = self.max_sql_size.swap(value, Ordering::AcqRel);
        if value >= old {
            return;
        }
        let mut cache = self.cache.write();
        // Java 实现删除的是 `oldMax - newMax` 个最旧条目，而不是简单裁剪到
        // `newMax`。当当前条目数远小于 oldMax 时，这个历史行为可能清空整个表。
        let remove_count = i64::from(old).saturating_sub(i64::from(value)).max(0);
        for _ in 0..usize::try_from(remove_count).unwrap_or(usize::MAX) {
            if cache.shift_remove_index(0).is_none() {
                break;
            }
        }
    }

    /// 返回被容量淘汰的已执行 SQL 数。
    pub fn skip_sql_count(&self) -> u64 {
        self.skip_sql_count.load(Ordering::Acquire)
    }

    /// 原子取得并重置被容量淘汰的已执行 SQL 数。
    pub(crate) fn take_skip_sql_count(&self) -> u64 {
        self.skip_sql_count.swap(0, Ordering::AcqRel)
    }

    /// 按 Java `RdbcDataSourceStat#reset()` 重置 SQL 聚合项。
    ///
    /// 从未成功执行且当前未运行的条目被移除；其余条目保留 SQL 身份并清零区间
    /// 统计。不能直接清空 cache，否则管理端 reset 后会丢失活跃 SQL 对象。
    pub fn reset(&self) {
        let mut cache = self.cache.write();
        cache.retain(|_, stat| {
            if stat.execute_count() == 0 && stat.running_count.load(Ordering::Acquire) == 0 {
                false
            } else {
                stat.reset();
                true
            }
        });
        self.skip_sql_count.store(0, Ordering::Release);
    }
}

fn sql_key(sql: &str, merge_sql: bool) -> ParameterizedSql {
    if merge_sql {
        parameterize(sql)
    } else {
        ParameterizedSql {
            template: sql.to_owned(),
            fingerprint: fingerprint(sql),
        }
    }
}

impl Default for SqlMerger {
    fn default() -> Self {
        Self::new()
    }
}
