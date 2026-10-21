use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::{ColumnIndex, ResultSet};
use druid_rdbc::types::Value;
use std::sync::atomic::{AtomicUsize, Ordering};

/// 验证原生字符串覆写不会被标准标量转换覆盖。对应 Java: java.sql.ResultSet。
pub struct NativeStringResultSet {
    pub scalar_reads: AtomicUsize,
    pub string_reads: AtomicUsize,
    pub fail: bool,
}
impl NativeStringResultSet {
    /// 无参数创建原生字符串分派探针，返回独立计数状态。
    pub fn new() -> Self {
        Self {
            scalar_reads: AtomicUsize::new(0),
            string_reads: AtomicUsize::new(0),
            fail: false,
        }
    }
}
druid_rdbc::impl_standard_wrapper!(NativeStringResultSet => ResultSet);
#[async_trait::async_trait]
impl ResultSet for NativeStringResultSet {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        Ok(false)
    }
    fn value(&self, _column: ColumnIndex) -> Result<Value, RdbcError> {
        self.scalar_reads.fetch_add(1, Ordering::SeqCst);
        Ok(Value::Int(7))
    }
    fn string_value(&self, _column: ColumnIndex) -> Result<Value, RdbcError> {
        self.string_reads.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            Err(RdbcError::DriverError("native string failure".to_owned()))
        } else {
            Ok(Value::String("native:7".to_owned()))
        }
    }
    fn find_column(&self, label: &str) -> Result<ColumnIndex, RdbcError> {
        if label == "value" {
            ColumnIndex::new(1)
        } else {
            Err(RdbcError::InvalidArgument("unknown label".to_owned()))
        }
    }
}
