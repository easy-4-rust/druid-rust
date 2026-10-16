//! RDBC output-stream platform object.
//!
//! Corresponds to the Java platform type `java.io.OutputStream` and carries
//! the driver-provided stream returned by
//! `java.sql.Blob#setBinaryStream(long)`.

use crate::error::RdbcError;
use std::fmt;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

static NEXT_OUTPUT_STREAM_ID: AtomicU64 = AtomicU64::new(1);

struct RdbcOutputStreamState {
    writer: Option<Box<dyn Write + Send>>,
    close: Option<Box<dyn FnMut() -> std::io::Result<()> + Send>>,
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
                    close: None,
                }),
            }),
        }
    }

    /// 包装需要独立关闭远端句柄的写入器；关闭回调失败时保留句柄以便重试。
    pub fn with_close(
        writer: impl Write + Send + 'static,
        close: impl FnMut() -> std::io::Result<()> + Send + 'static,
    ) -> Self {
        Self {
            inner: Arc::new(RdbcOutputStreamInner {
                id: NEXT_OUTPUT_STREAM_ID.fetch_add(1, Ordering::Relaxed),
                state: Mutex::new(RdbcOutputStreamState {
                    writer: Some(Box::new(writer)),
                    close: Some(Box::new(close)),
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
    pub fn write(&self, bytes: &[u8]) -> Result<usize, RdbcError> {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let writer = state
            .writer
            .as_mut()
            .ok_or_else(|| RdbcError::DriverError("OutputStream is closed".to_string()))?;
        writer
            .write(bytes)
            .map_err(|error| RdbcError::DriverError(format!("OutputStream write failed: {error}")))
    }

    /// Flushes the underlying output stream.
    pub fn flush(&self) -> Result<(), RdbcError> {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let writer = state
            .writer
            .as_mut()
            .ok_or_else(|| RdbcError::DriverError("OutputStream is closed".to_string()))?;
        writer
            .flush()
            .map_err(|error| RdbcError::DriverError(format!("OutputStream flush failed: {error}")))
    }

    /// Flushes and closes the output stream. Repeated calls are idempotent.
    pub fn close(&self) -> Result<(), RdbcError> {
        let mut state = self
            .inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.close.is_none() {
            let Some(mut writer) = state.writer.take() else {
                return Ok(());
            };
            return writer.flush().map_err(|error| {
                RdbcError::DriverError(format!("OutputStream close failed: {error}"))
            });
        }
        let Some(writer) = state.writer.as_mut() else {
            return Ok(());
        };
        writer.flush().map_err(|error| {
            RdbcError::DriverError(format!("OutputStream close failed: {error}"))
        })?;
        if let Some(close) = state.close.as_mut() {
            close().map_err(|error| {
                RdbcError::DriverError(format!("OutputStream close failed: {error}"))
            })?;
        }
        state.writer.take();
        state.close.take();
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::RdbcOutputStream;
    use std::io;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    #[test]
    fn failed_remote_close_retains_writer_for_retry() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&attempts);
        let stream = RdbcOutputStream::with_close(Vec::new(), move || {
            if counter.fetch_add(1, Ordering::SeqCst) == 0 {
                Err(io::Error::other("remote close failed"))
            } else {
                Ok(())
            }
        });
        stream.write(&[1, 2]).expect("write before close");
        assert!(stream.close().is_err());
        assert!(!stream.is_closed());
        stream.write(&[3]).expect("writer remains usable");
        stream.close().expect("retry close");
        stream.close().expect("duplicate close");
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }
}
