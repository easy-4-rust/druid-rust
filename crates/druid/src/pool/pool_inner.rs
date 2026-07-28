//! 对应 Java 类：com.alibaba.druid.pool.DruidDataSource（内部状态）
//!
//! 连接池内部状态：空闲队列、活跃计数、等待通知。

use crate::core::{
    ConnectionRecycleDisposition, ConnectionState, DruidConnectionHolder, DruidError,
    PhysicalConnection, PhysicalConnectionFactory, PreparedStatementCacheStats,
};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Notify;

/// 连接池内部状态。
pub struct PoolInner {
    pub(crate) factory: Arc<dyn PhysicalConnectionFactory>,
    pub(crate) config: super::config::PoolInnerConfig,
    pub(crate) idle: parking_lot::Mutex<VecDeque<DruidConnectionHolder>>,
    pub(crate) notify: Notify,
    pub(crate) active_count: AtomicUsize,
    pub(crate) total_count: AtomicUsize,
    pub(crate) next_id: AtomicU64,
    pub(crate) closed: AtomicBool,
    // 统计
    pub(crate) create_count: AtomicU64,
    pub(crate) close_count: AtomicU64,
    pub(crate) destroy_count: AtomicU64,
    pub(crate) connect_count: AtomicU64,
    pub(crate) connect_error_count: AtomicU64,
    pub(crate) recycle_count: AtomicU64,
    pub(crate) recycle_error_count: AtomicU64,
    pub(crate) discard_count: AtomicU64,
    pub(crate) keep_alive_check_count: AtomicU64,
    pub(crate) keep_alive_check_error_count: AtomicU64,
    pub(crate) prepared_statement_stats: Arc<PreparedStatementCacheStats>,
}

impl PoolInner {
    pub fn new(
        factory: Arc<dyn PhysicalConnectionFactory>,
        config: super::config::PoolInnerConfig,
    ) -> Self {
        Self {
            factory,
            config,
            idle: parking_lot::Mutex::new(VecDeque::new()),
            notify: Notify::new(),
            active_count: AtomicUsize::new(0),
            total_count: AtomicUsize::new(0),
            next_id: AtomicU64::new(1),
            closed: AtomicBool::new(false),
            create_count: AtomicU64::new(0),
            close_count: AtomicU64::new(0),
            destroy_count: AtomicU64::new(0),
            connect_count: AtomicU64::new(0),
            connect_error_count: AtomicU64::new(0),
            recycle_count: AtomicU64::new(0),
            recycle_error_count: AtomicU64::new(0),
            discard_count: AtomicU64::new(0),
            keep_alive_check_count: AtomicU64::new(0),
            keep_alive_check_error_count: AtomicU64::new(0),
            prepared_statement_stats: Arc::new(PreparedStatementCacheStats::default()),
        }
    }

    pub fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    pub fn can_grow(&self) -> bool {
        self.total_count.load(Ordering::Acquire) < self.config.max_open
    }

    pub fn should_evict(&self) -> bool {
        let idle_count = self.idle.lock().len();
        idle_count > self.config.min_idle
    }

    /// 创建新连接。
    pub async fn create_connection(&self) -> Result<DruidConnectionHolder, DruidError> {
        let reserved = self
            .total_count
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                (current < self.config.max_open).then_some(current + 1)
            })
            .is_ok();
        if !reserved {
            return Err(DruidError::PoolExhausted);
        }

        let create_started = Instant::now();
        match self.factory.create().await {
            Ok(mut conn) => {
                // Java 在原始驱动连接成功后立即增加 createCount，默认属性初始化失败
                // 也不回退该计数。
                self.create_count.fetch_add(1, Ordering::Relaxed);
                if self.closed.load(Ordering::Acquire) {
                    let _ = self.factory.close(&mut conn).await;
                    self.total_count.fetch_sub(1, Ordering::AcqRel);
                    self.destroy_count.fetch_add(1, Ordering::Relaxed);
                    return Err(DruidError::PoolClosed);
                }

                if let Err(error) = self.initialize_physical_connection(conn.as_mut()).await {
                    // 对应 Java createPhysicalConnection() 的异常路径：初始化失败时
                    // 关闭刚创建的物理连接，但它从未进入池，不增加 destroyCount。
                    let _ = self.factory.close(&mut conn).await;
                    self.total_count.fetch_sub(1, Ordering::AcqRel);
                    self.connect_error_count.fetch_add(1, Ordering::Relaxed);
                    return Err(error);
                }

                let mut holder = DruidConnectionHolder::with_connection(
                    conn,
                    self.next_id(),
                    create_started.elapsed(),
                    0,
                );
                holder.configure_statement_pool(
                    self.config.pool_prepared_statements,
                    self.config.max_pool_prepared_statements_per_connection,
                    self.config.share_prepared_statements,
                    self.config.use_oracle_implicit_cache,
                    self.prepared_statement_stats.clone(),
                );
                let restore_schema = self.config.db_type_name.as_deref().is_some_and(|db_type| {
                    [
                        "mysql",
                        "oceanbase",
                        "ads",
                        "drds",
                        "mariadb",
                        "tidb",
                        "h2",
                        "lealone",
                        "goldendb",
                        "polardbx",
                    ]
                    .iter()
                    .any(|candidate| db_type.eq_ignore_ascii_case(candidate))
                });
                holder.set_restore_schema_on_recycle(restore_schema);
                Ok(holder)
            }
            Err(e) => {
                self.total_count.fetch_sub(1, Ordering::Relaxed);
                self.connect_error_count.fetch_add(1, Ordering::Relaxed);
                Err(e)
            }
        }
    }

    /// 按 Java `DruidAbstractDataSource#initPhysicalConnection` 顺序初始化连接。
    async fn initialize_physical_connection(
        &self,
        connection: &mut dyn PhysicalConnection,
    ) -> Result<(), DruidError> {
        let skip_auto_commit = self.config.db_type_name.as_deref() == Some("odps");

        if !skip_auto_commit && connection.auto_commit() != self.config.default_auto_commit {
            connection
                .set_auto_commit(self.config.default_auto_commit)
                .await?;
        }

        if let Some(default_read_only) = self.config.default_read_only {
            if connection.read_only() != default_read_only {
                connection.set_read_only(default_read_only).await?;
            }
        }

        if let Some(default_transaction_isolation) = self.config.default_transaction_isolation {
            if connection.transaction_isolation() != default_transaction_isolation {
                connection
                    .set_transaction_isolation(default_transaction_isolation)
                    .await?;
            }
        }

        if let Some(default_catalog) = self.config.default_catalog.as_deref() {
            if !default_catalog.is_empty() {
                connection.set_catalog(default_catalog).await?;
            }
        }

        Ok(())
    }

    /// 归还连接到空闲队列。
    pub fn return_connection(
        &self,
        holder: DruidConnectionHolder,
        disposition: ConnectionRecycleDisposition,
    ) {
        let was_active = self
            .active_count
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                (current > 0).then_some(current - 1)
            })
            .is_ok();
        if !was_active {
            self.discard_count.fetch_add(1, Ordering::Relaxed);
            self.destroy_holder(holder);
            return;
        }

        // Java closeCount 统计逻辑池化连接关闭，而不是物理 socket 关闭。
        self.close_count.fetch_add(1, Ordering::Relaxed);
        let holder_was_active = holder.mark_idle();

        if disposition.has_recycle_error() {
            self.recycle_error_count.fetch_add(1, Ordering::Relaxed);
        }
        if !holder_was_active || !disposition.is_reusable() {
            self.discard_count.fetch_add(1, Ordering::Relaxed);
            self.destroy_holder(holder);
            return;
        }

        let unusable = holder
            .physical_connection()
            .is_none_or(|connection| connection.is_closed() || connection.is_discarded());
        if self.closed.load(Ordering::Acquire) || unusable || holder.is_discard() {
            self.destroy_holder(holder);
            return;
        }

        let physical_age = holder.physical_age();
        let lifetime_expired = physical_age >= self.config.max_lifetime;
        let physical_timeout_expired = self
            .config
            .physical_connection_timeout
            .is_some_and(|timeout| !timeout.is_zero() && physical_age > timeout);
        let max_use_count_reached =
            self.config.max_use_count > 0 && holder.use_count() >= self.config.max_use_count as u64;
        if lifetime_expired || physical_timeout_expired || max_use_count_reached {
            self.discard_count.fetch_add(1, Ordering::Relaxed);
            self.destroy_holder(holder);
            return;
        }

        let returned = {
            let mut queue = self.idle.lock();
            if queue.len() >= self.config.max_idle {
                Err(holder)
            } else {
                queue.push_back(holder);
                Ok(())
            }
        };

        // Java 在 putLast 尝试完成后递增 recycleCount，即使池满导致 putLast=false。
        self.recycle_count.fetch_add(1, Ordering::Relaxed);
        if let Err(holder) = returned {
            self.destroy_holder(holder);
        } else {
            self.notify.notify_one();
        }
    }

    /// 销毁 canonical holder 中的物理连接。
    pub fn destroy_holder(&self, mut holder: DruidConnectionHolder) {
        holder.mark_discarded();
        holder.clear_statement_cache();
        if let Some(connection) = holder.take_physical_connection() {
            self.destroy_connection(connection);
        } else {
            self.record_destroy();
        }
    }

    /// 销毁连接。
    pub fn destroy_connection(&self, mut conn: Box<dyn PhysicalConnection>) {
        self.record_destroy();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                let _ = conn.close().await;
            });
        }
    }

    fn record_destroy(&self) {
        let _ = self
            .total_count
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                (current > 0).then_some(current - 1)
            });
        self.destroy_count.fetch_add(1, Ordering::Relaxed);
    }

    /// 等待 holder 中的物理连接完成关闭。
    pub async fn destroy_holder_now(&self, mut holder: DruidConnectionHolder) {
        holder.mark_discarded();
        holder.clear_statement_cache();
        self.record_destroy();
        if let Some(mut connection) = holder.take_physical_connection() {
            let _ = self.factory.close(&mut connection).await;
        }
    }

    /// 按 Java `DruidDataSource#shrink(checkTime, keepAlive)` 驱逐和保活空闲连接。
    ///
    /// # 参数
    /// - `check_time`：是否按空闲时间、物理寿命和保活时间筛选。
    /// - `keep_alive`：是否对达到间隔的连接执行有效性检查。
    pub async fn shrink(&self, check_time: bool, keep_alive: bool) {
        let (evict_connections, keep_alive_connections) = {
            let mut queue = self.idle.lock();
            if queue.is_empty() {
                return;
            }

            let check_count = queue.len().saturating_sub(self.config.min_idle);
            let mut retained = VecDeque::with_capacity(queue.len());
            let mut evicted = Vec::new();
            let mut keep_alive_candidates = Vec::new();
            let mut index = 0usize;

            while let Some(holder) = queue.pop_front() {
                let physical_timeout_expired = self
                    .config
                    .physical_connection_timeout
                    .is_some_and(|timeout| !timeout.is_zero() && holder.physical_age() > timeout);
                if physical_timeout_expired {
                    evicted.push(holder);
                    index += 1;
                    continue;
                }

                if !check_time {
                    if index < check_count {
                        evicted.push(holder);
                    } else {
                        retained.push_back(holder);
                        retained.append(&mut queue);
                        break;
                    }
                    index += 1;
                    continue;
                }

                let idle_duration = holder.idle_duration();
                if idle_duration < self.config.idle_timeout
                    && idle_duration < self.config.keep_alive_between_time
                {
                    retained.push_back(holder);
                    retained.append(&mut queue);
                    break;
                }

                if idle_duration >= self.config.idle_timeout
                    && (index < check_count || idle_duration > self.config.max_evictable_idle_time)
                {
                    evicted.push(holder);
                    index += 1;
                    continue;
                }

                let keep_alive_due = keep_alive
                    && idle_duration >= self.config.keep_alive_between_time
                    && holder.last_keep_elapsed().unwrap_or(Duration::MAX)
                        >= self.config.keep_alive_between_time;
                if keep_alive_due {
                    keep_alive_candidates.push(holder);
                } else {
                    retained.push_back(holder);
                }
                index += 1;
            }

            *queue = retained;
            (evicted, keep_alive_candidates)
        };

        for holder in evict_connections {
            self.destroy_holder_now(holder).await;
        }

        if keep_alive_connections.is_empty() {
            return;
        }

        self.keep_alive_check_count
            .fetch_add(keep_alive_connections.len() as u64, Ordering::Relaxed);
        let mut validated = VecDeque::new();
        for mut holder in keep_alive_connections.into_iter().rev() {
            holder.increment_keep_alive_check_count();
            let entered_validation =
                holder.try_transition(ConnectionState::Idle, ConnectionState::Validating);
            let valid = if entered_validation {
                match holder.physical_connection_box_mut() {
                    Some(connection) => self.factory.validate(connection).await.is_ok(),
                    None => false,
                }
            } else {
                false
            };

            if valid && holder.try_transition(ConnectionState::Validating, ConnectionState::Idle) {
                holder.record_keep_alive();
                validated.push_front(holder);
            } else {
                holder.mark_discarded();
                self.keep_alive_check_error_count
                    .fetch_add(1, Ordering::Relaxed);
                self.discard_count.fetch_add(1, Ordering::Relaxed);
                self.destroy_holder_now(holder).await;
            }
        }

        if !validated.is_empty() {
            let mut queue = self.idle.lock();
            validated.append(&mut queue);
            *queue = validated;
            self.notify.notify_waiters();
        }
    }

    /// 关闭池。
    pub async fn close(&self) {
        self.closed.store(true, Ordering::Relaxed);
        let idle: Vec<DruidConnectionHolder> = {
            let mut queue = self.idle.lock();
            queue.drain(..).collect()
        };
        for mut holder in idle {
            holder.mark_discarded();
            if let Some(mut connection) = holder.take_physical_connection() {
                let _ = self.factory.close(&mut connection).await;
            }
            let _ = self
                .total_count
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                    (current > 0).then_some(current - 1)
                });
            self.destroy_count.fetch_add(1, Ordering::Relaxed);
        }
        self.notify.notify_waiters();
    }
}
