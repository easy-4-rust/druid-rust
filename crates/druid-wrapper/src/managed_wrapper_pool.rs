#[cfg(feature = "sqlx-deadpool")]
use crate::sqlx::deadpool::SqlxDeadpoolPool;
use crate::WrapperPoolState;
use druid::core::{DruidPooledConnection, Pool, PoolState};
use druid::pool::DruidDataSource;
use druid_rdbc::error::RdbcError;
use parking_lot::RwLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// 统一管理 native/external wrapper pool 的门面。
///
/// 规划迁移 Java `ManagedBasicDataSource`、c3p0/Proxool `DataSource` 的共同
/// `DataSource` 语义。它只委托一个已选定的 Pool，不拥有第二层 idle queue。
pub struct ManagedWrapperPool {
    provider: String,
    inner: Arc<dyn Pool>,
    enabled: AtomicBool,
    closed: AtomicBool,
    shutdown_complete: AtomicBool,
    shutdown_lock: tokio::sync::Mutex<()>,
    object_name: RwLock<Option<String>>,
    shutdown: Option<Arc<dyn WrapperPoolShutdown>>,
}

#[async_trait::async_trait]
pub(crate) trait WrapperPoolShutdown: Send + Sync {
    async fn shutdown(&self);
}

#[async_trait::async_trait]
impl WrapperPoolShutdown for DruidDataSource {
    async fn shutdown(&self) {
        self.close().await;
    }
}

#[async_trait::async_trait]
#[cfg(feature = "sqlx-deadpool")]
impl WrapperPoolShutdown for SqlxDeadpoolPool {
    async fn shutdown(&self) {
        self.close();
    }
}

impl ManagedWrapperPool {
    /// 创建单池 managed facade。
    #[must_use]
    pub fn new(provider: impl Into<String>, inner: Arc<dyn Pool>) -> Self {
        Self {
            provider: provider.into(),
            inner,
            enabled: AtomicBool::new(true),
            closed: AtomicBool::new(false),
            shutdown_complete: AtomicBool::new(false),
            shutdown_lock: tokio::sync::Mutex::new(()),
            object_name: RwLock::new(None),
            shutdown: None,
        }
    }

    pub(crate) fn with_shutdown<T>(provider: impl Into<String>, inner: Arc<T>) -> Self
    where
        T: Pool + WrapperPoolShutdown + 'static,
    {
        let pool: Arc<dyn Pool> = Arc::clone(&inner) as Arc<_>;
        let shutdown: Arc<dyn WrapperPoolShutdown> = inner;
        Self {
            provider: provider.into(),
            inner: pool,
            enabled: AtomicBool::new(true),
            closed: AtomicBool::new(false),
            shutdown_complete: AtomicBool::new(false),
            shutdown_lock: tokio::sync::Mutex::new(()),
            object_name: RwLock::new(None),
            shutdown: Some(shutdown),
        }
    }

    /// 返回选择的 provider 名。
    #[must_use]
    pub fn provider(&self) -> &str {
        &self.provider
    }

    /// 返回启用状态。
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }

    /// 启用或禁用获取新连接。
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Release);
    }

    /// 返回管理对象名。
    #[must_use]
    pub fn object_name(&self) -> Option<String> {
        self.object_name.read().clone()
    }

    /// 设置管理对象名。
    pub fn set_object_name(&self, object_name: Option<String>) {
        *self.object_name.write() = object_name;
    }

    /// 返回 wrapper 管理快照。
    #[must_use]
    pub fn wrapper_state(&self) -> WrapperPoolState {
        WrapperPoolState::from_pool_state(&self.provider, self.state())
    }

    /// 关闭 facade 及支持显式 shutdown 的底层 pool。
    pub async fn close(&self) {
        // 门禁先关闭；若上一次 close future 被取消，后续调用仍须继续底层清理。
        self.closed.store(true, Ordering::Release);
        self.enabled.store(false, Ordering::Release);
        let _shutdown_guard = self.shutdown_lock.lock().await;
        if self.shutdown_complete.load(Ordering::Acquire) {
            return;
        }
        if let Some(shutdown) = &self.shutdown {
            shutdown.shutdown().await;
        }
        self.shutdown_complete.store(true, Ordering::Release);
    }

    fn ensure_enabled(&self) -> Result<(), RdbcError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(RdbcError::PoolClosed);
        }
        if self.is_enabled() {
            Ok(())
        } else {
            Err(RdbcError::DataSourceDisabled)
        }
    }
}

#[async_trait::async_trait]
impl Pool for ManagedWrapperPool {
    async fn get(&self) -> Result<DruidPooledConnection, RdbcError> {
        self.ensure_enabled()?;
        self.inner.get().await
    }

    async fn get_timeout(&self, timeout: Duration) -> Result<DruidPooledConnection, RdbcError> {
        self.ensure_enabled()?;
        self.inner.get_timeout(timeout).await
    }

    fn state(&self) -> PoolState {
        let mut state = self.inner.state();
        state.closed |= self.closed.load(Ordering::Acquire);
        state
    }

    fn driver_name(&self) -> &str {
        self.inner.driver_name()
    }

    fn name(&self) -> &str {
        self.inner.name()
    }
}

#[cfg(test)]
mod tests {
    use super::{ManagedWrapperPool, WrapperPoolShutdown};
    use druid::core::{DruidPooledConnection, Pool, PoolState};
    use druid_rdbc::error::RdbcError;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::{Notify, Semaphore};

    struct SlowShutdownPool {
        calls: AtomicUsize,
        entered: Notify,
        release: Semaphore,
    }

    #[async_trait::async_trait]
    impl WrapperPoolShutdown for SlowShutdownPool {
        async fn shutdown(&self) {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.entered.notify_one();
            let permit = self.release.acquire().await.expect("semaphore stays open");
            permit.forget();
        }
    }

    #[async_trait::async_trait]
    impl Pool for SlowShutdownPool {
        async fn get(&self) -> Result<DruidPooledConnection, RdbcError> {
            Err(RdbcError::PoolClosed)
        }

        async fn get_timeout(
            &self,
            _timeout: Duration,
        ) -> Result<DruidPooledConnection, RdbcError> {
            Err(RdbcError::PoolClosed)
        }

        fn state(&self) -> PoolState {
            PoolState::default()
        }

        fn driver_name(&self) -> &str {
            "slow-shutdown"
        }

        fn name(&self) -> &str {
            "slow-shutdown"
        }
    }

    #[tokio::test]
    async fn cancelled_close_can_be_retried_until_underlying_shutdown_finishes() {
        let inner = Arc::new(SlowShutdownPool {
            calls: AtomicUsize::new(0),
            entered: Notify::new(),
            release: Semaphore::new(0),
        });
        let wrapper = Arc::new(ManagedWrapperPool::with_shutdown(
            "native",
            Arc::clone(&inner),
        ));
        let first_wrapper = Arc::clone(&wrapper);
        let first = tokio::spawn(async move { first_wrapper.close().await });
        tokio::time::timeout(Duration::from_secs(1), inner.entered.notified())
            .await
            .expect("first close must reach the physical pool");
        first.abort();
        first.await.expect_err("first close was cancelled");

        inner.release.add_permits(1);
        tokio::time::timeout(Duration::from_secs(1), wrapper.close())
            .await
            .expect("retry must complete physical shutdown");
        assert_eq!(inner.calls.load(Ordering::SeqCst), 2);
        assert!(wrapper.state().closed);
        wrapper.close().await;
        assert_eq!(inner.calls.load(Ordering::SeqCst), 2);
    }
}
