//! 原 NString Filter 索引、标签及精确错误短路。
use crate::core::{DruidError, ResultSetFilter, ResultSetFilterContext, Value};
use druid_rdbc::result_set::{ColumnIndex, ResultSetRead};
use std::sync::{Arc, Mutex};

/// 记录并短路 NString 读取。对应 Java: com.alibaba.druid.filter.Filter。
pub(super) struct NStringShortCircuitFilter {
    pub(super) calls: Arc<Mutex<Vec<String>>>,
}

impl ResultSetFilter for NStringShortCircuitFilter {
    fn standard_result_set_read(
        &self,
        _context: &ResultSetFilterContext,
        column: Option<ColumnIndex>,
        label: Option<&str>,
        request: &ResultSetRead,
        next: &mut dyn FnMut() -> Result<Value, DruidError>,
    ) -> Result<Value, DruidError> {
        if !matches!(request, ResultSetRead::NString) {
            return next();
        }
        let (call, value) = if let Some(label) = label {
            (
                format!("result_set_get_n_string_by_label:{label}"),
                "过滤-标签",
            )
        } else {
            (
                format!("result_set_get_n_string:{}", column.unwrap().get()),
                "过滤-索引",
            )
        };
        self.calls.lock().unwrap().push(call);
        if label == Some("fail") {
            Err(DruidError::from(druid_rdbc::error::RdbcError::DriverError(
                "filtered NString failure".to_owned(),
            )))
        } else {
            Ok(Value::String(value.to_owned()))
        }
    }
}
