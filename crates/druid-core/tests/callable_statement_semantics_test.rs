//! Java Druid CallableStatement 池化语义纵向契约。
//!
//! Java oracle：
//! - `DruidPooledConnection#prepareCall(...)`
//! - `DruidPooledCallableStatement`
//! - `PoolableCallableStatementTest`
//! - `ConnectionTest4`

extern crate druid_core as druid;
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid::core::{
    CallableCalendar, CallableCalendarArgument, CallableInputParameter, CallableOutParameter,
    CallableParameter, DruidError, DruidPooledCallableStatement,
    DruidPooledCallableStatementHandle, ExecResult, PhysicalCallableStatement, PhysicalConnection,
    PhysicalPreparedStatement, PreparedStatementKey, PreparedStatementMethodType, RdbcBlob,
    RdbcCharacterLength, RdbcClob, RdbcInputStream, RdbcNClob, RdbcObject, RdbcOutputStream,
    RdbcReader, RdbcResultSet, RdbcRowId, RdbcStreamLength, RdbcString, RdbcTargetType,
    RdbcTypeMap, RdbcUrl, RdbcWriter, RdbcXmlRepresentationType, RdbcXmlResult, RdbcXmlSource,
    ResultSetStatement, Row, SqlTextPreparedStatement, StatementExecuteResult,
    StatementGeneratedKeys, Value, Wrapper, WrapperExt,
};
use druid::pool::DruidPool;
use druid::spi::{
    RdbcArrayAccess, RdbcBlobAccess, RdbcClobAccess, RdbcNClobAccess, RdbcRefAccess,
    RdbcResourceAccess, RdbcResourceCapabilities, RdbcResourceFactory, RdbcSqlXmlAccess,
};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
struct UnsupportedUnwrapType;

/// Callable 契约测试使用的只读物理 Blob。
#[derive(Debug)]
struct TestBlobAccess {
    bytes: Vec<u8>,
    freed: AtomicBool,
}

impl TestBlobAccess {
    fn ensure_open(&self) -> Result<(), DruidError> {
        if self.freed.load(Ordering::Acquire) {
            Err(DruidError::DriverError("Blob has been freed".to_string()))
        } else {
            Ok(())
        }
    }
}

#[async_trait::async_trait]
impl RdbcResourceAccess for TestBlobAccess {
    fn capabilities(&self) -> RdbcResourceCapabilities {
        RdbcResourceCapabilities::blob()
    }

    async fn free(&self) -> Result<(), DruidError> {
        self.freed.store(true, Ordering::Release);
        Ok(())
    }
}

#[async_trait::async_trait]
impl RdbcBlobAccess for TestBlobAccess {
    async fn length(&self) -> Result<i64, DruidError> {
        self.ensure_open()?;
        Ok(i64::try_from(self.bytes.len()).expect("test Blob length fits i64"))
    }

    async fn get_bytes(&self, position: i64, length: i32) -> Result<Vec<u8>, DruidError> {
        self.ensure_open()?;
        let start = usize::try_from(position - 1)
            .map_err(|_| DruidError::DriverError("invalid Blob position".to_string()))?;
        let length = usize::try_from(length)
            .map_err(|_| DruidError::DriverError("invalid Blob length".to_string()))?;
        let end = start
            .checked_add(length)
            .map(|end| end.min(self.bytes.len()))
            .ok_or_else(|| DruidError::DriverError("Blob range overflow".to_string()))?;
        self.bytes
            .get(start..end)
            .map(<[u8]>::to_vec)
            .ok_or_else(|| DruidError::DriverError("invalid Blob range".to_string()))
    }

    async fn get_binary_stream(&self) -> Result<RdbcInputStream, DruidError> {
        self.ensure_open()?;
        Ok(RdbcInputStream::from_bytes(self.bytes.clone()))
    }

    async fn position_bytes(&self, pattern: &[u8], start: i64) -> Result<Option<i64>, DruidError> {
        self.ensure_open()?;
        let start = usize::try_from(start - 1)
            .map_err(|_| DruidError::DriverError("invalid Blob position".to_string()))?;
        Ok(self
            .bytes
            .get(start..)
            .and_then(|bytes| {
                bytes
                    .windows(pattern.len())
                    .position(|window| window == pattern)
            })
            .map(|position| i64::try_from(start + position + 1).expect("test position fits i64")))
    }

    async fn position_blob(
        &self,
        pattern: &RdbcBlob,
        start: i64,
    ) -> Result<Option<i64>, DruidError> {
        let length = i32::try_from(pattern.length().await?)
            .map_err(|_| DruidError::DriverError("test pattern is too large".to_string()))?;
        let bytes = pattern.get_bytes(1, length).await?;
        self.position_bytes(&bytes, start).await
    }

    async fn set_bytes(&self, _position: i64, _bytes: &[u8]) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "test_read_only_blob_set_bytes",
        })
    }

    async fn set_bytes_range(
        &self,
        _position: i64,
        _bytes: &[u8],
        _offset: i32,
        _length: i32,
    ) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "test_read_only_blob_set_bytes_range",
        })
    }

    async fn set_binary_stream(&self, _position: i64) -> Result<RdbcOutputStream, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "test_read_only_blob_set_binary_stream",
        })
    }

    async fn truncate(&self, _length: i64) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "test_read_only_blob_truncate",
        })
    }

    async fn get_binary_stream_range(
        &self,
        position: i64,
        length: i64,
    ) -> Result<RdbcInputStream, DruidError> {
        let length = i32::try_from(length)
            .map_err(|_| DruidError::DriverError("invalid Blob range length".to_string()))?;
        Ok(RdbcInputStream::from_bytes(
            self.get_bytes(position, length).await?,
        ))
    }
}

fn test_blob(bytes: impl Into<Vec<u8>>) -> RdbcBlob {
    RdbcResourceFactory::blob(Arc::new(TestBlobAccess {
        bytes: bytes.into(),
        freed: AtomicBool::new(false),
    }))
}

/// Callable 契约测试使用的只读物理 Clob/NClob。
#[derive(Debug)]
struct TestClobAccess {
    code_units: Vec<u16>,
    freed: AtomicBool,
}

impl TestClobAccess {
    fn ensure_open(&self) -> Result<(), DruidError> {
        if self.freed.load(Ordering::Acquire) {
            Err(DruidError::DriverError("Clob has been freed".to_string()))
        } else {
            Ok(())
        }
    }

    fn start(&self, position: i64) -> Result<usize, DruidError> {
        self.ensure_open()?;
        position
            .checked_sub(1)
            .and_then(|position| usize::try_from(position).ok())
            .filter(|position| *position <= self.code_units.len())
            .ok_or_else(|| DruidError::DriverError("invalid Clob position".to_string()))
    }
}

#[async_trait::async_trait]
impl RdbcResourceAccess for TestClobAccess {
    fn capabilities(&self) -> RdbcResourceCapabilities {
        RdbcResourceCapabilities::clob()
    }

    async fn free(&self) -> Result<(), DruidError> {
        self.freed.store(true, Ordering::Release);
        Ok(())
    }
}

#[async_trait::async_trait]
impl RdbcClobAccess for TestClobAccess {
    async fn length(&self) -> Result<i64, DruidError> {
        self.ensure_open()?;
        Ok(i64::try_from(self.code_units.len()).expect("test Clob length fits i64"))
    }

    async fn get_sub_string(&self, position: i64, length: i32) -> Result<RdbcString, DruidError> {
        let start = self.start(position)?;
        let length = usize::try_from(length)
            .map_err(|_| DruidError::DriverError("invalid Clob length".to_string()))?;
        let end = start
            .checked_add(length)
            .map(|end| end.min(self.code_units.len()))
            .ok_or_else(|| DruidError::DriverError("Clob range overflow".to_string()))?;
        Ok(RdbcString::from_utf16(self.code_units[start..end].to_vec()))
    }

    async fn get_character_stream(&self) -> Result<RdbcReader, DruidError> {
        self.ensure_open()?;
        Ok(RdbcReader::from_utf16(self.code_units.clone()))
    }

    async fn get_ascii_stream(&self) -> Result<RdbcInputStream, DruidError> {
        self.ensure_open()?;
        let bytes = self
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
        &self,
        pattern: &RdbcClob,
        start: i64,
    ) -> Result<Option<i64>, DruidError> {
        let length = i32::try_from(pattern.length().await?)
            .map_err(|_| DruidError::DriverError("test Clob pattern is too large".to_string()))?;
        let value = pattern.get_sub_string(1, length).await?;
        self.position_string(&value, start).await
    }

    async fn set_string(&self, _position: i64, _value: &RdbcString) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "test_read_only_clob_set_string",
        })
    }

    async fn set_string_range(
        &self,
        _position: i64,
        _value: &RdbcString,
        _offset: i32,
        _length: i32,
    ) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "test_read_only_clob_set_string_range",
        })
    }

    async fn set_ascii_stream(&self, _position: i64) -> Result<RdbcOutputStream, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "test_read_only_clob_set_ascii_stream",
        })
    }

    async fn set_character_stream(&self, _position: i64) -> Result<RdbcWriter, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "test_read_only_clob_set_character_stream",
        })
    }

    async fn truncate(&self, _length: i64) -> Result<(), DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "test_read_only_clob_truncate",
        })
    }

    async fn get_character_stream_range(
        &self,
        position: i64,
        length: i64,
    ) -> Result<RdbcReader, DruidError> {
        let length = i32::try_from(length)
            .map_err(|_| DruidError::DriverError("invalid Clob range length".to_string()))?;
        Ok(RdbcReader::from_utf16(
            self.get_sub_string(position, length)
                .await?
                .as_utf16()
                .to_vec(),
        ))
    }
}

impl RdbcNClobAccess for TestClobAccess {}

fn test_clob(value: &str) -> RdbcClob {
    RdbcResourceFactory::clob(Arc::new(TestClobAccess {
        code_units: value.encode_utf16().collect(),
        freed: AtomicBool::new(false),
    }))
}

fn test_n_clob(value: &str) -> RdbcNClob {
    RdbcResourceFactory::n_clob(Arc::new(TestClobAccess {
        code_units: value.encode_utf16().collect(),
        freed: AtomicBool::new(false),
    }))
}

#[derive(Debug)]
struct TestRefAccess;

#[async_trait::async_trait]
impl RdbcResourceAccess for TestRefAccess {
    fn capabilities(&self) -> RdbcResourceCapabilities {
        RdbcResourceCapabilities::reference()
    }
}

#[async_trait::async_trait]
impl RdbcRefAccess for TestRefAccess {
    async fn base_type_name(&self) -> Result<String, DruidError> {
        Ok("schema.kind".to_string())
    }

    async fn object(&self) -> Result<RdbcObject, DruidError> {
        Ok(RdbcObject::from(Value::String("ref-value".to_string())))
    }

    async fn object_with_type_map(
        &self,
        _type_map: &RdbcTypeMap,
    ) -> Result<RdbcObject, DruidError> {
        self.object().await
    }

    async fn set_object(&self, _value: RdbcObject) -> Result<(), DruidError> {
        Ok(())
    }
}

#[derive(Debug)]
struct TestArrayAccess;

#[async_trait::async_trait]
impl RdbcResourceAccess for TestArrayAccess {
    fn capabilities(&self) -> RdbcResourceCapabilities {
        RdbcResourceCapabilities::array()
    }
}

#[async_trait::async_trait]
impl RdbcArrayAccess for TestArrayAccess {
    async fn base_type_name(&self) -> Result<String, DruidError> {
        Ok("INTEGER".to_string())
    }

    async fn base_type(&self) -> Result<i32, DruidError> {
        Ok(4)
    }

    async fn values(&self) -> Result<Vec<RdbcObject>, DruidError> {
        Ok(vec![RdbcObject::from(Value::Int(1))])
    }

    async fn values_with_type_map(
        &self,
        _type_map: &RdbcTypeMap,
    ) -> Result<Vec<RdbcObject>, DruidError> {
        self.values().await
    }

    async fn values_range(&self, _index: i64, _count: i32) -> Result<Vec<RdbcObject>, DruidError> {
        self.values().await
    }

    async fn values_range_with_type_map(
        &self,
        index: i64,
        count: i32,
        _type_map: &RdbcTypeMap,
    ) -> Result<Vec<RdbcObject>, DruidError> {
        self.values_range(index, count).await
    }

    async fn result_set(&self) -> Result<RdbcResultSet, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "test_array_result_set",
        })
    }

    async fn result_set_with_type_map(
        &self,
        _type_map: &RdbcTypeMap,
    ) -> Result<RdbcResultSet, DruidError> {
        self.result_set().await
    }

    async fn result_set_range(
        &self,
        _index: i64,
        _count: i32,
    ) -> Result<RdbcResultSet, DruidError> {
        self.result_set().await
    }

    async fn result_set_range_with_type_map(
        &self,
        index: i64,
        count: i32,
        _type_map: &RdbcTypeMap,
    ) -> Result<RdbcResultSet, DruidError> {
        self.result_set_range(index, count).await
    }
}

#[derive(Debug)]
struct TestSqlXmlAccess;

#[async_trait::async_trait]
impl RdbcResourceAccess for TestSqlXmlAccess {
    fn capabilities(&self) -> RdbcResourceCapabilities {
        RdbcResourceCapabilities::sql_xml()
    }
}

#[async_trait::async_trait]
impl RdbcSqlXmlAccess for TestSqlXmlAccess {
    async fn binary_stream(&self) -> Result<RdbcInputStream, DruidError> {
        Ok(RdbcInputStream::from_bytes(b"<x/>".to_vec()))
    }

    async fn set_binary_stream(&self) -> Result<RdbcOutputStream, DruidError> {
        Ok(RdbcOutputStream::new(Vec::<u8>::new()))
    }

    async fn character_stream(&self) -> Result<RdbcReader, DruidError> {
        Ok(RdbcReader::from_string("<x/>"))
    }

    async fn set_character_stream(&self) -> Result<RdbcWriter, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "test_sql_xml_writer",
        })
    }

    async fn string(&self) -> Result<RdbcString, DruidError> {
        Ok(RdbcString::from("<x/>"))
    }

    async fn set_string(&self, _value: &RdbcString) -> Result<(), DruidError> {
        Ok(())
    }

    async fn source(
        &self,
        _representation: &RdbcXmlRepresentationType,
    ) -> Result<RdbcXmlSource, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "test_sql_xml_source",
        })
    }

    async fn result(
        &self,
        _representation: &RdbcXmlRepresentationType,
    ) -> Result<RdbcXmlResult, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "test_sql_xml_result",
        })
    }
}

/// 测试驱动的物理 CallableStatement；验证池化层只依赖最小 SPI。
struct TestCallableStatement {
    sql: String,
    closed: AtomicBool,
    registrations: Mutex<HashMap<CallableParameter, CallableOutParameter>>,
    named_parameters: Mutex<HashMap<String, CallableInputParameter>>,
    outputs: HashMap<CallableParameter, RdbcObject>,
    calendar_reads: Mutex<Vec<(CallableParameter, CallableCalendarArgument)>>,
    type_map_reads: Mutex<Vec<(CallableParameter, Option<RdbcTypeMap>)>>,
    typed_reads: Mutex<Vec<(CallableParameter, RdbcTargetType)>>,
    last_was_null: AtomicBool,
}

impl TestCallableStatement {
    fn new(sql: impl Into<String>) -> Self {
        let date = NaiveDate::from_ymd_opt(2026, 7, 28).expect("valid test date");
        let time = NaiveTime::from_hms_nano_opt(19, 30, 45, 123_456_789).expect("valid test time");
        let timestamp = NaiveDateTime::new(date, time);
        let blob = test_blob(b"callable-blob".to_vec());
        let clob = test_clob("callable-clob");
        let n_clob = test_n_clob("国家字符");
        let url = RdbcUrl::new("https://example.test/callable");
        let reference = RdbcResourceFactory::reference(Arc::new(TestRefAccess));
        let array = RdbcResourceFactory::array(Arc::new(TestArrayAccess));
        let row_id = RdbcRowId::new(vec![1, 2, 3]);
        let sql_xml = RdbcResourceFactory::sql_xml(Arc::new(TestSqlXmlAccess));
        Self {
            sql: sql.into(),
            closed: AtomicBool::new(false),
            registrations: Mutex::new(HashMap::new()),
            named_parameters: Mutex::new(HashMap::new()),
            outputs: HashMap::from([
                (CallableParameter::Index(1), RdbcObject::from(Value::Int(7))),
                (CallableParameter::Index(2), RdbcObject::from(Value::Null)),
                (
                    CallableParameter::Index(4),
                    RdbcObject::from(Value::Int(300)),
                ),
                (
                    CallableParameter::Index(5),
                    RdbcObject::BigDecimal(
                        BigDecimal::from_str("123.4500").expect("valid decimal"),
                    ),
                ),
                (CallableParameter::Index(6), RdbcObject::Date(date)),
                (CallableParameter::Index(7), RdbcObject::Time(time)),
                (
                    CallableParameter::Index(8),
                    RdbcObject::Timestamp(timestamp),
                ),
                (CallableParameter::Index(9), RdbcObject::Blob(blob.clone())),
                (CallableParameter::Index(10), RdbcObject::Clob(clob.clone())),
                (
                    CallableParameter::Index(11),
                    RdbcObject::NClob(n_clob.clone()),
                ),
                (
                    CallableParameter::Index(12),
                    RdbcObject::CharacterStream(RdbcReader::from_string("reader-index")),
                ),
                (
                    CallableParameter::Index(13),
                    RdbcObject::NCharacterStream(RdbcReader::from_string("国字-index")),
                ),
                (
                    CallableParameter::Index(14),
                    RdbcObject::NString("国字-string-index".to_string()),
                ),
                (CallableParameter::Index(15), RdbcObject::Url(url.clone())),
                (
                    CallableParameter::Index(16),
                    RdbcObject::Ref(reference.clone()),
                ),
                (
                    CallableParameter::Index(17),
                    RdbcObject::Array(array.clone()),
                ),
                (
                    CallableParameter::Index(18),
                    RdbcObject::RowId(row_id.clone()),
                ),
                (
                    CallableParameter::Index(19),
                    RdbcObject::SqlXml(sql_xml.clone()),
                ),
                (
                    CallableParameter::Name("name".to_string()),
                    RdbcObject::from(Value::String("druid".to_string())),
                ),
                (
                    CallableParameter::Name("flag".to_string()),
                    RdbcObject::from(Value::Bool(true)),
                ),
                (
                    CallableParameter::Name("count".to_string()),
                    RdbcObject::from(Value::Int(9)),
                ),
                (
                    CallableParameter::Name("ratio".to_string()),
                    RdbcObject::from(Value::Float(1.5)),
                ),
                (
                    CallableParameter::Name("bytes".to_string()),
                    RdbcObject::from(Value::Bytes(vec![1, 2, 3])),
                ),
                (
                    CallableParameter::Name("decimal".to_string()),
                    RdbcObject::BigDecimal(
                        BigDecimal::from_str("123.4500").expect("valid decimal"),
                    ),
                ),
                (
                    CallableParameter::Name("date".to_string()),
                    RdbcObject::Date(date),
                ),
                (
                    CallableParameter::Name("time".to_string()),
                    RdbcObject::Time(time),
                ),
                (
                    CallableParameter::Name("timestamp".to_string()),
                    RdbcObject::Timestamp(timestamp),
                ),
                (
                    CallableParameter::Name("blob".to_string()),
                    RdbcObject::Blob(blob),
                ),
                (
                    CallableParameter::Name("clob".to_string()),
                    RdbcObject::Clob(clob),
                ),
                (
                    CallableParameter::Name("n_clob".to_string()),
                    RdbcObject::NClob(n_clob),
                ),
                (
                    CallableParameter::Name("character_stream".to_string()),
                    RdbcObject::CharacterStream(RdbcReader::from_string("reader-name")),
                ),
                (
                    CallableParameter::Name("n_character_stream".to_string()),
                    RdbcObject::NCharacterStream(RdbcReader::from_string("国字-name")),
                ),
                (
                    CallableParameter::Name("n_string".to_string()),
                    RdbcObject::NString("国字-string-name".to_string()),
                ),
                (
                    CallableParameter::Name("url".to_string()),
                    RdbcObject::Url(url),
                ),
                (
                    CallableParameter::Name("ref".to_string()),
                    RdbcObject::Ref(reference),
                ),
                (
                    CallableParameter::Name("array".to_string()),
                    RdbcObject::Array(array),
                ),
                (
                    CallableParameter::Name("row_id".to_string()),
                    RdbcObject::RowId(row_id),
                ),
                (
                    CallableParameter::Name("sql_xml".to_string()),
                    RdbcObject::SqlXml(sql_xml),
                ),
            ]),
            calendar_reads: Mutex::new(Vec::new()),
            type_map_reads: Mutex::new(Vec::new()),
            typed_reads: Mutex::new(Vec::new()),
            last_was_null: AtomicBool::new(false),
        }
    }
}

impl PhysicalPreparedStatement for TestCallableStatement {
    fn sql(&self) -> &str {
        &self.sql
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_callable(&self) -> Option<&dyn PhysicalCallableStatement> {
        Some(self)
    }

    fn clear_parameters(&self) -> Result<(), DruidError> {
        self.named_parameters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
        Ok(())
    }

    fn close(&self) -> Result<(), DruidError> {
        self.closed.store(true, Ordering::Release);
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
}

impl PhysicalCallableStatement for TestCallableStatement {
    fn register_out_parameter(
        &self,
        parameter: CallableParameter,
        out_parameter: CallableOutParameter,
    ) -> Result<(), DruidError> {
        self.registrations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(parameter, out_parameter);
        Ok(())
    }

    fn set_named_parameter(
        &self,
        parameter_name: &str,
        parameter: CallableInputParameter,
    ) -> Result<(), DruidError> {
        self.named_parameters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(parameter_name.to_string(), parameter);
        Ok(())
    }

    fn out_parameter(&self, parameter: &CallableParameter) -> Result<RdbcObject, DruidError> {
        let value = self.outputs.get(parameter).cloned().ok_or_else(|| {
            DruidError::DriverError(format!("OUT parameter {parameter:?} is unavailable"))
        })?;
        self.last_was_null.store(value.is_null(), Ordering::Release);
        Ok(value)
    }

    fn out_parameter_with_type_map(
        &self,
        parameter: &CallableParameter,
        type_map: Option<&RdbcTypeMap>,
    ) -> Result<RdbcObject, DruidError> {
        self.type_map_reads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((parameter.clone(), type_map.cloned()));
        self.out_parameter(parameter)
    }

    fn out_parameter_as(
        &self,
        parameter: &CallableParameter,
        target_type: &RdbcTargetType,
    ) -> Result<RdbcObject, DruidError> {
        self.typed_reads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((parameter.clone(), target_type.clone()));
        self.out_parameter(parameter)
    }

    fn was_null(&self) -> Result<bool, DruidError> {
        Ok(self.last_was_null.load(Ordering::Acquire))
    }

    fn date_out_parameter(
        &self,
        parameter: &CallableParameter,
        calendar: &CallableCalendarArgument,
    ) -> Result<Option<NaiveDate>, DruidError> {
        self.calendar_reads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((parameter.clone(), calendar.clone()));
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::Date(value) => Ok(Some(value)),
            other => Err(DruidError::DriverError(format!(
                "expected Date, got {other}"
            ))),
        }
    }

    fn time_out_parameter(
        &self,
        parameter: &CallableParameter,
        calendar: &CallableCalendarArgument,
    ) -> Result<Option<NaiveTime>, DruidError> {
        self.calendar_reads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((parameter.clone(), calendar.clone()));
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::Time(value) => Ok(Some(value)),
            other => Err(DruidError::DriverError(format!(
                "expected Time, got {other}"
            ))),
        }
    }

    fn timestamp_out_parameter(
        &self,
        parameter: &CallableParameter,
        calendar: &CallableCalendarArgument,
    ) -> Result<Option<NaiveDateTime>, DruidError> {
        self.calendar_reads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((parameter.clone(), calendar.clone()));
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::Timestamp(value) => Ok(Some(value)),
            other => Err(DruidError::DriverError(format!(
                "expected Timestamp, got {other}"
            ))),
        }
    }
}

struct CallableConnection {
    prepare_count: Arc<AtomicU64>,
    prepared_keys: Arc<Mutex<Vec<PreparedStatementKey>>>,
    schema: String,
    closed: bool,
}

#[async_trait::async_trait]
impl PhysicalConnection for CallableConnection {
    async fn exec(&mut self, _sql: &str, _params: Vec<Value>) -> Result<ExecResult, DruidError> {
        Ok(ExecResult {
            rows_affected: 1,
            last_insert_id: None,
            row_count: None,
        })
    }

    async fn fetch(&mut self, _sql: &str, _params: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        Ok(vec![Row::new(vec![Value::Int(1)])])
    }

    async fn execute(
        &mut self,
        sql: &str,
        _params: Vec<Value>,
        _generated_keys: StatementGeneratedKeys,
    ) -> Result<Vec<StatementExecuteResult>, DruidError> {
        if sql == "{call multi()}" {
            Ok(vec![
                StatementExecuteResult::ResultSet(vec![Row::new(vec![Value::Int(10)])]),
                StatementExecuteResult::Update(ExecResult {
                    rows_affected: 2,
                    last_insert_id: None,
                    row_count: None,
                }),
                StatementExecuteResult::ResultSet(vec![Row::new(vec![Value::Int(20)])]),
            ])
        } else {
            Ok(vec![StatementExecuteResult::Update(ExecResult {
                rows_affected: 1,
                last_insert_id: None,
                row_count: None,
            })])
        }
    }

    async fn prepare_physical_call(
        &mut self,
        key: &PreparedStatementKey,
    ) -> Result<Arc<dyn PhysicalPreparedStatement>, DruidError> {
        self.prepare_count.fetch_add(1, Ordering::Relaxed);
        self.prepared_keys
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(key.clone());
        match key.sql() {
            "unsupported" => Err(DruidError::UnsupportedOperation {
                operation: "test_prepare_call",
            }),
            "ordinary-handle" => Ok(Arc::new(SqlTextPreparedStatement::new(key.sql()))),
            _ => Ok(Arc::new(TestCallableStatement::new(key.sql()))),
        }
    }

    async fn begin(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    async fn commit(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    async fn ping(&mut self) -> Result<(), DruidError> {
        Ok(())
    }

    async fn close(&mut self) -> Result<(), DruidError> {
        self.closed = true;
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.closed
    }

    fn schema(&self) -> Option<&str> {
        Some(&self.schema)
    }

    async fn set_schema(&mut self, schema: &str) -> Result<(), DruidError> {
        self.schema = schema.to_string();
        Ok(())
    }
}

struct CallableFactory {
    prepare_count: Arc<AtomicU64>,
    prepared_keys: Arc<Mutex<Vec<PreparedStatementKey>>>,
}

#[async_trait::async_trait]
impl druid::core::PhysicalConnectionFactory for CallableFactory {
    async fn create(&self) -> Result<Box<dyn PhysicalConnection>, DruidError> {
        Ok(Box::new(CallableConnection {
            prepare_count: self.prepare_count.clone(),
            prepared_keys: self.prepared_keys.clone(),
            schema: "main".to_string(),
            closed: false,
        }))
    }

    async fn validate(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
    ) -> Result<(), DruidError> {
        connection.ping().await
    }

    async fn close(&self, connection: &mut Box<dyn PhysicalConnection>) -> Result<(), DruidError> {
        connection.close().await
    }
}

async fn callable_pool() -> (
    DruidPool,
    Arc<AtomicU64>,
    Arc<Mutex<Vec<PreparedStatementKey>>>,
) {
    let prepare_count = Arc::new(AtomicU64::new(0));
    let prepared_keys = Arc::new(Mutex::new(Vec::new()));
    let pool = DruidPool::builder()
        .name("callable")
        .db_type_name("mysql")
        .factory(Arc::new(CallableFactory {
            prepare_count: prepare_count.clone(),
            prepared_keys: prepared_keys.clone(),
        }))
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(8)
        .build()
        .await
        .unwrap();
    (pool, prepare_count, prepared_keys)
}

#[tokio::test]
async fn callable_result_set_keeps_the_same_dynamic_statement_identity() {
    let (pool, _, _) = callable_pool().await;
    let mut connection = pool.get().await.unwrap();
    let mut callable = connection.prepare_call("{call query()}").await.unwrap();
    let expected_key = callable.key().clone();
    let mut result_set = callable
        .fetch_result_set(&mut connection, Vec::new())
        .await
        .unwrap();

    let identity = result_set
        .callable_statement()
        .expect("CallableStatement ResultSet 必须保留 callable 动态身份");
    assert!(identity.is_same_statement(&callable));
    assert_eq!(identity.key(), &expected_key);
    assert!(result_set.prepared_statement().is_some());
    assert!(identity.is_wrapper_for_type::<DruidPooledCallableStatementHandle>());
    assert!(identity
        .unwrap_ref::<DruidPooledCallableStatementHandle>()
        .is_some());
    assert!(identity.is_wrapper_for_type::<dyn PhysicalCallableStatement>());
    assert!(identity
        .unwrap(Some(TypeId::of::<dyn PhysicalCallableStatement>()))
        .and_then(|value| value.callable_statement())
        .is_some());
    assert!(identity.is_wrapper_for_type::<dyn PhysicalPreparedStatement>());
    assert!(identity.is_wrapper_for_type::<TestCallableStatement>());
    assert!(identity.unwrap_ref::<TestCallableStatement>().is_some());
    assert!(!identity.is_wrapper_for(None));
    assert!(identity.unwrap(None).is_none());
    assert!(!identity.is_wrapper_for_type::<UnsupportedUnwrapType>());
    assert!(identity.unwrap_ref::<UnsupportedUnwrapType>().is_none());
    let statement_object = result_set.statement_object(&mut connection).unwrap();
    assert!(matches!(statement_object, ResultSetStatement::Callable(_)));
    assert!(statement_object
        .callable_statement()
        .expect("动态平台对象必须保留 CallableStatement 身份")
        .is_same_statement(&callable));
    assert!(
        statement_object
            .prepared_statement()
            .expect("CallableStatement 必须保留继承的 PreparedStatement 身份")
            .key()
            == &expected_key
    );
    assert!(statement_object
        .pooled_statement()
        .is_same_statement(result_set.statement()));
    assert!(!statement_object.is_closed());

    drop(callable);
    assert!(!result_set.callable_statement().unwrap().is_closed());
    assert!(result_set.next(&mut connection).unwrap());
    assert_eq!(
        result_set.object(&mut connection, 1).unwrap(),
        Value::Int(1)
    );
    result_set.callable_statement().unwrap().close().unwrap();
    result_set.callable_statement().unwrap().close().unwrap();
    assert!(statement_object.is_closed());
    assert!(result_set.callable_statement().unwrap().is_closed());
    assert!(result_set.is_closed());

    connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn callable_generic_execute_preserves_inherited_ordered_results() {
    let (pool, _, _) = callable_pool().await;
    let mut connection = pool.get().await.unwrap();
    let mut callable = connection.prepare_call("{call multi()}").await.unwrap();

    assert!(callable.execute(&mut connection, Vec::new()).await.unwrap());
    assert_eq!(callable.update_count(&mut connection).unwrap(), -1);
    let mut first = callable.result_set(&mut connection).unwrap().unwrap();
    assert!(first
        .callable_statement()
        .unwrap()
        .is_same_statement(&callable));
    assert!(first.next(&mut connection).unwrap());
    assert_eq!(first.object(&mut connection, 1).unwrap(), Value::Int(10));

    assert!(!callable.more_results(&mut connection).unwrap());
    assert!(first.is_closed());
    assert_eq!(callable.update_count(&mut connection).unwrap(), 2);
    assert!(callable
        .more_results_with_current(&mut connection, 1)
        .unwrap());
    let mut second = callable.result_set(&mut connection).unwrap().unwrap();
    assert!(second.callable_statement().is_some());
    assert!(second.next(&mut connection).unwrap());
    assert_eq!(second.object(&mut connection, 1).unwrap(), Value::Int(20));

    callable.close().unwrap();
    connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
#[allow(deprecated)]
async fn prepare_call_overloads_preserve_keys_delegation_and_cache_lifecycle() {
    let (pool, prepare_count, prepared_keys) = callable_pool().await;
    let mut connection = pool.get().await.unwrap();

    let mut callable = connection.prepare_call("{call demo(?, ?)}").await.unwrap();
    let decimal = BigDecimal::from_str("123.4500").expect("valid decimal");
    let date = NaiveDate::from_ymd_opt(2026, 7, 28).expect("valid date");
    let time =
        NaiveTime::from_hms_nano_opt(19, 30, 45, 123_456_789).expect("valid nanosecond time");
    let timestamp = NaiveDateTime::new(date, time);
    let shanghai = CallableCalendar::new("Asia/Shanghai").unwrap();
    let utc = CallableCalendar::new("UTC").unwrap();
    let input_blob = test_blob(b"input-blob".to_vec());
    let input_stream = RdbcInputStream::from_bytes(b"stream-content".to_vec());
    let input_clob = test_clob("input-clob");
    let input_n_clob = test_n_clob("输入国家字符");
    let clob_reader = RdbcReader::from_string("clob-reader");
    let n_clob_reader = RdbcReader::from_string("nclob-reader");
    let character_reader = RdbcReader::from_string("character-reader");
    let character_reader_long = RdbcReader::from_string("character-reader-long");
    let n_character_reader = RdbcReader::from_string("国家字符-reader");
    let ascii_stream = RdbcInputStream::from_bytes(b"ascii-stream".to_vec());
    let ascii_stream_long = RdbcInputStream::from_bytes(b"ascii-stream-long".to_vec());
    let binary_stream = RdbcInputStream::from_bytes(vec![0, 1, 2]);
    let binary_stream_long = RdbcInputStream::from_bytes(vec![3, 4, 5]);
    assert_eq!(
        callable.key().method_type(),
        PreparedStatementMethodType::Precall1
    );
    callable.register_out_parameter(1, 4).unwrap();
    callable.register_out_parameter_with_scale(2, 3, 2).unwrap();
    callable
        .register_out_parameter_with_type_name(3, 2002, "schema.kind")
        .unwrap();
    callable.register_named_out_parameter("name", 12).unwrap();
    callable
        .register_named_out_parameter_with_scale("count", 3, 0)
        .unwrap();
    callable
        .register_named_out_parameter_with_type_name("flag", 16, "boolean")
        .unwrap();

    callable.set_named_null("nullable", 4).unwrap();
    callable
        .set_named_null_with_type_name("typed_null", 2002, "schema.kind")
        .unwrap();
    callable.set_named_boolean("in_bool", true).unwrap();
    callable.set_named_byte("in_byte", 8).unwrap();
    callable.set_named_short("in_short", 16).unwrap();
    callable.set_named_int("in_int", 1).unwrap();
    callable.set_named_long("in_long", 2).unwrap();
    callable.set_named_float("in_float", 1.25).unwrap();
    callable.set_named_double("in_double", 2.5).unwrap();
    callable
        .set_named_string("in_string", Some("value".to_string()))
        .unwrap();
    callable.set_named_string("in_string_null", None).unwrap();
    callable
        .set_named_n_string("in_nstring", Some("字符".to_string()))
        .unwrap();
    callable
        .set_named_n_string("in_nstring_null", None)
        .unwrap();
    callable
        .set_named_bytes("in_bytes", Some(vec![8, 9]))
        .unwrap();
    callable.set_named_bytes("in_bytes_null", None).unwrap();
    callable.set_named_url("in_url", None).unwrap();
    callable.set_named_row_id("in_row_id", None).unwrap();
    callable.set_named_sql_xml("in_sql_xml", None).unwrap();
    callable
        .set_named_ascii_stream("in_ascii_stream", Some(ascii_stream.clone()))
        .unwrap();
    callable
        .set_named_ascii_stream_with_int_length("in_ascii_stream_int", None, -7)
        .unwrap();
    callable
        .set_named_ascii_stream_with_length(
            "in_ascii_stream_long",
            Some(ascii_stream_long.clone()),
            -8,
        )
        .unwrap();
    callable
        .set_named_binary_stream("in_binary_stream", Some(binary_stream.clone()))
        .unwrap();
    callable
        .set_named_binary_stream_with_int_length("in_binary_stream_int", None, -9)
        .unwrap();
    callable
        .set_named_binary_stream_with_length(
            "in_binary_stream_long",
            Some(binary_stream_long.clone()),
            -10,
        )
        .unwrap();
    callable
        .set_named_blob("in_blob", Some(input_blob.clone()))
        .unwrap();
    callable.set_named_blob("in_blob_null", None).unwrap();
    callable
        .set_named_blob_stream("in_blob_stream", Some(input_stream.clone()))
        .unwrap();
    callable
        .set_named_blob_stream_with_length("in_blob_stream_length", None, -1)
        .unwrap();
    callable
        .set_named_clob("in_clob", Some(input_clob.clone()))
        .unwrap();
    callable
        .set_named_clob_reader("in_clob_reader", Some(clob_reader.clone()))
        .unwrap();
    callable
        .set_named_clob_reader_with_length("in_clob_reader_length", None, -2)
        .unwrap();
    callable
        .set_named_n_clob("in_n_clob", Some(input_n_clob.clone()))
        .unwrap();
    callable
        .set_named_n_clob_reader("in_n_clob_reader", Some(n_clob_reader.clone()))
        .unwrap();
    callable
        .set_named_n_clob_reader_with_length("in_n_clob_reader_length", None, -3)
        .unwrap();
    callable
        .set_named_character_stream("in_character_stream", Some(character_reader.clone()))
        .unwrap();
    callable
        .set_named_character_stream_with_int_length("in_character_stream_int", None, -4)
        .unwrap();
    callable
        .set_named_character_stream_with_length(
            "in_character_stream_long",
            Some(character_reader_long.clone()),
            -5,
        )
        .unwrap();
    callable
        .set_named_n_character_stream("in_n_character_stream", Some(n_character_reader.clone()))
        .unwrap();
    callable
        .set_named_n_character_stream_with_length("in_n_character_stream_long", None, -6)
        .unwrap();
    callable
        .set_named_object("in_object", Value::String("object".to_string()))
        .unwrap();
    callable
        .set_named_object_with_sql_type("in_typed_object", Value::Int(11), 4)
        .unwrap();
    callable
        .set_named_object_with_sql_type_and_scale("in_scaled_object", Value::Float(3.5), 3, 2)
        .unwrap();
    callable
        .set_named_big_decimal("in_decimal", Some(decimal.clone()))
        .unwrap();
    callable.set_named_date("in_date", Some(date)).unwrap();
    callable
        .set_named_date_with_calendar("in_date_cal", None, Some(shanghai.clone()))
        .unwrap();
    callable.set_named_time("in_time", Some(time)).unwrap();
    callable
        .set_named_time_with_calendar("in_time_cal", Some(time), None)
        .unwrap();
    callable
        .set_named_timestamp("in_timestamp", Some(timestamp))
        .unwrap();
    callable
        .set_named_timestamp_with_calendar("in_timestamp_cal", Some(timestamp), Some(utc.clone()))
        .unwrap();

    assert_eq!(callable.get_byte(1).unwrap(), 7);
    assert_eq!(callable.get_short(1).unwrap(), 7);
    assert_eq!(callable.get_int(1).unwrap(), 7);
    assert_eq!(callable.get_long(1).unwrap(), 7);
    assert_eq!(
        callable.get_object(2).unwrap(),
        RdbcObject::from(Value::Null)
    );
    assert!(callable.was_null().unwrap());
    assert_eq!(
        callable.get_named_string("name").unwrap().as_deref(),
        Some("druid")
    );
    assert!(callable.get_named_boolean("flag").unwrap());
    assert_eq!(callable.get_named_int("count").unwrap(), 9);
    assert_eq!(callable.get_named_float("ratio").unwrap(), 1.5);
    assert_eq!(callable.get_named_double("ratio").unwrap(), 1.5);
    assert_eq!(
        callable.get_named_bytes("bytes").unwrap(),
        Some(vec![1, 2, 3])
    );
    let output_blob = callable.get_blob(9).unwrap().unwrap();
    assert_eq!(output_blob.length().await.unwrap(), 13);
    assert_eq!(
        output_blob
            .get_binary_stream()
            .await
            .unwrap()
            .read_to_end()
            .unwrap(),
        b"callable-blob"
    );
    assert_eq!(
        callable.get_named_blob("blob").unwrap().unwrap(),
        output_blob
    );
    assert_eq!(callable.get_blob(2).unwrap(), None);
    let output_clob = callable.get_clob(10).unwrap().unwrap();
    assert_eq!(
        output_clob
            .get_character_stream()
            .await
            .unwrap()
            .read_to_string()
            .unwrap(),
        "callable-clob"
    );
    assert_eq!(
        callable.get_named_clob("clob").unwrap().unwrap(),
        output_clob
    );
    assert_eq!(callable.get_clob(2).unwrap(), None);
    let output_n_clob = callable.get_n_clob(11).unwrap().unwrap();
    assert_eq!(
        output_n_clob
            .get_character_stream()
            .await
            .unwrap()
            .read_to_string()
            .unwrap(),
        "国家字符"
    );
    assert_eq!(
        callable.get_named_n_clob("n_clob").unwrap().unwrap(),
        output_n_clob
    );
    assert_eq!(callable.get_n_clob(2).unwrap(), None);
    assert_eq!(
        callable
            .get_character_stream(12)
            .unwrap()
            .unwrap()
            .read_to_string()
            .unwrap(),
        "reader-index"
    );
    assert_eq!(
        callable
            .get_named_character_stream("character_stream")
            .unwrap()
            .unwrap()
            .read_to_string()
            .unwrap(),
        "reader-name"
    );
    assert!(callable.get_character_stream(2).unwrap().is_none());
    assert_eq!(
        callable
            .get_n_character_stream(13)
            .unwrap()
            .unwrap()
            .read_to_string()
            .unwrap(),
        "国字-index"
    );
    assert_eq!(
        callable
            .get_named_n_character_stream("n_character_stream")
            .unwrap()
            .unwrap()
            .read_to_string()
            .unwrap(),
        "国字-name"
    );
    assert!(callable.get_n_character_stream(2).unwrap().is_none());
    assert_eq!(
        callable.get_n_string(14).unwrap().as_deref(),
        Some("国字-string-index")
    );
    assert_eq!(
        callable.get_named_n_string("n_string").unwrap().as_deref(),
        Some("国字-string-name")
    );
    assert!(callable.get_n_string(2).unwrap().is_none());
    assert_eq!(
        callable.get_url(15).unwrap().unwrap().external_form(),
        "https://example.test/callable"
    );
    assert_eq!(
        callable
            .get_named_url("url")
            .unwrap()
            .unwrap()
            .external_form(),
        "https://example.test/callable"
    );
    assert_eq!(
        callable
            .get_ref(16)
            .unwrap()
            .unwrap()
            .base_type_name()
            .await
            .unwrap(),
        "schema.kind"
    );
    assert_eq!(
        callable
            .get_named_ref("ref")
            .unwrap()
            .unwrap()
            .object()
            .await
            .unwrap(),
        RdbcObject::from(Value::String("ref-value".to_string()))
    );
    assert_eq!(
        callable
            .get_array(17)
            .unwrap()
            .unwrap()
            .base_type()
            .await
            .unwrap(),
        4
    );
    assert_eq!(
        callable
            .get_named_array("array")
            .unwrap()
            .unwrap()
            .values()
            .await
            .unwrap(),
        vec![RdbcObject::from(Value::Int(1))]
    );
    assert_eq!(
        callable.get_row_id(18).unwrap().unwrap().bytes(),
        &[1, 2, 3]
    );
    assert_eq!(
        callable
            .get_named_row_id("row_id")
            .unwrap()
            .unwrap()
            .bytes(),
        &[1, 2, 3]
    );
    assert_eq!(
        callable
            .get_sql_xml(19)
            .unwrap()
            .unwrap()
            .string()
            .await
            .unwrap()
            .to_rust_string()
            .unwrap(),
        "<x/>"
    );
    assert_eq!(
        callable
            .get_named_sql_xml("sql_xml")
            .unwrap()
            .unwrap()
            .binary_stream()
            .await
            .unwrap()
            .read_to_end()
            .unwrap(),
        b"<x/>"
    );
    assert!(callable.get_url(2).unwrap().is_none());
    assert!(callable.get_ref(2).unwrap().is_none());
    assert!(callable.get_array(2).unwrap().is_none());
    assert!(callable.get_row_id(2).unwrap().is_none());
    assert!(callable.get_sql_xml(2).unwrap().is_none());

    let mut type_map = RdbcTypeMap::new();
    type_map.insert("schema.kind", RdbcTargetType::String);
    assert_eq!(
        callable
            .get_object_with_type_map(1, Some(&type_map))
            .unwrap(),
        RdbcObject::from(Value::Int(7))
    );
    assert_eq!(
        callable
            .get_named_object_with_type_map("name", None)
            .unwrap(),
        RdbcObject::from(Value::String("druid".to_string()))
    );
    assert_eq!(
        callable.get_object_as(1, &RdbcTargetType::Integer).unwrap(),
        RdbcObject::from(Value::Int(7))
    );
    assert_eq!(
        callable
            .get_named_object_as("name", &RdbcTargetType::String)
            .unwrap(),
        RdbcObject::from(Value::String("druid".to_string()))
    );
    assert!(!callable.is_wrapper_for(None));
    assert!(callable.unwrap(None).is_none());
    assert!(callable.is_wrapper_for_type::<DruidPooledCallableStatement>());
    assert!(callable
        .unwrap_ref::<DruidPooledCallableStatement>()
        .is_some());
    assert!(callable.is_wrapper_for_type::<TestCallableStatement>());
    assert!(callable.unwrap_ref::<TestCallableStatement>().is_some());

    let callable_interface = callable
        .unwrap(Some(TypeId::of::<dyn PhysicalCallableStatement>()))
        .expect("必须解包 CallableStatement 接口");
    assert_eq!(
        format!("{callable_interface:?}"),
        "Unwrapped::CallableStatement"
    );
    assert!(callable_interface.callable_statement().is_some());
    assert!(callable_interface.prepared_statement().is_none());
    assert!(callable_interface.physical_connection().is_none());
    assert!(callable_interface
        .downcast_ref::<TestCallableStatement>()
        .is_none());

    let prepared_interface = callable
        .unwrap(Some(TypeId::of::<dyn PhysicalPreparedStatement>()))
        .expect("CallableStatement 也必须解包 PreparedStatement 接口");
    assert_eq!(
        format!("{prepared_interface:?}"),
        "Unwrapped::PreparedStatement"
    );
    assert!(prepared_interface.prepared_statement().is_some());
    assert!(prepared_interface.callable_statement().is_none());
    assert!(prepared_interface.physical_connection().is_none());
    assert!(prepared_interface
        .downcast_ref::<TestCallableStatement>()
        .is_none());
    assert!(!callable.is_wrapper_for_type::<UnsupportedUnwrapType>());
    assert!(callable.unwrap_ref::<UnsupportedUnwrapType>().is_none());
    assert_eq!(callable.get_big_decimal(5).unwrap(), Some(decimal.clone()));
    assert_eq!(
        callable.get_big_decimal_with_scale(5, 2).unwrap(),
        Some(BigDecimal::from_str("123.45").unwrap())
    );
    assert_eq!(
        callable.get_named_big_decimal("decimal").unwrap(),
        Some(decimal)
    );
    assert_eq!(callable.get_date(6).unwrap(), Some(date));
    assert_eq!(
        callable.get_date_with_calendar(6, None).unwrap(),
        Some(date)
    );
    assert_eq!(callable.get_named_date("date").unwrap(), Some(date));
    assert_eq!(
        callable
            .get_named_date_with_calendar("date", Some(shanghai.clone()))
            .unwrap(),
        Some(date)
    );
    assert_eq!(callable.get_time(7).unwrap(), Some(time));
    assert_eq!(
        callable
            .get_time_with_calendar(7, Some(utc.clone()))
            .unwrap(),
        Some(time)
    );
    assert_eq!(callable.get_named_time("time").unwrap(), Some(time));
    assert_eq!(
        callable.get_named_time_with_calendar("time", None).unwrap(),
        Some(time)
    );
    assert_eq!(callable.get_timestamp(8).unwrap(), Some(timestamp));
    assert_eq!(
        callable
            .get_timestamp_with_calendar(8, Some(shanghai.clone()))
            .unwrap(),
        Some(timestamp)
    );
    assert_eq!(
        callable.get_named_timestamp("timestamp").unwrap(),
        Some(timestamp)
    );
    assert_eq!(
        callable
            .get_named_timestamp_with_calendar("timestamp", None)
            .unwrap(),
        Some(timestamp)
    );
    assert_eq!(
        callable
            .exec(&mut connection, vec![])
            .await
            .unwrap()
            .rows_affected,
        1
    );
    assert_eq!(
        callable.fetch(&mut connection, vec![]).await.unwrap().len(),
        1
    );

    let physical = callable
        .physical_callable_statement()
        .unwrap()
        .as_any()
        .downcast_ref::<TestCallableStatement>()
        .unwrap();
    {
        let registrations = physical
            .registrations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(registrations.len(), 6);
        assert_eq!(
            registrations
                .get(&CallableParameter::Index(2))
                .unwrap()
                .scale(),
            Some(2)
        );
        assert_eq!(
            registrations
                .get(&CallableParameter::Index(3))
                .unwrap()
                .type_name(),
            Some("schema.kind")
        );
    }
    let stored_stream = {
        let named_parameters = physical
            .named_parameters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(named_parameters.len(), 49);
        assert_eq!(
            named_parameters.get("nullable"),
            Some(&CallableInputParameter::Null {
                sql_type: 4,
                type_name: None,
            })
        );
        assert_eq!(
            named_parameters.get("typed_null"),
            Some(&CallableInputParameter::Null {
                sql_type: 2002,
                type_name: Some("schema.kind".to_string()),
            })
        );
        assert_eq!(
            named_parameters.get("in_byte"),
            Some(&CallableInputParameter::Byte(8))
        );
        assert_eq!(
            named_parameters.get("in_short"),
            Some(&CallableInputParameter::Short(16))
        );
        assert_eq!(
            named_parameters.get("in_float"),
            Some(&CallableInputParameter::Float(1.25))
        );
        assert_eq!(
            named_parameters.get("in_url"),
            Some(&CallableInputParameter::Url(None))
        );
        assert_eq!(
            named_parameters.get("in_row_id"),
            Some(&CallableInputParameter::RowId(None))
        );
        assert_eq!(
            named_parameters.get("in_sql_xml"),
            Some(&CallableInputParameter::SqlXml(None))
        );
        assert!(matches!(
            named_parameters.get("in_ascii_stream"),
            Some(CallableInputParameter::AsciiStream {
                stream: Some(_),
                length: RdbcStreamLength::Unspecified,
            })
        ));
        assert_eq!(
            named_parameters.get("in_ascii_stream_int"),
            Some(&CallableInputParameter::AsciiStream {
                stream: None,
                length: RdbcStreamLength::Int(-7),
            })
        );
        assert!(matches!(
            named_parameters.get("in_ascii_stream_long"),
            Some(CallableInputParameter::AsciiStream {
                stream: Some(_),
                length: RdbcStreamLength::Long(-8),
            })
        ));
        assert!(matches!(
            named_parameters.get("in_binary_stream"),
            Some(CallableInputParameter::BinaryStream {
                stream: Some(_),
                length: RdbcStreamLength::Unspecified,
            })
        ));
        assert_eq!(
            named_parameters.get("in_binary_stream_int"),
            Some(&CallableInputParameter::BinaryStream {
                stream: None,
                length: RdbcStreamLength::Int(-9),
            })
        );
        assert!(matches!(
            named_parameters.get("in_binary_stream_long"),
            Some(CallableInputParameter::BinaryStream {
                stream: Some(_),
                length: RdbcStreamLength::Long(-10),
            })
        ));
        assert_eq!(
            named_parameters.get("in_string"),
            Some(&CallableInputParameter::String(Some("value".to_string())))
        );
        assert_eq!(
            named_parameters.get("in_string_null"),
            Some(&CallableInputParameter::String(None))
        );
        assert_eq!(
            named_parameters.get("in_nstring"),
            Some(&CallableInputParameter::NString(Some("字符".to_string())))
        );
        assert_eq!(
            named_parameters.get("in_nstring_null"),
            Some(&CallableInputParameter::NString(None))
        );
        assert_eq!(
            named_parameters.get("in_bytes"),
            Some(&CallableInputParameter::Bytes(Some(vec![8, 9])))
        );
        assert_eq!(
            named_parameters.get("in_bytes_null"),
            Some(&CallableInputParameter::Bytes(None))
        );
        assert_eq!(
            named_parameters.get("in_typed_object"),
            Some(&CallableInputParameter::Object {
                value: Value::Int(11),
                target_sql_type: Some(4),
                scale: None,
            })
        );
        assert_eq!(
            named_parameters.get("in_scaled_object"),
            Some(&CallableInputParameter::Object {
                value: Value::Float(3.5),
                target_sql_type: Some(3),
                scale: Some(2),
            })
        );
        assert_eq!(
            named_parameters.get("in_decimal"),
            Some(&CallableInputParameter::BigDecimal(Some(
                BigDecimal::from_str("123.4500").unwrap()
            )))
        );
        assert_eq!(
            named_parameters.get("in_blob"),
            Some(&CallableInputParameter::Blob(Some(input_blob)))
        );
        assert_eq!(
            named_parameters.get("in_blob_null"),
            Some(&CallableInputParameter::Blob(None))
        );
        assert_eq!(
            named_parameters.get("in_blob_stream"),
            Some(&CallableInputParameter::BlobStream {
                stream: Some(input_stream.clone()),
                length: RdbcStreamLength::Unspecified,
            })
        );
        assert_eq!(
            named_parameters.get("in_blob_stream_length"),
            Some(&CallableInputParameter::BlobStream {
                stream: None,
                length: RdbcStreamLength::Long(-1),
            })
        );
        let stored_stream = match named_parameters.get("in_blob_stream").unwrap() {
            CallableInputParameter::BlobStream {
                stream: Some(stream),
                ..
            } => stream.clone(),
            other => panic!("expected BlobStream, got {other:?}"),
        };
        assert_eq!(
            named_parameters.get("in_clob"),
            Some(&CallableInputParameter::Clob(Some(input_clob)))
        );
        assert_eq!(
            named_parameters.get("in_clob_reader"),
            Some(&CallableInputParameter::ClobReader {
                reader: Some(clob_reader.clone()),
                length: RdbcCharacterLength::Unspecified,
            })
        );
        assert_eq!(
            named_parameters.get("in_clob_reader_length"),
            Some(&CallableInputParameter::ClobReader {
                reader: None,
                length: RdbcCharacterLength::Long(-2),
            })
        );
        assert_eq!(
            named_parameters.get("in_n_clob"),
            Some(&CallableInputParameter::NClob(Some(input_n_clob)))
        );
        assert_eq!(
            named_parameters.get("in_n_clob_reader"),
            Some(&CallableInputParameter::NClobReader {
                reader: Some(n_clob_reader.clone()),
                length: RdbcCharacterLength::Unspecified,
            })
        );
        assert_eq!(
            named_parameters.get("in_n_clob_reader_length"),
            Some(&CallableInputParameter::NClobReader {
                reader: None,
                length: RdbcCharacterLength::Long(-3),
            })
        );
        assert_eq!(
            named_parameters.get("in_character_stream"),
            Some(&CallableInputParameter::CharacterStream {
                reader: Some(character_reader.clone()),
                length: RdbcCharacterLength::Unspecified,
            })
        );
        assert_eq!(
            named_parameters.get("in_character_stream_int"),
            Some(&CallableInputParameter::CharacterStream {
                reader: None,
                length: RdbcCharacterLength::Int(-4),
            })
        );
        assert_eq!(
            named_parameters.get("in_character_stream_long"),
            Some(&CallableInputParameter::CharacterStream {
                reader: Some(character_reader_long.clone()),
                length: RdbcCharacterLength::Long(-5),
            })
        );
        assert_eq!(
            named_parameters.get("in_n_character_stream"),
            Some(&CallableInputParameter::NCharacterStream {
                reader: Some(n_character_reader.clone()),
                length: RdbcCharacterLength::Unspecified,
            })
        );
        assert_eq!(
            named_parameters.get("in_n_character_stream_long"),
            Some(&CallableInputParameter::NCharacterStream {
                reader: None,
                length: RdbcCharacterLength::Long(-6),
            })
        );
        assert_eq!(
            named_parameters.get("in_date"),
            Some(&CallableInputParameter::Date {
                value: Some(date),
                calendar: CallableCalendarArgument::Unspecified,
            })
        );
        assert_eq!(
            named_parameters.get("in_date_cal"),
            Some(&CallableInputParameter::Date {
                value: None,
                calendar: CallableCalendarArgument::Specified(Some(shanghai.clone())),
            })
        );
        assert_eq!(
            named_parameters.get("in_time_cal"),
            Some(&CallableInputParameter::Time {
                value: Some(time),
                calendar: CallableCalendarArgument::Specified(None),
            })
        );
        assert_eq!(
            named_parameters.get("in_timestamp_cal"),
            Some(&CallableInputParameter::Timestamp {
                value: Some(timestamp),
                calendar: CallableCalendarArgument::Specified(Some(utc)),
            })
        );
        stored_stream
    };
    assert_eq!(
        stored_stream.read_to_end().unwrap(),
        b"stream-content",
        "池化 setter 不得提前消费输入流"
    );
    assert_eq!(
        ascii_stream.read_to_end().unwrap(),
        b"ascii-stream",
        "ASCII setter 不得提前消费输入流"
    );
    assert_eq!(
        ascii_stream_long.read_to_end().unwrap(),
        b"ascii-stream-long"
    );
    assert_eq!(binary_stream.read_to_end().unwrap(), vec![0, 1, 2]);
    assert_eq!(binary_stream_long.read_to_end().unwrap(), vec![3, 4, 5]);
    assert_eq!(
        clob_reader.read_to_string().unwrap(),
        "clob-reader",
        "Clob setter 不得提前消费 Reader"
    );
    assert_eq!(
        n_clob_reader.read_to_string().unwrap(),
        "nclob-reader",
        "NClob setter 不得提前消费 Reader"
    );
    assert_eq!(
        character_reader.read_to_string().unwrap(),
        "character-reader",
        "CharacterStream setter 不得提前消费 Reader"
    );
    assert_eq!(
        character_reader_long.read_to_string().unwrap(),
        "character-reader-long"
    );
    assert_eq!(
        n_character_reader.read_to_string().unwrap(),
        "国家字符-reader"
    );
    {
        let calendar_reads = physical
            .calendar_reads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(calendar_reads.len(), 12);
        assert_eq!(
            calendar_reads[0],
            (
                CallableParameter::Index(6),
                CallableCalendarArgument::Unspecified
            )
        );
        assert_eq!(
            calendar_reads[1],
            (
                CallableParameter::Index(6),
                CallableCalendarArgument::Specified(None)
            )
        );
        assert_eq!(
            calendar_reads[3],
            (
                CallableParameter::Name("date".to_string()),
                CallableCalendarArgument::Specified(Some(shanghai))
            )
        );
    }
    {
        let type_map_reads = physical
            .type_map_reads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(type_map_reads.len(), 2);
        assert_eq!(type_map_reads[0].0, CallableParameter::Index(1));
        assert_eq!(type_map_reads[0].1.as_ref(), Some(&type_map));
        assert_eq!(
            type_map_reads[1],
            (CallableParameter::Name("name".to_string()), None)
        );
    }
    {
        let typed_reads = physical
            .typed_reads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(
            typed_reads.as_slice(),
            &[
                (CallableParameter::Index(1), RdbcTargetType::Integer),
                (
                    CallableParameter::Name("name".to_string()),
                    RdbcTargetType::String,
                ),
            ]
        );
    }
    callable.close().unwrap();
    assert!(callable.is_wrapper_for_type::<dyn PhysicalCallableStatement>());
    assert!(callable
        .unwrap(Some(TypeId::of::<dyn PhysicalCallableStatement>()))
        .is_some());
    drop(callable);

    let mut cached = connection.prepare_call("{call demo(?, ?)}").await.unwrap();
    assert_eq!(prepare_count.load(Ordering::Relaxed), 1);
    assert_eq!(
        cached
            .physical_callable_statement()
            .unwrap()
            .as_any()
            .downcast_ref::<TestCallableStatement>()
            .unwrap()
            .named_parameters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len(),
        0
    );
    cached.close().unwrap();

    let mut held = connection
        .prepare_call_with_holdability("{call demo()}", 1004, 1007, 2)
        .await
        .unwrap();
    assert_eq!(
        held.key().method_type(),
        PreparedStatementMethodType::Precall2
    );
    assert_eq!(held.key().result_set_type(), 1004);
    assert_eq!(held.key().result_set_concurrency(), 1007);
    assert_eq!(held.key().result_set_holdability(), 2);
    held.close().unwrap();

    let mut result_set = connection
        .prepare_call_with_result_set("{call demo()}", 1003, 1008)
        .await
        .unwrap();
    assert_eq!(
        result_set.key().method_type(),
        PreparedStatementMethodType::Precall3
    );
    assert_eq!(result_set.key().result_set_type(), 1003);
    assert_eq!(result_set.key().result_set_concurrency(), 1008);
    assert_eq!(result_set.key().result_set_holdability(), 0);
    result_set.close().unwrap();

    {
        let keys = prepared_keys
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(keys.len(), 3);
        assert_eq!(keys[0].catalog(), None);
        assert_eq!(keys[0].method_type(), PreparedStatementMethodType::Precall1);
        assert_eq!(keys[1].method_type(), PreparedStatementMethodType::Precall2);
        assert_eq!(keys[2].method_type(), PreparedStatementMethodType::Precall3);
    }

    let state = pool.state();
    assert_eq!(state.prepared_statement_count, 3);
    assert_eq!(state.cached_prepared_statement_count, 3);
    assert_eq!(state.cached_prepared_statement_hit_count, 1);
    assert_eq!(state.cached_prepared_statement_miss_count, 3);
    assert_eq!(state.cached_prepared_statement_access_count, 4);

    connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn callable_errors_invalidate_cache_and_non_callable_handles_are_rejected() {
    let (pool, prepare_count, _) = callable_pool().await;
    let mut connection = pool.get().await.unwrap();

    let mut invalid_index = connection.prepare_call("invalid-index").await.unwrap();
    assert!(invalid_index.register_out_parameter(0, 4).is_err());
    assert!(invalid_index.get_blob(0).is_err());
    assert!(invalid_index.get_clob(0).is_err());
    assert!(invalid_index.get_n_clob(0).is_err());
    assert!(invalid_index.get_character_stream(0).is_err());
    assert!(invalid_index.get_n_character_stream(0).is_err());
    assert!(invalid_index.get_n_string(0).is_err());
    assert!(invalid_index.get_url(0).is_err());
    assert!(invalid_index.get_ref(0).is_err());
    assert!(invalid_index.get_array(0).is_err());
    assert!(invalid_index.get_row_id(0).is_err());
    assert!(invalid_index.get_sql_xml(0).is_err());
    assert!(invalid_index.get_object_with_type_map(0, None).is_err());
    assert!(invalid_index
        .get_object_as(0, &RdbcTargetType::String)
        .is_err());
    invalid_index.close().unwrap();

    let mut invalid_name = connection.prepare_call("invalid-name").await.unwrap();
    assert!(invalid_name
        .set_named_string("", Some("value".to_string()))
        .is_err());
    assert!(invalid_name.set_named_blob("", None).is_err());
    assert!(invalid_name.set_named_url("", None).is_err());
    assert!(invalid_name.set_named_row_id("", None).is_err());
    assert!(invalid_name.set_named_sql_xml("", None).is_err());
    assert!(invalid_name.set_named_ascii_stream("", None).is_err());
    assert!(invalid_name
        .set_named_ascii_stream_with_int_length("", None, 1)
        .is_err());
    assert!(invalid_name
        .set_named_ascii_stream_with_length("", None, 1)
        .is_err());
    assert!(invalid_name.set_named_binary_stream("", None).is_err());
    assert!(invalid_name
        .set_named_binary_stream_with_int_length("", None, 1)
        .is_err());
    assert!(invalid_name
        .set_named_binary_stream_with_length("", None, 1)
        .is_err());
    assert!(invalid_name.set_named_clob("", None).is_err());
    assert!(invalid_name.set_named_clob_reader("", None).is_err());
    assert!(invalid_name
        .set_named_clob_reader_with_length("", None, 1)
        .is_err());
    assert!(invalid_name.set_named_n_clob("", None).is_err());
    assert!(invalid_name.set_named_n_clob_reader("", None).is_err());
    assert!(invalid_name
        .set_named_n_clob_reader_with_length("", None, 1)
        .is_err());
    assert!(invalid_name.set_named_character_stream("", None).is_err());
    assert!(invalid_name
        .set_named_character_stream_with_int_length("", None, 1)
        .is_err());
    assert!(invalid_name
        .set_named_character_stream_with_length("", None, 1)
        .is_err());
    assert!(invalid_name.set_named_n_character_stream("", None).is_err());
    assert!(invalid_name
        .set_named_n_character_stream_with_length("", None, 1)
        .is_err());
    invalid_name.close().unwrap();

    let mut unavailable = connection.prepare_call("missing-out").await.unwrap();
    assert!(unavailable.get_named_object("missing").is_err());
    assert!(unavailable.get_named_blob("missing").is_err());
    assert!(unavailable.get_named_clob("missing").is_err());
    assert!(unavailable.get_named_n_clob("missing").is_err());
    assert!(unavailable.get_named_character_stream("missing").is_err());
    assert!(unavailable.get_named_n_character_stream("missing").is_err());
    assert!(unavailable.get_named_n_string("missing").is_err());
    assert!(unavailable.get_named_url("missing").is_err());
    assert!(unavailable.get_named_ref("missing").is_err());
    assert!(unavailable.get_named_array("missing").is_err());
    assert!(unavailable.get_named_row_id("missing").is_err());
    assert!(unavailable.get_named_sql_xml("missing").is_err());
    assert!(unavailable
        .get_named_object_with_type_map("missing", None)
        .is_err());
    assert!(unavailable
        .get_named_object_as("missing", &RdbcTargetType::String)
        .is_err());
    unavailable.close().unwrap();

    let mut wrong_type = connection.prepare_call("wrong-type").await.unwrap();
    assert!(wrong_type.get_string(1).is_err());
    assert!(wrong_type.get_blob(1).is_err());
    assert!(wrong_type.get_clob(1).is_err());
    assert!(wrong_type.get_n_clob(1).is_err());
    assert!(wrong_type.get_character_stream(1).is_err());
    assert!(wrong_type.get_n_character_stream(1).is_err());
    assert!(wrong_type.get_n_string(1).is_err());
    assert!(wrong_type.get_url(1).is_err());
    assert!(wrong_type.get_ref(1).is_err());
    assert!(wrong_type.get_array(1).is_err());
    assert!(wrong_type.get_row_id(1).is_err());
    assert!(wrong_type.get_sql_xml(1).is_err());
    wrong_type.close().unwrap();

    let mut byte_overflow = connection.prepare_call("byte-overflow").await.unwrap();
    assert!(byte_overflow.get_byte(4).is_err());
    byte_overflow.close().unwrap();

    let wrong_handle = connection.prepare_call("ordinary-handle").await;
    assert!(matches!(
        wrong_handle,
        Err(DruidError::UnsupportedOperation {
            operation: "prepare_physical_call"
        })
    ));
    assert!(matches!(
        connection.prepare_call("unsupported").await,
        Err(DruidError::UnsupportedOperation {
            operation: "test_prepare_call"
        })
    ));
    assert_eq!(prepare_count.load(Ordering::Relaxed), 7);

    let state = pool.state();
    assert_eq!(state.prepared_statement_count, 6);
    assert_eq!(state.cached_prepared_statement_delete_count, 6);
    assert_eq!(state.closed_prepared_statement_count, 6);

    connection.close().await.unwrap();
    pool.close().await;
}
