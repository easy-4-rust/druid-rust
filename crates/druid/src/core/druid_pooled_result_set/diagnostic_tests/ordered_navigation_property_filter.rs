use crate::core::{
    DruidError, ResultSetFilter, ResultSetFilterContext, ResultSetPropertyOperation,
    ResultSetPropertyValue,
};
use druid_rdbc::result_set::ResultSetNavigation;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

/// 观察实际标准链的进入、退出、短路及错误；对应 Java: ResultSet Filter。
pub(super) struct OrderedNavigationPropertyFilter {
    pub(super) name: &'static str,
    pub(super) calls: Arc<Mutex<Vec<String>>>,
    /// 0委托、1成功短路、2精确错误、3错误属性类型、4等待取消。
    pub(super) mode: u8,
}

#[async_trait::async_trait]
impl ResultSetFilter for OrderedNavigationPropertyFilter {
    async fn standard_result_set_navigation<'a>(
        &'a self,
        _context: &'a ResultSetFilterContext,
        navigation: ResultSetNavigation,
        next: Pin<Box<dyn Future<Output = Result<bool, DruidError>> + Send + 'a>>,
    ) -> Result<bool, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("{}:nav:{navigation:?}:before", self.name));
        let result = match self.mode {
            1 => Ok(true),
            2 => Err(DruidError::DriverError(
                "ordered navigation failure".to_owned(),
            )),
            4 => std::future::pending().await,
            _ => next.await,
        };
        self.calls.lock().unwrap().push(format!(
            "{}:nav:{}",
            self.name,
            if result.is_ok() { "ok" } else { "error" }
        ));
        result
    }

    fn standard_result_set_property(
        &self,
        _context: &ResultSetFilterContext,
        request: ResultSetPropertyOperation<'_>,
        next: &mut dyn FnMut() -> Result<ResultSetPropertyValue, DruidError>,
    ) -> Result<ResultSetPropertyValue, DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("{}:property:{request:?}:before", self.name));
        let result = match self.mode {
            1 => Ok(ResultSetPropertyValue::Integer(77)),
            2 => Err(DruidError::DriverError(
                "ordered property failure".to_owned(),
            )),
            3 => Ok(ResultSetPropertyValue::Boolean(true)),
            _ => next(),
        };
        self.calls.lock().unwrap().push(format!(
            "{}:property:{}",
            self.name,
            if result.is_ok() { "ok" } else { "error" }
        ));
        result
    }
}
