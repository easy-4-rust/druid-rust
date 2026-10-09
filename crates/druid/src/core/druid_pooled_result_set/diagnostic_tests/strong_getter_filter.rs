use crate::core::{DruidError, ResultSetFilter, ResultSetFilterContext, Value};
use druid_rdbc::result_set::{ColumnIndex, ResultSetRead};
use std::sync::{Arc, Mutex};

/// 观察强类型读取的真实 around-hook 次序与返回值替换。
/// 对应 Java: com.alibaba.druid.filter.FilterAdapter 的 ResultSet getter hooks。
pub(super) struct StrongGetterFilter {
    before: &'static str,
    after: &'static str,
    events: Arc<Mutex<Vec<&'static str>>>,
    replace: bool,
}

impl StrongGetterFilter {
    /// 创建 hook；参数为前后事件名、共享记录和替换开关，返回过滤器。
    pub(super) fn new(
        before: &'static str,
        after: &'static str,
        events: Arc<Mutex<Vec<&'static str>>>,
        replace: bool,
    ) -> Self {
        Self {
            before,
            after,
            events,
            replace,
        }
    }
}

impl ResultSetFilter for StrongGetterFilter {
    fn standard_result_set_read(
        &self,
        _context: &ResultSetFilterContext,
        column: Option<ColumnIndex>,
        label: Option<&str>,
        _request: &ResultSetRead,
        next: &mut dyn FnMut() -> Result<Value, DruidError>,
    ) -> Result<Value, DruidError> {
        if label.is_none() || column.is_some() {
            return next();
        }
        // 标签观察使用完整读取hook，保留原事件/替换，不虚构已解析下标。
        self.events.lock().unwrap().push(self.before);
        let mut result = next();
        self.events.lock().unwrap().push(self.after);
        if self.replace && matches!(result, Ok(Value::Decimal(_))) {
            result = Ok(Value::Decimal(druid_rdbc::types::RdbcBigDecimal::from(2)));
        }
        result
    }
    fn standard_result_set_value_before(
        &self,
        _context: &ResultSetFilterContext,
        _column: ColumnIndex,
    ) -> Result<(), DruidError> {
        self.events.lock().unwrap().push(self.before);
        Ok(())
    }

    fn standard_result_set_value_after(
        &self,
        _context: &ResultSetFilterContext,
        _column: ColumnIndex,
        result: &mut Result<Value, DruidError>,
    ) -> Result<(), DruidError> {
        self.events.lock().unwrap().push(self.after);
        if self.replace && matches!(result, Ok(Value::Decimal(_))) {
            *result = Ok(Value::Decimal(druid_rdbc::types::RdbcBigDecimal::from(2)));
        }
        Ok(())
    }
}
