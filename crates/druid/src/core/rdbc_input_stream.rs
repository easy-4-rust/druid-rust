//! RDBC input-stream platform object.
//!
//! Corresponds to the Java platform type `java.io.InputStream`. It preserves
//! the stream identity, current read position, and closed state used by
//! `CallableStatement#setBlob(String, InputStream[, long])` without eagerly
//! materializing the stream into a byte array at the pooling layer.

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

/// A shareable RDBC input-stream handle.
///
/// Cloning copies only the handle. All clones share the same read cursor and
/// closed state, matching Java `InputStream` object-reference semantics.
#[derive(Clone)]
pub struct RdbcInputStream {
    inner: Arc<RdbcInputStreamInner>,
}

impl RdbcInputStream {
    /// Wraps a physical input stream without reading from it eagerly.
    ///
    /// # Parameters
    /// - `reader`: the stream supplied by a driver or caller.
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

    /// Creates a stateful input stream from bytes, primarily for adapters and
    /// contract tests.
    pub fn from_bytes(bytes: impl Into<Vec<u8>>) -> Self {
        Self::new(Cursor::new(bytes.into()))
    }

    /// Reads bytes and advances the shared cursor.
    ///
    /// # Parameters
    /// - `buffer`: the destination buffer for this read operation.
    ///
    /// # Returns
    /// The number of bytes read, or an error if the stream is closed or the
    /// underlying read operation fails.
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

    /// Reads from the current shared cursor to the end of the stream.
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

    /// Closes the input stream. Repeated calls are idempotent.
    pub fn close(&self) -> Result<(), DruidError> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .reader
            .take();
        Ok(())
    }

    /// Returns whether the input stream has been closed.
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

/// Identifies the Java overload selected for a binary-stream setter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RdbcStreamLength {
    /// The caller selected an overload that does not provide a length.
    Unspecified,
    /// `setAsciiStream/setBinaryStream(..., int)`, preserving the Java `int`.
    Int(i32),
    /// A Blob, ASCII, or binary stream overload that takes a Java `long`.
    Long(i64),
}
