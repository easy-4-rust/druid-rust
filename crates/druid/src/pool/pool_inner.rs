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
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::Notify;

/// 连接池内部状态。
pub struct PoolInner {
    pub(crate) factory: Arc<dyn PhysicalConnectionFactory>,
    pub(crate) config: super::config::PoolInnerConfig,
    pub(crate) idle: parking_lot::Mutex<VecDeque<DruidConnectionHolder>>,
    pub(crate) notify: Notify,
    pub(crate) active_count: AtomicUsize,
    pub(crate) wait_count: AtomicUsize,
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
    close_sender: parking_lot::RwLock<Option<UnboundedSender<Option<Box<dyn PhysicalConnection>>>>>,
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
            wait_count: AtomicUsize::new(0),
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
            close_sender: parking_lot::RwLock::new(None),
        }
    }

    /// 安装由 canonical `DruidPool` 持有的物理关闭 worker sender。
    pub(crate) fn install_close_sender(
        &self,
        sender: UnboundedSender<Option<Box<dyn PhysicalConnection>>>,
    ) {
        *self.close_sender.write() = Some(sender);
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

    /// 重置数据源累计统计，保留当前连接数量和缓存占用。
    pub(crate) fn reset_stats(&self) {
        self.create_count.store(0, Ordering::Release);
        self.close_count.store(0, Ordering::Release);
        self.destroy_count.store(0, Ordering::Release);
        self.connect_count.store(0, Ordering::Release);
        self.connect_error_count.store(0, Ordering::Release);
        self.recycle_count.store(0, Ordering::Release);
        self.recycle_error_count.store(0, Ordering::Release);
        self.discard_count.store(0, Ordering::Release);
        self.keep_alive_check_count.store(0, Ordering::Release);
        self.keep_alive_check_error_count
            .store(0, Ordering::Release);
        self.prepared_statement_stats.reset();
    }

    /// 使用数据源配置的 Java `ValidConnectionChecker` 校验物理连接。
    pub(crate) async fn validate_connection(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
    ) -> Result<(), DruidError> {
        if let Some(checker) = self.config.valid_connection_checker.as_ref() {
            let valid = checker
                .is_valid_connection(
                    connection,
                    self.config.validation_query.as_deref(),
                    self.config.validation_query_timeout,
                )
                .await?;
            return if valid {
                Ok(())
            } else {
                Err(DruidError::ValidationFailed(
                    "ValidConnectionChecker returned false".to_owned(),
                ))
            };
        }
        self.factory.validate(connection).await
    }

    /// 按 Java `initialSize` 预建空闲物理连接。
    pub(crate) async fn fill_initial(&self) -> Result<(), DruidError> {
        self.fill(self.config.initial_size).await?;
        Ok(())
    }

    /// 将池内物理连接总数填充到指定数量，返回本次创建数。
    ///
    /// 对应 Java：`DruidDataSource#fill(int)`。目标会被 `maxActive` 截断；
    /// 已有活跃连接计入总数，新连接只进入空闲队列。
    pub(crate) async fn fill(&self, to_count: usize) -> Result<usize, DruidError> {
        let target = to_count.min(self.config.max_open);
        let mut created = 0usize;
        while self.total_count.load(Ordering::Acquire) < target {
            let holder = self.create_connection().await?;
            self.idle.lock().push_back(holder);
            created += 1;
        }
        if created > 0 {
            self.notify.notify_waiters();
        }
        Ok(created)
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
        let mut reservation = ConnectionSlotReservation::new(&self.total_count);

        let create_started = Instant::now();
        match self.factory.create().await {
            Ok(mut conn) => {
                // Java 在原始驱动连接成功后立即增加 createCount，默认属性初始化失败
                // 也不回退该计数。
                self.create_count.fetch_add(1, Ordering::Relaxed);
                if self.closed.load(Ordering::Acquire) {
                    let _ = self.factory.close(&mut conn).await;
                    self.destroy_count.fetch_add(1, Ordering::Relaxed);
                    return Err(DruidError::PoolClosed);
                }

                if let Err(error) = self.initialize_physical_connection(conn.as_mut()).await {
                    // 对应 Java createPhysicalConnection() 的异常路径：初始化失败时
                    // 关闭刚创建的物理连接，但它从未进入池，不增加 destroyCount。
                    let _ = self.factory.close(&mut conn).await;
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
                reservation.commit();
                Ok(holder)
            }
            Err(e) => {
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
        // 所有 return 分支结束后再决定是否关闭 worker，保证最后一条
        // connection command 一定排在 shutdown command 之前。
        let _termination = CloseWorkerTerminationGuard { inner: self };
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
            // Java Druid 的 maxIdle 已 deprecated，真实 idle 容量由 maxActive
            // 限制；保留配置字段仅用于兼容读取，不能改变 putLast 语义。
            if queue.len() >= self.config.max_open {
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
    pub fn destroy_connection(&self, conn: Box<dyn PhysicalConnection>) {
        self.record_destroy();
        let sender = self.close_sender.read().clone();
        if let Some(sender) = sender {
            if let Err(error) = sender.send(Some(conn)) {
                // worker 已退出时，Drop 仍会释放 driver 资源；禁止重新 spawn
                // 一条不可追踪任务。
                drop(error.0);
            }
        } else {
            // 只有直接构造公开 PoolInner 的测试/低层调用会走到这里；canonical
            // DruidPool 在对外可借用前必定安装 worker。
            drop(conn);
        }
    }

    /// 当池已关闭且最后一个活跃租约已归还时请求关闭 worker。
    pub(crate) fn request_close_worker_shutdown_if_idle(&self) {
        if !self.closed.load(Ordering::Acquire) || self.active_count.load(Ordering::Acquire) != 0 {
            return;
        }
        if let Some(sender) = self.close_sender.read().clone() {
            let _ = sender.send(None);
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
        if self.closed.load(Ordering::Acquire) {
            return;
        }
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

                let physical_timeout_expired = self
                    .config
                    .physical_connection_timeout
                    .is_some_and(|timeout| !timeout.is_zero() && holder.physical_age() > timeout);
                if physical_timeout_expired {
                    evicted.push(holder);
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

        // 候选连接已经脱离 idle 队列。先全部纳入 RAII 守卫，再进入任何
        // `.await`；这样显式 shrink future 或维护任务被取消时，剩余 holder
        // 会统一销毁并归还 totalCount，不会成为池账本之外的悬空资源。
        let evict_connections: Vec<DetachedHolder<'_>> = evict_connections
            .into_iter()
            .map(|holder| DetachedHolder::new(self, holder))
            .collect();
        let keep_alive_connections: Vec<DetachedHolder<'_>> = keep_alive_connections
            .into_iter()
            .map(|holder| DetachedHolder::new(self, holder))
            .collect();

        for mut candidate in evict_connections {
            candidate.destroy_now().await;
        }

        if keep_alive_connections.is_empty() {
            if keep_alive && self.total_count.load(Ordering::Acquire) < self.config.min_idle {
                let _ = self.fill(self.config.min_idle).await;
            }
            return;
        }

        self.keep_alive_check_count
            .fetch_add(keep_alive_connections.len() as u64, Ordering::Relaxed);
        let mut validated: VecDeque<DetachedHolder<'_>> = VecDeque::new();
        for mut candidate in keep_alive_connections.into_iter().rev() {
            let holder = candidate.holder_mut();
            holder.increment_keep_alive_check_count();
            let entered_validation =
                holder.try_transition(ConnectionState::Idle, ConnectionState::Validating);
            let valid = if entered_validation {
                match holder.physical_connection_box_mut() {
                    Some(connection) => self.validate_connection(connection).await.is_ok(),
                    None => false,
                }
            } else {
                false
            };

            if valid && holder.try_transition(ConnectionState::Validating, ConnectionState::Idle) {
                holder.record_keep_alive();
                validated.push_front(candidate);
            } else {
                holder.mark_discarded();
                self.keep_alive_check_error_count
                    .fetch_add(1, Ordering::Relaxed);
                self.discard_count.fetch_add(1, Ordering::Relaxed);
                candidate.destroy_now().await;
            }
        }

        if !validated.is_empty() && !self.closed.load(Ordering::Acquire) {
            let mut queue = self.idle.lock();
            // close 可能在读取 closed 与拿到 idle 锁之间发生；拿锁后再次确认，
            // 防止已经关闭的数据源重新出现空闲连接。
            if !self.closed.load(Ordering::Acquire) {
                let mut returned = VecDeque::with_capacity(validated.len() + queue.len());
                while let Some(mut candidate) = validated.pop_front() {
                    returned.push_back(candidate.take());
                }
                returned.append(&mut queue);
                *queue = returned;
                self.notify.notify_waiters();
            }
        }

        // Java keepAlive shrink 在驱逐或校验失败后会触发 emptySignal(fillCount)。
        // Rust 没有独立 creator 线程，直接异步补齐到 minIdle，创建预留仍由
        // ConnectionSlotReservation 保证错误与取消安全。
        if keep_alive && self.total_count.load(Ordering::Acquire) < self.config.min_idle {
            let _ = self.fill(self.config.min_idle).await;
        }
    }

    /// 关闭池。
    pub async fn close(&self) {
        self.closed.store(true, Ordering::Relaxed);
        let idle: Vec<DetachedHolder<'_>> = {
            let mut queue = self.idle.lock();
            queue
                .drain(..)
                .map(|holder| DetachedHolder::new(self, holder))
                .collect()
        };
        for mut candidate in idle {
            candidate.destroy_now().await;
        }
        self.notify.notify_waiters();
    }
}

/// 物理连接创建期间的容量预留守卫。
///
/// Rust future 可在任意 `.await` 被取消；Java creator 线程则依靠 finally
/// 释放 creatingCount。守卫只有在 holder 完整构造后 commit，其余错误或取消
/// 路径都会恢复 `total_count`。
struct ConnectionSlotReservation<'a> {
    total_count: &'a AtomicUsize,
    committed: bool,
}

impl<'a> ConnectionSlotReservation<'a> {
    fn new(total_count: &'a AtomicUsize) -> Self {
        Self {
            total_count,
            committed: false,
        }
    }

    fn commit(&mut self) {
        self.committed = true;
    }
}

impl Drop for ConnectionSlotReservation<'_> {
    fn drop(&mut self) {
        if !self.committed {
            let _ = self
                .total_count
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                    (current > 0).then_some(current - 1)
                });
        }
    }
}

/// 已从池队列摘出的连接持有者守卫。
///
/// 对应 Java `shrink` 在锁内维护的 `evictConnections` /
/// `keepAliveConnections` 临时数组。Rust 的异步调用可在校验或关闭时被取消，
/// 因此临时所有权必须由 Drop 托底；显式 `take` 表示 holder 已安全回到 idle
/// 队列，`destroy_now` 表示已经计入销毁并等待物理关闭。
struct DetachedHolder<'a> {
    inner: &'a PoolInner,
    holder: Option<DruidConnectionHolder>,
}

impl<'a> DetachedHolder<'a> {
    fn new(inner: &'a PoolInner, holder: DruidConnectionHolder) -> Self {
        Self {
            inner,
            holder: Some(holder),
        }
    }

    fn holder_mut(&mut self) -> &mut DruidConnectionHolder {
        self.holder.as_mut().expect("detached holder is present")
    }

    fn take(&mut self) -> DruidConnectionHolder {
        self.holder.take().expect("detached holder is present")
    }

    async fn destroy_now(&mut self) {
        if let Some(holder) = self.holder.take() {
            self.inner.destroy_holder_now(holder).await;
        }
    }
}

impl Drop for DetachedHolder<'_> {
    fn drop(&mut self) {
        if let Some(holder) = self.holder.take() {
            self.inner.destroy_holder(holder);
        }
    }
}

/// 确保最后一个活跃连接的关闭命令先于 worker shutdown 命令入队。
struct CloseWorkerTerminationGuard<'a> {
    inner: &'a PoolInner,
}

impl Drop for CloseWorkerTerminationGuard<'_> {
    fn drop(&mut self) {
        self.inner.request_close_worker_shutdown_if_idle();
    }
}
