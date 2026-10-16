//! 经现有 Agent 运行时发送按请求 ID 定位的取消命令。

use super::AgentRuntime;
use druid_rdbc::error::RdbcError;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

/// 可测试的 Agent 取消传输；请求 ID 是唯一取消目标。
#[async_trait::async_trait]
pub(crate) trait JdbcAgentCancelSender: Send + Sync {
    async fn cancel_request(&self, session_id: &str, request_id: u64) -> Result<(), RdbcError>;
}

/// 复用当前共享 AgentRuntime 的取消传输，不创建第二个 JVM 或连接。
pub(crate) struct NativeJdbcAgentCancelSender {
    runtime: Arc<AgentRuntime>,
    timeout: Duration,
}

impl NativeJdbcAgentCancelSender {
    pub(crate) fn new(runtime: Arc<AgentRuntime>, timeout: Duration) -> Self {
        Self { runtime, timeout }
    }
}

#[async_trait::async_trait]
impl JdbcAgentCancelSender for NativeJdbcAgentCancelSender {
    async fn cancel_request(&self, session_id: &str, request_id: u64) -> Result<(), RdbcError> {
        let response = tokio::time::timeout(
            self.timeout,
            self.runtime.request(
                "cancel",
                json!({"sessionId":session_id,"targetRequestId":request_id}),
            ),
        )
        .await
        .map_err(|_| RdbcError::DriverError("JDBC Agent cancellation timed out".to_owned()))??;
        match response
            .get("cancelled")
            .and_then(serde_json::Value::as_bool)
        {
            Some(_) => Ok(()),
            None => Err(RdbcError::DriverError(
                "JDBC Agent cancellation response has no cancelled flag".to_owned(),
            )),
        }
    }
}
