use super::jdbc_agent_connection::JdbcAgentTransport;
use super::jdbc_agent_xa_session::JdbcAgentXaSession;
use druid_rdbc::error::RdbcError;
use serde_json::Value as JsonValue;
use std::sync::Arc;

/// 为一个逻辑句柄或 XA 资源提供同 session 的隔离传输视图。
/// 对应 Java: `AgentSession` 暴露的逻辑 JDBC Connection 与 XAResource。
pub(super) struct JdbcAgentXaSessionTransport {
    session: Arc<JdbcAgentXaSession>,
    logical_generation: Option<u64>,
    closed: bool,
}

impl JdbcAgentXaSessionTransport {
    /// 创建指定逻辑代次的 SQL 传输；None 表示只路由 XA RPC 的资源视图。
    pub(super) fn new(session: Arc<JdbcAgentXaSession>, logical_generation: Option<u64>) -> Self {
        Self {
            session,
            logical_generation,
            closed: false,
        }
    }
}

#[async_trait::async_trait]
impl JdbcAgentTransport for JdbcAgentXaSessionTransport {
    async fn request(
        &mut self,
        operation: &str,
        payload: JsonValue,
    ) -> Result<JsonValue, RdbcError> {
        if self.closed {
            return Err(RdbcError::ConnectionDiscarded);
        }
        self.session
            .request(operation, payload, self.logical_generation)
            .await
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        if let Some(generation) = self.logical_generation {
            self.session.close_logical_connection(generation).await
        } else {
            Ok(())
        }
    }

    fn is_unusable(&self) -> bool {
        self.closed
            || self.session.is_unusable()
            || self
                .logical_generation
                .is_some_and(|generation| self.session.initial_generation() != generation)
    }
}
