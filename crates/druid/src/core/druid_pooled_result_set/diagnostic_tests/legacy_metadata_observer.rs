use crate::core::{DruidError, ResultSetFilter, ResultSetFilterContext};
use druid_rdbc::metadata::ResultSetMetaData;
use std::sync::{Arc, Mutex};

/// 只实现既有before/after方法，验证新入口保留默认观察与after替换。对应 Java: FilterAdapter。
pub(super) struct LegacyMetadataObserver {
    pub(super) calls: Arc<Mutex<Vec<String>>>,
    pub(super) replacement: Option<Arc<dyn ResultSetMetaData>>,
}
impl ResultSetFilter for LegacyMetadataObserver {
    fn standard_result_set_meta_data_before(
        &self,
        _context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        self.calls.lock().unwrap().push("legacy:before".to_owned());
        Ok(())
    }
    fn standard_result_set_meta_data_after(
        &self,
        _context: &ResultSetFilterContext,
        result: &mut Result<Arc<dyn ResultSetMetaData>, DruidError>,
    ) -> Result<(), DruidError> {
        self.calls.lock().unwrap().push(format!(
            "legacy:after:{}",
            if result.is_ok() { "ok" } else { "error" }
        ));
        if let Some(metadata) = &self.replacement {
            *result = Ok(Arc::clone(metadata));
        }
        Ok(())
    }
}
