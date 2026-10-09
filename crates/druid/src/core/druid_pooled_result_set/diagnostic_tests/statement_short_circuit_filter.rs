use crate::core::{DruidError, DruidPooledStatement, ResultSetFilter, ResultSetFilterContext};
use druid_rdbc::statement::Statement;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// 保留创建语句默认身份，或以同一共享逻辑句柄短路替换。对应 Java: Filter#resultSet_getStatement。
pub(super) struct StatementShortCircuitFilter {
    pub(super) calls: Arc<Mutex<Vec<String>>>,
    pub(super) replacement: Arc<Mutex<Option<DruidPooledStatement>>>,
    pub(super) fail: Arc<AtomicBool>,
}
impl ResultSetFilter for StatementShortCircuitFilter {
    fn standard_result_set_statement(
        &self,
        _context: &ResultSetFilterContext,
        next: &mut dyn FnMut() -> Result<Option<Box<dyn Statement>>, DruidError>,
    ) -> Result<Option<Box<dyn Statement>>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push("result_set_get_statement".to_owned());
        if self.fail.load(Ordering::Acquire) {
            return Err(DruidError::from(druid_rdbc::error::RdbcError::DriverError(
                "filtered statement failure".to_owned(),
            )));
        }
        if let Some(statement) = self.replacement.lock().unwrap().as_ref() {
            return Ok(Some(Box::new(statement.clone())));
        }
        next()
    }
}
