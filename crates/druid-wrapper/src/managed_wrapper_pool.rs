use crate::sqlx::deadpool::SqlxDeadpoolPool;
use crate::WrapperPoolState;
use druid::core::{DruidError, DruidPooledConnection, Pool, PoolState};
use druid::pool::DruidDataSource;
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
        if self.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        self.enabled.store(false, Ordering::Release);
        if let Some(shutdown) = &self.shutdown {
            shutdown.shutdown().await;
        }
    }

    fn ensure_enabled(&self) -> Result<(), DruidError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(DruidError::PoolClosed);
        }
        if self.is_enabled() {
            Ok(())
        } else {
            Err(DruidError::DataSourceDisabled)
        }
    }
}

#[async_trait::async_trait]
impl Pool for ManagedWrapperPool {
    async fn get(&self) -> Result<DruidPooledConnection, DruidError> {
        self.ensure_enabled()?;
        self.inner.get().await
    }

    async fn get_timeout(&self, timeout: Duration) -> Result<DruidPooledConnection, DruidError> {
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
