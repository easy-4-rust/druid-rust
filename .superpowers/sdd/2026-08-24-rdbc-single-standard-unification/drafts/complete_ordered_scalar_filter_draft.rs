//! 完整标量around链注册顺序、短路和错误回退候选。
use crate::core::{DruidError, ResultSetFilter, ResultSetFilterContext, Value};
use druid_rdbc::result_set::{ColumnIndex, ResultSetRead};
use std::sync::{Arc, Mutex};

/// 在同一合同入口观察原始请求的进入和退出。对应 Java: com.alibaba.druid.filter.Filter。
pub(super) struct OrderedScalarFilter {
    pub(super) name: &'static str,
    pub(super) calls: Arc<Mutex<Vec<String>>>,
    pub(super) short_circuit: bool,
}
impl ResultSetFilter for OrderedScalarFilter {
    fn standard_result_set_read(
        &self,
        _context: &ResultSetFilterContext,
        _column: Option<ColumnIndex>,
        _label: Option<&str>,
        _request: &ResultSetRead,
        next: &mut dyn FnMut() -> Result<Value, DruidError>,
    ) -> Result<Value, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("{}:before", self.name));
        let result = if self.short_circuit {
            Ok(Value::Null)
        } else {
            next()
        };
        self.calls.lock().unwrap().push(format!(
            "{}:{}",
            self.name,
            if result.is_ok() { "ok" } else { "error" }
        ));
        result
    }
}
