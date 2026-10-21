use super::*;

impl PoolInner {
    pub fn new(
        factory: Arc<dyn ConnectionFactory>,
        config: super::super::config::PoolInnerConfig,
    ) -> Self {
        Self::new_with_stats(factory, config, Arc::new(StatsCollector::default()))
    }

    /// 使用数据源共享分层统计创建内部池状态。
    pub(crate) fn new_with_stats(
        factory: Arc<dyn ConnectionFactory>,
        config: super::super::config::PoolInnerConfig,
        stats_collector: Arc<StatsCollector>,
    ) -> Self {
        let close_queue_capacity = config.close_queue_capacity.max(1);
        Self {
            factory,
            filter_chain: parking_lot::RwLock::new(None),
            config,
            idle: parking_lot::Mutex::new(VecDeque::new()),
            notify: Notify::new(),
            active_count: AtomicUsize::new(0),
            pending_owner_discards: AtomicUsize::new(0),
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
            driver_connect_error_count: AtomicU64::new(0),
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
            close_queue_permits: Arc::new(Semaphore::new(close_queue_capacity)),
            close_queue_rejected_total: AtomicU64::new(0),
            close_timeout_total: Arc::new(AtomicU64::new(0)),
            close_worker_terminated: Arc::new(AtomicBool::new(false)),
            refill_target: AtomicUsize::new(0),
        }
    }

    /// 安装由 canonical `DruidPool` 持有的物理关闭 worker sender。
    pub(crate) fn install_close_sender(&self, sender: mpsc::Sender<ConnectionCloseCommand>) {
        *self.close_sender.write() = Some(sender);
    }

    /// 安装由 canonical `DruidPool` 持有的补池 worker sender。
    pub(crate) fn install_create_sender(&self, sender: watch::Sender<Option<usize>>) {
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
            create_error_count: self.driver_connect_error_count.load(Ordering::Acquire),
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

    pub(crate) fn record_create_error(&self, error: &DruidError) {
        let mut state = self.create_failure_state.lock();
        let now = Self::now_millis();
        state.create_error = Some(error.clone());
        state.last_create_error = Some(error.clone());
        state.last_create_error_time_millis = now;
    }

    pub(crate) fn record_create_success(&self) {
        {
            let mut state = self.create_failure_state.lock();
            state.create_error = None;
        }
        self.set_fail_continuous(false);
    }

    pub(crate) fn clear_on_fatal_error(&self) {
        let mut state = self.fatal_error_state.lock();
        if state.on_fatal_error {
            state.on_fatal_error = false;
        }
    }

    pub(crate) fn fatal_shrink_snapshot(&self) -> FatalShrinkSnapshot {
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

    pub(crate) fn record_peak(value: usize, peak: &AtomicUsize, peak_time_millis: &AtomicU64) {
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

    pub(crate) fn now_millis() -> u64 {
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
            driver_connect_count: self.create_count.swap(0, Ordering::AcqRel),
            driver_close_count: self.destroy_count.swap(0, Ordering::AcqRel),
            driver_connect_error_count: self.driver_connect_error_count.swap(0, Ordering::AcqRel),
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
        connection: &mut Box<dyn Connection>,
    ) -> Result<(), DruidError> {
        let result = if connection.is_closed()? {
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

    /// 对 holder 的实际 raw slot 执行同一 borrow/return/keep-alive 校验。
    pub(crate) async fn test_holder_internal(&self, holder: &mut DruidConnectionHolder) -> bool {
        let Some(connection) = holder.standard_connection_box_mut() else {
            return false;
        };
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

}
