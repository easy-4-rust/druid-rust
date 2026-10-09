use druid::core::{DruidError, ResultSetFilter, ResultSetFilterContext};
use druid_rdbc::metadata::ResultSetMetaData;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

/// 外部过滤器只通过公开接口委托、短路平台句柄或报告精确错误。对应 Java: Filter#resultSet_getMetaData。
pub struct ExternalMetadataFilter {
    pub name: &'static str,
    pub calls: Arc<Mutex<Vec<String>>>,
    pub mode: Arc<AtomicU8>,
    pub metadata: Arc<dyn ResultSetMetaData>,
}
impl ResultSetFilter for ExternalMetadataFilter {
    fn standard_result_set_metadata(
        &self,
        _context: &ResultSetFilterContext,
        next: &mut dyn FnMut() -> Result<Arc<dyn ResultSetMetaData>, DruidError>,
    ) -> Result<Arc<dyn ResultSetMetaData>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("{}:before", self.name));
        let result = match self.mode.load(Ordering::Acquire) {
            1 => Ok(Arc::clone(&self.metadata)),
            2 => Err(DruidError::from(druid_rdbc::error::RdbcError::DriverError(
                "external metadata failure".to_owned(),
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
