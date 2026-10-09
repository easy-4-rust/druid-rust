use crate::core::{DruidError, ResultSetFilter, ResultSetFilterContext};
use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::{ColumnIndex, ResultSetRead};
use druid_rdbc::types::Value;
use std::sync::{Arc, Mutex};

/// 原整数getter包围过滤器，记录进入/返回原值后再加原delta。
/// 对应 Java: com.alibaba.druid.filter.Filter#resultSet_getInt。
pub(super) struct IntWrappingFilter {
    pub(super) label: &'static str,
    pub(super) delta: i32,
    pub(super) calls: Arc<Mutex<Vec<String>>>,
}
#[async_trait::async_trait]
impl ResultSetFilter for IntWrappingFilter {
    fn standard_result_set_read(
        &self,
        _context: &ResultSetFilterContext,
        column: Option<ColumnIndex>,
        label: Option<&str>,
        request: &ResultSetRead,
        next: &mut dyn FnMut() -> Result<Value, DruidError>,
    ) -> Result<Value, DruidError> {
        if !matches!(request, ResultSetRead::Int) || label.is_some() {
            return next();
        }
        let column = column.expect("原下标整数重载").get();
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!("{}:int-before:{column}", self.label));
        let Value::Int(value) = next()? else {
            return Err(DruidError::from(RdbcError::DriverError(
                "expected original integer scalar".to_owned(),
            )));
        };
        let value = i32::try_from(value)
            .map_err(|error| DruidError::from(RdbcError::DriverError(error.to_string())))?;
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!("{}:int-after:{value}", self.label));
        Ok(Value::Int(i64::from(value + self.delta)))
    }
}
