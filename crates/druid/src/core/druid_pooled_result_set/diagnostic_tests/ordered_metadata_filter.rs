use crate::core::{DruidError, ResultSetFilter, ResultSetFilterContext};
use druid_rdbc::metadata::ResultSetMetaData;
use std::sync::{Arc, Mutex};

/// 观察元数据注册顺序及完整平台句柄短路。对应 Java: Filter#resultSet_getMetaData。
pub(super) struct OrderedMetadataFilter {
    pub(super) name: &'static str,
    pub(super) calls: Arc<Mutex<Vec<String>>>,
    pub(super) replacement: Option<Arc<dyn ResultSetMetaData>>,
}
impl ResultSetFilter for OrderedMetadataFilter {
    fn standard_result_set_metadata(
        &self,
        _context: &ResultSetFilterContext,
        next: &mut dyn FnMut() -> Result<Arc<dyn ResultSetMetaData>, DruidError>,
    ) -> Result<Arc<dyn ResultSetMetaData>, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("{}:before", self.name));
        let result = match &self.replacement {
            Some(metadata) => Ok(Arc::clone(metadata)),
            None => next(),
        };
        self.calls.lock().unwrap().push(format!(
            "{}:{}",
            self.name,
            if result.is_ok() { "ok" } else { "error" }
        ));
        result
    }
}
