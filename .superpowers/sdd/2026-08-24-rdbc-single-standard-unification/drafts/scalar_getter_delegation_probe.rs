//! 固定BASE标量重载探针；待当前全量回归终态后接入真实测试模块。
use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetRead};
use druid_rdbc::types::{RdbcObject, Value};
use std::sync::{Arc, Mutex};
/// 原标量下标/列名入口及值不合并。对应 Java: java.sql.ResultSet。
#[derive(Debug)]
pub(super) struct ScalarGetterDelegationProbe {
    calls: Arc<Mutex<Vec<&'static str>>>,
    fail_on: Option<&'static str>,
}
impl ScalarGetterDelegationProbe {
    /// 参数为真实调用记录及失败入口，返回稀疏标准驱动。
    pub(super) fn new(calls: Arc<Mutex<Vec<&'static str>>>, fail_on: Option<&'static str>) -> Self {
        Self { calls, fail_on }
    }
    fn record(&self, call: &'static str, value: Value) -> Result<RdbcObject, RdbcError> {
        self.calls.lock().unwrap().push(call);
        if self.fail_on == Some(call) { return Err(RdbcError::DriverError(format!("{call} failed"))); }
        Ok(RdbcObject::Scalar(value))
    }
}
druid_rdbc::impl_standard_wrapper!(ScalarGetterDelegationProbe => ResultSet);
#[async_trait::async_trait]
impl ResultSet for ScalarGetterDelegationProbe {
    async fn next(&mut self) -> Result<bool, RdbcError> { Ok(false) }
    fn object_by_label(&self, label: &str) -> Result<RdbcObject, RdbcError> {
        assert_eq!(label, "object"); self.record("value_by_label", Value::Int(90))
    }
    fn read_object(&self, column: ColumnIndex, request: &ResultSetRead) -> Result<RdbcObject, RdbcError> {
        let (index, call, value) = match request {
            ResultSetRead::String => (1, "string", Value::String("index".to_owned())),
            ResultSetRead::Boolean => (2, "boolean", Value::Bool(true)),
            ResultSetRead::Long => (3, "long", Value::Int(101)),
            ResultSetRead::Int => (4, "int", Value::Int(103)),
            ResultSetRead::Short => (5, "short", Value::Int(105)),
            ResultSetRead::Byte => (6, "byte", Value::Int(107)),
            ResultSetRead::Double => (7, "double", Value::Float(109.5)),
            ResultSetRead::Float => (8, "float", Value::Float(111.5)),
            ResultSetRead::Bytes => (9, "bytes", Value::Bytes(vec![113])),
            _ => return Err(RdbcError::DriverError("unexpected strong descriptor".to_owned())),
        };
        assert_eq!(column.get(), index); self.record(call, value)
    }
    fn read_object_by_label(&self, label: &str, request: &ResultSetRead) -> Result<RdbcObject, RdbcError> {
        let (expected_label, call, value) = match request {
            ResultSetRead::String => ("string", "string_by_label", Value::String("label".to_owned())),
            ResultSetRead::Boolean => ("boolean", "boolean_by_label", Value::Bool(false)),
            ResultSetRead::Long => ("long", "long_by_label", Value::Int(102)),
            ResultSetRead::Int => ("int", "int_by_label", Value::Int(104)),
            ResultSetRead::Short => ("short", "short_by_label", Value::Int(106)),
            ResultSetRead::Byte => ("byte", "byte_by_label", Value::Int(108)),
            ResultSetRead::Double => ("double", "double_by_label", Value::Float(110.5)),
            ResultSetRead::Float => ("float", "float_by_label", Value::Float(112.5)),
            ResultSetRead::Bytes => ("bytes", "bytes_by_label", Value::Bytes(vec![114])),
            _ => return Err(RdbcError::DriverError("unexpected strong descriptor".to_owned())),
        };
        if label != "failing" { assert_eq!(label, expected_label); }
        self.record(call, value)
    }
    // 通用value和find_column保留标准不支持，防止错误地把原生重载合并为值表。
    async fn close(&mut self) -> Result<(), RdbcError> { Ok(()) }
    fn is_closed(&self) -> Result<bool, RdbcError> { Ok(false) }
}
