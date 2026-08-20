//! deadpool 外部连接池桥接。

use super::sqlx_deadpool_connection_manager::SqlxDeadpoolConnectionManager;
use deadpool::managed::{Pool as DeadpoolPool, PoolError, Timeouts};
use druid_core::core::{
    ConnectionRecycleDisposition, DruidError, DruidPooledConnection, FilterChain,
    PhysicalConnectionLease, Pool as DruidPool, PoolState,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// deadpool 外部连接池桥接。
///
/// 对应 Java: `javax.sql.DataSource` 的连接获取边界。该对象直接实现
/// druid-rust 的 `Pool`，不会作为 `ConnectionFactory` 嵌套到 `DruidPool`。
/// deadpool 租约经 `PhysicalConnectionLease` 委托到底层
/// `SqlxConnectionAdapter`，对外仍返回统一的 `DruidPooledConnection`。
pub struct SqlxDeadpoolPool {
    name: String,
    url: String,
    acquire_timeout: Duration,
    pool: DeadpoolPool<SqlxDeadpoolConnectionManager>,
    filter_chain: Option<Arc<FilterChain>>,
    connection_sequence: AtomicU64,
    connect_count: AtomicU64,
    connect_error_count: AtomicU64,
    close_count: Arc<AtomicU64>,
    recycle_count: Arc<AtomicU64>,
    recycle_error_count: Arc<AtomicU64>,
    discard_count: Arc<AtomicU64>,
}

impl SqlxDeadpoolPool {
    /// 创建 deadpool 外部连接池桥接。
    ///
    /// `max_open` 必须大于零；所有等待、创建和回收操作使用
    /// `acquire_timeout` 作为默认超时。
    pub fn connect(
        name: impl Into<String>,
        url: impl Into<String>,
        max_open: usize,
        acquire_timeout: Duration,
        filter_chain: Option<Arc<FilterChain>>,
    ) -> Result<Self, DruidError> {
        if max_open == 0 {
            return Err(DruidError::Other(
                "deadpool max_open must be greater than zero".to_string(),
            ));
        }
        let url = url.into();
        let manager = SqlxDeadpoolConnectionManager::new(url.clone());
        let pool = DeadpoolPool::builder(manager)
            .max_size(max_open)
            .wait_timeout(Some(acquire_timeout))
            .create_timeout(Some(acquire_timeout))
            .recycle_timeout(Some(acquire_timeout))
            .runtime(deadpool::Runtime::Tokio1)
            .build()
            .map_err(|error| DruidError::Other(error.to_string()))?;
        Ok(Self::from_pool(
            name,
            url,
            acquire_timeout,
            pool,
            filter_chain,
        ))
    }

    /// 用已有 deadpool Pool 创建桥接。
    ///
    /// 参数 `pool` 的对象必须由 `SqlxDeadpoolConnectionManager` 管理；
    /// 该方法不会再创建 `DruidPool`。
    pub fn from_pool(
        name: impl Into<String>,
        url: impl Into<String>,
        acquire_timeout: Duration,
        pool: DeadpoolPool<SqlxDeadpoolConnectionManager>,
        filter_chain: Option<Arc<FilterChain>>,
    ) -> Self {
        Self {
            name: name.into(),
            url: url.into(),
            acquire_timeout,
            pool,
            filter_chain,
            connection_sequence: AtomicU64::new(0),
            connect_count: AtomicU64::new(0),
            connect_error_count: AtomicU64::new(0),
            close_count: Arc::new(AtomicU64::new(0)),
            recycle_count: Arc::new(AtomicU64::new(0)),
            recycle_error_count: Arc::new(AtomicU64::new(0)),
            discard_count: Arc::new(AtomicU64::new(0)),
        }
    }

    /// 返回底层 deadpool Pool，只用于外部池自身的高级配置或观测。
    pub fn inner(&self) -> &DeadpoolPool<SqlxDeadpoolConnectionManager> {
        &self.pool
    }

    /// 关闭 deadpool；在途租约归还后不再重新进入池。
    pub fn close(&self) {
        self.pool.close();
    }

    fn map_pool_error(error: PoolError<DruidError>) -> DruidError {
        match error {
            PoolError::Timeout(_) => DruidError::AcquireTimeout,
            PoolError::Backend(error) => error,
            PoolError::Closed => DruidError::PoolClosed,
            PoolError::NoRuntimeSpecified | PoolError::PostCreateHook(_) => {
                DruidError::Other(error.to_string())
            }
        }
    }

    async fn acquire(&self, timeout: Duration) -> Result<DruidPooledConnection, DruidError> {
        self.connect_count.fetch_add(1, Ordering::Relaxed);
        let timeouts = Timeouts {
            wait: Some(timeout),
            create: Some(timeout),
            recycle: Some(timeout),
        };
        let lease = match self.pool.timeout_get(&timeouts).await {
            Ok(lease) => lease,
            Err(error) => {
                self.connect_error_count.fetch_add(1, Ordering::Relaxed);
                return Err(Self::map_pool_error(error));
            }
        };

        let id = self.connection_sequence.fetch_add(1, Ordering::Relaxed) + 1;
        let close_count = self.close_count.clone();
        let recycle_count = self.recycle_count.clone();
        let recycle_error_count = self.recycle_error_count.clone();
        let discard_count = self.discard_count.clone();
        Ok(DruidPooledConnection::with_recycle_policy(
            Box::new(PhysicalConnectionLease::new(lease)),
            id,
            self.name.clone(),
            self.filter_chain.clone(),
            false,
            None,
            Box::new(move |mut connection, _connection_id, disposition| {
                close_count.fetch_add(1, Ordering::Relaxed);
                match disposition {
                    ConnectionRecycleDisposition::Reusable => {
                        recycle_count.fetch_add(1, Ordering::Relaxed);
                    }
                    ConnectionRecycleDisposition::Discard { recycle_error } => {
                        connection.mark_discarded();
                        discard_count.fetch_add(1, Ordering::Relaxed);
                        if recycle_error.is_some() {
                            recycle_error_count.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
                // 丢弃透明租约桥接会触发 deadpool 自身唯一的归还路径。
                drop(connection);
                false
            }),
        ))
    }
}

#[async_trait::async_trait]
impl DruidPool for SqlxDeadpoolPool {
    /// 获取池化连接。
    async fn get(&self) -> Result<DruidPooledConnection, DruidError> {
        self.acquire(self.acquire_timeout).await
    }

    /// 在指定超时内获取池化连接。
    async fn get_timeout(&self, timeout: Duration) -> Result<DruidPooledConnection, DruidError> {
        self.acquire(timeout).await
    }

    /// 返回映射后的 deadpool 状态。
    fn state(&self) -> PoolState {
        let state = self.pool.status();
        PoolState {
            name: self.name.clone(),
            driver_name: self.driver_name().to_string(),
            url: self.url.clone(),
            max_open: state.max_size,
            active_count: state.size.saturating_sub(state.available),
            idle_count: state.available,
            wait_count: state.waiting,
            create_count: self.pool.manager().create_count(),
            close_count: self.close_count.load(Ordering::Relaxed),
            connect_count: self.connect_count.load(Ordering::Relaxed),
            connect_error_count: self.connect_error_count.load(Ordering::Relaxed),
            recycle_count: self.recycle_count.load(Ordering::Relaxed),
            recycle_error_count: self.recycle_error_count.load(Ordering::Relaxed),
            discard_count: self.discard_count.load(Ordering::Relaxed),
            closed: self.pool.is_closed(),
            ..PoolState::default()
        }
    }

    /// 返回驱动桥接名称。
    fn driver_name(&self) -> &'static str {
        "sqlx-deadpool"
    }

    /// 返回数据源名称。
    fn name(&self) -> &str {
        &self.name
    }
}
