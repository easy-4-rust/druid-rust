//! 对应 Java 类：com.alibaba.druid.pool.DruidDataSource
//!
//! Druid 风格连接池。

use super::active_connection_lease::ActiveConnectionLease;
use super::config::DruidPoolBuilder;
use super::connection_close_worker::ConnectionCloseWorker;
use super::pool_inner::PoolInner;
use super::pool_validation_factory::PoolValidationFactory;
use crate::core::{
    Db2ExceptionSorter, DruidConnectionHolder, DruidError, DruidPooledConnection, ExceptionSorter,
    FilterChain, InformixExceptionSorter, MockExceptionSorter, MsSqlValidConnectionChecker,
    MySqlExceptionSorter, MySqlValidConnectionChecker, OceanBaseOracleExceptionSorter,
    OceanBaseValidConnectionChecker, OracleExceptionSorter, OracleValidConnectionChecker,
    PgExceptionSorter, PgValidConnectionChecker, PhoenixExceptionSorter, PhysicalConnectionFactory,
    PoolState, SybaseExceptionSorter, ValidConnectionChecker,
};
use crate::sql::WallProvider;
use crate::stats::StatsCollector;
use dashmap::DashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Notify, OnceCell};

/// Druid 风格连接池。
///
/// 对应 Druid Java 的 `DruidDataSource`，实现 max_open / min_idle /
/// acquire_timeout / FilterChain 装配 / DruidPooledConnection::drop 归还。
pub struct DruidPool {
    name: String,
    driver_name: String,
    inner: Arc<PoolInner>,
    filter_chain: Option<Arc<FilterChain>>,
    exception_sorter: Option<Arc<dyn ExceptionSorter>>,
    filters_initialized: AtomicBool,
    initialized: OnceCell<()>,
    active_leases: Arc<DashMap<u64, ActiveConnectionLease>>,
    remove_abandoned_count: Arc<AtomicU64>,
    maintenance_shutdown: Arc<Notify>,
    maintenance_task: parking_lot::Mutex<Option<tokio::task::JoinHandle<()>>>,
    close_worker: parking_lot::Mutex<Option<ConnectionCloseWorker>>,
    close_worker_task: parking_lot::Mutex<Option<tokio::task::JoinHandle<()>>>,
    stats_collector: Arc<StatsCollector>,
    wall_provider: Arc<WallProvider>,
}

impl DruidPool {
    pub fn new(
        name: String,
        driver_name: String,
        factory: Arc<dyn PhysicalConnectionFactory>,
        config: super::config::PoolInnerConfig,
        filter_chain: Option<Arc<FilterChain>>,
    ) -> Self {
        Self::new_with_observability(
            name,
            driver_name,
            factory,
            config,
            filter_chain,
            Arc::new(StatsCollector::default()),
            Arc::new(WallProvider::default()),
        )
    }

    /// 创建具有共享 Stat/Wall 管理对象的池。
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_observability(
        name: String,
        driver_name: String,
        factory: Arc<dyn PhysicalConnectionFactory>,
        mut config: super::config::PoolInnerConfig,
        filter_chain: Option<Arc<FilterChain>>,
        stats_collector: Arc<StatsCollector>,
        wall_provider: Arc<WallProvider>,
    ) -> Self {
        let exception_sorter =
            Self::select_exception_sorter(config.db_type_name.as_deref(), &driver_name);
        if config.valid_connection_checker.is_none() {
            config.valid_connection_checker =
                Self::select_valid_connection_checker(config.db_type_name.as_deref(), &driver_name);
        }
        let close_factory = Arc::clone(&factory);
        let inner = Arc::new(PoolInner::new(factory, config));
        let (close_sender, close_receiver) = tokio::sync::mpsc::unbounded_channel();
        inner.install_close_sender(close_sender);
        Self {
            name,
            driver_name,
            inner,
            filter_chain,
            exception_sorter,
            filters_initialized: AtomicBool::new(false),
            initialized: OnceCell::new(),
            active_leases: Arc::new(DashMap::new()),
            remove_abandoned_count: Arc::new(AtomicU64::new(0)),
            maintenance_shutdown: Arc::new(Notify::new()),
            maintenance_task: parking_lot::Mutex::new(None),
            close_worker: parking_lot::Mutex::new(Some(ConnectionCloseWorker::new(
                close_factory,
                close_receiver,
            ))),
            close_worker_task: parking_lot::Mutex::new(None),
            stats_collector,
            wall_provider,
        }
    }

    fn select_valid_connection_checker(
        db_type_name: Option<&str>,
        driver_name: &str,
    ) -> Option<Arc<dyn ValidConnectionChecker>> {
        let identity = db_type_name.unwrap_or(driver_name).to_ascii_lowercase();
        if identity.contains("oceanbase") {
            if identity.contains("mysql") {
                Some(Arc::new(OceanBaseValidConnectionChecker::mysql_mode()))
            } else {
                Some(Arc::new(OceanBaseValidConnectionChecker::new()))
            }
        } else if identity.contains("oracle") {
            Some(Arc::new(OracleValidConnectionChecker::new()))
        } else if identity.contains("mysql") || identity.contains("mariadb") {
            Some(Arc::new(MySqlValidConnectionChecker::new()))
        } else if identity.contains("postgres") || identity == "pg" {
            Some(Arc::new(PgValidConnectionChecker))
        } else if identity.contains("sqlserver") || identity.contains("mssql") {
            Some(Arc::new(MsSqlValidConnectionChecker))
        } else {
            None
        }
    }

    fn select_exception_sorter(
        db_type_name: Option<&str>,
        driver_name: &str,
    ) -> Option<Arc<dyn ExceptionSorter>> {
        let identity = db_type_name.unwrap_or(driver_name).to_ascii_lowercase();
        if identity.contains("oceanbase") && identity.contains("oracle") {
            Some(Arc::new(OceanBaseOracleExceptionSorter::new()))
        } else if identity.contains("oracle") {
            Some(Arc::new(OracleExceptionSorter::new()))
        } else if identity.contains("mysql") || identity.contains("mariadb") {
            Some(Arc::new(MySqlExceptionSorter))
        } else if identity.contains("postgres") || identity == "pg" {
            Some(Arc::new(PgExceptionSorter))
        } else if identity.contains("phoenix") {
            Some(Arc::new(PhoenixExceptionSorter))
        } else if identity.contains("informix") {
            Some(Arc::new(InformixExceptionSorter))
        } else if identity.contains("sybase") {
            Some(Arc::new(SybaseExceptionSorter))
        } else if identity.contains("db2") {
            Some(Arc::new(Db2ExceptionSorter))
        } else if identity.contains("mock") {
            Some(Arc::new(MockExceptionSorter))
        } else {
            None
        }
    }

    pub fn builder() -> DruidPoolBuilder {
        DruidPoolBuilder::new()
    }

    pub async fn get(&self) -> Result<DruidPooledConnection, DruidError> {
        self.get_with_timeout(self.inner.config.acquire_timeout)
            .await
    }

    /// 幂等初始化数据源并按 `initialSize` 预建连接。
    ///
    /// 对应 Java: `DruidDataSource#init()`。
    pub async fn init(&self) -> Result<(), DruidError> {
        self.start_close_worker();
        self.initialized
            .get_or_try_init(|| self.inner.fill_initial())
            .await
            .map(|_| ())?;
        self.start_maintenance();
        Ok(())
    }

    fn start_maintenance(&self) {
        let period = self.inner.config.time_between_eviction_runs;
        if period.is_zero() {
            return;
        }
        let mut task = self.maintenance_task.lock();
        if task.is_some() {
            return;
        }
        let inner = Arc::clone(&self.inner);
        let active_leases = Arc::clone(&self.active_leases);
        let remove_abandoned_count = Arc::clone(&self.remove_abandoned_count);
        let shutdown = Arc::clone(&self.maintenance_shutdown);
        *task = Some(tokio::spawn(async move {
            loop {
                tokio::select! {
                    () = shutdown.notified() => break,
                    () = tokio::time::sleep(period) => {}
                }
                if inner.closed.load(Ordering::Acquire) {
                    break;
                }
                inner.shrink(true, inner.config.keep_alive).await;
                if inner.config.keep_alive {
                    let _ = inner.fill(inner.config.min_idle).await;
                }
                remove_abandoned_leases(&inner, &active_leases, remove_abandoned_count.as_ref());
            }
        }));
    }

    /// 启动每池唯一的受监管物理关闭 worker。
    fn start_close_worker(&self) {
        let mut task = self.close_worker_task.lock();
        if task.is_some() {
            return;
        }
        if let Some(worker) = self.close_worker.lock().take() {
            *task = Some(worker.spawn());
        }
    }

    pub async fn get_timeout(
        &self,
        timeout: Duration,
    ) -> Result<DruidPooledConnection, DruidError> {
        self.get_with_timeout(timeout).await
    }

    async fn get_with_timeout(
        &self,
        timeout: Duration,
    ) -> Result<DruidPooledConnection, DruidError> {
        if self.inner.closed.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(DruidError::PoolClosed);
        }
        self.init().await?;
        // Java maxWait=-1 表示无限等待。Rust factory 将负值保存为
        // Duration::MAX；这里必须避免 `Instant + Duration::MAX` 溢出。
        let deadline = (timeout != Duration::MAX)
            .then(|| Instant::now().checked_add(timeout))
            .flatten();
        self.inner
            .connect_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        loop {
            let (idle_connection, remaining_idle) = {
                let mut idle = self.inner.idle.lock();
                let connection = idle.pop_front();
                (connection, idle.len())
            };
            if let Some(holder) = idle_connection {
                let mut candidate = BorrowCandidate::new(Arc::clone(&self.inner), holder);
                let holder = candidate.holder_mut();
                let lifetime_expired = holder.physical_age() >= self.inner.config.max_lifetime;
                let idle_expired = remaining_idle >= self.inner.config.min_idle
                    && holder.idle_duration() >= self.inner.config.idle_timeout;
                if lifetime_expired || idle_expired {
                    continue;
                }
                let physically_unusable = holder
                    .physical_connection()
                    .is_none_or(|connection| connection.is_closed() || connection.is_discarded());
                if physically_unusable {
                    continue;
                }
                let validate_on_borrow = self.inner.config.test_on_borrow
                    || (self.inner.config.test_while_idle
                        && holder.idle_duration() >= self.inner.config.time_between_eviction_runs);
                if validate_on_borrow {
                    let validation_failed = match holder.physical_connection_box_mut() {
                        Some(connection) => {
                            self.inner.validate_connection(connection).await.is_err()
                        }
                        None => true,
                    };
                    if validation_failed {
                        continue;
                    }
                    holder.record_valid();
                }
                if !holder.mark_active() {
                    continue;
                }
                let holder = candidate.take();
                self.inner
                    .active_count
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return Ok(self.wrap_connection(holder));
            }
            match self.create_connection_until(deadline).await {
                Ok(holder) => {
                    if !holder.mark_active() {
                        self.inner.destroy_holder(holder);
                        continue;
                    }
                    self.inner
                        .active_count
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    return Ok(self.wrap_connection(holder));
                }
                Err(DruidError::PoolExhausted) if !self.inner.idle.lock().is_empty() => continue,
                Err(DruidError::PoolExhausted) => {}
                Err(_) if !self.inner.idle.lock().is_empty() => continue,
                Err(e) => return Err(e),
            }
            let _waiter = WaitTaskRegistration::register(Arc::clone(&self.inner))?;
            let notify = self.inner.notify.notified();
            tokio::pin!(notify);
            if let Some(deadline) = deadline {
                match tokio::time::timeout_at(deadline.into(), notify).await {
                    Ok(_) => continue,
                    Err(_) => return Err(DruidError::AcquireTimeout),
                }
            } else {
                notify.await;
            }
        }
    }

    /// 按 Java creator 的重试阈值创建物理连接，并受本次 maxWait 截止时间约束。
    async fn create_connection_until(
        &self,
        deadline: Option<Instant>,
    ) -> Result<DruidConnectionHolder, DruidError> {
        let mut error_count = 0usize;
        loop {
            // Java 的 creator 与 waiter 分离；创建重试期间若已有连接归还，
            // waiter 会优先消费它。Rust 在同一 future 内创建，因此每轮必须
            // 主动让回外层 idle 路径，避免可用连接被无休止的 driver 重试遮蔽。
            if !self.inner.idle.lock().is_empty() {
                return Err(DruidError::PoolExhausted);
            }
            let attempt = self.inner.create_connection();
            let result = if let Some(deadline) = deadline {
                match tokio::time::timeout_at(deadline.into(), attempt).await {
                    Ok(result) => result,
                    Err(_) => return Err(DruidError::AcquireTimeout),
                }
            } else {
                attempt.await
            };

            match result {
                Ok(holder) => return Ok(holder),
                Err(DruidError::PoolExhausted) => return Err(DruidError::PoolExhausted),
                Err(DruidError::PoolClosed) => return Err(DruidError::PoolClosed),
                Err(error) => {
                    error_count = error_count.saturating_add(1);
                    if error_count <= self.inner.config.connection_error_retry_attempts {
                        continue;
                    }
                    if self.inner.config.break_after_acquire_failure || self.inner.config.fail_fast
                    {
                        return Err(error);
                    }
                    error_count = 0;

                    let delay = self.inner.config.time_between_connect_error;
                    if delay.is_zero() {
                        tokio::task::yield_now().await;
                        continue;
                    }
                    if let Some(deadline) = deadline {
                        if tokio::time::timeout_at(deadline.into(), tokio::time::sleep(delay))
                            .await
                            .is_err()
                        {
                            return Err(DruidError::AcquireTimeout);
                        }
                    } else {
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }
    }

    pub fn state(&self) -> PoolState {
        PoolState {
            name: self.name.clone(),
            driver_name: self.driver_name.clone(),
            max_open: self.inner.config.max_open,
            active_count: self
                .inner
                .active_count
                .load(std::sync::atomic::Ordering::Relaxed),
            idle_count: self.inner.idle.lock().len(),
            wait_count: self.inner.wait_count.load(Ordering::Relaxed),
            max_wait_thread_count: self.inner.config.max_wait_thread_count,
            create_count: self
                .inner
                .create_count
                .load(std::sync::atomic::Ordering::Relaxed),
            close_count: self
                .inner
                .close_count
                .load(std::sync::atomic::Ordering::Relaxed),
            destroy_count: self
                .inner
                .destroy_count
                .load(std::sync::atomic::Ordering::Relaxed),
            connect_count: self
                .inner
                .connect_count
                .load(std::sync::atomic::Ordering::Relaxed),
            connect_error_count: self
                .inner
                .connect_error_count
                .load(std::sync::atomic::Ordering::Relaxed),
            recycle_count: self
                .inner
                .recycle_count
                .load(std::sync::atomic::Ordering::Relaxed),
            recycle_error_count: self
                .inner
                .recycle_error_count
                .load(std::sync::atomic::Ordering::Relaxed),
            discard_count: self
                .inner
                .discard_count
                .load(std::sync::atomic::Ordering::Relaxed),
            keep_alive_check_count: self
                .inner
                .keep_alive_check_count
                .load(std::sync::atomic::Ordering::Relaxed),
            keep_alive_check_error_count: self
                .inner
                .keep_alive_check_error_count
                .load(std::sync::atomic::Ordering::Relaxed),
            prepared_statement_count: self
                .inner
                .prepared_statement_stats
                .prepared_statement_count(),
            closed_prepared_statement_count: self
                .inner
                .prepared_statement_stats
                .closed_prepared_statement_count(),
            cached_prepared_statement_count: self
                .inner
                .prepared_statement_stats
                .cached_prepared_statement_count(),
            cached_prepared_statement_delete_count: self
                .inner
                .prepared_statement_stats
                .cached_prepared_statement_delete_count(),
            cached_prepared_statement_hit_count: self
                .inner
                .prepared_statement_stats
                .cached_prepared_statement_hit_count(),
            cached_prepared_statement_miss_count: self
                .inner
                .prepared_statement_stats
                .cached_prepared_statement_miss_count(),
            cached_prepared_statement_access_count: self
                .inner
                .prepared_statement_stats
                .cached_prepared_statement_access_count(),
            leak_detection_count: self.remove_abandoned_count.load(Ordering::Relaxed),
            closed: self.inner.closed.load(std::sync::atomic::Ordering::Relaxed),
            ..Default::default()
        }
    }

    /// 重置累计池统计，当前 active/idle/缓存占用保持可见。
    pub fn reset_stats(&self) {
        self.inner.reset_stats();
        self.remove_abandoned_count.store(0, Ordering::Release);
        self.stats_collector.reset();
        self.wall_provider.reset();
    }

    /// 返回与 `StatFilter` 共享的数据源统计对象。
    #[must_use]
    pub fn stats_collector(&self) -> &Arc<StatsCollector> {
        &self.stats_collector
    }

    /// 返回与 `WallFilter` 共享的 Wall provider。
    #[must_use]
    pub fn wall_provider(&self) -> &Arc<WallProvider> {
        &self.wall_provider
    }

    /// 返回空闲队列中 holder 的管理快照。
    #[must_use]
    pub fn pooling_connection_info(&self) -> Vec<serde_json::Value> {
        self.inner
            .idle
            .lock()
            .iter()
            .map(|holder| {
                serde_json::json!({
                    "connectionId": holder.connection_id(),
                    "useCount": holder.use_count(),
                    "connectTimeMillis": holder.physical_age().as_millis(),
                    "idleMillis": holder.idle_duration().as_millis(),
                    "lastExecIdleMillis": holder.last_exec_idle_duration().as_millis(),
                    "lastKeepElapsedMillis": holder.last_keep_elapsed().map(|value| value.as_millis()),
                    "lastValidElapsedMillis": holder.last_valid_elapsed().map(|value| value.as_millis()),
                    "keepAliveCheckCount": holder.keep_alive_check_count(),
                    "state": format!("{:?}", holder.state()),
                })
            })
            .collect()
    }

    /// 返回当前活跃连接在借出点捕获的调用栈。
    ///
    /// 对应 Java：`DruidAbstractDataSource#getActiveConnectionStackTrace()`。
    /// 只有启用 `removeAbandoned` 的连接才进入活跃租约表，已归还或已失效的
    /// weak lease 会被过滤。
    #[must_use]
    pub fn active_connection_stack_trace(&self) -> Vec<String> {
        self.active_leases
            .iter()
            .filter_map(|entry| {
                entry
                    .lease_active
                    .upgrade()
                    .filter(|active| active.load(Ordering::Acquire))
                    .map(|_| entry.connect_stack_trace.clone())
            })
            .collect()
    }

    /// 将超过 `min_idle` 的空闲连接收缩掉。
    ///
    /// 对应 Java：`DruidDataSource#shrink()`，即
    /// `shrink(false, false)`。
    pub async fn shrink(&self) {
        self.inner.shrink(false, false).await;
    }

    /// 按时间执行空闲连接收缩。
    ///
    /// 对应 Java：`DruidDataSource#shrink(boolean)`；保活参数取数据源配置。
    ///
    /// # 参数
    /// - `check_time`：是否应用空闲与物理寿命阈值。
    pub async fn shrink_check_time(&self, check_time: bool) {
        self.inner
            .shrink(check_time, self.inner.config.keep_alive)
            .await;
    }

    /// 按显式时间与保活选项执行空闲连接收缩。
    ///
    /// 对应 Java：`DruidDataSource#shrink(boolean, boolean)`。
    ///
    /// # 参数
    /// - `check_time`：是否应用空闲与物理寿命阈值。
    /// - `keep_alive`：是否验证到期的空闲连接。
    pub async fn shrink_with_options(&self, check_time: bool, keep_alive: bool) {
        self.inner.shrink(check_time, keep_alive).await;
    }

    /// 使超过阈值且当前未执行 SQL 的借出连接租约失效。
    ///
    /// 对应 Java：`DruidDataSource#removeAbandoned()`。Java 可由扫描线程直接
    /// `close()` 活跃 JDBC 连接；Rust 不能安全地跨线程取得其独占可变引用，
    /// 因而先原子失效租约，物理连接在所有者下一次操作或 Drop 时丢弃。
    /// 返回本轮新失效的连接数。
    pub fn remove_abandoned(&self) -> usize {
        remove_abandoned_leases(
            &self.inner,
            &self.active_leases,
            self.remove_abandoned_count.as_ref(),
        )
    }

    /// 将池内物理连接总数填充到 `minIdle`，返回新建数量。
    ///
    /// 对应 Java：`DruidDataSource#fill()`。
    pub async fn fill(&self) -> Result<usize, DruidError> {
        self.init().await?;
        self.inner.fill(self.inner.config.min_idle).await
    }

    /// 将池内物理连接总数填充到指定数量，返回新建数量。
    ///
    /// 对应 Java：`DruidDataSource#fill(int)`。
    pub async fn fill_to(&self, to_count: usize) -> Result<usize, DruidError> {
        self.init().await?;
        self.inner.fill(to_count).await
    }

    pub async fn close(&self) {
        self.start_close_worker();
        self.maintenance_shutdown.notify_one();
        // 先把池标记为 closed 并排空 idle，再等待维护任务退出。这样 close
        // future 在等待后台任务期间被取消时，新借用也已经被拒绝，且
        // PoolInner::close 的 DetachedHolder 会完成剩余资源计数清理。
        self.inner.close().await;
        let maintenance_task = self.maintenance_task.lock().take();
        if let Some(maintenance_task) = maintenance_task {
            let _ = maintenance_task.await;
        }
        self.inner.request_close_worker_shutdown_if_idle();
        if self.inner.active_count.load(Ordering::Acquire) == 0 {
            let close_worker_task = self.close_worker_task.lock().take();
            if let Some(close_worker_task) = close_worker_task {
                let _ = close_worker_task.await;
            }
        }
        if self
            .filters_initialized
            .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            if let Some(filter_chain) = &self.filter_chain {
                filter_chain.destroy_filters().await;
            }
        }
    }
    pub fn filter_chain(&self) -> Option<&Arc<FilterChain>> {
        self.filter_chain.as_ref()
    }
    pub fn driver_name(&self) -> &str {
        &self.driver_name
    }
    pub fn name(&self) -> &str {
        &self.name
    }

    pub(crate) fn mark_filters_initialized(&self) {
        self.filters_initialized.store(true, Ordering::Release);
    }

    fn wrap_connection(&self, holder: DruidConnectionHolder) -> DruidPooledConnection {
        let connection_id = holder.connection_id();
        let pool = self.inner.clone();
        let active_leases = Arc::clone(&self.active_leases);
        let recycle_validator = self.inner.config.test_on_return.then(|| {
            Arc::new(PoolValidationFactory::new(self.inner.clone()))
                as Arc<dyn PhysicalConnectionFactory>
        });
        let mut connection = DruidPooledConnection::with_holder(
            holder,
            self.name.clone(),
            self.filter_chain.clone(),
            self.inner
                .config
                .keep_connection_underlying_transaction_isolation,
            recycle_validator,
            Box::new(move |holder, disposition| {
                active_leases.remove(&connection_id);
                pool.return_connection(holder, disposition);
            }),
        );
        if let Some(exception_sorter) = self.exception_sorter.clone() {
            connection.set_exception_sorter(exception_sorter);
        }
        if self.inner.config.remove_abandoned {
            self.active_leases.insert(
                connection_id,
                ActiveConnectionLease::new(
                    Arc::downgrade(&connection.lease_active_token()),
                    Arc::downgrade(&connection.execution_running_token()),
                ),
            );
        }
        connection
    }
}

/// idle holder 借出/校验期间的取消守卫。
///
/// 在异步 validation 完成并转为 active 前，future 被取消会立即丢弃候选 holder，
/// 归还容量并异步关闭物理连接，避免它既不在 idle 队列也不在 active 租约中。
struct BorrowCandidate {
    inner: Arc<PoolInner>,
    holder: Option<DruidConnectionHolder>,
}

/// 等待连接任务计数守卫。
///
/// 对应 Java `notEmptyWaitThreadCount`。Rust future 被取消时 Drop 必须减计数，
/// 否则 `maxWaitThreadCount` 会永久拒绝后续请求。
struct WaitTaskRegistration {
    inner: Arc<PoolInner>,
}

impl WaitTaskRegistration {
    fn register(inner: Arc<PoolInner>) -> Result<Self, DruidError> {
        if let Some(max) = inner.config.max_wait_thread_count.filter(|max| *max > 0) {
            let result =
                inner
                    .wait_count
                    .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                        (current < max).then_some(current + 1)
                    });
            if let Err(current) = result {
                return Err(DruidError::MaxWaitThreadCountExceeded { max, current });
            }
        } else {
            inner.wait_count.fetch_add(1, Ordering::AcqRel);
        }
        Ok(Self { inner })
    }
}

impl Drop for WaitTaskRegistration {
    fn drop(&mut self) {
        let _ =
            self.inner
                .wait_count
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                    (current > 0).then_some(current - 1)
                });
    }
}

impl BorrowCandidate {
    fn new(inner: Arc<PoolInner>, holder: DruidConnectionHolder) -> Self {
        Self {
            inner,
            holder: Some(holder),
        }
    }

    fn holder_mut(&mut self) -> &mut DruidConnectionHolder {
        self.holder
            .as_mut()
            .expect("borrow candidate holder is present")
    }

    fn take(&mut self) -> DruidConnectionHolder {
        self.holder
            .take()
            .expect("borrow candidate holder is present")
    }
}

impl Drop for BorrowCandidate {
    fn drop(&mut self) {
        if let Some(holder) = self.holder.take() {
            self.inner.destroy_holder(holder);
        }
    }
}

fn remove_abandoned_leases(
    inner: &PoolInner,
    active_leases: &DashMap<u64, ActiveConnectionLease>,
    remove_abandoned_count: &AtomicU64,
) -> usize {
    if !inner.config.remove_abandoned {
        return 0;
    }

    let mut stale_ids = Vec::new();
    let mut abandoned_ids = Vec::new();
    for entry in active_leases.iter() {
        let connection_id = *entry.key();
        let lease = entry.value();
        let Some(lease_active) = lease.lease_active.upgrade() else {
            stale_ids.push(connection_id);
            continue;
        };
        if !lease_active.load(Ordering::Acquire) {
            stale_ids.push(connection_id);
            continue;
        }
        if lease
            .execution_running
            .upgrade()
            .is_some_and(|running| running.load(Ordering::Acquire))
        {
            continue;
        }
        if lease.borrowed_at.elapsed() >= inner.config.remove_abandoned_timeout
            && lease_active
                .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
        {
            abandoned_ids.push(connection_id);
        }
    }

    for connection_id in stale_ids {
        active_leases.remove(&connection_id);
    }
    for connection_id in &abandoned_ids {
        active_leases.remove(connection_id);
        if inner.config.log_abandoned {
            tracing::warn!(
                connection_id,
                timeout_ms = inner.config.remove_abandoned_timeout.as_millis(),
                "remove abandoned pooled connection lease"
            );
        }
    }
    remove_abandoned_count.fetch_add(abandoned_ids.len() as u64, Ordering::Relaxed);
    abandoned_ids.len()
}

#[async_trait::async_trait]
impl crate::core::Pool for DruidPool {
    async fn get(&self) -> Result<DruidPooledConnection, DruidError> {
        DruidPool::get(self).await
    }

    async fn get_timeout(&self, timeout: Duration) -> Result<DruidPooledConnection, DruidError> {
        DruidPool::get_timeout(self, timeout).await
    }

    fn state(&self) -> PoolState {
        DruidPool::state(self)
    }

    fn driver_name(&self) -> &str {
        DruidPool::driver_name(self)
    }

    fn name(&self) -> &str {
        DruidPool::name(self)
    }
}
