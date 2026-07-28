//! JDBC 字符 Reader 平台对象。
//!
//! 对应 Java 平台对象：`java.io.Reader`。Java Reader 以 UTF-16 code unit
//! 工作，不能直接缩成 Rust UTF-8 `String` 或字节 `Read`；本对象因此把
//! `u16` 序列作为无损驱动边界。

use super::DruidError;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

static NEXT_READER_ID: AtomicU64 = AtomicU64::new(1);

/// 物理字符 Reader SPI。
///
/// 对应 Java：`java.io.Reader#read(char[])` 与 `Reader#close()`。
pub trait PhysicalCharacterReader: fmt::Debug + Send {
    /// 读取 UTF-16 code unit 并推进游标；返回 0 表示流结束。
    fn read_utf16(&mut self, buffer: &mut [u16]) -> Result<usize, DruidError>;

    /// 关闭底层 Reader。
    fn close(&mut self) -> Result<(), DruidError>;
}

#[derive(Debug)]
struct Utf16SliceReader {
    code_units: Vec<u16>,
    position: usize,
}

impl PhysicalCharacterReader for Utf16SliceReader {
    fn read_utf16(&mut self, buffer: &mut [u16]) -> Result<usize, DruidError> {
        let remaining = &self.code_units[self.position..];
        let length = remaining.len().min(buffer.len());
        buffer[..length].copy_from_slice(&remaining[..length]);
        self.position += length;
        Ok(length)
    }

    fn close(&mut self) -> Result<(), DruidError> {
        Ok(())
    }
}

struct JdbcReaderState {
    reader: Option<Box<dyn PhysicalCharacterReader>>,
}

struct JdbcReaderInner {
    id: u64,
    state: Mutex<JdbcReaderState>,
}

/// 可共享的 Java Reader 句柄。
///
/// Clone 保留 Java 引用语义：所有克隆共享 UTF-16 游标和关闭状态。
#[derive(Clone)]
pub struct JdbcReader {
    inner: Arc<JdbcReaderInner>,
}

impl JdbcReader {
    /// 包装物理字符 Reader。
    ///
    /// # 参数
    /// - `reader`：驱动或调用方提供的字符流。
    pub fn new(reader: impl PhysicalCharacterReader + 'static) -> Self {
        Self {
            inner: Arc::new(JdbcReaderInner {
                id: NEXT_READER_ID.fetch_add(1, Ordering::Relaxed),
                state: Mutex::new(JdbcReaderState {
                    reader: Some(Box::new(reader)),
                }),
            }),
        }
    }

    /// 从 Rust 字符串创建 UTF-16 Reader。
    pub fn from_string(value: impl AsRef<str>) -> Self {
        Self::from_utf16(value.as_ref().encode_utf16().collect())
    }

    /// 从原始 UTF-16 code unit 创建 Reader。
    ///
    /// 该入口允许 Adapter 保留 Java Reader 中未配对 surrogate，直到调用方
    /// 明确要求转换为 Rust String。
    pub fn from_utf16(code_units: Vec<u16>) -> Self {
        Self::new(Utf16SliceReader {
            code_units,
            position: 0,
        })
    }

    /// 读取 UTF-16 code unit 并推进共享游标。
    pub fn read_utf16(&self, buffer: &mut [u16]) -> Result<usize, DruidError> {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state
            .reader
            .as_mut()
            .ok_or_else(|| DruidError::DriverError("Reader is closed".to_string()))?
            .read_utf16(buffer)
    }

    /// 从当前游标读取剩余全部 UTF-16 code unit。
    pub fn read_to_end_utf16(&self) -> Result<Vec<u16>, DruidError> {
        let mut result = Vec::new();
        let mut buffer = [0_u16; 1024];
        loop {
            let length = self.read_utf16(&mut buffer)?;
            if length == 0 {
                return Ok(result);
            }
            result.extend_from_slice(&buffer[..length]);
        }
    }

    /// 从当前游标读取并严格转换为 Rust UTF-8 String。
    ///
    /// Java 允许未配对 surrogate；遇到此类内容时返回错误，不进行替换字符式
    /// 有损转换。
    pub fn read_to_string(&self) -> Result<String, DruidError> {
        String::from_utf16(&self.read_to_end_utf16()?).map_err(|error| {
            DruidError::DriverError(format!("Reader contains invalid UTF-16: {error}"))
        })
    }

    /// 关闭 Reader；重复关闭保持幂等。
    pub fn close(&self) -> Result<(), DruidError> {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(mut reader) = state.reader.take() else {
            return Ok(());
        };
        reader.close()
    }

    /// 返回 Reader 是否已经关闭。
    pub fn is_closed(&self) -> bool {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .reader
            .is_none()
    }
}

impl fmt::Debug for JdbcReader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JdbcReader")
            .field("id", &self.inner.id)
            .field("closed", &self.is_closed())
            .finish()
    }
}

impl PartialEq for JdbcReader {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Eq for JdbcReader {}

/// Reader setter 的 Java 长度重载身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JdbcCharacterLength {
    /// 未调用长度重载。
    Unspecified,
    /// Java `int length` 重载。
    Int(i32),
    /// Java `long length` 重载。
    Long(i64),
}
