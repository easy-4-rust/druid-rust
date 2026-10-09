use crate::core::{
    DruidError, ResultSetFilter, ResultSetFilterContext, StandardResultSetFilterChain,
};
use std::sync::{Arc, Mutex};

/// 保留原next/close包围顺序与短路；下游失败时不记录成功后置事件。
/// 对应 Java: com.alibaba.druid.filter.Filter#resultSet_next/resultSet_close。
pub(super) struct OrderingResultSetFilter {
    pub(super) label: &'static str,
    pub(super) calls: Arc<Mutex<Vec<String>>>,
    pub(super) short_circuit_next: bool,
}
impl OrderingResultSetFilter {
    fn record(&self, operation: &str) {
        self.calls
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(format!("{}:{operation}", self.label));
    }
}
#[async_trait::async_trait]
impl ResultSetFilter for OrderingResultSetFilter {
    fn result_set_open_after(&self, _context: &ResultSetFilterContext) -> Result<(), DruidError> {
        self.record("open");
        Ok(())
    }
    async fn standard_result_set_next(
        &self,
        chain: &mut StandardResultSetFilterChain<'_>,
    ) -> Result<bool, DruidError> {
        self.record("next-before");
        if self.short_circuit_next {
            self.record("next-short");
            return Ok(false);
        }
        let result = chain.result_set_next().await;
        if result.is_ok() {
            self.record("next-after");
        }
        result
    }
    async fn standard_result_set_close(
        &self,
        chain: &mut StandardResultSetFilterChain<'_>,
    ) -> Result<(), DruidError> {
        self.record("close-before");
        let result = chain.result_set_close().await;
        if result.is_ok() {
            self.record("close-after");
        }
        result
    }
}
