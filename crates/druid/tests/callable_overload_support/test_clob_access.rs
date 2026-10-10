#![allow(dead_code, unused_imports)]
use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::ClobPattern;
use druid_rdbc::resource::{
    Array, Blob, Clob, NClob, RdbcBlob, RdbcClob, RdbcInputStream, RdbcNClob, RdbcOutputStream,
    RdbcReader, RdbcWriter, RdbcXmlRepresentationType, RdbcXmlResult, RdbcXmlSource, Ref,
    ResourceHandleFactory, ResourceOperations, SqlXml,
};
use druid_rdbc::types::{RdbcObject, RdbcString, RdbcTypeMap, Value};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
/// 保留原始 Callable 测试资源逻辑。对应 Java: java.sql.CallableStatement。
/// Callable 契约测试使用的只读物理 Clob/NClob。
#[derive(Debug)]
pub(crate) struct TestClobAccess {
    code_units: Vec<u16>,
    freed: AtomicBool,
}

impl TestClobAccess {
    fn ensure_open(&self) -> Result<(), RdbcError> {
        if self.freed.load(Ordering::Acquire) {
            Err(RdbcError::DriverError("Clob has been freed".to_string()))
        } else {
            Ok(())
        }
    }

    fn start(&self, position: i64) -> Result<usize, RdbcError> {
        self.ensure_open()?;
        position
            .checked_sub(1)
            .and_then(|position| usize::try_from(position).ok())
            .filter(|position| *position <= self.code_units.len())
            .ok_or_else(|| RdbcError::DriverError("invalid Clob position".to_string()))
    }
}

#[async_trait::async_trait]
impl Clob for TestClobAccess {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::READ
            | ResourceOperations::RANGE
            | ResourceOperations::STREAM
            | ResourceOperations::SEARCH
            | ResourceOperations::FREE
    }

    async fn free(&mut self) -> Result<(), RdbcError> {
        self.freed.store(true, Ordering::Release);
        Ok(())
    }

    async fn length(&mut self) -> Result<i64, RdbcError> {
        self.ensure_open()?;
        Ok(i64::try_from(self.code_units.len()).expect("test Clob length fits i64"))
    }

    async fn get_sub_string(
        &mut self,
        position: i64,
        length: i32,
    ) -> Result<RdbcString, RdbcError> {
        let start = self.start(position)?;
        let length = usize::try_from(length)
            .map_err(|_| RdbcError::DriverError("invalid Clob length".to_string()))?;
        let end = start
            .checked_add(length)
            .map(|end| end.min(self.code_units.len()))
            .ok_or_else(|| RdbcError::DriverError("Clob range overflow".to_string()))?;
        Ok(RdbcString::from_utf16(self.code_units[start..end].to_vec()))
    }

    async fn get_character_stream(&mut self) -> Result<RdbcReader, RdbcError> {
        self.ensure_open()?;
        Ok(RdbcReader::from_utf16(self.code_units.clone()))
    }

    async fn get_ascii_stream(&mut self) -> Result<RdbcInputStream, RdbcError> {
        self.ensure_open()?;
        let bytes = self
            .code_units
            .iter()
            .map(|value| {
                u8::try_from(*value).map_err(|_| {
                    RdbcError::DriverError("test Clob contains non-ASCII data".to_string())
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(RdbcInputStream::from_bytes(bytes))
    }

    async fn position_string(
        &mut self,
        pattern: &RdbcString,
        start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        let start = self.start(start)?;
        let position = if pattern.is_empty() {
            Some(start)
        } else {
            self.code_units[start..]
                .windows(pattern.len())
                .position(|window| window == pattern.as_utf16())
                .map(|position| start + position)
        };
        Ok(position
            .map(|position| i64::try_from(position + 1).expect("test Clob position fits i64")))
    }

    async fn position_clob(
        &mut self,
        pattern: ClobPattern<'_>,
        start: i64,
    ) -> Result<Option<i64>, RdbcError> {
        let value = match pattern {
            ClobPattern::SameResource => {
                let length = i32::try_from(self.length().await?).map_err(|_| {
                    RdbcError::DriverError("test Clob pattern is too large".to_string())
                })?;
                let value = self.get_sub_string(1, length).await?;
                value
            }
            ClobPattern::Other(pattern) => {
                let length = i32::try_from(pattern.length().await?).map_err(|_| {
                    RdbcError::DriverError("test Clob pattern is too large".to_string())
                })?;
                let value = pattern.get_sub_string(1, length).await?;
                value
            }
        };
        self.position_string(&value, start).await
    }

    async fn set_string(&mut self, _position: i64, _value: &RdbcString) -> Result<i32, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "test_read_only_clob_set_string",
        })
    }

    async fn set_string_range(
        &mut self,
        _position: i64,
        _value: &RdbcString,
        _offset: i32,
        _length: i32,
    ) -> Result<i32, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "test_read_only_clob_set_string_range",
        })
    }

    async fn set_ascii_stream(&mut self, _position: i64) -> Result<RdbcOutputStream, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "test_read_only_clob_set_ascii_stream",
        })
    }

    async fn set_character_stream(&mut self, _position: i64) -> Result<RdbcWriter, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "test_read_only_clob_set_character_stream",
        })
    }

    async fn truncate(&mut self, _length: i64) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "test_read_only_clob_truncate",
        })
    }

    async fn get_character_stream_range(
        &mut self,
        position: i64,
        length: i64,
    ) -> Result<RdbcReader, RdbcError> {
        let length = i32::try_from(length)
            .map_err(|_| RdbcError::DriverError("invalid Clob range length".to_string()))?;
        Ok(RdbcReader::from_utf16(
            self.get_sub_string(position, length)
                .await?
                .as_utf16()
                .to_vec(),
        ))
    }
}

impl NClob for TestClobAccess {}

pub(crate) fn test_clob(value: &str) -> RdbcClob {
    ResourceHandleFactory::clob(Box::new(TestClobAccess {
        code_units: value.encode_utf16().collect(),
        freed: AtomicBool::new(false),
    }))
}

pub(crate) fn test_n_clob(value: &str) -> RdbcNClob {
    ResourceHandleFactory::n_clob(Box::new(TestClobAccess {
        code_units: value.encode_utf16().collect(),
        freed: AtomicBool::new(false),
    }))
}
