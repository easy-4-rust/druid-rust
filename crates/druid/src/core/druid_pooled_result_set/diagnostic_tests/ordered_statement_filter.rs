use crate::core::{DruidError, ResultSetFilter, ResultSetFilterContext};
use druid_rdbc::statement::Statement;
use std::sync::{Arc, Mutex};

/// 观察创建语句链的正向进入、反向退出及短路边界。对应 Java: Filter#resultSet_getStatement。
pub(super) struct OrderedStatementFilter {
    pub(super) name: &'static str,
    pub(super) calls: Arc<Mutex<Vec<String>>>,
    pub(super) short_circuit: bool,
    pub(super) fail: bool,
}
impl ResultSetFilter for OrderedStatementFilter {
    fn standard_result_set_statement(
        &self,
        _context: &ResultSetFilterContext,
        next: &mut dyn FnMut() -> Result<Option<Box<dyn Statement>>, DruidError>,
    ) -> Result<Option<Box<dyn Statement>>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("{}:before", self.name));
        let result = if self.fail {
            Err(DruidError::from(druid_rdbc::error::RdbcError::DriverError(
                "ordered statement failure".to_owned(),
            )))
        } else if self.short_circuit {
            Ok(None)
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
