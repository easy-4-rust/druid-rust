//! java.sql.Clob/NClob 与 java.io.Reader/Writer 的资源语义契约。

use druid_core::{
    CallableOutputValue, DruidError, JavaString, JdbcClob, JdbcInputStream, JdbcNClob,
    JdbcOutputStream, JdbcReader, JdbcWriter, PhysicalCharacterReader, PhysicalCharacterWriter,
    PhysicalClob, PhysicalNClob,
};
use std::any::Any;
use std::io::{Error, Write};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
struct ClobState {
    code_units: Vec<u16>,
    freed: bool,
}

#[derive(Debug)]
struct InMemoryPhysicalClob {
    state: Arc<Mutex<ClobState>>,
}

impl InMemoryPhysicalClob {
    fn new(value: JavaString) -> Self {
        Self {
            state: Arc::new(Mutex::new(ClobState {
                code_units: value.as_utf16().to_vec(),
                freed: false,
            })),
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, ClobState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn checked_start(state: &ClobState, position: i64) -> Result<usize, DruidError> {
        if state.freed {
            return Err(DruidError::DriverError("Clob has been freed".to_string()));
        }
        let start = position
            .checked_sub(1)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| {
                DruidError::DriverError("Clob position must be one-based".to_string())
            })?;
        if start > state.code_units.len() {
            return Err(DruidError::DriverError(
                "Clob position exceeds its length".to_string(),
            ));
        }
        Ok(start)
    }

    fn checked_length(length: i64) -> Result<usize, DruidError> {
        usize::try_from(length)
            .map_err(|_| DruidError::DriverError("Clob length must be non-negative".to_string()))
    }

    fn write_units(
        state: &mut ClobState,
        position: usize,
        code_units: &[u16],
    ) -> Result<usize, DruidError> {
        if state.freed {
            return Err(DruidError::DriverError("Clob has been freed".to_string()));
        }
        let end = position
            .checked_add(code_units.len())
            .ok_or_else(|| DruidError::DriverError("Clob write range overflow".to_string()))?;
        if end > state.code_units.len() {
            state.code_units.resize(end, 0);
        }
        state.code_units[position..end].copy_from_slice(code_units);
        Ok(code_units.len())
    }
}

#[derive(Debug)]
struct ClobCharacterWriter {
    state: Arc<Mutex<ClobState>>,
    position: usize,
}

impl PhysicalCharacterWriter for ClobCharacterWriter {
    fn write_utf16(&mut self, code_units: &[u16]) -> Result<usize, DruidError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let length = InMemoryPhysicalClob::write_units(&mut state, self.position, code_units)?;
        self.position += length;
        Ok(length)
    }

    fn flush(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    fn close(&mut self) -> Result<(), DruidError> {
        Ok(())
    }
}

struct ClobAsciiWriter {
    state: Arc<Mutex<ClobState>>,
    position: usize,
}

impl Write for ClobAsciiWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let code_units = bytes
            .iter()
            .map(|value| u16::from(*value))
            .collect::<Vec<_>>();
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let length = InMemoryPhysicalClob::write_units(&mut state, self.position, &code_units)
            .map_err(|error| Error::other(error.to_string()))?;
        self.position += length;
        Ok(length)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl PhysicalClob for InMemoryPhysicalClob {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn length(&self) -> Result<i64, DruidError> {
        let state = self.state();
        if state.freed {
            return Err(DruidError::DriverError("Clob has been freed".to_string()));
        }
        i64::try_from(state.code_units.len())
            .map_err(|_| DruidError::DriverError("Clob length exceeds i64".to_string()))
    }

    fn get_sub_string(&self, position: i64, length: i32) -> Result<JavaString, DruidError> {
        let state = self.state();
        let start = Self::checked_start(&state, position)?;
        let length = Self::checked_length(i64::from(length))?;
        let end = start
            .checked_add(length)
            .map(|value| value.min(state.code_units.len()))
            .ok_or_else(|| DruidError::DriverError("Clob range overflow".to_string()))?;
        Ok(JavaString::from_utf16(
            state.code_units[start..end].to_vec(),
        ))
    }

    fn get_character_stream(&self) -> Result<JdbcReader, DruidError> {
        let state = self.state();
        if state.freed {
            return Err(DruidError::DriverError("Clob has been freed".to_string()));
        }
        Ok(JdbcReader::from_utf16(state.code_units.clone()))
    }

    fn get_ascii_stream(&self) -> Result<JdbcInputStream, DruidError> {
        let state = self.state();
        if state.freed {
            return Err(DruidError::DriverError("Clob has been freed".to_string()));
        }
        let bytes = state
            .code_units
            .iter()
            .map(|value| {
                u8::try_from(*value).map_err(|_| {
                    DruidError::DriverError("test Clob contains non-ASCII data".to_string())
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(JdbcInputStream::from_bytes(bytes))
    }

    fn position_string(&self, pattern: &JavaString, start: i64) -> Result<Option<i64>, DruidError> {
        let state = self.state();
        let start = Self::checked_start(&state, start)?;
        let pattern = pattern.as_utf16();
        let position = if pattern.is_empty() {
            Some(start)
        } else {
            state.code_units[start..]
                .windows(pattern.len())
                .position(|window| window == pattern)
                .map(|position| start + position)
        };
        position
            .map(|position| {
                i64::try_from(position + 1)
                    .map_err(|_| DruidError::DriverError("Clob position exceeds i64".to_string()))
            })
            .transpose()
    }

    fn position_clob(&self, pattern: &JdbcClob, start: i64) -> Result<Option<i64>, DruidError> {
        let length = i32::try_from(pattern.length()?)
            .map_err(|_| DruidError::DriverError("pattern Clob is too large".to_string()))?;
        self.position_string(&pattern.get_sub_string(1, length)?, start)
    }

    fn set_string(&self, position: i64, value: &JavaString) -> Result<i32, DruidError> {
        let length = i32::try_from(value.len())
            .map_err(|_| DruidError::DriverError("Clob write is too large".to_string()))?;
        self.set_string_range(position, value, 0, length)
    }

    fn set_string_range(
        &self,
        position: i64,
        value: &JavaString,
        offset: i32,
        length: i32,
    ) -> Result<i32, DruidError> {
        let mut state = self.state();
        let position = Self::checked_start(&state, position)?;
        let offset = Self::checked_length(i64::from(offset))?;
        let length = Self::checked_length(i64::from(length))?;
        let source_end = offset
            .checked_add(length)
            .filter(|end| *end <= value.len())
            .ok_or_else(|| DruidError::DriverError("Clob source range is invalid".to_string()))?;
        Self::write_units(&mut state, position, &value.as_utf16()[offset..source_end])?;
        i32::try_from(length)
            .map_err(|_| DruidError::DriverError("Clob write exceeds i32".to_string()))
    }

    fn set_ascii_stream(&self, position: i64) -> Result<JdbcOutputStream, DruidError> {
        let state = self.state();
        let position = Self::checked_start(&state, position)?;
        drop(state);
        Ok(JdbcOutputStream::new(ClobAsciiWriter {
            state: self.state.clone(),
            position,
        }))
    }

    fn set_character_stream(&self, position: i64) -> Result<JdbcWriter, DruidError> {
        let state = self.state();
        let position = Self::checked_start(&state, position)?;
        drop(state);
        Ok(JdbcWriter::new(ClobCharacterWriter {
            state: self.state.clone(),
            position,
        }))
    }

    fn truncate(&self, length: i64) -> Result<(), DruidError> {
        let mut state = self.state();
        if state.freed {
            return Err(DruidError::DriverError("Clob has been freed".to_string()));
        }
        let length = Self::checked_length(length)?;
        if length > state.code_units.len() {
            return Err(DruidError::DriverError(
                "Clob truncate length exceeds current length".to_string(),
            ));
        }
        state.code_units.truncate(length);
        Ok(())
    }

    fn free(&self) -> Result<(), DruidError> {
        let mut state = self.state();
        state.freed = true;
        state.code_units.clear();
        Ok(())
    }

    fn is_freed(&self) -> bool {
        self.state().freed
    }

    fn get_character_stream_range(
        &self,
        position: i64,
        length: i64,
    ) -> Result<JdbcReader, DruidError> {
        let state = self.state();
        let start = Self::checked_start(&state, position)?;
        let length = Self::checked_length(length)?;
        let end = start
            .checked_add(length)
            .filter(|end| *end <= state.code_units.len())
            .ok_or_else(|| DruidError::DriverError("Clob range is invalid".to_string()))?;
        Ok(JdbcReader::from_utf16(
            state.code_units[start..end].to_vec(),
        ))
    }
}

impl PhysicalNClob for InMemoryPhysicalClob {}

fn clob(value: impl Into<JavaString>) -> JdbcClob {
    JdbcClob::new(Arc::new(InMemoryPhysicalClob::new(value.into())))
}

fn n_clob(value: impl Into<JavaString>) -> JdbcNClob {
    JdbcNClob::new(Arc::new(InMemoryPhysicalClob::new(value.into())))
}

#[derive(Debug)]
struct FailingReader {
    fail_close: bool,
}

impl PhysicalCharacterReader for FailingReader {
    fn read_utf16(&mut self, _buffer: &mut [u16]) -> Result<usize, DruidError> {
        Err(DruidError::DriverError(
            "injected Reader failure".to_string(),
        ))
    }

    fn close(&mut self) -> Result<(), DruidError> {
        if self.fail_close {
            Err(DruidError::DriverError(
                "injected Reader close failure".to_string(),
            ))
        } else {
            Ok(())
        }
    }
}

#[derive(Debug)]
struct FailingWriter;

impl PhysicalCharacterWriter for FailingWriter {
    fn write_utf16(&mut self, _code_units: &[u16]) -> Result<usize, DruidError> {
        Err(DruidError::DriverError(
            "injected Writer failure".to_string(),
        ))
    }

    fn flush(&mut self) -> Result<(), DruidError> {
        Err(DruidError::DriverError(
            "injected Writer flush failure".to_string(),
        ))
    }

    fn close(&mut self) -> Result<(), DruidError> {
        Err(DruidError::DriverError(
            "injected Writer close failure".to_string(),
        ))
    }
}

#[test]
fn java_string_and_reader_preserve_utf16_identity_cursor_and_close() {
    let supplementary = JavaString::from("A😀B");
    assert_eq!(supplementary.len(), 4);
    assert!(!supplementary.is_empty());
    assert_eq!(supplementary.to_rust_string().unwrap(), "A😀B");
    assert!(format!("{supplementary:?}").contains("utf16_length"));
    let owned = JavaString::from(String::from("owned"));
    assert_eq!(owned.to_rust_string().unwrap(), "owned");

    let invalid = JavaString::from_utf16(vec![0xD800]);
    assert!(invalid.to_rust_string().is_err());

    let reader = JdbcReader::from_utf16(supplementary.as_utf16().to_vec());
    let clone = reader.clone();
    assert_eq!(reader, clone);
    assert_ne!(reader, JdbcReader::from_string("A😀B"));
    let mut first = [0_u16; 2];
    assert_eq!(reader.read_utf16(&mut first).unwrap(), 2);
    assert_eq!(
        clone.read_to_end_utf16().unwrap(),
        supplementary.as_utf16()[2..]
    );
    assert!(format!("{reader:?}").contains("closed: false"));
    reader.close().unwrap();
    reader.close().unwrap();
    assert!(clone.is_closed());
    assert!(clone.read_to_string().is_err());

    let invalid_reader = JdbcReader::from_utf16(vec![0xD800]);
    assert!(invalid_reader.read_to_string().is_err());
    let failing = JdbcReader::new(FailingReader { fail_close: true });
    assert!(failing.read_utf16(&mut first).is_err());
    assert!(failing.close().is_err());
    assert!(failing.is_closed());
}

#[test]
fn clob_and_n_clob_delegate_complete_jdbc_character_resource_contract() {
    let value = clob("abcdef");
    assert_eq!(value, value.clone());
    assert_ne!(value, clob("abcdef"));
    assert!(format!("{value:?}").contains("JdbcClob"));
    assert!(value
        .physical()
        .as_any()
        .downcast_ref::<InMemoryPhysicalClob>()
        .is_some());
    assert_eq!(value.length().unwrap(), 6);
    assert_eq!(
        value
            .get_sub_string(2, 3)
            .unwrap()
            .to_rust_string()
            .unwrap(),
        "bcd"
    );
    assert_eq!(
        value
            .get_character_stream()
            .unwrap()
            .read_to_string()
            .unwrap(),
        "abcdef"
    );
    assert_eq!(
        value.get_ascii_stream().unwrap().read_to_end().unwrap(),
        b"abcdef"
    );
    assert_eq!(
        value.position_string(&JavaString::from("cd"), 1).unwrap(),
        Some(3)
    );
    assert_eq!(value.position_clob(&clob("de"), 1).unwrap(), Some(4));

    assert_eq!(value.set_string(2, &JavaString::from("XY")).unwrap(), 2);
    assert_eq!(
        value
            .set_string_range(4, &JavaString::from("12ZZ"), 2, 2)
            .unwrap(),
        2
    );
    assert_eq!(
        value
            .get_sub_string(1, 6)
            .unwrap()
            .to_rust_string()
            .unwrap(),
        "aXYZZf"
    );

    let ascii_writer = value.set_ascii_stream(6).unwrap();
    assert_eq!(ascii_writer.write(b"789").unwrap(), 3);
    ascii_writer.close().unwrap();
    let character_writer = value.set_character_stream(2).unwrap();
    let character_writer_clone = character_writer.clone();
    assert_eq!(character_writer, character_writer_clone);
    assert_eq!(character_writer.write_str("中").unwrap(), 1);
    assert_eq!(
        character_writer_clone
            .write_utf16(JavaString::from("文").as_utf16())
            .unwrap(),
        1
    );
    character_writer.flush().unwrap();
    assert!(format!("{character_writer:?}").contains("closed: false"));
    character_writer.close().unwrap();
    character_writer.close().unwrap();
    assert!(character_writer_clone.is_closed());
    assert!(character_writer_clone.write_str("!").is_err());
    assert!(character_writer_clone.flush().is_err());

    assert_eq!(
        value
            .get_character_stream_range(2, 2)
            .unwrap()
            .read_to_string()
            .unwrap(),
        "中文"
    );
    value.truncate(5).unwrap();
    assert_eq!(value.length().unwrap(), 5);
    assert!(value.get_sub_string(0, 1).is_err());
    assert!(value
        .set_string_range(1, &JavaString::from("x"), -1, 1)
        .is_err());
    assert!(value.truncate(6).is_err());

    let national = n_clob("国家字符");
    assert_eq!(national, national.clone());
    assert_ne!(national, n_clob("国家字符"));
    assert_eq!(national.length().unwrap(), 4);
    assert_eq!(
        national
            .as_clob()
            .get_sub_string(1, 2)
            .unwrap()
            .to_rust_string()
            .unwrap(),
        "国家"
    );
    assert!(national
        .physical_n_clob()
        .as_any()
        .downcast_ref::<InMemoryPhysicalClob>()
        .is_some());
    assert!(format!("{national:?}").contains("JdbcNClob"));
    assert_eq!(
        format!("{}", CallableOutputValue::Clob(value.clone())),
        "<Clob>"
    );
    assert_eq!(
        format!("{}", CallableOutputValue::NClob(national)),
        "<NClob>"
    );
    assert_eq!(
        format!(
            "{}",
            CallableOutputValue::CharacterStream(JdbcReader::from_string("x"))
        ),
        "<CharacterStream>"
    );
    assert_eq!(
        format!(
            "{}",
            CallableOutputValue::NCharacterStream(JdbcReader::from_string("x"))
        ),
        "<NCharacterStream>"
    );

    value.free().unwrap();
    value.free().unwrap();
    assert!(value.is_freed());
    assert!(value.length().is_err());
    assert!(value.get_character_stream().is_err());
    assert!(value.get_ascii_stream().is_err());

    let failing_writer = JdbcWriter::new(FailingWriter);
    assert!(failing_writer.write_str("x").is_err());
    assert!(failing_writer.flush().is_err());
    assert!(failing_writer.close().is_err());
    assert!(failing_writer.is_closed());
}
