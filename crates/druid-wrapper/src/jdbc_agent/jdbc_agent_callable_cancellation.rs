//! JDBC Agent CallableStatement 的标准异步取消句柄。

use super::jdbc_agent_cancellation_context::JdbcAgentCancellationContext;
use super::jdbc_agent_cancellation_slot::JdbcAgentCancellationSlot;
use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::StatementCancellation;
use std::sync::Arc;

/// 只取消获取句柄时对应的执行代次及其当前远端请求。
/// 对应 Java: java.sql.CallableStatement#cancel。
pub(crate) struct JdbcAgentCallableCancellation {
    context: Arc<JdbcAgentCancellationContext>,
    slot: Arc<JdbcAgentCancellationSlot>,
    generation: u64,
}

impl JdbcAgentCallableCancellation {
    pub(crate) fn new(
        context: Arc<JdbcAgentCancellationContext>,
        slot: Arc<JdbcAgentCancellationSlot>,
        generation: u64,
    ) -> Self {
        Self {
            context,
            slot,
            generation,
        }
    }
}

#[async_trait::async_trait]
impl StatementCancellation for JdbcAgentCallableCancellation {
    async fn cancel(&self) -> Result<(), RdbcError> {
        if let Some(request_id) = self.slot.target_request(self.generation).await {
            self.context.cancel_request(request_id).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::jdbc_agent_cancel_sender::JdbcAgentCancelSender;
    use super::*;
    use std::sync::Mutex;

    struct RecordingSender(Mutex<Vec<u64>>);

    #[async_trait::async_trait]
    impl JdbcAgentCancelSender for RecordingSender {
        async fn cancel_request(&self, _: &str, request_id: u64) -> Result<(), RdbcError> {
            self.0.lock().unwrap().push(request_id);
            Ok(())
        }
    }

    #[tokio::test]
    async fn cancellation_targets_only_the_captured_execution() {
        let sender = Arc::new(RecordingSender(Mutex::new(Vec::new())));
        let context = JdbcAgentCancellationContext::new("s1".to_owned(), sender.clone());
        let slot = context.register("statement-1");
        let control = JdbcAgentCallableCancellation::new(
            Arc::clone(&context),
            Arc::clone(&slot),
            slot.control_generation().unwrap(),
        );
        let first = slot.begin_execution();
        let first_request = slot.begin_request(41);
        control.cancel().await.unwrap();
        drop(first_request);
        drop(first);

        let second = slot.begin_execution();
        let second_request = slot.begin_request(42);
        control.cancel().await.unwrap();
        assert_eq!(*sender.0.lock().unwrap(), vec![41]);
        let second_control = JdbcAgentCallableCancellation::new(
            Arc::clone(&context),
            Arc::clone(&slot),
            slot.control_generation().unwrap(),
        );
        second_control.cancel().await.unwrap();
        assert_eq!(*sender.0.lock().unwrap(), vec![41, 42]);
        drop(second_request);
        drop(second);
    }
}
