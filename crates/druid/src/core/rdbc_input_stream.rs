//! RDBC 输入流平台对象。
//!
//! 对应 Java 平台对象：`java.io.InputStream`。该对象用于保持
//! `CallableStatement#setBlob(String, InputStream[, long])` 的流身份、
//! 当前读取位置和关闭状态，禁止在池化层提前物化为字节数组。

use super::DruidError;
use std::fmt;
use std::io::{Cursor, Read};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

static NEXT_INPUT_STREAM_ID: AtomicU64 = AtomicU64::new(1);

struct RdbcInputStreamState {
    reader: Option<Box<dyn Read + Send>>,
}

struct RdbcInputStreamInner {
    id: u64,
    state: Mutex<RdbcInputStreamState>,
}

/// 可共享的 RDBC 输入流句柄。
///
/// Clone 只克隆句柄，所有克隆共享同一个读取游标和关闭状态，对应 Java
/// `InputStream` 对象引用语义。
#[derive(Clone)]
pub struct RdbcInputStream {
    inner: Arc<RdbcInputStreamInner>,
}

impl RdbcInputStream {
    /// 包装一个真实输入流。
    ///
    /// # 参数
    /// - `reader`：由驱动或调用方提供的流；池化层不会提前读取。
    pub fn new(reader: impl Read + Send + 'static) -> Self {
        Self {
            inner: Arc::new(RdbcInputStreamInner {
                id: NEXT_INPUT_STREAM_ID.fetch_add(1, Ordering::Relaxed),
                state: Mutex::new(RdbcInputStreamState {
                    reader: Some(Box::new(reader)),
                }),
            }),
        }
    }

    /// 从字节创建有状态输入流，主要用于 Adapter 和契约测试。
    pub fn from_bytes(bytes: impl Into<Vec<u8>>) -> Self {
        Self::new(Cursor::new(bytes.into()))
    }

    /// 读取数据并推进共享游标。
    ///
    /// # 参数
    /// - `buffer`：接收本次读取内容的缓冲区。
    ///
    /// # 返回
    /// 实际读取字节数；流已关闭或底层读取失败时返回错误。
    pub fn read(&self, buffer: &mut [u8]) -> Result<usize, DruidError> {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let reader = state
            .reader
            .as_mut()
            .ok_or_else(|| DruidError::DriverError("InputStream is closed".to_string()))?;
        reader
            .read(buffer)
            .map_err(|error| DruidError::DriverError(format!("InputStream read failed: {error}")))
    }

    /// 从当前共享游标读取到流末尾。
    pub fn read_to_end(&self) -> Result<Vec<u8>, DruidError> {
        let mut bytes = Vec::new();
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let reader = state
            .reader
            .as_mut()
            .ok_or_else(|| DruidError::DriverError("InputStream is closed".to_string()))?;
        reader.read_to_end(&mut bytes).map_err(|error| {
            DruidError::DriverError(format!("InputStream read failed: {error}"))
        })?;
        Ok(bytes)
    }

    /// 关闭输入流；重复关闭保持幂等。
    pub fn close(&self) -> Result<(), DruidError> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .reader
            .take();
        Ok(())
    }

    /// 返回输入流是否已经关闭。
    pub fn is_closed(&self) -> bool {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .reader
            .is_none()
    }
}

impl fmt::Debug for RdbcInputStream {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RdbcInputStream")
            .field("id", &self.inner.id)
            .field("closed", &self.is_closed())
            .finish()
    }
}

impl PartialEq for RdbcInputStream {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Eq for RdbcInputStream {}

/// 二进制流 setter 的 Java 重载身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RdbcStreamLength {
    /// 调用方使用未提供长度的重载。
    Unspecified,
    /// `setAsciiStream/setBinaryStream(..., int)`，原样保留 Java int。
    Int(i32),
    /// Blob/ASCII/Binary stream 的 long 重载，原样保留 Java long。
    Long(i64),
}
