use super::jdbc_agent_connection::JdbcAgentConnection;
use super::jdbc_agent_xa_resource::JdbcAgentXaResource;
use super::jdbc_agent_xa_session::JdbcAgentXaSession;
use super::jdbc_agent_xa_session_transport::JdbcAgentXaSessionTransport;
use druid_rdbc::connection::Connection;
use druid_rdbc::data_source::XaResourceManagerIdentity;
use druid_rdbc::data_source::{PooledConnection, XaConnection, XaError, XaResource};
use druid_rdbc::error::RdbcError;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use tokio::sync::Mutex;

/// JDBC Agent 的 XA owner；逻辑连接关闭后仍保留同一 XA session。
/// 对应 Java: `javax.sql.XAConnection` 与 Agent `AgentSession`。
pub struct JdbcAgentXaConnection {
    session: Arc<JdbcAgentXaSession>,
    session_metadata: JsonValue,
    resource: JdbcAgentXaResource,
    logical_generation: u64,
    logical_issued: bool,
    closed: bool,
}

impl JdbcAgentXaConnection {
    /// 创建已协商 XA 能力的 Agent owner 和标准资源。
    pub(super) fn new(
        transport: Box<dyn super::jdbc_agent_connection::JdbcAgentTransport>,
        session_metadata: JsonValue,
        manager_identity: XaResourceManagerIdentity,
    ) -> Self {
        let session = Arc::new(JdbcAgentXaSession::new(transport));
        let logical_generation = session.initial_generation();
        let resource_transport = JdbcAgentXaSessionTransport::new(Arc::clone(&session), None);
        let resource = JdbcAgentXaResource::new(
            Arc::new(Mutex::new(Box::new(resource_transport))),
            true,
            manager_identity.clone(),
        );
        Self {
            session,
            session_metadata,
            resource,
            logical_generation,
            logical_issued: false,
            closed: false,
        }
    }

    fn ensure_owner_open(&self) -> Result<(), RdbcError> {
        if self.closed || self.session.is_unusable() {
            Err(RdbcError::ConnectionDiscarded)
        } else {
            Ok(())
        }
    }
}

impl std::fmt::Debug for JdbcAgentXaConnection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JdbcAgentXaConnection")
            .field("logical_issued", &self.logical_issued)
            .field("closed", &self.closed)
            .finish_non_exhaustive()
    }
}

#[async_trait::async_trait]
impl PooledConnection for JdbcAgentXaConnection {
    async fn connection(&mut self) -> Result<Box<dyn Connection>, RdbcError> {
        self.ensure_owner_open()?;
        if self.logical_issued {
            self.logical_generation = self.session.reopen_logical_connection().await?;
        }
        self.logical_issued = true;
        let transport = JdbcAgentXaSessionTransport::new(
            Arc::clone(&self.session),
            Some(self.logical_generation),
        );
        Ok(Box::new(JdbcAgentConnection::from_session(
            Box::new(transport),
            self.session_metadata.clone(),
        )))
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        self.session.close_owner().await
    }
}

impl XaConnection for JdbcAgentXaConnection {
    fn xa_resource_name(&self) -> &str {
        "jdbc-agent"
    }

    fn xa_resource(&mut self) -> Result<&mut (dyn XaResource + '_), XaError> {
        if self.closed || self.session.is_unusable() {
            return Err(XaError::with_source(
                druid_rdbc::data_source::XaErrorCode::XAER_RMFAIL,
                "JDBC Agent XA owner is closed or unusable",
                RdbcError::ConnectionDiscarded,
            ));
        }
        Ok(&mut self.resource)
    }
}
