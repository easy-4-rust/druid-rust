//! JDBC 字符 Writer 平台对象。
//!
//! 对应 Java 平台对象：`java.io.Writer`。字符写入以 UTF-16 code unit 为
//! 无损边界，避免 Rust String 无法表达未配对 surrogate 时静默替换数据。

use super::DruidError;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

static NEXT_WRITER_ID: AtomicU64 = AtomicU64::new(1);

/// 物理字符 Writer SPI。
pub trait PhysicalCharacterWriter: fmt::Debug + Send {
    /// 写入 UTF-16 code unit。
    fn write_utf16(&mut self, code_units: &[u16]) -> Result<usize, DruidError>;

    /// 刷新底层 Writer。
    fn flush(&mut self) -> Result<(), DruidError>;

    /// 关闭底层 Writer。
    fn close(&mut self) -> Result<(), DruidError>;
}

struct JdbcWriterState {
    writer: Option<Box<dyn PhysicalCharacterWriter>>,
}

struct JdbcWriterInner {
    id: u64,
    state: Mutex<JdbcWriterState>,
}

/// 可共享的 Java Writer 句柄。
///
/// Clone 共享写入位置和关闭状态。
#[derive(Clone)]
pub struct JdbcWriter {
    inner: Arc<JdbcWriterInner>,
}

impl JdbcWriter {
    /// 包装物理字符 Writer。
    pub fn new(writer: impl PhysicalCharacterWriter + 'static) -> Self {
        Self {
            inner: Arc::new(JdbcWriterInner {
                id: NEXT_WRITER_ID.fetch_add(1, Ordering::Relaxed),
                state: Mutex::new(JdbcWriterState {
                    writer: Some(Box::new(writer)),
                }),
            }),
        }
    }

    /// 写入 UTF-16 code unit。
    pub fn write_utf16(&self, code_units: &[u16]) -> Result<usize, DruidError> {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state
            .writer
            .as_mut()
            .ok_or_else(|| DruidError::DriverError("Writer is closed".to_string()))?
            .write_utf16(code_units)
    }

    /// 将 Rust String 编码为 UTF-16 后写入。
    pub fn write_str(&self, value: &str) -> Result<usize, DruidError> {
        self.write_utf16(&value.encode_utf16().collect::<Vec<_>>())
    }

    /// 刷新底层 Writer。
    pub fn flush(&self) -> Result<(), DruidError> {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state
            .writer
            .as_mut()
            .ok_or_else(|| DruidError::DriverError("Writer is closed".to_string()))?
            .flush()
    }

    /// 关闭 Writer；重复关闭保持幂等。
    pub fn close(&self) -> Result<(), DruidError> {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(mut writer) = state.writer.take() else {
            return Ok(());
        };
        writer.close()
    }

    /// 返回 Writer 是否已经关闭。
    pub fn is_closed(&self) -> bool {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .writer
            .is_none()
    }
}

impl fmt::Debug for JdbcWriter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JdbcWriter")
            .field("id", &self.inner.id)
            .field("closed", &self.is_closed())
            .finish()
    }
}

impl PartialEq for JdbcWriter {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Eq for JdbcWriter {}
