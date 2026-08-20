//! 对应 Java 类：com.alibaba.druid.pool.DruidDataSource
//!
//! Druid 风格连接池。

use super::active_connection_lease::ActiveConnectionLease;
use super::config::DruidPoolBuilder;
use super::connection_close_worker::ConnectionCloseWorker;
use super::connection_create_worker::ConnectionCreateWorker;
use super::pool_inner::PoolInner;
use super::pool_validation_factory::PoolValidationFactory;
use crate::core::fatal_error_handler::FatalErrorHandler;
use crate::core::{
    DataSourceConnectionProvider, Db2ExceptionSorter, DruidConnectionHolder, DruidError,
    DruidPooledConnection, ExceptionSorter, FilterChain, InformixExceptionSorter,
    MockExceptionSorter, MsSqlValidConnectionChecker, MySqlExceptionSorter,
    MySqlValidConnectionChecker, OceanBaseOracleExceptionSorter, OceanBaseValidConnectionChecker,
    OracleExceptionSorter, OracleValidConnectionChecker, PgExceptionSorter,
    PgValidConnectionChecker, PhoenixExceptionSorter, PhysicalConnectionFactory, PoolState,
    SybaseExceptionSorter, ValidConnectionChecker,
};
use crate::sql::WallProvider;
use crate::stats::{DruidDataSourceStatValue, StatsCollector};
use dashmap::DashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex as AsyncMutex, Notify};

/// Druid 风格连接池。
///
/// 对应 Druid Java 的 `DruidDataSource`，实现 `max_open` / `min_idle` /
/// `acquire_timeout` / `FilterChain` 装配 / `DruidPooledConnection::drop` 归还。
pub struct DruidPool {
    name: String,
    driver_name: String,
    inner: Arc<PoolInner>,
    filter_chain: Option<Arc<FilterChain>>,
    exception_sorter: Option<Arc<dyn ExceptionSorter>>,
    filters_initialized: AtomicBool,
    initialized: AtomicBool,
    reset_stat_enable: AtomicBool,
    reset_count: AtomicU64,
    lifecycle_lock: AsyncMutex<()>,
    active_leases: Arc<DashMap<u64, ActiveConnectionLease>>,
    remove_abandoned_count: Arc<AtomicU64>,
    maintenance_shutdown: Arc<Notify>,
    maintenance_task: parking_lot::Mutex<Option<tokio::task::JoinHandle<()>>>,
    stat_publish_shutdown: Arc<Notify>,
    stat_publish_task: parking_lot::Mutex<Option<tokio::task::JoinHandle<()>>>,
    create_worker: parking_lot::Mutex<Option<ConnectionCreateWorker>>,
    create_worker_task: parking_lot::Mutex<Option<tokio::task::JoinHandle<()>>>,
    close_worker: parking_lot::Mutex<Option<ConnectionCloseWorker>>,
    close_worker_task: parking_lot::Mutex<Option<tokio::task::JoinHandle<()>>>,
    stats_collector: Arc<StatsCollector>,
    wall_provider: Arc<WallProvider>,
    statement_id_seed: Arc<AtomicU64>,
    result_set_id_seed: Arc<AtomicU64>,
    metadata_id_seed: Arc<AtomicU64>,
    transaction_id_seed: Arc<AtomicU64>,
    stat_snapshot_context: Arc<DataSourceStatSnapshotContext>,
}

/// 可安全移入 Tokio 任务的数据源区间快照上下文。
///
/// 只持有生产统计所需的共享对象，不持有 `DruidPool` 自引用，也不复制连接池。
struct DataSourceStatSnapshotContext {
    name: String,
    driver_name: String,
    inner: Arc<PoolInner>,
    filter_chain: Option<Arc<FilterChain>>,
    exception_sorter: Option<Arc<dyn ExceptionSorter>>,
    stats_collector: Arc<StatsCollector>,
}

impl DataSourceStatSnapshotContext {
    fn snapshot_and_reset(&self) -> DruidDataSourceStatValue {
        let pool = self.inner.stat_snapshot_and_reset();
        let runtime = self.stats_collector.runtime_snapshot_and_reset();
        let sql_list = self
            .stats_collector
            .sql_merger
            .all_stats()
            .into_iter()
            .map(|stat| stat.stat_value())
            .collect();
        self.stats_collector.sql_merger.reset();
        let transaction = runtime.transaction_histogram;

        DruidDataSourceStatValue {
            name: self.name.clone(),
            db_type: self.inner.config.db_type_name.clone(),
            driver_class_name: self.driver_name.clone(),
            url: self.inner.factory.connection_url().map(str::to_owned),
            user_name: self.inner.factory.user_name().map(str::to_owned),
            filter_class_names: self
                .filter_chain
                .as_ref()
                .map_or_else(Vec::new, |chain| chain.filter_class_names().to_vec()),
            remove_abandoned: self.inner.config.remove_abandoned,
            initial_size: self.inner.config.initial_size,
            min_idle: self.inner.config.min_idle,
            max_active: self.inner.config.max_open,
            query_timeout: self.inner.config.query_timeout,
            transaction_query_timeout: self.inner.config.transaction_query_timeout,
            login_timeout: self.inner.config.login_timeout,
            valid_connection_checker_class_name: self
                .inner
                .config
                .valid_connection_checker
                .as_ref()
                .map(|checker| checker.class_name().to_owned()),
            exception_sorter_class_name: self
                .exception_sorter
                .as_ref()
                .map(|sorter| sorter.class_name().to_owned()),
            test_on_borrow: self.inner.config.test_on_borrow,
            test_on_return: self.inner.config.test_on_return,
            test_while_idle: self.inner.config.test_while_idle,
            default_auto_commit: self.inner.config.default_auto_commit,
            default_read_only: self.inner.config.default_read_only.unwrap_or(false),
            default_transaction_isolation: self.inner.config.default_transaction_isolation,
            active_count: pool.active_count,
            active_peak: pool.active_peak,
            active_peak_time: (pool.active_peak_time_millis > 0)
                .then_some(pool.active_peak_time_millis),
            pooling_count: pool.pooling_count,
            pooling_peak: pool.pooling_peak,
            pooling_peak_time: (pool.pooling_peak_time_millis > 0)
                .then_some(pool.pooling_peak_time_millis),
            connect_count: pool.connect_count,
            close_count: pool.close_count,
            wait_thread_count: pool.wait_thread_count,
            not_empty_wait_count: pool.not_empty_wait_count,
            not_empty_wait_nanos: pool.not_empty_wait_nanos,
            logic_connect_error_count: pool.logic_connect_error_count,
            physical_connect_count: pool.physical_connect_count,
            physical_close_count: pool.physical_close_count,
            physical_connect_error_count: pool.physical_connect_error_count,
            execute_count: runtime.execute_count,
            error_count: runtime.error_count,
            commit_count: runtime.commit_count,
            rollback_count: runtime.rollback_count,
            pstmt_cache_hit_count: pool.pstmt_cache_hit_count,
            pstmt_cache_miss_count: pool.pstmt_cache_miss_count,
            start_transaction_count: runtime.start_transaction_count,
            keep_alive_check_count: pool.keep_alive_check_count,
            connection_hold_time_histogram: runtime.connection_hold_time_histogram,
            txn_0_1: transaction[0],
            txn_1_10: transaction[1],
            txn_10_100: transaction[2],
            txn_100_1000: transaction[3],
            txn_1000_10000: transaction[4],
            txn_10000_100000: transaction[5],
            txn_more: transaction[6],
            clob_open_count: runtime.clob_open_count,
            blob_open_count: runtime.blob_open_count,
            sql_skip_count: runtime.sql_skip_count,
            sql_list,
        }
    }
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
        config: super::config::PoolInnerConfig,
        filter_chain: Option<Arc<FilterChain>>,
        stats_collector: Arc<StatsCollector>,
        wall_provider: Arc<WallProvider>,
    ) -> Self {
        Self::new_with_observability_and_exception_sorter(
            name,
            driver_name,
            factory,
            config,
            filter_chain,
            stats_collector,
            wall_provider,
            None,
        )
    }

    /// 创建具有共享 Stat/Wall 对象及显式异常分类器的池。
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_observability_and_exception_sorter(
        name: String,
        driver_name: String,
        factory: Arc<dyn PhysicalConnectionFactory>,
        mut config: super::config::PoolInnerConfig,
        filter_chain: Option<Arc<FilterChain>>,
        stats_collector: Arc<StatsCollector>,
        wall_provider: Arc<WallProvider>,
        exception_sorter: Option<Arc<dyn ExceptionSorter>>,
    ) -> Self {
        let exception_sorter = exception_sorter.or_else(|| {
            Self::select_exception_sorter(config.db_type_name.as_deref(), &driver_name)
        });
        if config.valid_connection_checker.is_none() {
            config.valid_connection_checker =
                Self::select_valid_connection_checker(config.db_type_name.as_deref(), &driver_name);
        }
        let close_factory = Arc::clone(&factory);
        let inner = Arc::new(PoolInner::new_with_stats(
            factory,
            config,
            Arc::clone(&stats_collector),
        ));
        inner.install_filter_chain(filter_chain.clone());
        let (close_sender, close_receiver) = tokio::sync::mpsc::unbounded_channel();
        inner.install_close_sender(close_sender);
        let (create_sender, create_receiver) = tokio::sync::mpsc::unbounded_channel();
        inner.install_create_sender(create_sender);
        let stat_snapshot_context = Arc::new(DataSourceStatSnapshotContext {
            name: name.clone(),
            driver_name: driver_name.clone(),
            inner: Arc::clone(&inner),
            filter_chain: filter_chain.clone(),
            exception_sorter: exception_sorter.clone(),
            stats_collector: Arc::clone(&stats_collector),
        });
        Self {
            name,
            driver_name,
            inner: Arc::clone(&inner),
            filter_chain: filter_chain.clone(),
            exception_sorter,
            filters_initialized: AtomicBool::new(false),
            initialized: AtomicBool::new(false),
            reset_stat_enable: AtomicBool::new(true),
            reset_count: AtomicU64::new(0),
            lifecycle_lock: AsyncMutex::new(()),
            active_leases: Arc::new(DashMap::new()),
            remove_abandoned_count: Arc::new(AtomicU64::new(0)),
            maintenance_shutdown: Arc::new(Notify::new()),
            maintenance_task: parking_lot::Mutex::new(None),
            stat_publish_shutdown: Arc::new(Notify::new()),
            stat_publish_task: parking_lot::Mutex::new(None),
            create_worker: parking_lot::Mutex::new(Some(ConnectionCreateWorker::new(
                Arc::clone(&inner),
                create_receiver,
            ))),
            create_worker_task: parking_lot::Mutex::new(None),
            close_worker: parking_lot::Mutex::new(Some(ConnectionCloseWorker::new(
                close_factory,
                filter_chain.clone(),
                close_receiver,
            ))),
            close_worker_task: parking_lot::Mutex::new(None),
            stats_collector,
            wall_provider,
            statement_id_seed: Arc::new(AtomicU64::new(20_000)),
            result_set_id_seed: Arc::new(AtomicU64::new(50_000)),
            metadata_id_seed: Arc::new(AtomicU64::new(80_000)),
            transaction_id_seed: Arc::new(AtomicU64::new(60_000)),
            stat_snapshot_context,
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
        self.get_connection().await
    }

    /// 获取池化连接。
    ///
    /// 对应 Java：`DruidDataSource#getConnection()`。保留 `get()` 作为 Rust
    /// `Pool` 习惯入口，但 canonical Java 迁移名称不能缺失。
    pub async fn get_connection(&self) -> Result<DruidPooledConnection, DruidError> {
        self.get_connection_with_max_wait(self.inner.config.acquire_timeout)
            .await
    }

    /// 使用本次 maxWait 获取池化连接。
    ///
    /// 对应 Java：`DruidDataSource#getConnection(long)`。
    pub async fn get_connection_with_max_wait(
        &self,
        max_wait: Duration,
    ) -> Result<DruidPooledConnection, DruidError> {
        match &self.filter_chain {
            Some(filter_chain) if !filter_chain.is_empty() => {
                filter_chain
                    .data_source_get_connection(self, max_wait)
                    .await
            }
            _ => self.get_connection_direct(max_wait).await,
        }
    }

    /// 绕过数据源获取 Filter，直接进入 native pool 状态机。
    ///
    /// 对应 Java：`DruidDataSource#getConnectionDirect(long)`。物理驱动建连
    /// Filter 仍在 `PoolInner` 内执行；绕过的只是 `dataSource_getConnection`
    /// 这一外层 hook。
    pub async fn get_connection_direct(
        &self,
        max_wait: Duration,
    ) -> Result<DruidPooledConnection, DruidError> {
        self.get_with_timeout(max_wait).await
    }

    /// 幂等初始化数据源并按 `initialSize` 预建连接。
    ///
    /// 对应 Java: `DruidDataSource#init()`。
    pub async fn init(&self) -> Result<(), DruidError> {
        if self.initialized.load(Ordering::Acquire) {
            return Ok(());
        }
        let _lifecycle = self.lifecycle_lock.lock().await;
        if self.initialized.load(Ordering::Acquire) {
            return Ok(());
        }
        self.inner.ensure_not_closed()?;
        if self
            .filters_initialized
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            if let Some(filter_chain) = &self.filter_chain {
                if let Err(error) = filter_chain.init_filters().await {
                    // Java init 的 finally 对 Filter 初始化错误同样设置 inited=true；
                    // close 仍会按注册顺序 destroy 已进入生命周期的 Filter。
                    self.initialized.store(true, Ordering::Release);
                    return Err(error);
                }
            }
        }

        self.install_create_worker();
        self.install_close_worker();
        let initial_result = self.inner.fill_initial().await;
        // Java 在同步初始建连失败且 initExceptionThrow=true 时，仍先启动
        // creator/destroy 线程并在 finally 中设置 inited=true，再把错误返回。
        self.initialized.store(true, Ordering::Release);
        self.start_maintenance();
        self.start_stat_publisher();
        initial_result
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

    /// 启动每池唯一的周期统计发布任务。
    fn start_stat_publisher(&self) {
        let interval = self.inner.config.stat_publish_interval;
        if interval.is_zero() {
            return;
        }
        let mut task = self.stat_publish_task.lock();
        if task.is_some() {
            return;
        }
        let context = Arc::clone(&self.stat_snapshot_context);
        let sink = Arc::clone(&self.inner.config.stat_sink);
        let shutdown = Arc::clone(&self.stat_publish_shutdown);
        *task = Some(tokio::spawn(async move {
            loop {
                // Java LogStatsThread 每轮先发布，再 sleep；单轮 sink 错误不能终止任务。
                let stat_value = context.snapshot_and_reset();
                if let Err(error) = sink.publish(&stat_value) {
                    tracing::warn!(%error, "publish datasource statistics failed");
                }
                tokio::select! {
                    () = shutdown.notified() => break,
                    () = tokio::time::sleep(interval) => {}
                }
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

    /// 启动每池唯一的受监管补池 worker。
    fn start_create_worker(&self) {
        let mut task = self.create_worker_task.lock();
        if task.is_some() {
            return;
        }
        if let Some(worker) = self.create_worker.lock().take() {
            *task = Some(worker.spawn());
        }
    }

    /// 为首次 init 或 restart 后的新代次安装补池 worker。
    fn install_create_worker(&self) {
        if self.create_worker.lock().is_none() && self.create_worker_task.lock().is_none() {
            let (create_sender, create_receiver) = tokio::sync::mpsc::unbounded_channel();
            self.inner.install_create_sender(create_sender);
            *self.create_worker.lock() = Some(ConnectionCreateWorker::new(
                Arc::clone(&self.inner),
                create_receiver,
            ));
        }
        self.start_create_worker();
    }

    /// 为首次 init 或 restart 后的新代次安装唯一物理关闭 worker。
    fn install_close_worker(&self) {
        if self.close_worker.lock().is_none() && self.close_worker_task.lock().is_none() {
            let (close_sender, close_receiver) = tokio::sync::mpsc::unbounded_channel();
            self.inner.install_close_sender(close_sender);
            *self.close_worker.lock() = Some(ConnectionCloseWorker::new(
                Arc::clone(&self.inner.factory),
                self.filter_chain.clone(),
                close_receiver,
            ));
        }
        self.start_close_worker();
    }

    pub async fn get_timeout(
        &self,
        timeout: Duration,
    ) -> Result<DruidPooledConnection, DruidError> {
        self.get_connection_with_max_wait(timeout).await
    }

    async fn get_with_timeout(
        &self,
        timeout: Duration,
    ) -> Result<DruidPooledConnection, DruidError> {
        let mut not_full_timeout_retry_count = 0i32;
        loop {
            match self.get_connection_internal(timeout).await {
                Err(error @ DruidError::GetConnectionTimeout { .. })
                    if not_full_timeout_retry_count
                        < self.inner.config.not_full_timeout_retry_count
                        && !self.is_full() =>
                {
                    not_full_timeout_retry_count = not_full_timeout_retry_count.wrapping_add(1);
                    tracing::warn!(
                        retry = not_full_timeout_retry_count,
                        %error,
                        "get connection timeout retry"
                    );
                }
                result => return result,
            }
        }
    }

    async fn get_connection_internal(
        &self,
        timeout: Duration,
    ) -> Result<DruidPooledConnection, DruidError> {
        if !self.initialized.load(Ordering::Acquire) {
            self.init().await?;
        }
        if let Err(error) = self.inner.ensure_available() {
            // Java closed/disabled 的入口分支各递增一次；init 自身失败不计入
            // 逻辑获取失败，因此检查必须位于 init 之后。
            self.inner.record_connect_error(1);
            return Err(error);
        }
        let lifecycle_generation = self.inner.lifecycle_generation();
        let started_at = Instant::now();
        // Java 只有 maxWait > 0 才使用截止时间；0 与负数都无限等待。Rust
        // factory 把负数保存为 Duration::MAX，因此两种哨兵都不能参与加法。
        let deadline = (!timeout.is_zero() && timeout != Duration::MAX)
            .then(|| Instant::now().checked_add(timeout))
            .flatten();
        loop {
            if let Err(error) = self.ensure_borrow_generation(lifecycle_generation) {
                // 对应 Java 在 pollLast 被 close/disable 唤醒后的显式递增与
                // 外层 SQLException catch；生命周期代次变化沿用同一失败语义。
                self.inner.record_connect_error(2);
                return Err(error);
            }
            if let Err(error) = self.inner.ensure_fatal_error_available() {
                // Java onFatalError 分支显式递增一次，随后由锁内统一 catch
                // SQLException 再递增一次。
                self.inner.record_connect_error(2);
                return Err(error);
            }
            // Java 每次进入 getConnectionInternal 的锁内循环都递增，而非每个
            // 公共 get 调用只递增一次；无效 holder 后的重试也形成新一轮。
            self.inner.connect_count.fetch_add(1, Ordering::Relaxed);
            let (idle_connection, remaining_idle) = {
                let mut idle = self.inner.idle.lock();
                let connection = idle.pop_front();
                (connection, idle.len())
            };
            if let Some(holder) = idle_connection {
                let mut candidate = BorrowCandidate::new(Arc::clone(&self.inner), holder);
                let holder = candidate.holder_mut();
                if holder.user_password_version() < self.inner.user_password_version() {
                    continue;
                }
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
                        Some(connection) => !self.inner.test_connection_internal(connection).await,
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
                self.inner.record_active_acquire();
                if let Err(error) = self.ensure_borrow_generation(lifecycle_generation) {
                    self.inner.return_connection(
                        holder,
                        crate::core::ConnectionRecycleDisposition::discard(),
                    );
                    self.inner.record_connect_error(2);
                    return Err(error);
                }
                return Ok(self.wrap_connection(holder));
            }
            match self.create_connection_until(deadline).await {
                Ok(holder) => {
                    if holder.user_password_version() < self.inner.user_password_version() {
                        self.inner.discard_count.fetch_add(1, Ordering::Relaxed);
                        self.inner.destroy_holder(holder);
                        continue;
                    }
                    if !holder.mark_active() {
                        self.inner.destroy_holder(holder);
                        continue;
                    }
                    self.inner.record_active_acquire();
                    if let Err(error) = self.ensure_borrow_generation(lifecycle_generation) {
                        self.inner.return_connection(
                            holder,
                            crate::core::ConnectionRecycleDisposition::discard(),
                        );
                        self.inner.record_connect_error(2);
                        return Err(error);
                    }
                    return Ok(self.wrap_connection(holder));
                }
                Err(DruidError::PoolExhausted) if !self.inner.idle.lock().is_empty() => continue,
                Err(DruidError::PoolExhausted) => {}
                Err(DruidError::AcquireTimeout) => {
                    return Err(self.inner.connection_timeout_error(started_at.elapsed()));
                }
                Err(_) if !self.inner.idle.lock().is_empty() => continue,
                Err(e @ DruidError::DataSourceNotAvailable { .. }) => {
                    // Java failFast 从 pollLast 抛出后由锁内 SQLException catch
                    // 统一递增一次。
                    self.inner.record_connect_error(1);
                    return Err(e);
                }
                Err(e) => return Err(e),
            }
            let _waiter = match WaitTaskRegistration::register(Arc::clone(&self.inner)) {
                Ok(waiter) => waiter,
                Err(error) => {
                    // Java maxWaitThreadCount 分支先递增，再被统一 catch 再递增。
                    self.inner.record_connect_error(2);
                    return Err(error);
                }
            };
            let notify = self.inner.notify.notified();
            tokio::pin!(notify);
            if let Some(deadline) = deadline {
                match tokio::time::timeout_at(deadline.into(), notify).await {
                    Ok(_) => {},
                    Err(_) => {
                        return Err(self.inner.connection_timeout_error(started_at.elapsed()));
                    }
                }
            } else {
                notify.await;
            }
        }
    }

    fn ensure_borrow_generation(&self, expected_generation: u64) -> Result<(), DruidError> {
        if self.inner.lifecycle_generation() != expected_generation {
            // Java 只在进入 getConnectionInternal 时检查 closed。已经进入
            // takeLast/pollLast 的线程被 close 唤醒后走 `!enable` 分支。
            return Err(DruidError::DataSourceDisabled);
        }
        self.inner.ensure_available()
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
            if let Some(error) = self.inner.fail_fast_error() {
                return Err(error);
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
                Err(error @ DruidError::DataSourceClosed { .. }) => return Err(error),
                Err(_error) => {
                    error_count = error_count.saturating_add(1);
                    if error_count <= self.inner.config.connection_error_retry_attempts {
                        continue;
                    }

                    let delay = self.inner.config.time_between_connect_error;
                    if delay.is_zero() {
                        // Java 只在 timeBetweenConnectErrorMillis > 0 时切换
                        // failContinuous/break；零间隔继续立即重试。
                        tokio::task::yield_now().await;
                        continue;
                    }
                    self.inner.set_fail_continuous(true);
                    if let Some(fail_fast_error) = self.inner.fail_fast_error() {
                        return Err(fail_fast_error);
                    }
                    if self.inner.config.break_after_acquire_failure {
                        // Java creator 退出后 waiter 继续等待归还/外部唤醒，直到
                        // maxWait；用 PoolExhausted 进入同一等待分支。
                        return Err(DruidError::PoolExhausted);
                    }
                    error_count = 0;

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

    /// 返回物理建连超时秒数。对应 Java: `CommonDataSource#getLoginTimeout`。
    #[must_use]
    pub fn login_timeout(&self) -> i32 {
        self.inner.config.login_timeout
    }

    pub fn state(&self) -> PoolState {
        PoolState {
            name: self.name.clone(),
            driver_name: self.driver_name.clone(),
            url: self.inner.config.url.clone().unwrap_or_default(),
            max_open: self.inner.config.max_open,
            active_count: self
                .inner
                .active_count
                .load(std::sync::atomic::Ordering::Relaxed),
            active_peak: self.inner.active_peak.load(Ordering::Relaxed),
            active_peak_time_millis: self.inner.active_peak_time_millis.load(Ordering::Relaxed),
            idle_count: self.inner.idle.lock().len(),
            pooling_peak: self.inner.pooling_peak.load(Ordering::Relaxed),
            pooling_peak_time_millis: self.inner.pooling_peak_time_millis.load(Ordering::Relaxed),
            wait_count: self.inner.wait_count.load(Ordering::Relaxed),
            not_empty_wait_count: self.inner.not_empty_wait_count.load(Ordering::Relaxed),
            not_empty_wait_nanos: self.inner.not_empty_wait_nanos.load(Ordering::Relaxed),
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
            physical_connect_error_count: self
                .inner
                .physical_connect_error_count
                .load(Ordering::Relaxed),
            fail_continuous: self.inner.is_fail_continuous(),
            fail_continuous_time_millis: self.inner.fail_continuous_time_millis(),
            last_create_error: self
                .inner
                .last_create_error()
                .map(|error| error.to_string()),
            last_create_error_time_millis: self.inner.last_create_error_time_millis(),
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
            reset_stat_enable: self.is_reset_stat_enable(),
            reset_count: self.reset_count(),
            closed: self.inner.closed.load(std::sync::atomic::Ordering::Relaxed),
            ..Default::default()
        }
    }

    /// 取得并重置 Java `DruidDataSourceStatValue` 对应的区间统计。
    ///
    /// 当前连接数和配置不重置；峰值、累计计数、直方图、PreparedStatement
    /// 命中/未命中及 SQL 列表按 Java `getStatValueAndReset()` 原子取值语义
    /// 进入新快照。
    #[must_use]
    pub fn stat_value_and_reset(&self) -> DruidDataSourceStatValue {
        self.stat_snapshot_context.snapshot_and_reset()
    }

    /// 立即取得并发布一份区间统计快照。
    ///
    /// 对应 Java `DruidDataSource#logStats()` 的产品语义；输出端是 Rust
    /// [`super::DataSourceStatSink`]，而不是 Java logger。
    pub fn publish_stats(&self) -> Result<(), DruidError> {
        let stat_value = self.stat_value_and_reset();
        self.inner.config.stat_sink.publish(&stat_value)
    }

    /// 返回 Java `isResetStatEnable()`。
    #[must_use]
    pub fn is_reset_stat_enable(&self) -> bool {
        self.reset_stat_enable.load(Ordering::Acquire)
    }

    /// 设置 Java `resetStatEnable`。
    pub fn set_reset_stat_enable(&self, reset_stat_enable: bool) {
        self.reset_stat_enable
            .store(reset_stat_enable, Ordering::Release);
        self.stats_collector
            .set_reset_stat_enable(reset_stat_enable);
    }

    /// 返回实际执行 `resetStat()` 的累计次数。
    #[must_use]
    pub fn reset_count(&self) -> u64 {
        self.reset_count.load(Ordering::Acquire)
    }

    /// 重置累计池统计，当前 active/idle/缓存占用保持可见。
    ///
    /// Java `resetStatEnable=false` 时整个调用无副作用，也不增加 resetCount。
    pub fn reset_stats(&self) {
        if !self.is_reset_stat_enable() {
            return;
        }
        self.inner.reset_stats();
        self.remove_abandoned_count.store(0, Ordering::Release);
        self.reset_count.fetch_add(1, Ordering::AcqRel);
    }

    /// 返回与 `StatFilter` 共享的数据源统计对象。
    #[must_use]
    pub fn stats_collector(&self) -> &Arc<StatsCollector> {
        &self.stats_collector
    }

    /// 返回数据库类型。
    #[must_use]
    pub fn db_type_name(&self) -> Option<&str> {
        self.inner.config.db_type_name.as_deref()
    }

    /// 返回对外配置 URL。
    #[must_use]
    pub fn url(&self) -> Option<&str> {
        self.inner
            .config
            .url
            .as_deref()
            .or_else(|| self.inner.factory.connection_url())
    }

    /// 返回底层驱动 URL。
    #[must_use]
    pub fn raw_url(&self) -> Option<&str> {
        self.inner
            .config
            .raw_url
            .as_deref()
            .or_else(|| self.inner.factory.connection_url())
    }

    /// 返回未池化物理驱动工厂。
    #[must_use]
    pub fn raw_driver(&self) -> &dyn PhysicalConnectionFactory {
        self.inner.factory.as_ref()
    }

    /// 返回逻辑驱动连接属性。
    #[must_use]
    pub fn connect_properties(&self) -> &std::collections::HashMap<String, String> {
        self.inner.config.connection_properties.as_ref()
    }

    /// 返回 Filter 类型名称。
    #[must_use]
    pub fn filter_class_names(&self) -> Vec<String> {
        self.filter_chain
            .as_ref()
            .map_or_else(Vec::new, |chain| chain.filter_class_names().to_vec())
    }

    /// 分配 Java 语义的连接 ID。
    pub fn create_connection_id(&self) -> u64 {
        self.inner.next_id()
    }

    /// 分配 Java 语义的 Statement ID。
    pub fn create_statement_id(&self) -> u64 {
        self.statement_id_seed.fetch_add(1, Ordering::AcqRel)
    }

    /// 分配 Java 语义的 `ResultSet` ID。
    pub fn create_result_set_id(&self) -> u64 {
        self.result_set_id_seed.fetch_add(1, Ordering::AcqRel)
    }

    /// 分配 Java 语义的 metadata ID。
    pub fn create_metadata_id(&self) -> u64 {
        self.metadata_id_seed.fetch_add(1, Ordering::AcqRel)
    }

    /// 分配 Java 语义的事务 ID。
    pub fn create_transaction_id(&self) -> u64 {
        self.transaction_id_seed.fetch_add(1, Ordering::AcqRel)
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

    /// 仅在当前已经存在空闲连接时尝试获取。
    ///
    /// 对应 Java `DruidDataSource#tryGetConnection()`：poolingCount 为零时直接
    /// 返回 null，不触发 init 或物理创建；非零时再进入普通 get 完整门禁。
    pub async fn try_get_connection(&self) -> Result<Option<DruidPooledConnection>, DruidError> {
        if self.inner.idle.lock().is_empty() {
            return Ok(None);
        }
        self.get().await.map(Some)
    }

    /// 返回 active + pooling 是否已经达到 maxActive。
    #[must_use]
    pub fn is_full(&self) -> bool {
        self.inner
            .active_count
            .load(Ordering::Acquire)
            .saturating_add(self.inner.idle.lock().len())
            >= self.inner.config.max_open
    }

    /// 使超过阈值且当前未执行 SQL 的借出连接租约失效。
    ///
    /// 对应 Java：`DruidDataSource#removeAbandoned()`。Java 可由扫描线程直接
    /// `close()` 活跃 RDBC 连接；Rust 不能安全地跨线程取得其独占可变引用，
    /// 因而先原子失效租约，物理连接在所有者下一次操作或 Drop 时丢弃。
    /// 返回本轮新失效的连接数。
    pub fn remove_abandoned(&self) -> usize {
        remove_abandoned_leases(
            &self.inner,
            &self.active_leases,
            self.remove_abandoned_count.as_ref(),
        )
    }

    /// 返回 Java `isRemoveAbandoned()` 配置。
    #[must_use]
    pub fn is_remove_abandoned(&self) -> bool {
        self.inner.config.remove_abandoned
    }

    /// 强制丢弃指定池化连接。
    ///
    /// 对应 Java `discardConnection(Connection)` 的 nullable 参数与 boolean
    /// empty-signal 结果；`None` 返回 false。
    pub fn discard_connection(&self, connection: Option<&mut DruidPooledConnection>) -> bool {
        connection.is_some_and(DruidPooledConnection::discard_connection)
    }

    /// 将池内物理连接总数填充到 `maxActive`，返回新建数量。
    ///
    /// 对应 Java：`DruidDataSource#fill()`。
    pub async fn fill(&self) -> Result<usize, DruidError> {
        self.init().await?;
        self.inner.fill(self.inner.config.max_open).await
    }

    /// 将池内物理连接总数填充到指定数量，返回新建数量。
    ///
    /// 对应 Java：`DruidDataSource#fill(int)`。
    pub async fn fill_to(&self, to_count: i32) -> Result<usize, DruidError> {
        // Java 固定先报告 closed，再校验负数，最后才触发 init。
        self.inner.ensure_not_closed()?;
        let to_count = usize::try_from(to_count).map_err(|_| {
            DruidError::InvalidArgument("toCount can't not be less than zero".to_owned())
        })?;
        self.init().await?;
        self.inner.fill(to_count).await
    }

    /// 通知池：外部 `PhysicalConnectionFactory` 的连接凭据已经更新。
    ///
    /// 调用方应先更新 factory，再调用本方法。池会递增凭据版本，替换旧空闲
    /// 连接，并在旧活跃连接归还时销毁它们。
    pub async fn notify_credentials_changed(&self) -> Result<u64, DruidError> {
        self.init().await?;
        self.inner.credentials_changed().await
    }

    /// 返回当前凭据版本。
    #[must_use]
    pub fn user_password_version(&self) -> u64 {
        self.inner.user_password_version()
    }

    /// 返回数据源是否已经完成初始化。
    #[must_use]
    pub fn is_initialized(&self) -> bool {
        self.initialized.load(Ordering::Acquire)
    }

    /// 返回数据源是否已经关闭。
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.inner.closed.load(Ordering::Acquire)
    }

    /// 返回数据源是否允许继续借出连接。
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.inner.is_enabled()
    }

    /// 设置数据源借用门禁。
    ///
    /// 禁用会唤醒所有等待任务；已经借出的连接不被中断，但归还时直接销毁，
    /// 对应 Java `DruidDataSource#setEnable(false)`。
    pub fn set_enabled(&self, enabled: bool) {
        self.inner.set_enabled(enabled);
    }

    /// 关闭数据源。
    ///
    /// 对应 Java：尚未 init 时 close 是无副作用；已经初始化后 close 幂等，
    /// 关闭空闲连接、停止维护任务、拒绝新借用并销毁 Filter。
    pub async fn close(&self) {
        let _lifecycle = self.lifecycle_lock.lock().await;
        self.close_resources().await;
    }

    async fn close_resources(&self) {
        if !self.initialized.load(Ordering::Acquire) {
            self.destroy_initialized_filters().await;
            return;
        }
        if self.is_closed() {
            return;
        }
        self.inner.set_enabled(false);
        self.inner.advance_lifecycle_generation();
        self.start_create_worker();
        self.inner.request_create_worker_shutdown();
        self.start_close_worker();
        self.maintenance_shutdown.notify_one();
        self.stat_publish_shutdown.notify_one();
        // 先把池标记为 closed 并排空 idle，再等待维护任务退出。这样 close
        // future 在等待后台任务期间被取消时，新借用也已经被拒绝，且
        // PoolInner::close 的 DetachedHolder 会完成剩余资源计数清理。
        self.inner.close().await;
        let maintenance_task = self.maintenance_task.lock().take();
        if let Some(maintenance_task) = maintenance_task {
            let _ = maintenance_task.await;
        }
        let stat_publish_task = self.stat_publish_task.lock().take();
        if let Some(stat_publish_task) = stat_publish_task {
            let _ = stat_publish_task.await;
        }
        // 必须先等待旧代次 factory 调用完成。否则 close worker 可能已经退出，
        // restart 又恢复 enable，旧创建任务会把连接混入新代次或绕过 async close。
        self.inner.wait_for_creators().await;
        let create_worker_task = self.create_worker_task.lock().take();
        if let Some(create_worker_task) = create_worker_task {
            let _ = create_worker_task.await;
        }
        self.inner.request_close_worker_shutdown_if_idle();
        if self.inner.active_count.load(Ordering::Acquire) == 0 {
            let close_worker_task = self.close_worker_task.lock().take();
            if let Some(close_worker_task) = close_worker_task {
                let _ = close_worker_task.await;
            }
        }
        self.destroy_initialized_filters().await;
    }

    async fn destroy_initialized_filters(&self) {
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

    /// 将关闭后的池恢复到“尚未初始化”状态。
    ///
    /// 对应 Java `DruidDataSource#restart()`：活动连接不为零时拒绝重启；
    /// 成功时关闭旧资源、重置统计并恢复 enable，但不立即 init，下一次
    /// `get/init/fill` 才创建新代次资源。
    pub async fn restart(&self) -> Result<(), DruidError> {
        let _lifecycle = self.lifecycle_lock.lock().await;
        let active_count = self.inner.active_count.load(Ordering::Acquire);
        if active_count != 0 {
            return Err(DruidError::ActiveConnectionsPreventRestart { active_count });
        }

        // 先关闭门禁并推进 generation，保证已经开始但尚未形成租约的旧 future
        // 不能在 restart 恢复 enable 后混入新代次。
        self.inner.set_enabled(false);
        self.inner.advance_lifecycle_generation();
        let active_count = self.inner.active_count.load(Ordering::Acquire);
        if active_count != 0 {
            self.inner.set_enabled(true);
            return Err(DruidError::ActiveConnectionsPreventRestart { active_count });
        }

        self.close_resources().await;
        self.drain_close_worker_if_idle().await;
        self.reset_stats();
        self.active_leases.clear();
        self.initialized.store(false, Ordering::Release);
        self.inner.prepare_restart();
        Ok(())
    }

    async fn drain_close_worker_if_idle(&self) {
        if self.inner.active_count.load(Ordering::Acquire) != 0 {
            return;
        }
        self.inner.request_close_worker_shutdown_if_idle();
        let close_worker_task = self.close_worker_task.lock().take();
        if let Some(close_worker_task) = close_worker_task {
            let _ = close_worker_task.await;
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

    /// 返回 Java `DruidAbstractDataSource#isOnFatalError()` 状态。
    #[must_use]
    pub fn is_on_fatal_error(&self) -> bool {
        self.inner.is_on_fatal_error()
    }

    /// 返回 Java `getOnFatalErrorMaxActive()` 配置。
    #[must_use]
    pub fn on_fatal_error_max_active(&self) -> i32 {
        self.inner.on_fatal_error_max_active()
    }

    /// 返回 Java `isAsyncInit()` 配置。
    #[must_use]
    pub fn is_async_init(&self) -> bool {
        self.inner.config.async_init
    }

    /// 返回 Java `isInitExceptionThrow()` 配置。
    #[must_use]
    pub fn is_init_exception_throw(&self) -> bool {
        self.inner.config.init_exception_throw
    }

    /// 返回 Java `DruidAbstractDataSource#isFailContinuous()`。
    #[must_use]
    pub fn is_fail_continuous(&self) -> bool {
        self.inner.is_fail_continuous()
    }

    /// 返回 Java `getLastCreateError()` 对应错误。
    #[must_use]
    pub fn last_create_error(&self) -> Option<DruidError> {
        self.inner.last_create_error()
    }

    /// 返回 Java `getLastCreateErrorTimeMillis()`。
    #[must_use]
    pub fn last_create_error_time_millis(&self) -> u64 {
        self.inner.last_create_error_time_millis()
    }

    pub(crate) fn mark_filters_initialized(&self) {
        self.filters_initialized.store(true, Ordering::Release);
    }

    fn wrap_connection(&self, holder: DruidConnectionHolder) -> DruidPooledConnection {
        let connection_id = holder.connection_id();
        let connection_properties = holder.connection_properties();
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
                pool.return_connection(holder, disposition)
            }),
        );
        if let Some(exception_sorter) = self.exception_sorter.clone() {
            connection.set_exception_sorter(exception_sorter);
        }
        let fatal_error_handler: Arc<dyn FatalErrorHandler> = self.inner.clone();
        connection.set_fatal_error_handler(fatal_error_handler);
        connection.set_stats_collector(Arc::clone(&self.stats_collector));
        connection.set_query_timeouts(
            self.inner.config.query_timeout,
            self.inner.config.transaction_query_timeout,
        );
        connection.set_proxy_id_seeds(
            Arc::clone(&self.statement_id_seed),
            Arc::clone(&self.result_set_id_seed),
            Arc::clone(&self.metadata_id_seed),
            Arc::clone(&self.transaction_id_seed),
        );
        connection.set_connection_properties(connection_properties);
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
    started_at: Instant,
}

impl WaitTaskRegistration {
    fn register(inner: Arc<PoolInner>) -> Result<Self, DruidError> {
        if let Some(max) = inner.config.max_wait_thread_count.filter(|max| *max > 0) {
            let result =
                inner
                    .wait_count
                    .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                        (current <= max).then_some(current.saturating_add(1))
                    });
            if let Err(current) = result {
                return Err(DruidError::MaxWaitThreadCountExceeded { max, current });
            }
        } else {
            inner.wait_count.fetch_add(1, Ordering::AcqRel);
        }
        Ok(Self {
            inner,
            started_at: Instant::now(),
        })
    }
}

impl Drop for WaitTaskRegistration {
    fn drop(&mut self) {
        self.inner.record_not_empty_wait(self.started_at.elapsed());
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
impl DataSourceConnectionProvider for DruidPool {
    fn data_source_name(&self) -> &str {
        self.name()
    }

    fn data_source_state(&self) -> PoolState {
        self.state()
    }

    async fn get_connection_direct_for_filter(
        &self,
        max_wait: Duration,
    ) -> Result<DruidPooledConnection, DruidError> {
        self.get_connection_direct(max_wait).await
    }
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
