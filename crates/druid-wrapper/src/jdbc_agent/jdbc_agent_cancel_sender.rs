//! 经现有 Agent 运行时发送按请求 ID 定位的取消命令。

use super::AgentRuntime;
use druid_rdbc::error::RdbcError;
use serde_json::{json, Value as JsonValue};
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

fn validate_cancel_response(response: &JsonValue) -> Result<(), RdbcError> {
    match response.get("cancelled").and_then(JsonValue::as_bool) {
        Some(true) => Ok(()),
        Some(false) => Err(RdbcError::DriverError(
            "JDBC Agent cancellation target is no longer active".to_owned(),
        )),
        None => Err(RdbcError::DriverError(
            "JDBC Agent cancellation response has no cancelled flag".to_owned(),
        )),
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
        validate_cancel_response(&response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_cancel_response_is_not_reported_as_success() {
        assert!(validate_cancel_response(&json!({"cancelled": true})).is_ok());
        assert!(validate_cancel_response(&json!({"cancelled": false})).is_err());
        assert!(validate_cancel_response(&json!({})).is_err());
    }
}
