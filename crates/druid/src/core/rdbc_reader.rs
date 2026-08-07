//! RDBC character-reader platform object.
//!
//! Corresponds to the Java platform type `java.io.Reader`. A Java reader works
//! in UTF-16 code units and therefore cannot be represented losslessly as a
//! Rust UTF-8 `String` or byte-oriented `Read`. This type uses `u16` sequences
//! as the lossless driver boundary.

use super::DruidError;
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

static NEXT_READER_ID: AtomicU64 = AtomicU64::new(1);

/// Service-provider interface for a physical character reader.
///
/// Corresponds to `java.io.Reader#read(char[])` and `Reader#close()`.
pub trait PhysicalCharacterReader: fmt::Debug + Send {
    /// Reads UTF-16 code units and advances the cursor; zero denotes end of
    /// stream.
    fn read_utf16(&mut self, buffer: &mut [u16]) -> Result<usize, DruidError>;

    /// Closes the underlying reader.
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

struct RdbcReaderState {
    reader: Option<Box<dyn PhysicalCharacterReader>>,
}

struct RdbcReaderInner {
    id: u64,
    state: Mutex<RdbcReaderState>,
}

/// A shareable RDBC character-reader handle.
///
/// Cloning preserves Java reference semantics: all clones share the UTF-16
/// cursor and closed state.
#[derive(Clone)]
pub struct RdbcReader {
    inner: Arc<RdbcReaderInner>,
}

impl RdbcReader {
    /// Wraps a physical character reader.
    ///
    /// # Parameters
    /// - `reader`: the character stream supplied by a driver or caller.
    pub fn new(reader: impl PhysicalCharacterReader + 'static) -> Self {
        Self {
            inner: Arc::new(RdbcReaderInner {
                id: NEXT_READER_ID.fetch_add(1, Ordering::Relaxed),
                state: Mutex::new(RdbcReaderState {
                    reader: Some(Box::new(reader)),
                }),
            }),
        }
    }

    /// Creates a UTF-16 reader from a Rust string.
    pub fn from_string(value: impl AsRef<str>) -> Self {
        Self::from_utf16(value.as_ref().encode_utf16().collect())
    }

    /// Creates a reader from raw UTF-16 code units.
    ///
    /// This entry point lets an adapter preserve unpaired surrogates received
    /// from a Java reader until the caller explicitly requests conversion to a
    /// Rust string.
    pub fn from_utf16(code_units: Vec<u16>) -> Self {
        Self::new(Utf16SliceReader {
            code_units,
            position: 0,
        })
    }

    /// Reads UTF-16 code units and advances the shared cursor.
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

    /// Reads all remaining UTF-16 code units from the current cursor.
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

    /// Reads from the current cursor and strictly converts the result to a Rust
    /// UTF-8 string.
    ///
    /// Java permits unpaired surrogates. Such input produces an error instead
    /// of a lossy replacement-character conversion.
    pub fn read_to_string(&self) -> Result<String, DruidError> {
        String::from_utf16(&self.read_to_end_utf16()?).map_err(|error| {
            DruidError::DriverError(format!("Reader contains invalid UTF-16: {error}"))
        })
    }

    /// Closes the reader. Repeated calls are idempotent.
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

    /// Returns whether the reader has been closed.
    pub fn is_closed(&self) -> bool {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .reader
            .is_none()
    }
}

impl fmt::Debug for RdbcReader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RdbcReader")
            .field("id", &self.inner.id)
            .field("closed", &self.is_closed())
            .finish()
    }
}

impl PartialEq for RdbcReader {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.inner, &other.inner)
    }
}

impl Eq for RdbcReader {}

/// Identifies the Java length overload selected for a reader setter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RdbcCharacterLength {
    /// No length-bearing overload was selected.
    Unspecified,
    /// The overload taking a Java `int length`.
    Int(i32),
    /// The overload taking a Java `long length`.
    Long(i64),
}
