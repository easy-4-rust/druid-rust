use druid::core::{BeforeFilter, DruidError, ExecContext};
use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::PreparedInputParameter;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// 观察池化执行提供给 Filter 的完整参数快照，并可拒绝执行以隔离绑定阶段。
/// 对应 Java: FilterChain#preparedStatement_execute 的参数上下文。
#[derive(Default)]
pub(crate) struct PreparedDescriptorRecorder {
    pub(crate) snapshots: Mutex<Vec<Vec<PreparedInputParameter>>>,
    pub(crate) reject_execution: AtomicBool,
    pub(crate) batches: Mutex<Vec<Vec<Vec<PreparedInputParameter>>>>,
}

#[async_trait::async_trait]
impl BeforeFilter for PreparedDescriptorRecorder {
    fn name(&self) -> &str {
        "prepared-descriptor-contract"
    }

    async fn before(&self, context: &mut ExecContext<'_>) -> Result<(), DruidError> {
        if let Some(parameters) = context.prepared_parameters {
            self.snapshots.lock().unwrap().push(parameters.to_vec());
        }
        // 正常 Filter 拒绝路径：验证 setter 与逻辑快照，不触发 adapter 资源物化。
        if self.reject_execution.load(Ordering::Acquire) {
            return Err(DruidError::from(RdbcError::Other(
                "descriptor-execution-rejected".to_owned(),
            )));
        }
        Ok(())
    }
    async fn before_batch(
        &self,
        context: &mut druid::core::BatchExecContext<'_>,
    ) -> Result<(), DruidError> {
        if let Some(parameters) = context.prepared_parameter_sets {
            self.batches.lock().unwrap().push(parameters.to_vec());
        }
        Ok(())
    }
}
