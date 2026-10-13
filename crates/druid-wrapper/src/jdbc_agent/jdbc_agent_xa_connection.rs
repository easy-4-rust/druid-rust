use super::jdbc_agent_connection::JdbcAgentConnection;
use super::jdbc_agent_xa_resource::JdbcAgentXaResource;
use super::jdbc_agent_xa_session::JdbcAgentXaSession;
use super::jdbc_agent_xa_session_transport::JdbcAgentXaSessionTransport;
use druid_rdbc::connection::Connection;
use druid_rdbc::data_source::{
    ConnectionEventListener, PooledConnectionEventRegistry, PooledConnectionSource,
    XaResourceManagerIdentity,
};
use druid_rdbc::data_source::{PooledConnection, XaConnection, XaError, XaResource};
use druid_rdbc::error::{RdbcError, SqlException};
use serde_json::Value as JsonValue;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock, Weak};
use tokio::sync::{Mutex, MutexGuard};

/// JDBC Agent 的 XA owner；逻辑连接关闭后仍保留同一 XA session。
/// 对应 Java: `javax.sql.XAConnection` 与 Agent `AgentSession`。
pub struct JdbcAgentXaConnection {
    state: Arc<JdbcAgentXaOwnerState>,
    _owner_source: PooledConnectionSource,
    resource: JdbcAgentXaResource,
}

/// XA owner 在标准接口对象与事件源代理之间共享的状态。
/// 对应 Java: `javax.sql.XAConnection` 的 owner 与逻辑 Connection 租约。
pub(super) struct JdbcAgentXaOwnerState {
    session: Arc<JdbcAgentXaSession>,
    session_metadata: JsonValue,
    logical_generation: AtomicU64,
    logical_issued: AtomicBool,
    closed: AtomicBool,
    fatal_event_notified: AtomicBool,
    lease_gate: Mutex<()>,
    event_registry: PooledConnectionEventRegistry,
    owner_source: OnceLock<Weak<tokio::sync::Mutex<Box<dyn PooledConnection>>>>,
}

impl JdbcAgentXaOwnerState {
    fn source(&self) -> Option<PooledConnectionSource> {
        self.owner_source.get().and_then(Weak::upgrade)
    }

    fn ensure_owner_open(&self) -> Result<(), RdbcError> {
        if self.closed.load(Ordering::Acquire) || self.session.is_unusable() {
            Err(RdbcError::ConnectionDiscarded)
        } else {
            Ok(())
        }
    }

    async fn lock_lease(&self) -> Result<MutexGuard<'_, ()>, RdbcError> {
        let guard = self.lease_gate.lock().await;
        self.ensure_owner_open()?;
        Ok(guard)
    }

    pub(super) fn notify_connection_closed(&self) {
        if let Some(source) = self.source() {
            self.event_registry.notify_connection_closed(source);
        }
    }

    pub(super) fn notify_connection_error(&self, error: &RdbcError) {
        // 连接错误事件需要标准 SQL 异常载体。传输已不可用时，非 SQL 错误也
        // 必须通知外部池丢弃 owner；事件只放安全的分类信息，原始错误仍返回调用方。
        let exception = error.sql_exception().cloned().unwrap_or_else(|| {
            SqlException::new(
                0,
                Some("08006".to_owned()),
                Some("JDBC Agent transport is unusable; discard the XA owner".to_owned()),
            )
            .with_class_name("java.sql.SQLNonTransientConnectionException")
            .with_assignable_type("java.sql.SQLNonTransientException")
        });
        if self.fatal_event_notified.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Some(source) = self.source() {
            self.event_registry
                .notify_connection_error(source, Arc::new(exception));
        }
    }

    async fn connection(self: &Arc<Self>) -> Result<Box<dyn Connection>, RdbcError> {
        let _lease = self.lock_lease().await?;
        let (generation, closed_previous) = if self.logical_issued.swap(true, Ordering::AcqRel) {
            let (generation, closed_previous) = self.session.reopen_logical_connection().await?;
            (generation, closed_previous)
        } else {
            (self.logical_generation.load(Ordering::Acquire), false)
        };
        self.logical_generation.store(generation, Ordering::Release);
        let transport = JdbcAgentXaSessionTransport::new(
            Arc::clone(&self.session),
            Some(generation),
            Some(Arc::clone(self)),
        );
        let connection = Box::new(JdbcAgentConnection::from_session(
            Box::new(transport),
            self.session_metadata.clone(),
        ));
        drop(_lease);
        if closed_previous {
            self.notify_connection_closed();
        }
        Ok(connection)
    }

    async fn close_owner(&self) -> Result<(), RdbcError> {
        let _lease = self.lease_gate.lock().await;
        if self.closed.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        let result = self.session.close_owner().await;
        drop(_lease);
        if let Err(error) = &result {
            self.notify_connection_error(error);
        }
        result
    }
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
        let state = Arc::new(JdbcAgentXaOwnerState {
            session: Arc::clone(&session),
            session_metadata,
            logical_generation: AtomicU64::new(logical_generation),
            logical_issued: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            fatal_event_notified: AtomicBool::new(false),
            lease_gate: Mutex::new(()),
            event_registry: PooledConnectionEventRegistry::new(),
            owner_source: OnceLock::new(),
        });
        let owner_source: PooledConnectionSource =
            Arc::new(Mutex::new(Box::new(JdbcAgentXaEventSource {
                state: Arc::clone(&state),
            })));
        let _ = state.owner_source.set(Arc::downgrade(&owner_source));
        let resource_transport =
            JdbcAgentXaSessionTransport::new(Arc::clone(&session), None, Some(Arc::clone(&state)));
        let resource = JdbcAgentXaResource::new(
            Arc::new(Mutex::new(Box::new(resource_transport))),
            true,
            manager_identity.clone(),
        );
        Self {
            state,
            _owner_source: owner_source,
            resource,
        }
    }
}

impl std::fmt::Debug for JdbcAgentXaConnection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("JdbcAgentXaConnection")
            .field(
                "logical_issued",
                &self.state.logical_issued.load(Ordering::Acquire),
            )
            .field("closed", &self.state.closed.load(Ordering::Acquire))
            .finish_non_exhaustive()
    }
}

#[async_trait::async_trait]
impl PooledConnection for JdbcAgentXaConnection {
    async fn connection(&mut self) -> Result<Box<dyn Connection>, RdbcError> {
        self.state.connection().await
    }

    fn add_connection_event_listener(
        &mut self,
        listener: Arc<dyn ConnectionEventListener>,
    ) -> Result<(), RdbcError> {
        self.state
            .event_registry
            .add_connection_event_listener(listener);
        Ok(())
    }

    fn remove_connection_event_listener(
        &mut self,
        listener: &Arc<dyn ConnectionEventListener>,
    ) -> Result<bool, RdbcError> {
        Ok(self
            .state
            .event_registry
            .remove_connection_event_listener(listener))
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.state.close_owner().await
    }
}

impl XaConnection for JdbcAgentXaConnection {
    fn xa_resource_name(&self) -> &str {
        "jdbc-agent"
    }

    fn xa_resource(&mut self) -> Result<&mut (dyn XaResource + '_), XaError> {
        if self.state.closed.load(Ordering::Acquire) || self.state.session.is_unusable() {
            return Err(XaError::with_source(
                druid_rdbc::data_source::XaErrorCode::XAER_RMFAIL,
                "JDBC Agent XA owner is closed or unusable",
                RdbcError::ConnectionDiscarded,
            ));
        }
        Ok(&mut self.resource)
    }
}

/// 为连接事件保留稳定 owner 身份的标准池化来源视图。
/// 对应 Java: `ConnectionEvent#getSource()` 返回的原始 XAConnection。
struct JdbcAgentXaEventSource {
    state: Arc<JdbcAgentXaOwnerState>,
}

#[async_trait::async_trait]
impl PooledConnection for JdbcAgentXaEventSource {
    async fn connection(&mut self) -> Result<Box<dyn Connection>, RdbcError> {
        self.state.connection().await
    }

    fn add_connection_event_listener(
        &mut self,
        listener: Arc<dyn ConnectionEventListener>,
    ) -> Result<(), RdbcError> {
        self.state
            .event_registry
            .add_connection_event_listener(listener);
        Ok(())
    }

    fn remove_connection_event_listener(
        &mut self,
        listener: &Arc<dyn ConnectionEventListener>,
    ) -> Result<bool, RdbcError> {
        Ok(self
            .state
            .event_registry
            .remove_connection_event_listener(listener))
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.state.close_owner().await
    }
}

#[cfg(test)]
mod tests {
    use super::JdbcAgentXaConnection;
    use crate::jdbc_agent::jdbc_agent_connection::JdbcAgentTransport;
    use druid_rdbc::data_source::{
        ConnectionEvent, ConnectionEventListener, PooledConnection, PooledConnectionSource,
        XaConnection, XaFlags, XaResourceManagerIdentity, Xid,
    };
    use druid_rdbc::error::RdbcError;
    use serde_json::{json, Value as JsonValue};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct EventRecorder {
        closed: AtomicUsize,
        source_lock_acquired: AtomicUsize,
        sources: Mutex<Vec<PooledConnectionSource>>,
        error_states: Mutex<Vec<Option<String>>>,
    }

    impl ConnectionEventListener for EventRecorder {
        fn connection_closed(&self, event: &ConnectionEvent) {
            self.closed.fetch_add(1, Ordering::AcqRel);
            if event.source().try_lock().is_ok() {
                self.source_lock_acquired.fetch_add(1, Ordering::AcqRel);
            }
            self.sources
                .lock()
                .expect("event sources")
                .push(Arc::clone(event.source()));
        }

        fn connection_error_occurred(&self, event: &ConnectionEvent) {
            self.sources
                .lock()
                .expect("event sources")
                .push(Arc::clone(event.source()));
            self.error_states.lock().expect("event error states").push(
                event
                    .sql_exception()
                    .and_then(|error| error.sql_state().map(str::to_owned)),
            );
        }
    }

    struct EventTestTransport {
        unusable: Arc<AtomicBool>,
        fail_execute: bool,
        fail_close: bool,
    }

    #[async_trait::async_trait]
    impl JdbcAgentTransport for EventTestTransport {
        async fn request(
            &mut self,
            operation: &str,
            _payload: JsonValue,
        ) -> Result<JsonValue, RdbcError> {
            match operation {
                "execute" if self.fail_execute => Err(RdbcError::SqlException(Box::new(
                    druid_rdbc::error::SqlException::driver(91, "fatal database connection error")
                        .with_sql_state("08006")
                        .with_class_name("java.sql.SQLNonTransientConnectionException"),
                ))),
                "execute" => Ok(json!({"kind": "update", "rowsAffected": 1})),
                "xa_start" => Ok(json!({"started": 1})),
                _ => Ok(json!({})),
            }
        }

        async fn close(&mut self) -> Result<(), RdbcError> {
            self.unusable.store(true, Ordering::Release);
            if self.fail_close {
                Err(RdbcError::SqlException(Box::new(
                    druid_rdbc::error::SqlException::driver(92, "fatal session close error")
                        .with_sql_state("08006"),
                )))
            } else {
                Ok(())
            }
        }

        fn is_unusable(&self) -> bool {
            self.unusable.load(Ordering::Acquire)
        }
    }

    #[tokio::test]
    async fn logical_close_notifies_stable_owner_once_and_keeps_xa_live() {
        let mut owner = JdbcAgentXaConnection::new(
            Box::new(EventTestTransport {
                unusable: Arc::new(AtomicBool::new(false)),
                fail_execute: false,
                fail_close: false,
            }),
            json!({
                "capabilities": {"xa": true, "transactions": true},
                "autoCommit": true,
                "readOnly": false,
                "transactionIsolation": 2,
            }),
            XaResourceManagerIdentity::new(),
        );
        let recorder = Arc::new(EventRecorder::default());
        owner
            .add_connection_event_listener(Arc::clone(&recorder) as Arc<dyn ConnectionEventListener>)
            .expect("listener registration is supported");

        let mut first = owner.connection().await.expect("first lease");
        first.close().await.expect("logical close");
        first.close().await.expect("duplicate close");
        owner
            .xa_resource()
            .expect("XA owner remains live")
            .start(
                &Xid::new(1, vec![1], vec![2]).expect("valid Xid"),
                XaFlags::TMNOFLAGS,
            )
            .await
            .expect("XA operation after logical close");
        assert_eq!(recorder.closed.load(Ordering::Acquire), 1);

        let mut second = owner.connection().await.expect("next lease");
        second.close().await.expect("second logical close");
        assert_eq!(recorder.closed.load(Ordering::Acquire), 2);
        let sources = recorder.sources.lock().expect("event sources");
        assert_eq!(sources.len(), 2);
        assert!(Arc::ptr_eq(&sources[0], &sources[1]));
        drop(sources);
        assert_eq!(recorder.source_lock_acquired.load(Ordering::Acquire), 2);
        owner.close().await.expect("owner close");
    }

    #[tokio::test]
    async fn fatal_sql_error_notifies_with_original_exception_and_isolates_owner() {
        let mut owner = JdbcAgentXaConnection::new(
            Box::new(EventTestTransport {
                unusable: Arc::new(AtomicBool::new(false)),
                fail_execute: true,
                fail_close: false,
            }),
            json!({
                "capabilities": {"xa": true, "transactions": true},
                "autoCommit": true,
                "readOnly": false,
                "transactionIsolation": 2,
            }),
            XaResourceManagerIdentity::new(),
        );
        let recorder = Arc::new(EventRecorder::default());
        owner
            .add_connection_event_listener(Arc::clone(&recorder) as Arc<dyn ConnectionEventListener>)
            .expect("listener registration is supported");

        let mut logical = owner.connection().await.expect("logical lease");
        let error = logical
            .exec("UPDATE xa_probe SET value = 1", Vec::new())
            .await
            .expect_err("fatal connection error must reach caller");
        assert_eq!(
            error.sql_exception().and_then(|error| error.sql_state()),
            Some("08006")
        );
        assert_eq!(
            recorder
                .error_states
                .lock()
                .expect("error states")
                .as_slice(),
            &[Some("08006".to_owned())]
        );
        assert!(
            owner.xa_resource().is_err(),
            "poisoned owner cannot yield a live XA resource"
        );
        assert!(matches!(
            owner.connection().await,
            Err(RdbcError::ConnectionDiscarded)
        ));
        owner.close().await.expect("cleanup after fatal error");
    }

    #[tokio::test]
    async fn owner_close_failure_notifies_and_keeps_primary_sql_error() {
        let mut owner = JdbcAgentXaConnection::new(
            Box::new(EventTestTransport {
                unusable: Arc::new(AtomicBool::new(false)),
                fail_execute: false,
                fail_close: true,
            }),
            json!({
                "capabilities": {"xa": true, "transactions": true},
                "autoCommit": true,
                "readOnly": false,
                "transactionIsolation": 2,
            }),
            XaResourceManagerIdentity::new(),
        );
        let recorder = Arc::new(EventRecorder::default());
        owner
            .add_connection_event_listener(Arc::clone(&recorder) as Arc<dyn ConnectionEventListener>)
            .expect("listener registration is supported");

        let error = owner
            .close()
            .await
            .expect_err("close failure must propagate");
        assert_eq!(
            error.sql_exception().and_then(|error| error.sql_state()),
            Some("08006")
        );
        assert_eq!(
            recorder
                .error_states
                .lock()
                .expect("error states")
                .as_slice(),
            &[Some("08006".to_owned())]
        );
        assert!(recorder.closed.load(Ordering::Acquire) == 0);
        owner
            .close()
            .await
            .expect("duplicate owner close remains idempotent");
    }
}
