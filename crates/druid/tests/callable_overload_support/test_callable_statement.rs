#![allow(dead_code, unused_imports)]
use super::{
    test_blob, test_clob, test_n_clob, TestArrayAccess, TestCallableState, TestRefAccess,
    TestSqlXmlAccess,
};
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{
    RdbcArrayAccess, RdbcBlob, RdbcBlobAccess, RdbcClob, RdbcClobAccess, RdbcInputStream,
    RdbcNClob, RdbcNClobAccess, RdbcOutputStream, RdbcReader, RdbcRefAccess, RdbcResourceAccess,
    RdbcSqlXmlAccess, RdbcWriter, RdbcXmlRepresentationType, RdbcXmlResult, RdbcXmlSource,
    ResourceHandleFactory, ResourceOperations,
};
use druid_rdbc::statement::{
    CallableInputParameter, CallableOutParameter, CallableParameter, CallableStatement,
    ParameterIndex, PreparedStatement, Statement,
};
use druid_rdbc::types::{
    RdbcCalendarArgument as CallableCalendarArgument, RdbcRowId, RdbcTargetType, RdbcUrl,
};
use druid_rdbc::types::{RdbcObject, RdbcString, RdbcTypeMap, Value};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
/// BASE 的 Callable 驱动标准投影，保留全部 OUT 和委托记录。对应 Java: java.sql.CallableStatement。
pub(crate) struct TestCallableStatement {
    sql: String,
    closed: AtomicBool,
    timeout: i32,
    state: Arc<TestCallableState>,
    outputs: HashMap<CallableParameter, RdbcObject>,
}
impl TestCallableStatement {
    pub(crate) fn new(sql: impl Into<String>, state: Arc<TestCallableState>) -> Self {
        let date = NaiveDate::from_ymd_opt(2026, 7, 28).expect("valid test date");
        let time = NaiveTime::from_hms_nano_opt(19, 30, 45, 123_456_789).expect("valid test time");
        let timestamp = NaiveDateTime::new(date, time);
        let blob = test_blob(b"callable-blob".to_vec());
        let clob = test_clob("callable-clob");
        let n_clob = test_n_clob("国家字符");
        let url = RdbcUrl::new("https://example.test/callable");
        let reference = ResourceHandleFactory::reference(Arc::new(TestRefAccess));
        let array = ResourceHandleFactory::array(Arc::new(TestArrayAccess));
        let row_id = RdbcRowId::new(vec![1, 2, 3]);
        let sql_xml = ResourceHandleFactory::sql_xml(Arc::new(TestSqlXmlAccess));
        Self {
            sql: sql.into(),
            closed: AtomicBool::new(false),
            timeout: 0,
            state,

            outputs: HashMap::from([
                (
                    CallableParameter::Index(ParameterIndex::new(1).unwrap()),
                    RdbcObject::from(Value::Int(7)),
                ),
                (
                    CallableParameter::Index(ParameterIndex::new(2).unwrap()),
                    RdbcObject::from(Value::Null),
                ),
                (
                    CallableParameter::Index(ParameterIndex::new(4).unwrap()),
                    RdbcObject::from(Value::Int(300)),
                ),
                (
                    CallableParameter::Index(ParameterIndex::new(5).unwrap()),
                    RdbcObject::BigDecimal(
                        BigDecimal::from_str("123.4500").expect("valid decimal"),
                    ),
                ),
                (
                    CallableParameter::Index(ParameterIndex::new(6).unwrap()),
                    RdbcObject::Date(date),
                ),
                (
                    CallableParameter::Index(ParameterIndex::new(7).unwrap()),
                    RdbcObject::Time(time),
                ),
                (
                    CallableParameter::Index(ParameterIndex::new(8).unwrap()),
                    RdbcObject::Timestamp(timestamp),
                ),
                (
                    CallableParameter::Index(ParameterIndex::new(9).unwrap()),
                    RdbcObject::Blob(blob.clone()),
                ),
                (
                    CallableParameter::Index(ParameterIndex::new(10).unwrap()),
                    RdbcObject::Clob(clob.clone()),
                ),
                (
                    CallableParameter::Index(ParameterIndex::new(11).unwrap()),
                    RdbcObject::NClob(n_clob.clone()),
                ),
                (
                    CallableParameter::Index(ParameterIndex::new(12).unwrap()),
                    RdbcObject::CharacterStream(RdbcReader::from_string("reader-index")),
                ),
                (
                    CallableParameter::Index(ParameterIndex::new(13).unwrap()),
                    RdbcObject::NCharacterStream(RdbcReader::from_string("国字-index")),
                ),
                (
                    CallableParameter::Index(ParameterIndex::new(14).unwrap()),
                    RdbcObject::NString("国字-string-index".to_string()),
                ),
                (
                    CallableParameter::Index(ParameterIndex::new(15).unwrap()),
                    RdbcObject::Url(url.clone()),
                ),
                (
                    CallableParameter::Index(ParameterIndex::new(16).unwrap()),
                    RdbcObject::Ref(reference.clone()),
                ),
                (
                    CallableParameter::Index(ParameterIndex::new(17).unwrap()),
                    RdbcObject::Array(array.clone()),
                ),
                (
                    CallableParameter::Index(ParameterIndex::new(18).unwrap()),
                    RdbcObject::RowId(row_id.clone()),
                ),
                (
                    CallableParameter::Index(ParameterIndex::new(19).unwrap()),
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
        }
    }
}

druid_rdbc::impl_standard_wrapper!(TestCallableStatement => CallableStatement);
#[async_trait::async_trait]
impl Statement for TestCallableStatement {
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed.store(true, Ordering::Release);
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed.load(Ordering::Acquire))
    }
    fn query_timeout(&self) -> Result<i32, RdbcError> {
        Ok(self.timeout)
    }
    fn set_query_timeout(&mut self, seconds: i32) -> Result<(), RdbcError> {
        self.timeout = seconds;
        Ok(())
    }
}
#[async_trait::async_trait]
impl PreparedStatement for TestCallableStatement {
    fn sql(&self) -> Result<&str, RdbcError> {
        Ok(&self.sql)
    }
    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
        self.state
            .named_parameters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
        Ok(())
    }
    async fn execute_prepared_update(&mut self) -> Result<i64, RdbcError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(RdbcError::ConnectionDiscarded);
        }
        Ok(1)
    }
    async fn execute_prepared_query(
        &mut self,
    ) -> Result<Box<dyn druid_rdbc::result_set::ResultSet>, RdbcError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(RdbcError::ConnectionDiscarded);
        }
        Ok(Box::new(druid_rdbc::extensions::BufferedRowSet::new(vec![
            druid_rdbc::types::Row::new(vec![Value::Int(1)]),
        ])))
    }
}
#[async_trait::async_trait]
impl CallableStatement for TestCallableStatement {
    fn register_out_parameter(
        &mut self,
        parameter: CallableParameter,
        out_parameter: CallableOutParameter,
    ) -> Result<(), RdbcError> {
        self.state
            .registrations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(parameter, out_parameter);
        Ok(())
    }

    fn set_named_parameter(
        &mut self,
        parameter_name: &str,
        parameter: CallableInputParameter,
    ) -> Result<(), RdbcError> {
        self.state
            .named_parameters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(parameter_name.to_string(), parameter);
        Ok(())
    }

    fn out_parameter(&self, parameter: &CallableParameter) -> Result<RdbcObject, RdbcError> {
        let value = self.outputs.get(parameter).cloned().ok_or_else(|| {
            RdbcError::DriverError(format!("OUT parameter {parameter:?} is unavailable"))
        })?;
        self.state
            .last_was_null
            .store(value.is_null(), Ordering::Release);
        Ok(value)
    }

    fn out_parameter_with_type_map(
        &self,
        parameter: &CallableParameter,
        type_map: Option<&RdbcTypeMap>,
    ) -> Result<RdbcObject, RdbcError> {
        self.state
            .type_map_reads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((parameter.clone(), type_map.cloned()));
        self.out_parameter(parameter)
    }

    fn out_parameter_as(
        &self,
        parameter: &CallableParameter,
        target_type: &RdbcTargetType,
    ) -> Result<RdbcObject, RdbcError> {
        self.state
            .typed_reads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((parameter.clone(), target_type.clone()));
        self.out_parameter(parameter)
    }

    fn was_null(&self) -> Result<bool, RdbcError> {
        Ok(self.state.last_was_null.load(Ordering::Acquire))
    }

    fn date_out_parameter(
        &self,
        parameter: &CallableParameter,
        calendar: &CallableCalendarArgument,
    ) -> Result<Option<NaiveDate>, RdbcError> {
        self.state
            .calendar_reads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((parameter.clone(), calendar.clone()));
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::Date(value) => Ok(Some(value)),
            other => Err(RdbcError::DriverError(format!(
                "expected Date, got {other}"
            ))),
        }
    }

    fn time_out_parameter(
        &self,
        parameter: &CallableParameter,
        calendar: &CallableCalendarArgument,
    ) -> Result<Option<NaiveTime>, RdbcError> {
        self.state
            .calendar_reads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((parameter.clone(), calendar.clone()));
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::Time(value) => Ok(Some(value)),
            other => Err(RdbcError::DriverError(format!(
                "expected Time, got {other}"
            ))),
        }
    }

    fn timestamp_out_parameter(
        &self,
        parameter: &CallableParameter,
        calendar: &CallableCalendarArgument,
    ) -> Result<Option<NaiveDateTime>, RdbcError> {
        self.state
            .calendar_reads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push((parameter.clone(), calendar.clone()));
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::Timestamp(value) => Ok(Some(value)),
            other => Err(RdbcError::DriverError(format!(
                "expected Timestamp, got {other}"
            ))),
        }
    }
}
