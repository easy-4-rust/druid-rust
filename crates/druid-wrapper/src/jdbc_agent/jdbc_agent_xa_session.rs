use super::jdbc_agent_connection::JdbcAgentTransport;
use druid_rdbc::error::RdbcError;
use druid_rdbc::error::SqlExceptionKind;
use serde_json::Value as JsonValue;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tokio::sync::Mutex;

/// 序列化 JDBC Agent XA session 的 SQL、XA RPC 和 owner 关闭操作。
/// 对应 Java: `AgentSession` 的同一 session 操作锁与 XAConnection owner。
pub(super) struct JdbcAgentXaSession {
    transport: Mutex<Box<dyn JdbcAgentTransport>>,
    owner_closed: AtomicBool,
    poisoned: AtomicBool,
    logical_active: AtomicBool,
    logical_generation: AtomicU64,
}

impl JdbcAgentXaSession {
    /// 接管已通过 XA capability 协商的 Agent session。
    pub(super) fn new(transport: Box<dyn JdbcAgentTransport>) -> Self {
        Self {
            transport: Mutex::new(transport),
            owner_closed: AtomicBool::new(false),
            poisoned: AtomicBool::new(false),
            logical_active: AtomicBool::new(true),
            logical_generation: AtomicU64::new(1),
        }
    }

    /// 返回当前逻辑连接代次；初始 Agent session 已打开一个逻辑连接。
    pub(super) fn initial_generation(&self) -> u64 {
        self.logical_generation.load(Ordering::Acquire)
    }

    /// 在线程安全边界内发送请求，并拒绝已关闭 owner 或过期逻辑句柄。
    pub(super) async fn request(
        &self,
        operation: &str,
        payload: JsonValue,
        logical_generation: Option<u64>,
    ) -> Result<JsonValue, RdbcError> {
        let mut transport = self.transport.lock().await;
        self.ensure_owner_usable()?;
        self.ensure_transport_usable(&**transport)?;
        if let Some(generation) = logical_generation {
            self.ensure_logical_generation(generation)?;
        }
        let result = transport.request(operation, payload).await;
        if let Err(error) = &result {
            if transport.is_unusable() || Self::is_fatal_connection_error(error) {
                self.poisoned.store(true, Ordering::Release);
            }
        }
        result
    }

    /// 关闭仍属于当前代次的逻辑连接；关闭失败后隔离整个 session。
    pub(super) async fn close_logical_connection(
        &self,
        generation: u64,
    ) -> Result<bool, RdbcError> {
        let mut transport = self.transport.lock().await;
        self.ensure_owner_usable()?;
        if self.logical_generation.load(Ordering::Acquire) != generation
            || !self.logical_active.load(Ordering::Acquire)
        {
            return Ok(false);
        }
        self.ensure_transport_usable(&**transport)?;
        let result = transport
            .request("close_logical_connection", JsonValue::Null)
            .await
            .map(|_| true);
        self.logical_active.store(false, Ordering::Release);
        self.logical_generation.fetch_add(1, Ordering::AcqRel);
        if result.is_err() {
            self.poisoned.store(true, Ordering::Release);
        }
        result
    }

    /// 使旧逻辑句柄失效并在相同 XA owner 上重开句柄。
    pub(super) async fn reopen_logical_connection(&self) -> Result<(u64, bool), RdbcError> {
        let mut transport = self.transport.lock().await;
        self.ensure_owner_usable()?;
        self.ensure_transport_usable(&**transport)?;
        let closed_previous = if self.logical_active.load(Ordering::Acquire) {
            let close_result = transport
                .request("close_logical_connection", JsonValue::Null)
                .await;
            self.logical_active.store(false, Ordering::Release);
            self.logical_generation.fetch_add(1, Ordering::AcqRel);
            if let Err(error) = close_result {
                self.poisoned.store(true, Ordering::Release);
                return Err(error);
            }
            true
        } else {
            false
        };
        if let Err(error) = transport
            .request("reopen_logical_connection", JsonValue::Null)
            .await
        {
            self.poisoned.store(true, Ordering::Release);
            return Err(error);
        }
        self.logical_active.store(true, Ordering::Release);
        Ok((
            self.logical_generation.load(Ordering::Acquire),
            closed_previous,
        ))
    }

    /// 关闭 XA owner 及底层 Agent session，重复调用保持幂等。
    pub(super) async fn close_owner(&self) -> Result<(), RdbcError> {
        let mut transport = self.transport.lock().await;
        if self.owner_closed.swap(true, Ordering::AcqRel) {
            return Ok(());
        }
        self.logical_active.store(false, Ordering::Release);
        self.logical_generation.fetch_add(1, Ordering::AcqRel);
        transport.close().await
    }

    /// 判断 owner 是否关闭、协议是否不确定或当前传输是否不可用。
    pub(super) fn is_unusable(&self) -> bool {
        self.owner_closed.load(Ordering::Acquire)
            || self.poisoned.load(Ordering::Acquire)
            || self
                .transport
                .try_lock()
                .map(|transport| transport.is_unusable())
                .unwrap_or(false)
    }

    /// 验证 owner 和 session 的共享可用状态。
    fn ensure_owner_usable(&self) -> Result<(), RdbcError> {
        if self.owner_closed.load(Ordering::Acquire) || self.poisoned.load(Ordering::Acquire) {
            Err(RdbcError::ConnectionDiscarded)
        } else {
            Ok(())
        }
    }

    /// 将底层 Agent 报告的传输失效提升为 session 级 sticky 隔离状态。
    fn ensure_transport_usable(&self, transport: &dyn JdbcAgentTransport) -> Result<(), RdbcError> {
        if transport.is_unusable() {
            self.poisoned.store(true, Ordering::Release);
            Err(RdbcError::ConnectionDiscarded)
        } else {
            Ok(())
        }
    }

    /// 按 JDBC 异常分类识别必须丢弃的物理连接，保留原始异常载荷。
    fn is_fatal_connection_error(error: &RdbcError) -> bool {
        error.sql_exception().is_some_and(|exception| {
            matches!(
                exception.kind(),
                SqlExceptionKind::TransientConnection
                    | SqlExceptionKind::NonTransientConnection
                    | SqlExceptionKind::Recoverable
            )
        })
    }

    /// 验证逻辑句柄仍是当前活动租约。
    fn ensure_logical_generation(&self, generation: u64) -> Result<(), RdbcError> {
        if !self.logical_active.load(Ordering::Acquire)
            || self.logical_generation.load(Ordering::Acquire) != generation
        {
            Err(RdbcError::ConnectionDiscarded)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::JdbcAgentXaSession;
    use crate::jdbc_agent::jdbc_agent_connection::JdbcAgentTransport;
    use druid_rdbc::error::RdbcError;
    use serde_json::Value as JsonValue;
    use std::sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    };

    struct TransportThatDiesAfterOneRequest {
        unusable: Arc<AtomicBool>,
        attempts: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl JdbcAgentTransport for TransportThatDiesAfterOneRequest {
        async fn request(
            &mut self,
            _operation: &str,
            _payload: JsonValue,
        ) -> Result<JsonValue, RdbcError> {
            self.attempts.fetch_add(1, Ordering::AcqRel);
            self.unusable.store(true, Ordering::Release);
            Err(RdbcError::DriverError("Agent response was lost".to_owned()))
        }

        async fn close(&mut self) -> Result<(), RdbcError> {
            self.unusable.store(true, Ordering::Release);
            Ok(())
        }

        fn is_unusable(&self) -> bool {
            self.unusable.load(Ordering::Acquire)
        }
    }

    #[tokio::test]
    async fn uncertain_transport_failure_is_sticky_and_blocks_later_rpc() {
        let unusable = Arc::new(AtomicBool::new(false));
        let attempts = Arc::new(AtomicUsize::new(0));
        let session = JdbcAgentXaSession::new(Box::new(TransportThatDiesAfterOneRequest {
            unusable: Arc::clone(&unusable),
            attempts: Arc::clone(&attempts),
        }));

        assert!(matches!(
            session.request("xa_commit", JsonValue::Null, None).await,
            Err(RdbcError::DriverError(_))
        ));
        assert!(session.is_unusable());
        assert!(matches!(
            session.request("xa_recover", JsonValue::Null, None).await,
            Err(RdbcError::ConnectionDiscarded)
        ));
        assert_eq!(attempts.load(Ordering::Acquire), 1);
    }
}
