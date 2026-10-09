use druid::core::{DruidError, ResultSetFilter, ResultSetFilterContext, ResultSetRowOperation};
use std::sync::{Arc, Mutex};

/// 外部消费者按原操作验证有序委托、短路及错误反向退出。
/// 对应 Java: FilterChainImpl 的七个 ResultSet 行操作。
pub struct OrderedRowOperationFilter {
    pub name: &'static str,
    pub mode: u8,
    pub calls: Arc<Mutex<Vec<String>>>,
}

#[async_trait::async_trait]
impl ResultSetFilter for OrderedRowOperationFilter {
    fn standard_result_set_row_operation(
        &self,
        _context: &ResultSetFilterContext,
        operation: ResultSetRowOperation,
        next: &mut dyn FnMut() -> Result<(), DruidError>,
    ) -> Result<(), DruidError> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("{}:{operation:?}:before", self.name));
        let result = match self.mode {
            1 => Ok(()),
            2 => Err(DruidError::DriverError(
                "ordered row operation failure".to_owned(),
            )),
            _ => next(),
        };
        self.calls.lock().unwrap().push(format!(
            "{}:{operation:?}:{}",
            self.name,
            if result.is_ok() { "ok" } else { "error" }
        ));
        result
    }
}
