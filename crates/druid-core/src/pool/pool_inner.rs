//! 对应 Java 类：com.alibaba.druid.pool.DruidDataSource（内部状态）
//!
//! 连接池内部状态：空闲队列、活跃计数、等待通知。

use crate::core::fatal_error_handler::FatalErrorHandler;
use crate::core::Value as RdbcValue;
use crate::core::{
    ConnectionRecycleDisposition, ConnectionState, DruidConnectionHolder, DruidError, FilterChain,
    PhysicalConnection, PhysicalConnectionConnectResult, PhysicalConnectionFactory,
    PhysicalConnectionInfo, PreparedStatementCacheStats, RdbcString, StatementGeneratedKeys,
    ValidConnectionCheckerAdapter,
};
use crate::sql::{DbType, RdbcUtils};
use crate::stats::StatsCollector;
use serde_json::{Number as JsonNumber, Value as JsonValue};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::Notify;

use super::connection_close_worker::ConnectionCloseCommand;

/// `DruidAbstractDataSource` fatal-error 字段的同锁状态。
///
/// Java 依赖数据源锁原子更新这些字段；Rust 把它们收拢到同一互斥区，避免多个
/// 原子字段产生 Java 中不存在的组合快照。计数保留 Java `int` 的 wrapping
/// 算术。
#[derive(Default)]
struct FatalErrorState {
    on_fatal_error: bool,
    fatal_error_count: i32,
    fatal_error_count_last_shrink: i32,
    last_fatal_error_at: Option<Instant>,
    last_fatal_error_time_millis: u64,
    last_fatal_error_sql: Option<RdbcString>,
    last_fatal_error: Option<DruidError>,
}

/// 单次 shrink 开始时取得的 fatal-error 快照。
struct FatalShrinkSnapshot {
    on_fatal_error: bool,
    fatal_error_increment: i32,
    last_fatal_error_at: Option<Instant>,
}

/// Java `createError/lastCreateError/failContinuous` 的同锁快照。
#[derive(Default)]
struct CreateFailureState {
    create_started_at: Option<Instant>,
    create_error: Option<DruidError>,
    last_create_error: Option<DruidError>,
    last_create_error_time_millis: u64,
    fail_continuous: bool,
    fail_continuous_time_millis: u64,
}

/// 连接池内部状态。
pub struct PoolInner {
    pub(crate) factory: Arc<dyn PhysicalConnectionFactory>,
    /// 包围物理驱动建连的 Druid Filter chain。
    ///
    /// 受监管 creator 只持有 `PoolInner`，因此该链必须与 factory 同属内部
    /// 状态，才能覆盖后台和直接创建两条路径。
    filter_chain: parking_lot::RwLock<Option<Arc<FilterChain>>>,
    pub(crate) config: super::config::PoolInnerConfig,
    pub(crate) idle: parking_lot::Mutex<VecDeque<DruidConnectionHolder>>,
    pub(crate) notify: Notify,
    pub(crate) active_count: AtomicUsize,
    pub(crate) active_peak: AtomicUsize,
    pub(crate) active_peak_time_millis: AtomicU64,
    pub(crate) wait_count: AtomicUsize,
    /// 正在执行物理 factory 创建的任务数，用于 close/restart 排空旧代次。
    pub(crate) creating_count: AtomicUsize,
    creating_idle: Notify,
    pub(crate) total_count: AtomicUsize,
    pub(crate) next_id: AtomicU64,
    pub(crate) user_password_version: AtomicU64,
    /// 每次 restart 递增，用于隔离关闭前已经开始的异步获取/创建任务。
    pub(crate) lifecycle_generation: AtomicU64,
    /// Java `DruidDataSource.enable` 的运行期借用门禁。
    pub(crate) enabled: AtomicBool,
    pub(crate) closed: AtomicBool,
    /// Java `closeTimeMillis`；尚未关闭时沿用 Java 的 `-1` 哨兵所对应的零值。
    pub(crate) close_time_millis: AtomicU64,
    fatal_error_state: parking_lot::Mutex<FatalErrorState>,
    create_failure_state: parking_lot::Mutex<CreateFailureState>,
    // 统计
    pub(crate) create_count: AtomicU64,
    pub(crate) close_count: AtomicU64,
    pub(crate) destroy_count: AtomicU64,
    pub(crate) connect_count: AtomicU64,
    pub(crate) connect_error_count: AtomicU64,
    pub(crate) physical_connect_error_count: AtomicU64,
    pub(crate) pooling_peak: AtomicUsize,
    pub(crate) pooling_peak_time_millis: AtomicU64,
    pub(crate) not_empty_wait_count: AtomicU64,
    pub(crate) not_empty_wait_nanos: AtomicU64,
    pub(crate) recycle_count: AtomicU64,
    pub(crate) recycle_error_count: AtomicU64,
    pub(crate) discard_count: AtomicU64,
    pub(crate) keep_alive_check_count: AtomicU64,
    pub(crate) keep_alive_check_error_count: AtomicU64,
    pub(crate) prepared_statement_stats: Arc<PreparedStatementCacheStats>,
    pub(crate) stats_collector: Arc<StatsCollector>,
    create_sender: parking_lot::RwLock<Option<UnboundedSender<Option<usize>>>>,
    close_sender: parking_lot::RwLock<Option<UnboundedSender<Option<ConnectionCloseCommand>>>>,
}

impl PoolInner {
    pub fn new(
        factory: Arc<dyn PhysicalConnectionFactory>,
        config: super::config::PoolInnerConfig,
    ) -> Self {
        Self::new_with_stats(factory, config, Arc::new(StatsCollector::default()))
    }

    /// 使用数据源共享分层统计创建内部池状态。
    pub(crate) fn new_with_stats(
        factory: Arc<dyn PhysicalConnectionFactory>,
        config: super::config::PoolInnerConfig,
        stats_collector: Arc<StatsCollector>,
    ) -> Self {
        Self {
            factory,
            filter_chain: parking_lot::RwLock::new(None),
            config,
            idle: parking_lot::Mutex::new(VecDeque::new()),
            notify: Notify::new(),
            active_count: AtomicUsize::new(0),
            active_peak: AtomicUsize::new(0),
            active_peak_time_millis: AtomicU64::new(0),
            wait_count: AtomicUsize::new(0),
            creating_count: AtomicUsize::new(0),
            creating_idle: Notify::new(),
            total_count: AtomicUsize::new(0),
            // Java `createConnectionId()` 从 10000 执行 incrementAndGet，
            // 因而首个可观察连接 ID 是 10001。
            next_id: AtomicU64::new(10_001),
            user_password_version: AtomicU64::new(0),
            lifecycle_generation: AtomicU64::new(0),
            enabled: AtomicBool::new(true),
            closed: AtomicBool::new(false),
            close_time_millis: AtomicU64::new(0),
            fatal_error_state: parking_lot::Mutex::new(FatalErrorState::default()),
            create_failure_state: parking_lot::Mutex::new(CreateFailureState::default()),
            create_count: AtomicU64::new(0),
            close_count: AtomicU64::new(0),
            destroy_count: AtomicU64::new(0),
            connect_count: AtomicU64::new(0),
            connect_error_count: AtomicU64::new(0),
            physical_connect_error_count: AtomicU64::new(0),
            pooling_peak: AtomicUsize::new(0),
            pooling_peak_time_millis: AtomicU64::new(0),
            not_empty_wait_count: AtomicU64::new(0),
            not_empty_wait_nanos: AtomicU64::new(0),
            recycle_count: AtomicU64::new(0),
            recycle_error_count: AtomicU64::new(0),
            discard_count: AtomicU64::new(0),
            keep_alive_check_count: AtomicU64::new(0),
            keep_alive_check_error_count: AtomicU64::new(0),
            prepared_statement_stats: Arc::new(PreparedStatementCacheStats::default()),
            stats_collector,
            create_sender: parking_lot::RwLock::new(None),
            close_sender: parking_lot::RwLock::new(None),
        }
    }

    /// 安装由 canonical `DruidPool` 持有的物理关闭 worker sender。
    pub(crate) fn install_close_sender(
        &self,
        sender: UnboundedSender<Option<ConnectionCloseCommand>>,
    ) {
        *self.close_sender.write() = Some(sender);
    }

    /// 安装由 canonical `DruidPool` 持有的补池 worker sender。
    pub(crate) fn install_create_sender(&self, sender: UnboundedSender<Option<usize>>) {
        *self.create_sender.write() = Some(sender);
    }

    /// 安装共享的物理建连 Filter chain。
    pub(crate) fn install_filter_chain(&self, filter_chain: Option<Arc<FilterChain>>) {
        *self.filter_chain.write() = filter_chain;
    }

    pub fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// 返回当前数据源凭据版本。
    pub(crate) fn user_password_version(&self) -> u64 {
        self.user_password_version.load(Ordering::Acquire)
    }

    /// 标记外部 factory 的 URL/用户名/密码已经更新，并替换旧版空闲连接。
    ///
    /// 对应 Java：动态 `config(Properties)` 更新凭据后递增
    /// `userPasswordVersion` 并替换 pooling connections。调用方必须先原子更新
    /// factory 内的凭据，再调用本方法；池只保存版本，不复制密码。
    pub(crate) async fn credentials_changed(&self) -> Result<u64, DruidError> {
        let new_version = self.user_password_version.fetch_add(1, Ordering::AcqRel) + 1;
        let target_total = self.total_count.load(Ordering::Acquire);
        let stale: Vec<DruidConnectionHolder> = {
            let mut queue = self.idle.lock();
            let mut retained = VecDeque::with_capacity(queue.len());
            let mut stale = Vec::new();
            while let Some(holder) = queue.pop_front() {
                if holder.user_password_version() < new_version {
                    stale.push(holder);
                } else {
                    retained.push_back(holder);
                }
            }
            *queue = retained;
            stale
        };

        for holder in stale {
            self.discard_count.fetch_add(1, Ordering::Relaxed);
            // 同步登记销毁并交给受监管 close worker，避免本 future 在逐个
            // `await close` 期间被取消后，剩余已脱离队列的 holder 绕过计数。
            self.destroy_holder(holder);
        }
        self.fill(target_total).await?;
        Ok(new_version)
    }

    pub fn can_grow(&self) -> bool {
        self.total_count.load(Ordering::Acquire) < self.config.max_open
    }

    /// 按 Java 检查顺序验证数据源是否仍可借用。
    pub(crate) fn ensure_available(&self) -> Result<(), DruidError> {
        self.ensure_not_closed()?;
        if !self.enabled.load(Ordering::Acquire) {
            return Err(DruidError::DataSourceDisabled);
        }
        Ok(())
    }

    /// 增加 Java `connectErrorCount` 逻辑获取失败计数。
    ///
    /// Java 某些锁内分支会先显式递增，再由统一的 `catch(SQLException)` 再递增，
    /// 因而调用方需要传入精确次数，不能在公共 `get` 出口对所有错误笼统计数。
    pub(crate) fn record_connect_error(&self, count: u64) {
        self.connect_error_count.fetch_add(count, Ordering::Relaxed);
    }

    /// 执行 Java `onFatalErrorMaxActive` 借用门限。
    pub(crate) fn ensure_fatal_error_available(&self) -> Result<(), DruidError> {
        let active_count = self.active_count.load(Ordering::Acquire);
        let max_active = self.config.on_fatal_error_max_active;
        let state = self.fatal_error_state.lock();
        if !state.on_fatal_error
            || max_active <= 0
            || active_count < usize::try_from(max_active).unwrap_or(usize::MAX)
        {
            return Ok(());
        }
        Err(DruidError::OnFatalError {
            active_count,
            max_active,
            last_error_time_millis: state.last_fatal_error_time_millis,
            last_sql: state.last_fatal_error_sql.clone(),
            cause: state.last_fatal_error.clone().map(Box::new),
        })
    }

    /// 返回 Java `isOnFatalError()` 状态。
    pub(crate) fn is_on_fatal_error(&self) -> bool {
        self.fatal_error_state.lock().on_fatal_error
    }

    /// 返回配置的 Java `onFatalErrorMaxActive`。
    pub(crate) fn on_fatal_error_max_active(&self) -> i32 {
        self.config.on_fatal_error_max_active
    }

    /// 返回 Java `isFailContinuous()` 状态。
    pub(crate) fn is_fail_continuous(&self) -> bool {
        self.create_failure_state.lock().fail_continuous
    }

    /// 返回最近一次物理创建错误。
    pub(crate) fn last_create_error(&self) -> Option<DruidError> {
        self.create_failure_state.lock().last_create_error.clone()
    }

    /// 返回最近一次物理创建错误时间。
    pub(crate) fn last_create_error_time_millis(&self) -> u64 {
        self.create_failure_state
            .lock()
            .last_create_error_time_millis
    }

    /// 返回当前连续创建失败状态的开始/刷新时间。
    pub(crate) fn fail_continuous_time_millis(&self) -> u64 {
        self.create_failure_state.lock().fail_continuous_time_millis
    }

    /// 构造 Java `GetConnectionTimeoutException` 的完整诊断快照。
    pub(crate) fn connection_timeout_error(&self, waited: Duration) -> DruidError {
        let active_count = self.active_count.load(Ordering::Acquire);
        let creating_count = self.creating_count.load(Ordering::Acquire);
        let create_failure = self.create_failure_state.lock();
        let create_elapsed_millis = (creating_count > 0)
            .then(|| create_failure.create_started_at)
            .flatten()
            .map(|started_at| started_at.elapsed())
            .map(|elapsed| u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX))
            .filter(|elapsed| *elapsed > 0);
        let cause = create_failure.create_error.clone().map(Box::new);
        drop(create_failure);
        let running_sql = self
            .stats_collector
            .sql_merger
            .all_stats()
            .into_iter()
            .filter_map(|stat| {
                let value = stat.stat_value();
                (value.running_count > 0).then_some((value.running_count, value.sql))
            })
            .collect();
        DruidError::GetConnectionTimeout {
            wait_millis: u64::try_from(waited.as_millis()).unwrap_or(u64::MAX),
            active_count,
            max_active: self.config.max_open,
            creating_count,
            create_elapsed_millis,
            create_error_count: self.physical_connect_error_count.load(Ordering::Acquire),
            running_sql,
            cause,
        }
    }

    /// 当 failFast 与连续失败同时成立时构造 Java
    /// `DataSourceNotAvailableException(createError)` 对应错误。
    pub(crate) fn fail_fast_error(&self) -> Option<DruidError> {
        let state = self.create_failure_state.lock();
        (self.config.fail_fast && state.fail_continuous).then(|| {
            DruidError::DataSourceNotAvailable {
                cause: state.create_error.clone().map(Box::new),
            }
        })
    }

    /// 更新 Java `failContinuous` 及其时间。
    pub(crate) fn set_fail_continuous(&self, fail: bool) {
        let mut state = self.create_failure_state.lock();
        state.fail_continuous_time_millis = if fail { Self::now_millis() } else { 0 };
        if state.fail_continuous == fail {
            return;
        }
        state.fail_continuous = fail;
        tracing::info!(
            fail_continuous = fail,
            "datasource physical create continuity changed"
        );
        if fail && self.config.fail_fast {
            self.notify.notify_waiters();
        }
    }

    fn record_create_error(&self, error: &DruidError) {
        let mut state = self.create_failure_state.lock();
        let now = Self::now_millis();
        state.create_error = Some(error.clone());
        state.last_create_error = Some(error.clone());
        state.last_create_error_time_millis = now;
    }

    fn record_create_success(&self) {
        {
            let mut state = self.create_failure_state.lock();
            state.create_error = None;
        }
        self.set_fail_continuous(false);
    }

    fn clear_on_fatal_error(&self) {
        let mut state = self.fatal_error_state.lock();
        if state.on_fatal_error {
            state.on_fatal_error = false;
        }
    }

    fn fatal_shrink_snapshot(&self) -> FatalShrinkSnapshot {
        let mut state = self.fatal_error_state.lock();
        let fatal_error_increment = state
            .fatal_error_count
            .wrapping_sub(state.fatal_error_count_last_shrink);
        state.fatal_error_count_last_shrink = state.fatal_error_count;
        FatalShrinkSnapshot {
            on_fatal_error: state.on_fatal_error,
            fatal_error_increment,
            last_fatal_error_at: state.last_fatal_error_at,
        }
    }

    /// 验证数据源尚未关闭，但不检查运行期借用开关。
    ///
    /// Java `init/fill/creator` 在 `enable=false` 时仍可维护物理池；enable 只在
    /// get/recycle 边界生效，因此这些路径不能复用完整借用门禁。
    pub(crate) fn ensure_not_closed(&self) -> Result<(), DruidError> {
        if self.closed.load(Ordering::Acquire) {
            Err(self.data_source_closed_error())
        } else {
            Ok(())
        }
    }

    /// 构造携带实际关闭时刻的 Java `DataSourceClosedException` 对应错误。
    pub(crate) fn data_source_closed_error(&self) -> DruidError {
        DruidError::DataSourceClosed {
            close_time_millis: self.close_time_millis.load(Ordering::Acquire),
        }
    }

    /// 返回运行期 enable 状态。
    pub(crate) fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Acquire)
    }

    /// 返回数据源是否已经关闭。
    pub(crate) fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    /// 修改运行期 enable 状态并唤醒所有等待者重新检查门禁。
    pub(crate) fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Release);
        if !enabled {
            self.notify.notify_waiters();
        }
    }

    /// 返回当前生命周期代次。
    pub(crate) fn lifecycle_generation(&self) -> u64 {
        self.lifecycle_generation.load(Ordering::Acquire)
    }

    /// 使旧代次的异步获取、创建和等待任务失效。
    pub(crate) fn advance_lifecycle_generation(&self) -> u64 {
        let generation = self.lifecycle_generation.fetch_add(1, Ordering::AcqRel) + 1;
        self.notify.notify_waiters();
        generation
    }

    /// 为 Java `restart()` 恢复尚未初始化的数据源状态。
    ///
    /// 调用方必须已经持有生命周期锁、完成旧 worker 的排空，并确认没有活动连接。
    pub(crate) fn prepare_restart(&self) {
        self.closed.store(false, Ordering::Release);
        self.enabled.store(true, Ordering::Release);
        // restart 的 activeCount 门禁保证此处没有可归还的 holder。唤醒等待者，
        // 使旧代次任务立即观察 generation 变化。
        self.notify.notify_waiters();
    }

    pub fn should_evict(&self) -> bool {
        let idle_count = self.idle.lock().len();
        idle_count > self.config.min_idle
    }

    /// 记录一次成功借出并维护 Java `activePeak`。
    pub(crate) fn record_active_acquire(&self) {
        let active_count = self.active_count.fetch_add(1, Ordering::AcqRel) + 1;
        self.connect_count.fetch_add(1, Ordering::Relaxed);
        Self::record_peak(
            active_count,
            &self.active_peak,
            &self.active_peak_time_millis,
        );
    }

    /// 记录当前空闲连接数并维护 Java `poolingPeak`。
    pub(crate) fn record_pooling_count(&self, pooling_count: usize) {
        Self::record_peak(
            pooling_count,
            &self.pooling_peak,
            &self.pooling_peak_time_millis,
        );
    }

    /// 记录一次进入 `notEmpty` 等待队列的实际等待时长。
    pub(crate) fn record_not_empty_wait(&self, elapsed: Duration) {
        self.not_empty_wait_count.fetch_add(1, Ordering::Relaxed);
        self.not_empty_wait_nanos.fetch_add(
            u64::try_from(elapsed.as_nanos()).unwrap_or(u64::MAX),
            Ordering::Relaxed,
        );
    }

    fn record_peak(value: usize, peak: &AtomicUsize, peak_time_millis: &AtomicU64) {
        let mut current = peak.load(Ordering::Acquire);
        while value > current {
            match peak.compare_exchange_weak(current, value, Ordering::AcqRel, Ordering::Acquire) {
                Ok(_) => {
                    peak_time_millis.store(Self::now_millis(), Ordering::Release);
                    break;
                }
                Err(observed) => current = observed,
            }
        }
    }

    fn now_millis() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .try_into()
            .unwrap_or(u64::MAX)
    }

    /// 原子取得 Java `getStatValueAndReset()` 的池级区间统计。
    pub(crate) fn stat_snapshot_and_reset(&self) -> PoolStatSnapshot {
        let pooling_count = self.idle.lock().len();
        PoolStatSnapshot {
            active_count: self.active_count.load(Ordering::Acquire),
            active_peak: self.active_peak.swap(0, Ordering::AcqRel),
            active_peak_time_millis: self.active_peak_time_millis.swap(0, Ordering::AcqRel),
            pooling_count,
            pooling_peak: self.pooling_peak.swap(0, Ordering::AcqRel),
            pooling_peak_time_millis: self.pooling_peak_time_millis.swap(0, Ordering::AcqRel),
            connect_count: self.connect_count.swap(0, Ordering::AcqRel),
            close_count: self.close_count.swap(0, Ordering::AcqRel),
            wait_thread_count: self.wait_count.load(Ordering::Acquire),
            not_empty_wait_count: self.not_empty_wait_count.swap(0, Ordering::AcqRel),
            not_empty_wait_nanos: self.not_empty_wait_nanos.swap(0, Ordering::AcqRel),
            logic_connect_error_count: self.connect_error_count.swap(0, Ordering::AcqRel),
            physical_connect_count: self.create_count.swap(0, Ordering::AcqRel),
            physical_close_count: self.destroy_count.swap(0, Ordering::AcqRel),
            physical_connect_error_count: self
                .physical_connect_error_count
                .swap(0, Ordering::AcqRel),
            keep_alive_check_count: self.keep_alive_check_count.swap(0, Ordering::AcqRel),
            pstmt_cache_hit_count: self
                .prepared_statement_stats
                .take_cached_prepared_statement_hit_count(),
            pstmt_cache_miss_count: self
                .prepared_statement_stats
                .take_cached_prepared_statement_miss_count(),
        }
    }

    /// 重置数据源累计统计，保留当前连接数量和缓存占用。
    pub(crate) fn reset_stats(&self) {
        let _ = self.stat_snapshot_and_reset();
        // Java resetStat 把 activePeak 重置为当前 activeCount，而不是 0。
        self.active_peak
            .store(self.active_count.load(Ordering::Acquire), Ordering::Release);
        self.active_peak_time_millis.store(0, Ordering::Release);
        self.recycle_count.store(0, Ordering::Release);
        self.recycle_error_count.store(0, Ordering::Release);
        self.discard_count.store(0, Ordering::Release);
        self.keep_alive_check_error_count
            .store(0, Ordering::Release);
        self.prepared_statement_stats.reset();
        let mut create_state = self.create_failure_state.lock();
        create_state.last_create_error = None;
        create_state.last_create_error_time_millis = 0;
    }

    /// 使用数据源配置的 Java `ValidConnectionChecker` 校验物理连接。
    pub(crate) async fn validate_connection(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
    ) -> Result<(), DruidError> {
        let result = if connection.is_closed() {
            Err(DruidError::ValidationFailed(
                "validateConnection: connection closed".to_owned(),
            ))
        } else if let Some(checker) = self.config.valid_connection_checker.as_ref() {
            let valid = checker
                .is_valid_connection(
                    connection,
                    self.config.validation_query.as_deref(),
                    self.config.validation_query_timeout,
                )
                .await?;
            if valid {
                Ok(())
            } else {
                Err(DruidError::ValidationFailed(
                    "ValidConnectionChecker returned false".to_owned(),
                ))
            }
        } else if let Some(validation_query) = self.config.validation_query.as_deref() {
            let valid = ValidConnectionCheckerAdapter::exec_valid_query(
                connection,
                validation_query,
                self.config.validation_query_timeout,
            )
            .await?;
            if valid {
                Ok(())
            } else {
                Err(DruidError::ValidationFailed(
                    "validationQuery didn't return a row".to_owned(),
                ))
            }
        } else {
            Ok(())
        };
        if result.is_ok() {
            self.clear_on_fatal_error();
        }
        result
    }

    /// 执行 Java `testConnectionInternal` 的吞错布尔校验。
    ///
    /// 与公开 `validateConnection` 不同，本方法把 checker、验证 SQL、timeout
    /// 和 driver 的所有错误折叠为 false，供 borrow/return/keepAlive 使用。
    pub(crate) async fn test_connection_internal(
        &self,
        connection: &mut Box<dyn PhysicalConnection>,
    ) -> bool {
        self.validate_connection(connection).await.is_ok()
    }

    /// 按 Java `initialSize` 预建空闲物理连接。
    pub(crate) async fn fill_initial(&self) -> Result<(), DruidError> {
        if self.config.async_init {
            self.request_refill(self.config.initial_size);
            return Ok(());
        }

        loop {
            match self.fill(self.config.initial_size).await {
                Ok(_) => return Ok(()),
                Err(error) if self.config.init_exception_throw || self.is_closed() => {
                    return Err(error);
                }
                Err(error) => {
                    // Java initExceptionThrow=false 固定 sleep 3000ms 后继续，
                    // 不复用 creator 的 timeBetweenConnectErrorMillis。
                    tracing::error!(%error, "init datasource error, retry after 3000ms");
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
            }
        }
    }

    /// 将池内物理连接总数填充到指定数量，返回本次创建数。
    ///
    /// 对应 Java：`DruidDataSource#fill(int)`。目标会被 `maxActive` 截断；
    /// 已有活跃连接计入总数，新连接只进入空闲队列。
    pub(crate) async fn fill(&self, to_count: usize) -> Result<usize, DruidError> {
        self.ensure_not_closed()?;
        let target = to_count.min(self.config.max_open);
        let mut created = 0usize;
        while self.total_count.load(Ordering::Acquire) < target {
            let holder = match self.create_connection_to_limit(target).await {
                Ok(holder) => holder,
                // 其他并发 fill/borrow 已经占满目标时，与 Java 第二次
                // isFillable(toCount) 检查一样正常结束，而不是报告池耗尽。
                Err(DruidError::PoolExhausted) => break,
                Err(error) => return Err(error),
            };
            if holder.user_password_version() < self.user_password_version() {
                self.discard_count.fetch_add(1, Ordering::Relaxed);
                self.destroy_holder(holder);
                continue;
            }
            let pooling_count = {
                let mut idle = self.idle.lock();
                if self.closed.load(Ordering::Acquire) {
                    Err(holder)
                } else {
                    idle.push_back(holder);
                    Ok(idle.len())
                }
            };
            let pooling_count = match pooling_count {
                Ok(pooling_count) => pooling_count,
                Err(holder) => {
                    self.destroy_holder(holder);
                    return Err(self.data_source_closed_error());
                }
            };
            self.record_pooling_count(pooling_count);
            created += 1;
        }
        if created > 0 {
            self.notify.notify_waiters();
        }
        Ok(created)
    }

    /// 创建新连接。
    pub async fn create_connection(&self) -> Result<DruidConnectionHolder, DruidError> {
        self.create_connection_to_limit(self.config.max_open).await
    }

    async fn create_connection_to_limit(
        &self,
        capacity_limit: usize,
    ) -> Result<DruidConnectionHolder, DruidError> {
        let result = self
            .create_connection_to_limit_internal(capacity_limit)
            .await;
        match &result {
            Ok(_) => self.record_create_success(),
            Err(DruidError::PoolExhausted | DruidError::DataSourceClosed { .. }) => {}
            Err(error) => self.record_create_error(error),
        }
        result
    }

    async fn create_connection_to_limit_internal(
        &self,
        capacity_limit: usize,
    ) -> Result<DruidConnectionHolder, DruidError> {
        self.ensure_not_closed()?;
        let reserved = self
            .total_count
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                (current < capacity_limit).then_some(current + 1)
            })
            .is_ok();
        if !reserved {
            return Err(DruidError::PoolExhausted);
        }
        let mut reservation = ConnectionSlotReservation::new(&self.total_count);
        let _creating = CreatingTaskRegistration::new(&self.creating_count, &self.creating_idle);
        // 必须在调用 factory 之前冻结版本；若凭据在连接建立期间更新，使用旧
        // 凭据创建出的 holder 会保留旧版本并在借出/归还门禁被淘汰。
        let user_password_version = self.user_password_version();

        let connect_started_at = Instant::now();
        self.create_failure_state.lock().create_started_at = Some(connect_started_at);
        let filter_chain = self.filter_chain.read().clone();
        let mut connection_properties = (*self.config.connection_properties).clone();
        let create_result = match &filter_chain {
            Some(filter_chain) => {
                let mut next_connection_id = || self.next_id();
                filter_chain
                    .physical_connection_connect(
                        self.factory.as_ref(),
                        &mut connection_properties,
                        self.config.login_timeout,
                        &mut next_connection_id,
                    )
                    .await
            }
            None if self.config.login_timeout > 0 => {
                match tokio::time::timeout(
                    Duration::from_secs(self.config.login_timeout as u64),
                    self.factory
                        .create_info_with_properties(&connection_properties),
                )
                .await
                {
                    Ok(result) => result.map(|connection_info| {
                        PhysicalConnectionConnectResult::new(connection_info, self.next_id())
                    }),
                    Err(_) => Err(DruidError::LoginTimeout),
                }
            }
            None => self
                .factory
                .create_info_with_properties(&connection_properties)
                .await
                .map(|connection_info| {
                    PhysicalConnectionConnectResult::new(connection_info, self.next_id())
                }),
        };

        match create_result {
            Ok(connect_result) => {
                let (mut connection_info, connection_id) = connect_result.into_parts();
                // Java 在原始驱动连接成功后立即增加 createCount，默认属性初始化失败
                // 也不回退该计数。
                self.create_count.fetch_add(1, Ordering::Relaxed);
                if let Err(error) = self.ensure_not_closed() {
                    self.close_connected_connection(
                        filter_chain.as_ref(),
                        &mut connection_info,
                        connection_id,
                        connect_started_at.elapsed(),
                    )
                    .await;
                    self.destroy_count.fetch_add(1, Ordering::Relaxed);
                    return Err(error);
                }

                let initialize_result = match connection_info.physical_connection_box_mut() {
                    Some(connection) => {
                        self.initialize_physical_connection(connection.as_mut())
                            .await
                    }
                    None => Err(DruidError::ConnectionDiscarded),
                };
                if let Err(error) = initialize_result {
                    // 对应 Java createPhysicalConnection() 的异常路径：初始化失败时
                    // 关闭刚创建的物理连接，但它从未进入池，不增加 destroyCount。
                    self.close_connected_connection(
                        filter_chain.as_ref(),
                        &mut connection_info,
                        connection_id,
                        connect_started_at.elapsed(),
                    )
                    .await;
                    self.physical_connect_error_count
                        .fetch_add(1, Ordering::Relaxed);
                    return Err(error);
                }
                connection_info.mark_initialized();

                let init_sql_checked = match self
                    .initialize_sqls_and_variables(&mut connection_info)
                    .await
                {
                    Ok(checked) => checked,
                    Err(error) => {
                        self.close_connected_connection(
                            filter_chain.as_ref(),
                            &mut connection_info,
                            connection_id,
                            connect_started_at.elapsed(),
                        )
                        .await;
                        self.physical_connect_error_count
                            .fetch_add(1, Ordering::Relaxed);
                        return Err(error);
                    }
                };

                let validate_result = if init_sql_checked {
                    Ok(())
                } else {
                    match connection_info.physical_connection_box_mut() {
                        Some(connection) => self.validate_connection(connection).await,
                        None => Err(DruidError::ConnectionDiscarded),
                    }
                };
                if let Err(error) = validate_result {
                    self.close_connected_connection(
                        filter_chain.as_ref(),
                        &mut connection_info,
                        connection_id,
                        connect_started_at.elapsed(),
                    )
                    .await;
                    self.physical_connect_error_count
                        .fetch_add(1, Ordering::Relaxed);
                    return Err(error);
                }
                connection_info.mark_validated();

                let mut holder = DruidConnectionHolder::with_connection_info(
                    connection_info,
                    connection_id,
                    user_password_version,
                )?;
                holder.set_connection_properties(Arc::new(connection_properties));
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
                self.physical_connect_error_count
                    .fetch_add(1, Ordering::Relaxed);
                Err(e)
            }
        }
    }

    /// 关闭已经由 physical-connect 链创建、但尚未成功进入 holder 的连接。
    ///
    /// Java 初始化或校验失败时调用 `ConnectionProxy#close()`，因此必须重新从
    /// position 0 进入 physical-close 链，让 StatFilter/LogFilter 观察真实
    /// 关闭；该路径不增加池级 destroyCount。
    async fn close_connected_connection(
        &self,
        filter_chain: Option<&Arc<FilterChain>>,
        connection_info: &mut PhysicalConnectionInfo,
        connection_id: u64,
        physical_age: Duration,
    ) {
        let Some(connection) = connection_info.physical_connection_box_mut() else {
            return;
        };
        let result = match filter_chain {
            Some(filter_chain) if !filter_chain.is_empty() => {
                filter_chain
                    .physical_connection_close(
                        self.factory.as_ref(),
                        connection,
                        connection_id,
                        physical_age,
                    )
                    .await
            }
            _ => self.factory.close(connection).await,
        };
        if let Err(error) = result {
            tracing::warn!(
                %error,
                connection_id,
                "close failed physical connection failed"
            );
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

    /// 在 raw connection 上执行初始化 SQL并按 Java 规则采集 `MySQL` 变量。
    ///
    /// 返回值对应 Java `initSqls(...)` 的 `checked`：只要执行过初始化 SQL或
    /// `MySQL` variables 查询，创建流程就不再追加 validation query。
    async fn initialize_sqls_and_variables(
        &self,
        connection_info: &mut crate::core::PhysicalConnectionInfo,
    ) -> Result<bool, DruidError> {
        let mut variables = self.config.init_variants.then(HashMap::new);
        let mut global_variables = self.config.init_global_variants.then(HashMap::new);
        let connection = connection_info
            .physical_connection_box_mut()
            .ok_or(DruidError::ConnectionDiscarded)?;
        let mut checked = false;

        for sql in &self.config.connection_init_sqls {
            connection
                .execute(sql, Vec::new(), StatementGeneratedKeys::None)
                .await?;
            checked = true;
        }

        let is_mysql_family = self
            .config
            .db_type_name
            .as_deref()
            .and_then(DbType::of)
            .is_some_and(RdbcUtils::is_mysql_db_type);
        if is_mysql_family {
            if let Some(values) = variables.as_mut() {
                let rows = connection.fetch("show variables", Vec::new()).await?;
                Self::collect_variable_rows(values, rows);
                checked = true;
            }
            if let Some(values) = global_variables.as_mut() {
                let rows = connection
                    .fetch("show global variables", Vec::new())
                    .await?;
                Self::collect_variable_rows(values, rows);
                checked = true;
            }
        }

        connection_info.set_variables(variables);
        connection_info.set_global_variables(global_variables);
        Ok(checked)
    }

    fn collect_variable_rows(target: &mut HashMap<String, JsonValue>, rows: Vec<crate::core::Row>) {
        for row in rows {
            let Some(name) = row.get(0).and_then(Self::variable_name) else {
                continue;
            };
            let value = row.get(1).map_or(JsonValue::Null, Self::rdbc_value_to_json);
            target.insert(name, value);
        }
    }

    fn variable_name(value: &RdbcValue) -> Option<String> {
        match value {
            RdbcValue::Null => None,
            RdbcValue::String(value) => Some(value.clone()),
            RdbcValue::Bytes(value) => String::from_utf8(value.clone()).ok(),
            RdbcValue::Bool(value) => Some(value.to_string()),
            RdbcValue::Int(value) => Some(value.to_string()),
            RdbcValue::Float(value) => Some(value.to_string()),
            RdbcValue::Decimal(value) => Some(value.to_string()),
            RdbcValue::Date(value) => Some(value.to_string()),
            RdbcValue::Time(value) => Some(value.to_string()),
            RdbcValue::Timestamp(value) => Some(value.to_string()),
        }
    }

    fn rdbc_value_to_json(value: &RdbcValue) -> JsonValue {
        match value {
            RdbcValue::Null => JsonValue::Null,
            RdbcValue::Bool(value) => JsonValue::Bool(*value),
            RdbcValue::Int(value) => JsonValue::Number((*value).into()),
            RdbcValue::Float(value) => JsonNumber::from_f64(*value).map_or_else(|| JsonValue::String(value.to_string()), JsonValue::Number),
            RdbcValue::Decimal(value) => JsonValue::String(value.to_string()),
            RdbcValue::Date(value) => JsonValue::String(value.to_string()),
            RdbcValue::Time(value) => JsonValue::String(value.to_string()),
            RdbcValue::Timestamp(value) => JsonValue::String(value.to_string()),
            RdbcValue::String(value) => JsonValue::String(value.clone()),
            RdbcValue::Bytes(value) => JsonValue::Array(
                value
                    .iter()
                    .map(|byte| JsonValue::Number((*byte).into()))
                    .collect(),
            ),
        }
    }

    /// 归还连接到空闲队列。
    pub fn return_connection(
        &self,
        holder: DruidConnectionHolder,
        disposition: ConnectionRecycleDisposition,
    ) -> bool {
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
            return self.discard_recycled_holder(holder);
        }

        // Java closeCount 统计逻辑池化连接关闭，而不是物理 socket 关闭。
        self.close_count.fetch_add(1, Ordering::Relaxed);
        let holder_was_active = holder.mark_idle();

        if disposition.has_recycle_error() {
            self.recycle_error_count.fetch_add(1, Ordering::Relaxed);
        }
        if !holder_was_active || !disposition.is_reusable() {
            self.discard_count.fetch_add(1, Ordering::Relaxed);
            return self.discard_recycled_holder(holder);
        }

        let unusable = holder
            .physical_connection()
            .is_none_or(|connection| connection.is_closed() || connection.is_discarded());
        if self.closed.load(Ordering::Acquire)
            || !self.enabled.load(Ordering::Acquire)
            || unusable
            || holder.is_discard()
        {
            return self.discard_recycled_holder(holder);
        }

        let physical_age = holder.physical_age();
        let lifetime_expired = physical_age >= self.config.max_lifetime;
        let physical_timeout_expired = self
            .config
            .physical_connection_timeout
            .is_some_and(|timeout| !timeout.is_zero() && physical_age > timeout);
        let max_use_count_reached =
            self.config.max_use_count > 0 && holder.use_count() >= self.config.max_use_count as u64;
        let credentials_stale = holder.user_password_version() < self.user_password_version();
        if lifetime_expired
            || physical_timeout_expired
            || max_use_count_reached
            || credentials_stale
        {
            self.discard_count.fetch_add(1, Ordering::Relaxed);
            return self.discard_recycled_holder(holder);
        }

        let returned = {
            let mut queue = self.idle.lock();
            // Java Druid 的 maxIdle 已 deprecated，真实 idle 容量由 maxActive
            // 限制；保留配置字段仅用于兼容读取，不能改变 putLast 语义。
            if queue.len() >= self.config.max_open {
                Err(holder)
            } else {
                queue.push_back(holder);
                Ok(queue.len())
            }
        };

        // Java 在 putLast 尝试完成后递增 recycleCount，即使池满导致 putLast=false。
        self.recycle_count.fetch_add(1, Ordering::Relaxed);
        match returned {
            Err(holder) => self.discard_recycled_holder(holder),
            Ok(pooling_count) => {
                self.record_pooling_count(pooling_count);
                self.notify.notify_one();
                false
            }
        }
    }

    fn discard_recycled_holder(&self, holder: DruidConnectionHolder) -> bool {
        self.destroy_holder(holder);
        self.request_refill(self.config.min_idle)
    }

    /// 请求受监管 creator 把 active + pooling 补到目标值。
    pub(crate) fn request_refill(&self, to_count: usize) -> bool {
        // Rust waiter 自己承担 Java creator 的直接建连分支；容量释放后即使
        // minIdle 为零也必须唤醒它重新竞争。boolean 只表示是否额外请求后台补池。
        self.notify.notify_waiters();
        if self.closed.load(Ordering::Acquire) || to_count == 0 {
            return false;
        }
        let target = to_count.min(self.config.max_open);
        if self.total_count.load(Ordering::Acquire) >= target {
            return false;
        }
        let signalled = self
            .create_sender
            .read()
            .clone()
            .is_some_and(|sender| sender.send(Some(target)).is_ok());
        signalled
    }

    /// 请求 creator 在此前补池命令处理完成后退出。
    pub(crate) fn request_create_worker_shutdown(&self) {
        if let Some(sender) = self.create_sender.read().clone() {
            let _ = sender.send(None);
        }
    }

    /// 销毁 canonical holder 中的物理连接。
    pub fn destroy_holder(&self, mut holder: DruidConnectionHolder) {
        let connection_id = holder.connection_id();
        let physical_age = holder.physical_age();
        holder.mark_discarded();
        holder.clear_statement_cache();
        if let Some(connection) = holder.take_physical_connection() {
            self.destroy_connection(connection_id, physical_age, connection);
        } else {
            self.record_destroy();
        }
    }

    /// 销毁连接。
    fn destroy_connection(
        &self,
        connection_id: u64,
        physical_age: Duration,
        connection: Box<dyn PhysicalConnection>,
    ) {
        self.record_destroy();
        let sender = self.close_sender.read().clone();
        if let Some(sender) = sender {
            let command = ConnectionCloseCommand {
                connection_id,
                physical_age,
                connection,
            };
            if let Err(error) = sender.send(Some(command)) {
                // worker 已退出时，Drop 仍会释放 driver 资源；禁止重新 spawn
                // 一条不可追踪任务。
                drop(error.0);
            }
        } else {
            // 只有直接构造公开 PoolInner 的测试/低层调用会走到这里；canonical
            // DruidPool 在对外可借用前必定安装 worker。
            drop(connection);
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
        let connection_id = holder.connection_id();
        let physical_age = holder.physical_age();
        holder.mark_discarded();
        holder.clear_statement_cache();
        self.record_destroy();
        if let Some(mut connection) = holder.take_physical_connection() {
            let filter_chain = self.filter_chain.read().clone();
            let result = match filter_chain {
                Some(filter_chain) if !filter_chain.is_empty() => {
                    filter_chain
                        .physical_connection_close(
                            self.factory.as_ref(),
                            &mut connection,
                            connection_id,
                            physical_age,
                        )
                        .await
                }
                _ => self.factory.close(&mut connection).await,
            };
            if let Err(error) = result {
                tracing::warn!(
                    %error,
                    connection_id,
                    "close physical connection immediately failed"
                );
            }
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
        let fatal = self.fatal_shrink_snapshot();
        let (evict_connections, keep_alive_connections) = {
            let mut queue = self.idle.lock();

            let check_count = queue.len().saturating_sub(self.config.min_idle);
            let mut retained = VecDeque::with_capacity(queue.len());
            let mut evicted = Vec::new();
            let mut keep_alive_candidates = Vec::new();
            let mut index = 0usize;

            while let Some(holder) = queue.pop_front() {
                let predates_fatal_error = fatal
                    .last_fatal_error_at
                    .is_some_and(|fatal_at| holder.created_at < fatal_at);
                if (fatal.on_fatal_error || fatal.fatal_error_increment > 0) && predates_fatal_error
                {
                    // Java 把 fatal 前建立的空闲连接放入 keepAliveConnections，
                    // 无论调用方 keepAlive 参数是否开启，都必须重新校验。
                    keep_alive_candidates.push(holder);
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
            if fatal.fatal_error_increment > 0 {
                self.request_fatal_error_refill();
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
                    Some(connection) => self.test_connection_internal(connection).await,
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
                self.record_pooling_count(queue.len());
                self.notify.notify_waiters();
            }
        }

        // Java keepAlive shrink 在驱逐或校验失败后会触发 emptySignal(fillCount)。
        // Rust 没有独立 creator 线程，直接异步补齐到 minIdle，创建预留仍由
        // ConnectionSlotReservation 保证错误与取消安全。
        if keep_alive && self.total_count.load(Ordering::Acquire) < self.config.min_idle {
            let _ = self.fill(self.config.min_idle).await;
        }
        if fatal.fatal_error_increment > 0 {
            self.request_fatal_error_refill();
        }
    }

    /// 关闭池。
    pub async fn close(&self) {
        self.enabled.store(false, Ordering::Release);
        self.close_time_millis
            .store(Self::now_millis(), Ordering::Release);
        self.closed.store(true, Ordering::Release);
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

    /// 等待关闭前已经进入物理 factory 的创建任务完成或取消。
    ///
    /// 创建任务自身在观察 closed 后负责关闭尚未注册的物理连接；只有全部退出后，
    /// close worker 才能安全接收 FIFO shutdown，restart 才能恢复下一代次。
    pub(crate) async fn wait_for_creators(&self) {
        loop {
            let notified = self.creating_idle.notified();
            if self.creating_count.load(Ordering::Acquire) == 0 {
                return;
            }
            notified.await;
        }
    }
}

impl FatalErrorHandler for PoolInner {
    fn handle_fatal_error(&self, error: &DruidError, sql: Option<&str>) -> bool {
        let mut state = self.fatal_error_state.lock();
        let now = Instant::now();
        state.last_fatal_error_at = Some(now);
        state.last_fatal_error_time_millis = Self::now_millis();
        state.fatal_error_count = state.fatal_error_count.wrapping_add(1);
        let increment = state
            .fatal_error_count
            .wrapping_sub(state.fatal_error_count_last_shrink);
        if increment > self.config.on_fatal_error_max_active {
            // Java 主动推进 lastShrink 一次，避免随后 shrink 重复触发同一轮
            // emptySignal。
            state.fatal_error_count_last_shrink =
                state.fatal_error_count_last_shrink.wrapping_add(1);
            state.on_fatal_error = true;
        } else {
            state.on_fatal_error = false;
        }
        state.last_fatal_error = Some(error.clone());
        state.last_fatal_error_sql = sql.map(|sql| {
            let mut code_units = sql.encode_utf16().collect::<Vec<_>>();
            if code_units.len() > 1024 {
                code_units.truncate(1024);
            }
            RdbcString::from_utf16(code_units)
        });
        state.on_fatal_error
    }

    fn request_fatal_error_refill(&self) {
        // Java `emptySignal()` 请求一次创建，不受 minIdle 是否为零影响。
        self.request_refill(1);
    }

    fn clear_on_fatal_error(&self) {
        PoolInner::clear_on_fatal_error(self);
    }
}

/// Java `DruidDataSource#getStatValueAndReset()` 的池级区间快照。
pub(crate) struct PoolStatSnapshot {
    pub(crate) active_count: usize,
    pub(crate) active_peak: usize,
    pub(crate) active_peak_time_millis: u64,
    pub(crate) pooling_count: usize,
    pub(crate) pooling_peak: usize,
    pub(crate) pooling_peak_time_millis: u64,
    pub(crate) connect_count: u64,
    pub(crate) close_count: u64,
    pub(crate) wait_thread_count: usize,
    pub(crate) not_empty_wait_count: u64,
    pub(crate) not_empty_wait_nanos: u64,
    pub(crate) logic_connect_error_count: u64,
    pub(crate) physical_connect_count: u64,
    pub(crate) physical_close_count: u64,
    pub(crate) physical_connect_error_count: u64,
    pub(crate) keep_alive_check_count: u64,
    pub(crate) pstmt_cache_hit_count: u64,
    pub(crate) pstmt_cache_miss_count: u64,
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

/// 统计正在进行的物理创建任务，并在最后一个任务退出时唤醒生命周期等待者。
struct CreatingTaskRegistration<'a> {
    creating_count: &'a AtomicUsize,
    creating_idle: &'a Notify,
}

impl<'a> CreatingTaskRegistration<'a> {
    fn new(creating_count: &'a AtomicUsize, creating_idle: &'a Notify) -> Self {
        creating_count.fetch_add(1, Ordering::AcqRel);
        Self {
            creating_count,
            creating_idle,
        }
    }
}

impl Drop for CreatingTaskRegistration<'_> {
    fn drop(&mut self) {
        if self.creating_count.fetch_sub(1, Ordering::AcqRel) == 1 {
            self.creating_idle.notify_waiters();
        }
    }
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
