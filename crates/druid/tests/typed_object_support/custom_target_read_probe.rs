use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::{ColumnIndex, ResultSet};
use druid_rdbc::types::Value;
use std::sync::atomic::{AtomicUsize, Ordering};

/// 记录自定义目标类型对实际列值的读取，并可返回驱动前置错误。
/// 对应 Java: java.sql.ResultSet#getObject(int, Class)。
#[derive(Debug)]
pub struct CustomTargetReadProbe {
    pub value_calls: AtomicUsize,
    pub failure: Option<RdbcError>,
}

druid_rdbc::impl_standard_wrapper!(CustomTargetReadProbe => ResultSet);

#[async_trait::async_trait]
impl ResultSet for CustomTargetReadProbe {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        Ok(false)
    }

    fn value(&self, column: ColumnIndex) -> Result<Value, RdbcError> {
        assert_eq!(column.get(), 3);
        self.value_calls.fetch_add(1, Ordering::Relaxed);
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        Ok(Value::String("vendor-value".to_string()))
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }
}
