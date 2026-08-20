//! java.sql.Clob/NClob 与 java.io.Reader/Writer 的资源语义契约。

extern crate druid_core as druid;
use druid::core::{
    DruidError, PhysicalCharacterReader, PhysicalCharacterWriter, RdbcClob, RdbcInputStream,
    RdbcNClob, RdbcObject, RdbcOutputStream, RdbcReader, RdbcString, RdbcWriter,
};
use druid::spi::{
    RdbcClobAccess, RdbcNClobAccess, RdbcResourceAccess, RdbcResourceCapabilities,
    RdbcResourceFactory,
};
use std::io::{Error, Write};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
struct ClobState {
    code_units: Vec<u16>,
    freed: bool,
}

#[derive(Debug)]
struct InMemoryClobAccess {
    state: Arc<Mutex<ClobState>>,
}

impl InMemoryClobAccess {
    fn new(value: RdbcString) -> Self {
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
        let length = InMemoryClobAccess::write_units(&mut state, self.position, code_units)?;
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
        let length = InMemoryClobAccess::write_units(&mut state, self.position, &code_units)
            .map_err(|error| Error::other(error.to_string()))?;
        self.position += length;
        Ok(length)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl RdbcResourceAccess for InMemoryClobAccess {
    fn capabilities(&self) -> RdbcResourceCapabilities {
        RdbcResourceCapabilities::clob()
    }

    async fn free(&self) -> Result<(), DruidError> {
        let mut state = self.state();
        state.freed = true;
        state.code_units.clear();
        Ok(())
    }
}

#[async_trait::async_trait]
impl RdbcClobAccess for InMemoryClobAccess {
    async fn length(&self) -> Result<i64, DruidError> {
        let state = self.state();
        if state.freed {
            return Err(DruidError::DriverError("Clob has been freed".to_string()));
        }
        i64::try_from(state.code_units.len())
            .map_err(|_| DruidError::DriverError("Clob length exceeds i64".to_string()))
    }

    async fn get_sub_string(&self, position: i64, length: i32) -> Result<RdbcString, DruidError> {
        let state = self.state();
        let start = Self::checked_start(&state, position)?;
        let length = Self::checked_length(i64::from(length))?;
        let end = start
            .checked_add(length)
            .map(|value| value.min(state.code_units.len()))
            .ok_or_else(|| DruidError::DriverError("Clob range overflow".to_string()))?;
        Ok(RdbcString::from_utf16(
            state.code_units[start..end].to_vec(),
        ))
    }

    async fn get_character_stream(&self) -> Result<RdbcReader, DruidError> {
        let state = self.state();
        if state.freed {
            return Err(DruidError::DriverError("Clob has been freed".to_string()));
        }
        Ok(RdbcReader::from_utf16(state.code_units.clone()))
    }

    async fn get_ascii_stream(&self) -> Result<RdbcInputStream, DruidError> {
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
        Ok(RdbcInputStream::from_bytes(bytes))
    }

    async fn position_string(
        &self,
        pattern: &RdbcString,
        start: i64,
    ) -> Result<Option<i64>, DruidError> {
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

    async fn position_clob(
        &self,
        pattern: &RdbcClob,
        start: i64,
    ) -> Result<Option<i64>, DruidError> {
        let length = i32::try_from(pattern.length().await?)
            .map_err(|_| DruidError::DriverError("pattern Clob is too large".to_string()))?;
        let value = pattern.get_sub_string(1, length).await?;
        self.position_string(&value, start).await
    }

    async fn set_string(&self, position: i64, value: &RdbcString) -> Result<i32, DruidError> {
        let length = i32::try_from(value.len())
            .map_err(|_| DruidError::DriverError("Clob write is too large".to_string()))?;
        self.set_string_range(position, value, 0, length).await
    }

    async fn set_string_range(
        &self,
        position: i64,
        value: &RdbcString,
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

    async fn set_ascii_stream(&self, position: i64) -> Result<RdbcOutputStream, DruidError> {
        let state = self.state();
        let position = Self::checked_start(&state, position)?;
        drop(state);
        Ok(RdbcOutputStream::new(ClobAsciiWriter {
            state: self.state.clone(),
            position,
        }))
    }

    async fn set_character_stream(&self, position: i64) -> Result<RdbcWriter, DruidError> {
        let state = self.state();
        let position = Self::checked_start(&state, position)?;
        drop(state);
        Ok(RdbcWriter::new(ClobCharacterWriter {
            state: self.state.clone(),
            position,
        }))
    }

    async fn truncate(&self, length: i64) -> Result<(), DruidError> {
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

    async fn get_character_stream_range(
        &self,
        position: i64,
        length: i64,
    ) -> Result<RdbcReader, DruidError> {
        let state = self.state();
        let start = Self::checked_start(&state, position)?;
        let length = Self::checked_length(length)?;
        let end = start
            .checked_add(length)
            .filter(|end| *end <= state.code_units.len())
            .ok_or_else(|| DruidError::DriverError("Clob range is invalid".to_string()))?;
        Ok(RdbcReader::from_utf16(
            state.code_units[start..end].to_vec(),
        ))
    }
}

impl RdbcNClobAccess for InMemoryClobAccess {}

fn clob(value: impl Into<RdbcString>) -> RdbcClob {
    RdbcResourceFactory::clob(Arc::new(InMemoryClobAccess::new(value.into())))
}

fn n_clob(value: impl Into<RdbcString>) -> RdbcNClob {
    RdbcResourceFactory::n_clob(Arc::new(InMemoryClobAccess::new(value.into())))
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
fn rdbc_string_and_reader_preserve_utf16_identity_cursor_and_close() {
    let supplementary = RdbcString::from("A😀B");
    assert_eq!(supplementary.len(), 4);
    assert!(!supplementary.is_empty());
    assert_eq!(supplementary.to_rust_string().unwrap(), "A😀B");
    assert!(format!("{supplementary:?}").contains("utf16_length"));
    let owned = RdbcString::from(String::from("owned"));
    assert_eq!(owned.to_rust_string().unwrap(), "owned");

    let invalid = RdbcString::from_utf16(vec![0xD800]);
    assert!(invalid.to_rust_string().is_err());

    let reader = RdbcReader::from_utf16(supplementary.as_utf16().to_vec());
    let clone = reader.clone();
    assert_eq!(reader, clone);
    assert_ne!(reader, RdbcReader::from_string("A😀B"));
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

    let invalid_reader = RdbcReader::from_utf16(vec![0xD800]);
    assert!(invalid_reader.read_to_string().is_err());
    let failing = RdbcReader::new(FailingReader { fail_close: true });
    assert!(failing.read_utf16(&mut first).is_err());
    assert!(failing.close().is_err());
    assert!(failing.is_closed());
}

#[tokio::test]
async fn clob_and_n_clob_delegate_complete_rdbc_character_resource_contract() {
    let value = clob("abcdef");
    assert_eq!(value, value.clone());
    assert_ne!(value, clob("abcdef"));
    assert!(format!("{value:?}").contains("RdbcClob"));
    assert!(value
        .capabilities()
        .contains(RdbcResourceCapabilities::STREAM));
    assert_eq!(value.length().await.unwrap(), 6);
    assert_eq!(
        value
            .get_sub_string(2, 3)
            .await
            .unwrap()
            .to_rust_string()
            .unwrap(),
        "bcd"
    );
    assert_eq!(
        value
            .get_character_stream()
            .await
            .unwrap()
            .read_to_string()
            .unwrap(),
        "abcdef"
    );
    assert_eq!(
        value
            .get_ascii_stream()
            .await
            .unwrap()
            .read_to_end()
            .unwrap(),
        b"abcdef"
    );
    assert_eq!(
        value
            .position_string(&RdbcString::from("cd"), 1)
            .await
            .unwrap(),
        Some(3)
    );
    assert_eq!(value.position_clob(&clob("de"), 1).await.unwrap(), Some(4));

    assert_eq!(
        value.set_string(2, &RdbcString::from("XY")).await.unwrap(),
        2
    );
    assert_eq!(
        value
            .set_string_range(4, &RdbcString::from("12ZZ"), 2, 2)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        value
            .get_sub_string(1, 6)
            .await
            .unwrap()
            .to_rust_string()
            .unwrap(),
        "aXYZZf"
    );

    let ascii_writer = value.set_ascii_stream(6).await.unwrap();
    assert_eq!(ascii_writer.write(b"789").unwrap(), 3);
    ascii_writer.close().unwrap();
    let character_writer = value.set_character_stream(2).await.unwrap();
    let character_writer_clone = character_writer.clone();
    assert_eq!(character_writer, character_writer_clone);
    assert_eq!(character_writer.write_str("中").unwrap(), 1);
    assert_eq!(
        character_writer_clone
            .write_utf16(RdbcString::from("文").as_utf16())
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
            .await
            .unwrap()
            .read_to_string()
            .unwrap(),
        "中文"
    );
    value.truncate(5).await.unwrap();
    assert_eq!(value.length().await.unwrap(), 5);
    assert!(value.get_sub_string(0, 1).await.is_err());
    assert!(value
        .set_string_range(1, &RdbcString::from("x"), -1, 1)
        .await
        .is_err());
    assert!(value.truncate(6).await.is_err());

    let national = n_clob("国家字符");
    assert_eq!(national, national.clone());
    assert_ne!(national, n_clob("国家字符"));
    assert_eq!(national.length().await.unwrap(), 4);
    assert_eq!(
        national
            .as_clob()
            .get_sub_string(1, 2)
            .await
            .unwrap()
            .to_rust_string()
            .unwrap(),
        "国家"
    );
    assert!(national
        .capabilities()
        .contains(RdbcResourceCapabilities::READ));
    assert!(format!("{national:?}").contains("RdbcNClob"));
    assert_eq!(format!("{}", RdbcObject::Clob(value.clone())), "<Clob>");
    assert_eq!(format!("{}", RdbcObject::NClob(national)), "<NClob>");
    assert_eq!(
        format!(
            "{}",
            RdbcObject::CharacterStream(RdbcReader::from_string("x"))
        ),
        "<CharacterStream>"
    );
    assert_eq!(
        format!(
            "{}",
            RdbcObject::NCharacterStream(RdbcReader::from_string("x"))
        ),
        "<NCharacterStream>"
    );

    value.free().await.unwrap();
    value.free().await.unwrap();
    assert!(value.is_freed());
    assert!(value.length().await.is_err());
    assert!(value.get_character_stream().await.is_err());
    assert!(value.get_ascii_stream().await.is_err());

    let failing_writer = RdbcWriter::new(FailingWriter);
    assert!(failing_writer.write_str("x").is_err());
    assert!(failing_writer.flush().is_err());
    assert!(failing_writer.close().is_err());
    assert!(failing_writer.is_closed());
}
