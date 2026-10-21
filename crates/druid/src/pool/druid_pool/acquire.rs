use super::*;

impl DruidPool {
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

        self.install_create_worker().await;
        self.install_close_worker().await;
        let initial_result = self.inner.fill_initial().await;
        // Java 在同步初始建连失败且 initExceptionThrow=true 时，仍先启动
        // creator/destroy 线程并在 finally 中设置 inited=true，再把错误返回。
        self.initialized.store(true, Ordering::Release);
        self.start_maintenance().await;
        self.start_stat_publisher().await;
        initial_result
    }

    pub(crate) async fn start_maintenance(&self) {
        let period = self.inner.config.time_between_eviction_runs;
        if period.is_zero() {
            return;
        }
        let mut task = self.maintenance_task.lock().await;
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
    pub(crate) async fn start_stat_publisher(&self) {
        let interval = self.inner.config.stat_publish_interval;
        if interval.is_zero() {
            return;
        }
        let mut task = self.stat_publish_task.lock().await;
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
                // 发布协议是同步接口；隔离阻塞 sink，避免占住异步运行时 worker。
                let sink_for_publish = Arc::clone(&sink);
                match tokio::task::spawn_blocking(move || sink_for_publish.publish(&stat_value))
                    .await
                {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => {
                        tracing::warn!(%error, "publish datasource statistics failed");
                    }
                    Err(error) => {
                        tracing::warn!(%error, "datasource statistics publisher task failed");
                    }
                }
                tokio::select! {
                    () = shutdown.notified() => break,
                    () = tokio::time::sleep(interval) => {}
                }
            }
        }));
    }

    /// 启动每池唯一的受监管物理关闭 worker。
    pub(crate) async fn start_close_worker(&self) {
        let mut task = self.close_worker_task.lock().await;
        if task.is_some() {
            return;
        }
        if let Some(worker) = self.close_worker.lock().take() {
            *task = Some(worker.spawn());
        }
    }

    /// 启动每池唯一的受监管补池 worker。
    pub(crate) async fn start_create_worker(&self) {
        let mut task = self.create_worker_task.lock().await;
        if task.is_some() {
            return;
        }
        if let Some(worker) = self.create_worker.lock().take() {
            *task = Some(worker.spawn());
        }
    }

    /// 为首次 init 或 restart 后的新代次安装补池 worker。
    pub(crate) async fn install_create_worker(&self) {
        let has_task = self.create_worker_task.lock().await.is_some();
        if self.create_worker.lock().is_none() && !has_task {
            let (create_sender, create_receiver) = tokio::sync::watch::channel(Some(0));
            self.inner.install_create_sender(create_sender);
            *self.create_worker.lock() = Some(ConnectionCreateWorker::new(
                Arc::clone(&self.inner),
                create_receiver,
            ));
        }
        self.start_create_worker().await;
    }

    /// 为首次 init 或 restart 后的新代次安装唯一物理关闭 worker。
    pub(crate) async fn install_close_worker(&self) {
        let has_task = self.close_worker_task.lock().await.is_some();
        if self.close_worker.lock().is_none() && !has_task {
            let (close_sender, close_receiver) =
                tokio::sync::mpsc::channel(self.inner.config.close_queue_capacity.max(1));
            self.inner
                .close_worker_terminated
                .store(false, Ordering::Release);
            self.inner.install_close_sender(close_sender);
            *self.close_worker.lock() = Some(ConnectionCloseWorker::new(
                Arc::clone(&self.inner.factory),
                self.filter_chain.clone(),
                close_receiver,
                self.inner.config.close_timeout,
                Arc::clone(&self.inner.close_timeout_total),
                Arc::clone(&self.inner.close_worker_terminated),
            ));
        }
        self.start_close_worker().await;
    }

    pub async fn get_timeout(
        &self,
        timeout: Duration,
    ) -> Result<DruidPooledConnection, DruidError> {
        self.get_connection_with_max_wait(timeout).await
    }

    pub(crate) async fn get_with_timeout(
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

    pub(crate) async fn get_connection_internal(
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
            // 对应 Java: DruidDataSource#pollLast 取数组尾部，即“后归还者先复用”（LIFO）。
            // 该顺序决定热连接复用率、testWhileIdle 的实际校验对象与 shrink 的驱逐次序，
            // 不能退化为 FIFO；语义见 SEM-POOL-001。
            let (idle_connection, remaining_idle) = {
                let mut idle = self.inner.idle.lock();
                let connection = idle.pop_back();
                (connection, idle.len())
            };
            if let Some(holder) = idle_connection {
                let mut candidate = BorrowCandidate::new(Arc::clone(&self.inner), holder);
                let holder = candidate.holder_mut();
                if holder.user_password_version() < self.inner.user_password_version() {
                    continue;
                }
                let lifetime_expired = holder.connection_age() >= self.inner.config.max_lifetime;
                let idle_expired = remaining_idle >= self.inner.config.min_idle
                    && holder.idle_duration() >= self.inner.config.idle_timeout;
                if lifetime_expired || idle_expired {
                    continue;
                }
                if holder.raw_connection_is_unusable() {
                    continue;
                }
                let validate_on_borrow = self.inner.config.test_on_borrow
                    || (self.inner.config.test_while_idle
                        && holder.idle_duration() >= self.inner.config.time_between_eviction_runs);
                if validate_on_borrow {
                    let validation_failed = !self.inner.test_holder_internal(holder).await;
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
                    Ok(_) => {}
                    Err(_) => {
                        return Err(self.inner.connection_timeout_error(started_at.elapsed()));
                    }
                }
            } else {
                notify.await;
            }
        }
    }

    pub(crate) fn ensure_borrow_generation(&self, expected_generation: u64) -> Result<(), DruidError> {
        if self.inner.lifecycle_generation() != expected_generation {
            // Java 只在进入 getConnectionInternal 时检查 closed。已经进入
            // takeLast/pollLast 的线程被 close 唤醒后走 `!enable` 分支。
            return Err(DruidError::DataSourceDisabled);
        }
        self.inner.ensure_available()
    }

    /// 按 Java creator 的重试阈值创建物理连接，并受本次 maxWait 截止时间约束。
    pub(crate) async fn create_connection_until(
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

}
