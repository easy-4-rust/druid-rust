use super::AgentRequestHandle;
use druid::core::{
    DruidError, PhysicalPreparedStatement, PhysicalStatementOptions, PreparedStatementKey,
};
use serde_json::json;
use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};

/// JDBC Agent 中已创建真实 JDBC PreparedStatement 的远程句柄。
pub struct JdbcAgentPreparedStatement {
    sql: String,
    statement_id: String,
    session_id: String,
    request_handle: AgentRequestHandle,
    options: PhysicalStatementOptions,
    closed: AtomicBool,
}

impl JdbcAgentPreparedStatement {
    /// 创建与 Agent 端真实 PreparedStatement 一一对应的物理语句。
    pub(crate) fn new(
        key: &PreparedStatementKey,
        statement_id: String,
        session_id: String,
        request_handle: AgentRequestHandle,
    ) -> Self {
        Self {
            sql: key.sql().to_owned(),
            statement_id,
            session_id,
            request_handle,
            options: PhysicalStatementOptions {
                result_set_type: key.result_set_type(),
                result_set_concurrency: key.result_set_concurrency(),
                result_set_holdability: key.result_set_holdability(),
            },
            closed: AtomicBool::new(false),
        }
    }

    /// 返回协议侧远程语句 ID。
    pub(crate) fn statement_id(&self) -> &str {
        &self.statement_id
    }
}

impl std::fmt::Debug for JdbcAgentPreparedStatement {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JdbcAgentPreparedStatement")
            .field("sql", &self.sql)
            .field("statement_id", &self.statement_id)
            .field("session_id", &self.session_id)
            .field("closed", &self.closed.load(Ordering::Acquire))
            .finish_non_exhaustive()
    }
}

impl PhysicalPreparedStatement for JdbcAgentPreparedStatement {
    fn sql(&self) -> &str {
        &self.sql
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn statement_options(&self) -> PhysicalStatementOptions {
        self.options
    }

    fn close(&self) -> Result<(), DruidError> {
        if self.closed.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        self.request_handle.spawn_request(
            "close_statement",
            json!({
                "sessionId": self.session_id,
                "statementId": self.statement_id,
            }),
        );
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    fn cancel(&self) -> Result<(), DruidError> {
        if self.is_closed() {
            return Err(DruidError::ConnectionDiscarded);
        }
        self.request_handle.spawn_request(
            "cancel",
            json!({
                "sessionId": self.session_id,
                "statementId": self.statement_id,
            }),
        );
        Ok(())
    }
}
