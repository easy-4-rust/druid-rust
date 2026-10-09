use super::driver_state::DriverState;
use druid::core::{BeforeFilter, DruidError, ExecContext, ResultSetFilter, ResultSetFilterContext};
use std::sync::Arc;

/// 记录逻辑游标的登记和读取身份，捕获 getter 重复 open。
/// 对应 Java: com.alibaba.druid.filter.Filter 的测试观察器。
pub struct OpenFilter(pub Arc<DriverState>);
#[async_trait::async_trait]
impl BeforeFilter for OpenFilter {
    fn name(&self) -> &str {
        "current-result-handoff"
    }
    async fn before(&self, _context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        Ok(())
    }
}
impl ResultSetFilter for OpenFilter {
    fn result_set_open_after(&self, context: &ResultSetFilterContext) -> Result<(), DruidError> {
        self.0
            .open_ids
            .lock()
            .unwrap()
            .push(context.result_set_id());
        Ok(())
    }
    fn standard_result_set_next_before(
        &self,
        context: &ResultSetFilterContext,
    ) -> Result<(), DruidError> {
        self.0
            .read_ids
            .lock()
            .unwrap()
            .push(context.result_set_id());
        Ok(())
    }
}
