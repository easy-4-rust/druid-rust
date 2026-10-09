use druid::core::{AfterFilter, BeforeFilter, DruidError, ExecContext, ExecOperation, ExecResult};
use druid_rdbc::types::Value;
use std::sync::Mutex;

/// 单次执行观察：SQL、操作、参数、前置事件标记及可知行数。
type PreparedExecuteEvent = (String, ExecOperation, Vec<Value>, bool, Option<u64>);

/// 记录完整 Prepared generic execute 合同的实际 Filter 事件。
/// 对应 BASE: prepared_statement_semantics_test::PreparedExecuteRecorder。
#[derive(Default)]
pub struct PreparedExecuteRecorder {
    pub events: Mutex<Vec<PreparedExecuteEvent>>,
}

#[async_trait::async_trait]
impl BeforeFilter for PreparedExecuteRecorder {
    fn name(&self) -> &str {
        "prepared-generic-contract"
    }
    async fn before(&self, context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        self.events.lock().unwrap().push((
            context.sql.clone(),
            context.operation,
            context.params.to_vec(),
            true,
            None,
        ));
        Ok(())
    }
}

#[async_trait::async_trait]
impl AfterFilter for PreparedExecuteRecorder {
    fn name(&self) -> &str {
        "prepared-generic-contract"
    }
    async fn after(
        &self,
        context: &ExecContext<'_>,
        result: &Result<ExecResult, DruidError>,
        _elapsed: std::time::Duration,
    ) -> Result<(), DruidError> {
        self.events.lock().unwrap().push((
            context.sql.clone(),
            context.operation,
            context.params.to_vec(),
            false,
            result
                .as_ref()
                .ok()
                .map(|value| value.row_count.unwrap_or(value.rows_affected)),
        ));
        Ok(())
    }
}
