use super::*;

impl DruidPool {
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
            close_queue_depth: self.inner.config.close_queue_capacity.max(1)
                - self.inner.close_queue_permits.available_permits(),
            close_queue_rejected_total: self
                .inner
                .close_queue_rejected_total
                .load(Ordering::Relaxed),
            close_timeout_total: self.inner.close_timeout_total.load(Ordering::Relaxed),
            refill_target: self.inner.refill_target.load(Ordering::Relaxed),
            close_worker_terminated: self.inner.close_worker_terminated.load(Ordering::Acquire),
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
            driver_connect_error_count: self
                .inner
                .driver_connect_error_count
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
    /// [`super::super::DataSourceStatSink`]，而不是 Java logger。
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
    pub fn raw_driver(&self) -> &dyn ConnectionFactory {
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
                    "connectTimeMillis": holder.connection_age().as_millis(),
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

    /// 通知池：外部 `ConnectionFactory` 的连接凭据已经更新。
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

    /// 关闭并回收旧代次资源；返回是否已等到全部建连任务退出并完成清理。
    pub(crate) async fn close_resources(&self) -> bool {
        let deadline = tokio::time::Instant::now() + self.inner.config.shutdown_timeout;
        if !self.initialized.load(Ordering::Acquire) {
            return self.destroy_initialized_filters(deadline).await;
        }
        if !self.is_closed() {
            self.inner.set_enabled(false);
            self.inner.advance_lifecycle_generation();
        }
        self.start_create_worker().await;
        self.inner.request_create_worker_shutdown();
        self.start_close_worker().await;
        self.maintenance_shutdown.notify_one();
        self.stat_publish_shutdown.notify_one();
        // 先把池标记为 closed 并排空 idle，再等待维护任务退出。这样 close
        // future 在等待后台任务期间被取消时，新借用也已经被拒绝，且
        // PoolInner::close 的 DetachedHolder 会完成剩余资源计数清理。
        if !self.is_closed()
            && tokio::time::timeout_at(deadline, self.inner.close())
                .await
                .is_err()
        {
            tracing::warn!("pool idle connection shutdown exceeded configured deadline");
        }
        if !self
            .join_shutdown_task(&self.maintenance_task, "maintenance", deadline)
            .await
        {
            return false;
        }
        if !self
            .join_shutdown_task(&self.stat_publish_task, "stat publisher", deadline)
            .await
        {
            return false;
        }
        // 必须先等待旧代次 factory 调用完成。否则 close worker 可能已经退出，
        // restart 又恢复 enable，旧创建任务会把连接混入新代次或绕过 async close。
        if tokio::time::timeout_at(deadline, self.inner.wait_for_creators())
            .await
            .is_err()
        {
            // 不提前关闭 worker/Filter；旧代次 factory 完成后仍需能移交并关闭结果。
            tracing::warn!("pool creators exceeded configured shutdown deadline");
            return false;
        }
        if !self
            .join_shutdown_task(&self.create_worker_task, "creator worker", deadline)
            .await
        {
            return false;
        }
        self.inner.request_close_worker_shutdown_if_idle();
        if self.inner.close_worker_is_idle() && !self.join_close_worker(deadline).await {
            return false;
        }
        self.destroy_initialized_filters(deadline).await
    }

    pub(crate) async fn join_shutdown_task(
        &self,
        task: &AsyncMutex<Option<tokio::task::JoinHandle<()>>>,
        task_name: &str,
        deadline: tokio::time::Instant,
    ) -> bool {
        let mut task = task.lock().await;
        let Some(handle) = task.as_mut() else {
            return true;
        };
        match tokio::time::timeout_at(deadline, handle).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                tracing::warn!(%error, task_name, "pool shutdown task failed");
            }
            Err(_) => {
                tracing::warn!(task_name, "pool shutdown task exceeded configured deadline");
                return false;
            }
        }
        task.take();
        true
    }

    pub(crate) async fn destroy_initialized_filters(&self, deadline: tokio::time::Instant) -> bool {
        let mut task = self.filter_destroy_task.lock().await;
        if self
            .filters_initialized
            .compare_exchange(true, false, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            if let Some(filter_chain) = &self.filter_chain {
                let filter_chain = Arc::clone(filter_chain);
                *task = Some(tokio::spawn(async move {
                    filter_chain.destroy_filters().await;
                }));
            }
        }
        let Some(handle) = task.as_mut() else {
            return true;
        };
        if tokio::time::timeout_at(deadline, handle).await.is_err() {
            tracing::warn!("pool filter destroy exceeded configured shutdown deadline");
            return false;
        }
        task.take();
        true
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

        if !self.close_resources().await {
            return Err(DruidError::Other(
                "cannot restart while prior-generation shutdown work is still running".to_owned(),
            ));
        }
        if !self.drain_close_worker_if_idle().await {
            return Err(DruidError::Other(
                "cannot restart while prior-generation close worker is still running".to_owned(),
            ));
        }
        self.reset_stats();
        self.active_leases.clear();
        self.initialized.store(false, Ordering::Release);
        self.inner.prepare_restart();
        Ok(())
    }

    pub(crate) async fn drain_close_worker_if_idle(&self) -> bool {
        if !self.inner.close_worker_is_idle() {
            return false;
        }
        self.inner.request_close_worker_shutdown_if_idle();
        let deadline = tokio::time::Instant::now() + self.inner.config.shutdown_timeout;
        self.join_close_worker(deadline).await
    }

    pub(crate) async fn join_close_worker(&self, deadline: tokio::time::Instant) -> bool {
        let mut slot = self.close_worker_task.lock().await;
        let Some(task) = slot.as_mut() else {
            return true;
        };
        if tokio::time::timeout_at(deadline, &mut *task).await.is_err() {
            // 超时只结束本次等待；worker 继续持有已入队连接并完成关闭。
            // 保留 JoinHandle，后续 close/restart 可以继续等待同一项清理。
            tracing::warn!(
                "pool close worker exceeded configured shutdown deadline; cleanup continues"
            );
            return false;
        }
        slot.take();
        true
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

    pub(crate) fn wrap_connection(&self, holder: DruidConnectionHolder) -> DruidPooledConnection {
        let connection_id = holder.connection_id();
        let connection_properties = holder.connection_properties();
        let pool = self.inner.clone();
        let active_leases = Arc::clone(&self.active_leases);
        let disposition_signaled = Arc::new(AtomicBool::new(false));
        let signaled_for_observer = Arc::clone(&disposition_signaled);
        let active_for_observer = Arc::clone(&active_leases);
        let pool_for_observer = Arc::clone(&pool);
        let disposition_observer = Box::new(move |disposition| {
            signaled_for_observer.store(true, Ordering::Release);
            active_for_observer.remove(&connection_id);
            pool_for_observer.signal_owner_discard(&disposition);
        });
        let signaled_for_return = Arc::clone(&disposition_signaled);
        let return_connection = Box::new(move |holder, disposition| {
            active_leases.remove(&connection_id);
            if signaled_for_return.load(Ordering::Acquire) {
                pool.finish_signaled_discard(holder)
            } else {
                pool.return_connection(holder, disposition)
            }
        });
        let pool_for_explicit_return = Arc::clone(&self.inner);
        let active_for_explicit_return = Arc::clone(&self.active_leases);
        let signaled_for_explicit_return = Arc::clone(&disposition_signaled);
        let explicit_return = Box::new(move |holder, disposition| {
            active_for_explicit_return.remove(&connection_id);
            if signaled_for_explicit_return.load(Ordering::Acquire) {
                pool_for_explicit_return.finish_signaled_discard_with_overflow(holder)
            } else {
                pool_for_explicit_return
                    .return_connection_with_overflow(holder, disposition)
                    .1
            }
        });
        let mut connection = DruidPooledConnection::with_standard_holder_and_disposition_observer(
            holder,
            self.name.clone(),
            self.filter_chain.clone(),
            self.inner
                .config
                .keep_connection_underlying_transaction_isolation,
            self.inner.config.test_on_return,
            disposition_observer,
            return_connection,
        );
        connection.set_native_return_validation(Arc::clone(&self.inner));
        connection.set_native_explicit_return(explicit_return);
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
