//! RDBC output-stream platform object.
//!
//! Corresponds to the Java platform type `java.io.OutputStream` and carries
//! the driver-provided stream returned by
//! `java.sql.Blob#setBinaryStream(long)`.

use super::DruidError;
use std::fmt;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

static NEXT_OUTPUT_STREAM_ID: AtomicU64 = AtomicU64::new(1);

struct RdbcOutputStreamState {
    writer: Option<Box<dyn Write + Send>>,
}

struct RdbcOutputStreamInner {
    id: u64,
    state: Mutex<RdbcOutputStreamState>,
}

/// A shareable RDBC output-stream handle.
///
/// Cloning copies only the handle. All clones share the underlying write
/// position and closed state.
#[derive(Clone)]
pub struct RdbcOutputStream {
    inner: Arc<RdbcOutputStreamInner>,
}

impl RdbcOutputStream {
    /// Wraps a physical output stream supplied by a driver.
    ///
    /// # Parameters
    /// - `writer`: the destination, normally created by a Blob adapter.
    pub fn new(writer: impl Write + Send + 'static) -> Self {
        Self {
            inner: Arc::new(RdbcOutputStreamInner {
                id: NEXT_OUTPUT_STREAM_ID.fetch_add(1, Ordering::Relaxed),
                state: Mutex::new(RdbcOutputStreamState {
                    writer: Some(Box::new(writer)),
                }),
            }),
        }
    }

    /// Writes bytes and advances the shared position.
    ///
    /// # Parameters
    /// - `bytes`: the bytes to write.
    ///
    /// # Returns
    /// The number of bytes written, or an error if the stream is closed or the
    /// underlying write operation fails.
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

    /// Flushes the underlying output stream.
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

    /// Flushes and closes the output stream. Repeated calls are idempotent.
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

    /// Returns whether the output stream has been closed.
    pub fn is_closed(&self) -> bool {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .writer
            .is_none()
    }
}

impl fmt::Debug for RdbcOutputStream {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RdbcOutputStream")
            .field("id", &self.inner.id)
            .field("closed", &self.is_closed())
            .finish()
    }
}

impl PartialEq for RdbcOutputStream {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Eq for RdbcOutputStream {}
