//! RDBC character-writer platform object.
//!
//! Corresponds to the Java platform type `java.io.Writer`. Character writes
//! use UTF-16 code units as the lossless boundary, avoiding silent replacement
//! when a Rust string cannot represent unpaired surrogates.

use super::DruidError;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

static NEXT_WRITER_ID: AtomicU64 = AtomicU64::new(1);

/// Service-provider interface for a physical character writer.
pub trait PhysicalCharacterWriter: fmt::Debug + Send {
    /// Writes UTF-16 code units.
    fn write_utf16(&mut self, code_units: &[u16]) -> Result<usize, DruidError>;

    /// Flushes the underlying writer.
    fn flush(&mut self) -> Result<(), DruidError>;

    /// Closes the underlying writer.
    fn close(&mut self) -> Result<(), DruidError>;
}

struct RdbcWriterState {
    writer: Option<Box<dyn PhysicalCharacterWriter>>,
}

struct RdbcWriterInner {
    id: u64,
    state: Mutex<RdbcWriterState>,
}

/// A shareable RDBC character-writer handle.
///
/// All clones share the write position and closed state.
#[derive(Clone)]
pub struct RdbcWriter {
    inner: Arc<RdbcWriterInner>,
}

impl RdbcWriter {
    /// Wraps a physical character writer.
    pub fn new(writer: impl PhysicalCharacterWriter + 'static) -> Self {
        Self {
            inner: Arc::new(RdbcWriterInner {
                id: NEXT_WRITER_ID.fetch_add(1, Ordering::Relaxed),
                state: Mutex::new(RdbcWriterState {
                    writer: Some(Box::new(writer)),
                }),
            }),
        }
    }

    /// Writes UTF-16 code units.
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

    /// Encodes a Rust string as UTF-16 and writes the resulting code units.
    pub fn write_str(&self, value: &str) -> Result<usize, DruidError> {
        self.write_utf16(&value.encode_utf16().collect::<Vec<_>>())
    }

    /// Flushes the underlying writer.
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

    /// Closes the writer. Repeated calls are idempotent.
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

    /// Returns whether the writer has been closed.
    pub fn is_closed(&self) -> bool {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .writer
            .is_none()
    }
}

impl fmt::Debug for RdbcWriter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RdbcWriter")
            .field("id", &self.inner.id)
            .field("closed", &self.is_closed())
            .finish()
    }
}

impl PartialEq for RdbcWriter {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Eq for RdbcWriter {}
