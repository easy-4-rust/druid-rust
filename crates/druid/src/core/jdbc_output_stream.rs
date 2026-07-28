//! JDBC 输出流平台对象。
//!
//! 对应 Java 平台对象：`java.io.OutputStream`，用于承载
//! `java.sql.Blob#setBinaryStream(long)` 返回的驱动写入流。

use super::DruidError;
use std::fmt;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

static NEXT_OUTPUT_STREAM_ID: AtomicU64 = AtomicU64::new(1);

struct JdbcOutputStreamState {
    writer: Option<Box<dyn Write + Send>>,
}

struct JdbcOutputStreamInner {
    id: u64,
    state: Mutex<JdbcOutputStreamState>,
}

/// 可共享的 JDBC 输出流句柄。
///
/// Clone 只克隆句柄，所有克隆共享底层写入位置和关闭状态。
#[derive(Clone)]
pub struct JdbcOutputStream {
    inner: Arc<JdbcOutputStreamInner>,
}

impl JdbcOutputStream {
    /// 包装驱动提供的真实输出流。
    ///
    /// # 参数
    /// - `writer`：写入目标，通常由 Blob Adapter 创建。
    pub fn new(writer: impl Write + Send + 'static) -> Self {
        Self {
            inner: Arc::new(JdbcOutputStreamInner {
                id: NEXT_OUTPUT_STREAM_ID.fetch_add(1, Ordering::Relaxed),
                state: Mutex::new(JdbcOutputStreamState {
                    writer: Some(Box::new(writer)),
                }),
            }),
        }
    }

    /// 写入字节并推进共享位置。
    ///
    /// # 参数
    /// - `bytes`：待写入内容。
    ///
    /// # 返回
    /// 实际写入字节数；流已关闭或底层写入失败时返回错误。
    pub fn write(&self, bytes: &[u8]) -> Result<usize, DruidError> {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let writer = state
            .writer
            .as_mut()
            .ok_or_else(|| DruidError::DriverError("OutputStream is closed".to_string()))?;
        writer
            .write(bytes)
            .map_err(|error| DruidError::DriverError(format!("OutputStream write failed: {error}")))
    }

    /// 刷新底层输出流。
    pub fn flush(&self) -> Result<(), DruidError> {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let writer = state
            .writer
            .as_mut()
            .ok_or_else(|| DruidError::DriverError("OutputStream is closed".to_string()))?;
        writer
            .flush()
            .map_err(|error| DruidError::DriverError(format!("OutputStream flush failed: {error}")))
    }

    /// 刷新并关闭输出流；重复关闭保持幂等。
    pub fn close(&self) -> Result<(), DruidError> {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(mut writer) = state.writer.take() else {
            return Ok(());
        };
        writer
            .flush()
            .map_err(|error| DruidError::DriverError(format!("OutputStream close failed: {error}")))
    }

    /// 返回输出流是否已经关闭。
    pub fn is_closed(&self) -> bool {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .writer
            .is_none()
    }
}

impl fmt::Debug for JdbcOutputStream {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JdbcOutputStream")
            .field("id", &self.inner.id)
            .field("closed", &self.is_closed())
            .finish()
    }
}

impl PartialEq for JdbcOutputStream {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Eq for JdbcOutputStream {}
