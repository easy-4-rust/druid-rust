use druid::core::{DruidError, ResultSetFilter, ResultSetFilterContext};
use druid_rdbc::statement::Statement;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

/// 外部实现者以公开标准句柄实现默认、空句柄短路和错误。对应 Java: Filter#resultSet_getStatement。
pub struct ExternalStatementFilter {
    pub name: &'static str,
    pub mode: Arc<AtomicU8>,
    pub calls: Arc<Mutex<Vec<String>>>,
}
impl ResultSetFilter for ExternalStatementFilter {
    fn standard_result_set_statement(
        &self,
        _context: &ResultSetFilterContext,
        next: &mut dyn FnMut() -> Result<Option<Box<dyn Statement>>, DruidError>,
    ) -> Result<Option<Box<dyn Statement>>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("{}:before", self.name));
        let result = match self.mode.load(Ordering::Acquire) {
            1 => Ok(None),
            2 => Err(DruidError::from(druid_rdbc::error::RdbcError::DriverError(
                "external statement failure".to_owned(),
            ))),
            _ => next(),
        };
        self.calls.lock().unwrap().push(format!(
            "{}:{}",
            self.name,
            if result.is_ok() { "ok" } else { "error" }
        ));
        result
    }
}
