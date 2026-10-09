//! 完整对象 around-chain 的顺序、短路和错误回退观察器。
use super::complete_object_filter_request_draft::ResultSetObjectRead;
use crate::core::{DruidError, ResultSetFilter, ResultSetFilterContext};
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::types::RdbcObject;
use std::sync::{Arc, Mutex};

/// 在原对象返回通道观察注册顺序，不转化资源或调用者参数。
/// 对应 Java: com.alibaba.druid.filter.Filter 的 around-chain。
pub(super) struct OrderedObjectFilter {
    pub(super) name: &'static str,
    pub(super) calls: Arc<Mutex<Vec<String>>>,
    pub(super) short_circuit: Option<RdbcObject>,
}

impl ResultSetFilter for OrderedObjectFilter {
    fn standard_result_set_object(
        &self,
        _context: &ResultSetFilterContext,
        _column: Option<ColumnIndex>,
        _label: Option<&str>,
        _request: &ResultSetObjectRead<'_>,
        next: &mut dyn FnMut() -> Result<RdbcObject, DruidError>,
    ) -> Result<RdbcObject, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("{}:before", self.name));
        let result = match &self.short_circuit {
            Some(object) => Ok(object.clone()),
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
