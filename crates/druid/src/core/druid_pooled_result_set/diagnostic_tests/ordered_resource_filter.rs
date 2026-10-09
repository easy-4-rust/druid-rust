use crate::core::{DruidError, ResultSetFilter, ResultSetFilterContext};
use druid_rdbc::resource::{RdbcInputStream, RdbcReader};
use druid_rdbc::result_set::{ColumnIndex, ResultSetRead, ResultSetStreamKind};
use druid_rdbc::types::{RdbcObject, Value};
use std::sync::{Arc, Mutex};

/// 外部可实现的资源around-filter，观察进入/退出、短路与错误，不拥有驱动。
/// 对应 Java: com.alibaba.druid.filter.Filter 的资源getter。
pub struct OrderedResourceFilter {
    pub name: &'static str,
    pub mode: u8,
    pub calls: Arc<Mutex<Vec<String>>>,
    pub stream: RdbcInputStream,
    pub reader: RdbcReader,
}
impl OrderedResourceFilter {
    fn around<T>(
        &self,
        next: &mut dyn FnMut() -> Result<T, DruidError>,
        short: impl FnOnce() -> T,
    ) -> Result<T, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("{}:before", self.name));
        let result = match self.mode {
            1 => Ok(short()),
            2 => Err(DruidError::DriverError(
                "ordered resource failure".to_owned(),
            )),
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
impl ResultSetFilter for OrderedResourceFilter {
    fn standard_result_set_resource(
        &self,
        _context: &ResultSetFilterContext,
        _column: Option<ColumnIndex>,
        _label: Option<&str>,
        _request: &ResultSetRead,
        next: &mut dyn FnMut() -> Result<RdbcObject, DruidError>,
    ) -> Result<RdbcObject, DruidError> {
        self.around(next, || RdbcObject::Scalar(Value::Null))
    }
    fn standard_result_set_input_stream(
        &self,
        _context: &ResultSetFilterContext,
        _column: Option<ColumnIndex>,
        _label: Option<&str>,
        _kind: ResultSetStreamKind,
        next: &mut dyn FnMut() -> Result<Option<RdbcInputStream>, DruidError>,
    ) -> Result<Option<RdbcInputStream>, DruidError> {
        self.around(next, || Some(self.stream.clone()))
    }
    fn standard_result_set_reader(
        &self,
        _context: &ResultSetFilterContext,
        _column: Option<ColumnIndex>,
        _label: Option<&str>,
        _national: bool,
        next: &mut dyn FnMut() -> Result<Option<RdbcReader>, DruidError>,
    ) -> Result<Option<RdbcReader>, DruidError> {
        self.around(next, || Some(self.reader.clone()))
    }
}
