use crate::core::{DruidError, ResultSetFilter, ResultSetFilterContext};
use druid_rdbc::metadata::ResultSetMetaData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// 返回原元数据平台句柄或精确错误，不触发后续驱动。对应 Java: Filter#resultSet_getMetaData。
pub(super) struct MetadataShortCircuitFilter {
    pub(super) calls: Arc<Mutex<Vec<String>>>,
    pub(super) fail: Arc<AtomicBool>,
    pub(super) metadata: Arc<dyn ResultSetMetaData>,
}
impl ResultSetFilter for MetadataShortCircuitFilter {
    fn standard_result_set_metadata(
        &self,
        _context: &ResultSetFilterContext,
        _next: &mut dyn FnMut() -> Result<Arc<dyn ResultSetMetaData>, DruidError>,
    ) -> Result<Arc<dyn ResultSetMetaData>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push("result_set_get_meta_data".to_owned());
        if self.fail.load(Ordering::Acquire) {
            Err(DruidError::from(druid_rdbc::error::RdbcError::DriverError(
                "filtered metadata failure".to_owned(),
            )))
        } else {
            Ok(Arc::clone(&self.metadata))
        }
    }
}
