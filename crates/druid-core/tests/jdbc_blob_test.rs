//! java.sql.Blob / java.io 流对象的资源语义契约。

use druid_core::{
    CallableOutputValue, DruidError, JdbcBlob, JdbcInputStream, JdbcOutputStream, PhysicalBlob,
};
use std::any::Any;
use std::io::{Error, Read, Write};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
struct BlobState {
    bytes: Vec<u8>,
    freed: bool,
}

#[derive(Debug)]
struct InMemoryPhysicalBlob {
    state: Arc<Mutex<BlobState>>,
}

impl InMemoryPhysicalBlob {
    fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            state: Arc::new(Mutex::new(BlobState {
                bytes: bytes.into(),
                freed: false,
            })),
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, BlobState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn checked_start(
        state: &BlobState,
        position: i64,
        allow_end: bool,
    ) -> Result<usize, DruidError> {
        if state.freed {
            return Err(DruidError::DriverError("Blob has been freed".to_string()));
        }
        let start = position
            .checked_sub(1)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| {
                DruidError::DriverError("Blob position must be one-based".to_string())
            })?;
        let maximum = if allow_end {
            state.bytes.len()
        } else {
            state.bytes.len().saturating_sub(1)
        };
        if (state.bytes.is_empty() && !allow_end) || start > maximum {
            return Err(DruidError::DriverError(
                "Blob position exceeds its length".to_string(),
            ));
        }
        Ok(start)
    }

    fn checked_length(length: i64) -> Result<usize, DruidError> {
        usize::try_from(length)
            .map_err(|_| DruidError::DriverError("Blob length must be non-negative".to_string()))
    }
}

struct BlobWriteCursor {
    state: Arc<Mutex<BlobState>>,
    position: usize,
}

struct FailingReader;

impl Read for FailingReader {
    fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
        Err(Error::other("injected read failure"))
    }
}

enum WriterFailure {
    Write,
    Flush,
}

struct FailingWriter {
    failure: WriterFailure,
}

impl Write for FailingWriter {
    fn write(&mut self, _bytes: &[u8]) -> std::io::Result<usize> {
        match self.failure {
            WriterFailure::Write => Err(Error::other("injected write failure")),
            WriterFailure::Flush => Ok(0),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self.failure {
            WriterFailure::Write => Ok(()),
            WriterFailure::Flush => Err(Error::other("injected flush failure")),
        }
    }
}

impl Write for BlobWriteCursor {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.freed {
            return Err(Error::other("Blob has been freed"));
        }
        let end = self
            .position
            .checked_add(bytes.len())
            .ok_or_else(|| Error::other("Blob write position overflow"))?;
        if end > state.bytes.len() {
            state.bytes.resize(end, 0);
        }
        state.bytes[self.position..end].copy_from_slice(bytes);
        self.position = end;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl PhysicalBlob for InMemoryPhysicalBlob {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn length(&self) -> Result<i64, DruidError> {
        let state = self.state();
        if state.freed {
            return Err(DruidError::DriverError("Blob has been freed".to_string()));
        }
        i64::try_from(state.bytes.len())
            .map_err(|_| DruidError::DriverError("Blob length exceeds i64".to_string()))
    }

    fn get_bytes(&self, position: i64, length: i32) -> Result<Vec<u8>, DruidError> {
        let state = self.state();
        let start = Self::checked_start(&state, position, true)?;
        let length = Self::checked_length(i64::from(length))?;
        let end = start
            .checked_add(length)
            .map(|value| value.min(state.bytes.len()))
            .ok_or_else(|| DruidError::DriverError("Blob range overflow".to_string()))?;
        Ok(state.bytes[start..end].to_vec())
    }

    fn get_binary_stream(&self) -> Result<JdbcInputStream, DruidError> {
        let state = self.state();
        if state.freed {
            return Err(DruidError::DriverError("Blob has been freed".to_string()));
        }
        Ok(JdbcInputStream::from_bytes(state.bytes.clone()))
    }

    fn position_bytes(&self, pattern: &[u8], start: i64) -> Result<Option<i64>, DruidError> {
        let state = self.state();
        let start = Self::checked_start(&state, start, true)?;
        let result = if pattern.is_empty() {
            Some(start)
        } else {
            state.bytes[start..]
                .windows(pattern.len())
                .position(|window| window == pattern)
                .map(|position| start + position)
        };
        result
            .map(|position| {
                i64::try_from(position + 1)
                    .map_err(|_| DruidError::DriverError("Blob position exceeds i64".to_string()))
            })
            .transpose()
    }

    fn position_blob(&self, pattern: &JdbcBlob, start: i64) -> Result<Option<i64>, DruidError> {
        let length = pattern.length()?;
        let length = i32::try_from(length)
            .map_err(|_| DruidError::DriverError("pattern Blob is too large".to_string()))?;
        self.position_bytes(&pattern.get_bytes(1, length)?, start)
    }

    fn set_bytes(&self, position: i64, bytes: &[u8]) -> Result<i32, DruidError> {
        let length = i32::try_from(bytes.len())
            .map_err(|_| DruidError::DriverError("Blob write is too large".to_string()))?;
        self.set_bytes_range(position, bytes, 0, length)
    }

    fn set_bytes_range(
        &self,
        position: i64,
        bytes: &[u8],
        offset: i32,
        length: i32,
    ) -> Result<i32, DruidError> {
        let mut state = self.state();
        let start = Self::checked_start(&state, position, true)?;
        let offset = Self::checked_length(i64::from(offset))?;
        let length = Self::checked_length(i64::from(length))?;
        let source_end = offset
            .checked_add(length)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| DruidError::DriverError("Blob source range is invalid".to_string()))?;
        let target_end = start
            .checked_add(length)
            .ok_or_else(|| DruidError::DriverError("Blob target range overflow".to_string()))?;
        if target_end > state.bytes.len() {
            state.bytes.resize(target_end, 0);
        }
        state.bytes[start..target_end].copy_from_slice(&bytes[offset..source_end]);
        i32::try_from(length)
            .map_err(|_| DruidError::DriverError("Blob write exceeds i32".to_string()))
    }

    fn set_binary_stream(&self, position: i64) -> Result<JdbcOutputStream, DruidError> {
        let state = self.state();
        let position = Self::checked_start(&state, position, true)?;
        drop(state);
        Ok(JdbcOutputStream::new(BlobWriteCursor {
            state: self.state.clone(),
            position,
        }))
    }

    fn truncate(&self, length: i64) -> Result<(), DruidError> {
        let mut state = self.state();
        if state.freed {
            return Err(DruidError::DriverError("Blob has been freed".to_string()));
        }
        let length = Self::checked_length(length)?;
        if length > state.bytes.len() {
            return Err(DruidError::DriverError(
                "Blob truncate length exceeds current length".to_string(),
            ));
        }
        state.bytes.truncate(length);
        Ok(())
    }

    fn free(&self) -> Result<(), DruidError> {
        let mut state = self.state();
        state.freed = true;
        state.bytes.clear();
        Ok(())
    }

    fn is_freed(&self) -> bool {
        self.state().freed
    }

    fn get_binary_stream_range(
        &self,
        position: i64,
        length: i64,
    ) -> Result<JdbcInputStream, DruidError> {
        let state = self.state();
        let start = Self::checked_start(&state, position, true)?;
        let length = Self::checked_length(length)?;
        let end = start
            .checked_add(length)
            .filter(|end| *end <= state.bytes.len())
            .ok_or_else(|| DruidError::DriverError("Blob range is invalid".to_string()))?;
        Ok(JdbcInputStream::from_bytes(
            state.bytes[start..end].to_vec(),
        ))
    }
}

fn blob(bytes: impl Into<Vec<u8>>) -> JdbcBlob {
    JdbcBlob::new(Arc::new(InMemoryPhysicalBlob::new(bytes)))
}

#[test]
fn input_stream_clones_share_cursor_and_close_state() {
    let stream = JdbcInputStream::from_bytes([1, 2, 3, 4]);
    let clone = stream.clone();
    assert_eq!(stream, clone);
    assert_ne!(stream, JdbcInputStream::from_bytes([1, 2, 3, 4]));

    let mut first = [0; 2];
    assert_eq!(stream.read(&mut first).unwrap(), 2);
    assert_eq!(first, [1, 2]);
    assert_eq!(clone.read_to_end().unwrap(), vec![3, 4]);

    clone.close().unwrap();
    assert!(stream.is_closed());
    assert!(stream.read(&mut first).is_err());
    stream.close().unwrap();
    assert!(format!("{stream:?}").contains("closed: true"));

    let failing = JdbcInputStream::new(FailingReader);
    assert!(failing.read(&mut first).is_err());
    assert!(failing.read_to_end().is_err());
}

#[test]
fn blob_delegates_complete_jdbc_resource_contract_without_eager_materialization() {
    let value = blob(b"abcdef".to_vec());
    assert_eq!(value, value.clone());
    assert_ne!(value, blob(b"abcdef".to_vec()));
    assert!(value
        .physical()
        .as_any()
        .downcast_ref::<InMemoryPhysicalBlob>()
        .is_some());
    assert!(format!("{value:?}").contains("JdbcBlob"));
    assert_eq!(
        format!("{}", CallableOutputValue::Blob(value.clone())),
        "<Blob>"
    );
    assert_eq!(value.length().unwrap(), 6);
    assert_eq!(value.get_bytes(2, 3).unwrap(), b"bcd");
    assert_eq!(value.position_bytes(b"cd", 1).unwrap(), Some(3));
    assert_eq!(
        value.position_blob(&blob(b"de".to_vec()), 1).unwrap(),
        Some(4)
    );

    assert_eq!(value.set_bytes(2, b"XY").unwrap(), 2);
    assert_eq!(value.set_bytes_range(4, b"12ZZ", 2, 2).unwrap(), 2);
    assert_eq!(
        value.get_binary_stream().unwrap().read_to_end().unwrap(),
        b"aXYZZf"
    );

    let writer = value.set_binary_stream(6).unwrap();
    let writer_clone = writer.clone();
    assert_eq!(writer, writer_clone);
    assert_eq!(writer.write(b"789").unwrap(), 3);
    writer_clone.flush().unwrap();
    writer.close().unwrap();
    writer.close().unwrap();
    assert!(writer_clone.is_closed());
    assert!(format!("{writer_clone:?}").contains("closed: true"));
    assert!(writer_clone.write(b"!").is_err());
    assert_eq!(value.get_bytes(1, 8).unwrap(), b"aXYZZ789");

    assert_eq!(
        value
            .get_binary_stream_range(2, 3)
            .unwrap()
            .read_to_end()
            .unwrap(),
        b"XYZ"
    );
    value.truncate(5).unwrap();
    assert_eq!(value.length().unwrap(), 5);
    assert!(value.get_bytes(0, 1).is_err());
    assert!(value.set_bytes_range(1, b"x", -1, 1).is_err());
    assert!(value.truncate(6).is_err());

    value.free().unwrap();
    assert!(value.is_freed());
    value.free().unwrap();
    assert!(value.length().is_err());
    assert!(value.get_binary_stream().is_err());

    let write_failure = JdbcOutputStream::new(FailingWriter {
        failure: WriterFailure::Write,
    });
    assert!(write_failure.write(b"x").is_err());
    write_failure.close().unwrap();

    let flush_failure = JdbcOutputStream::new(FailingWriter {
        failure: WriterFailure::Flush,
    });
    assert!(flush_failure.flush().is_err());
    assert!(flush_failure.close().is_err());
    assert!(flush_failure.is_closed());
}
