//! bb8 外部连接池桥接。

use crate::sqlx_bb8_connection_manager::SqlxBb8ConnectionManager;
use druid_core::{
    ConnectionRecycleDisposition, DruidError, DruidPooledConnection, FilterChain,
    PhysicalConnectionLease, Pool, PoolState,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// bb8 外部连接池桥接。
///
/// 对应 Java: `javax.sql.DataSource` 的连接获取边界。该对象直接实现
/// druid-rust 的 `Pool`，不会作为 `ConnectionFactory` 再嵌套到 DruidPool。
/// 借出的 bb8 租约经 `PhysicalConnectionLease` 透明委托给底层
/// `SqlxConnectionAdapter`，对外仍统一返回 `DruidPooledConnection`。
pub struct SqlxBb8Pool {
    name: String,
    url: String,
    max_open: usize,
    acquire_timeout: Duration,
    pool: bb8::Pool<SqlxBb8ConnectionManager>,
    filter_chain: Option<Arc<FilterChain>>,
    connection_sequence: AtomicU64,
    connect_count: AtomicU64,
    connect_error_count: AtomicU64,
    close_count: Arc<AtomicU64>,
    recycle_count: Arc<AtomicU64>,
    recycle_error_count: Arc<AtomicU64>,
    discard_count: Arc<AtomicU64>,
}

impl SqlxBb8Pool {
    /// 创建并初始化 bb8 外部连接池桥接。
    ///
    /// `max_open` 必须大于零且不超过 `u32::MAX`；`acquire_timeout` 同时作为
    /// bb8 默认等待上限和 `Pool#get` 的等待上限。
    pub async fn connect(
        name: impl Into<String>,
        url: impl Into<String>,
        max_open: usize,
        acquire_timeout: Duration,
        filter_chain: Option<Arc<FilterChain>>,
    ) -> Result<Self, DruidError> {
        let max_size = u32::try_from(max_open)
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| DruidError::Other("bb8 max_open must be in 1..=u32::MAX".to_string()))?;
        let url = url.into();
        let manager = SqlxBb8ConnectionManager::new(url.clone());
        let pool = bb8::Pool::builder()
            .max_size(max_size)
            .connection_timeout(acquire_timeout)
            .build(manager)
            .await?;
        Ok(Self::from_pool(
            name,
            url,
            max_open,
            acquire_timeout,
            pool,
            filter_chain,
        ))
    }

    /// 用已有 bb8 Pool 创建桥接。
    ///
    /// 参数 `pool` 的连接类型必须是由 `SqlxBb8ConnectionManager` 管理的
    /// `PhysicalConnection`；该方法不会创建第二个池。
    pub fn from_pool(
        name: impl Into<String>,
        url: impl Into<String>,
        max_open: usize,
        acquire_timeout: Duration,
        pool: bb8::Pool<SqlxBb8ConnectionManager>,
        filter_chain: Option<Arc<FilterChain>>,
    ) -> Self {
        Self {
            name: name.into(),
            url: url.into(),
            max_open,
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

    /// 返回底层 bb8 Pool，只用于外部池自身的高级配置或观测。
    pub fn inner(&self) -> &bb8::Pool<SqlxBb8ConnectionManager> {
        &self.pool
    }

    async fn acquire(&self, timeout: Duration) -> Result<DruidPooledConnection, DruidError> {
        self.connect_count.fetch_add(1, Ordering::Relaxed);
        let lease = match tokio::time::timeout(timeout, self.pool.get_owned()).await {
            Ok(Ok(lease)) => lease,
            Ok(Err(bb8::RunError::TimedOut)) | Err(_) => {
                self.connect_error_count.fetch_add(1, Ordering::Relaxed);
                return Err(DruidError::AcquireTimeout);
            }
            Ok(Err(bb8::RunError::User(error))) => {
                self.connect_error_count.fetch_add(1, Ordering::Relaxed);
                return Err(error);
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
                // 丢弃透明租约桥接会触发 bb8 自身唯一的归还路径。
                drop(connection);
            }),
        ))
    }
}

#[async_trait::async_trait]
impl Pool for SqlxBb8Pool {
    /// 获取池化连接。
    async fn get(&self) -> Result<DruidPooledConnection, DruidError> {
        self.acquire(self.acquire_timeout).await
    }

    /// 在指定超时内获取池化连接。
    async fn get_timeout(&self, timeout: Duration) -> Result<DruidPooledConnection, DruidError> {
        self.acquire(timeout).await
    }

    /// 返回映射后的 bb8 连接池状态。
    fn state(&self) -> PoolState {
        let state = self.pool.state();
        PoolState {
            name: self.name.clone(),
            driver_name: self.driver_name().to_string(),
            url: self.url.clone(),
            max_open: self.max_open,
            active_count: state.connections.saturating_sub(state.idle_connections) as usize,
            idle_count: state.idle_connections as usize,
            wait_count: state.statistics.get_waited as usize,
            create_count: state.statistics.connections_created,
            close_count: self.close_count.load(Ordering::Relaxed),
            destroy_count: state.statistics.connections_closed_broken
                + state.statistics.connections_closed_invalid
                + state.statistics.connections_closed_max_lifetime
                + state.statistics.connections_closed_idle_timeout,
            connect_count: self.connect_count.load(Ordering::Relaxed),
            connect_error_count: self.connect_error_count.load(Ordering::Relaxed),
            recycle_count: self.recycle_count.load(Ordering::Relaxed),
            recycle_error_count: self.recycle_error_count.load(Ordering::Relaxed),
            discard_count: self.discard_count.load(Ordering::Relaxed),
            ..PoolState::default()
        }
    }

    /// 返回驱动桥接名称。
    fn driver_name(&self) -> &str {
        "sqlx-bb8"
    }

    /// 返回数据源名称。
    fn name(&self) -> &str {
        &self.name
    }
}
