use super::*;

impl PoolInner {
    /// 归还连接到空闲队列。
    pub fn return_connection(
        &self,
        holder: DruidConnectionHolder,
        disposition: ConnectionRecycleDisposition,
    ) -> bool {
        self.return_connection_with_overflow(holder, disposition).0
    }

    /// 显式归还时保留无法入队的 holder，供调用方在超时约束内直接关闭。
    pub(crate) fn return_connection_with_overflow(
        &self,
        mut holder: DruidConnectionHolder,
        disposition: ConnectionRecycleDisposition,
    ) -> (bool, Option<DruidConnectionHolder>) {
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
            return self.discard_recycled_holder_with_overflow(holder);
        }

        // Java closeCount 统计逻辑池化连接关闭，而不是物理 socket 关闭。
        self.close_count.fetch_add(1, Ordering::Relaxed);
        let holder_was_active = holder.mark_idle();

        if disposition.has_recycle_error() {
            self.recycle_error_count.fetch_add(1, Ordering::Relaxed);
        }
        if !holder_was_active || !disposition.is_reusable() {
            self.discard_count.fetch_add(1, Ordering::Relaxed);
            return self.discard_recycled_holder_with_overflow(holder);
        }

        let unusable = holder.raw_connection_is_unusable();
        if self.closed.load(Ordering::Acquire)
            || !self.enabled.load(Ordering::Acquire)
            || unusable
            || holder.is_discard()
        {
            return self.discard_recycled_holder_with_overflow(holder);
        }

        let connection_age = holder.connection_age();
        let lifetime_expired = connection_age >= self.config.max_lifetime;
        let driver_timeout_expired = self
            .config
            .effective_driver_connection_timeout()
            .is_some_and(|timeout| !timeout.is_zero() && connection_age > timeout);
        let max_use_count_reached =
            self.config.max_use_count > 0 && holder.use_count() >= self.config.max_use_count as u64;
        let credentials_stale = holder.user_password_version() < self.user_password_version();
        if lifetime_expired || driver_timeout_expired || max_use_count_reached || credentials_stale
        {
            self.discard_count.fetch_add(1, Ordering::Relaxed);
            return self.discard_recycled_holder_with_overflow(holder);
        }

        let mut return_completion = None;
        let returned = {
            let mut queue = self.idle.lock();
            // Java Druid 的 maxIdle 已 deprecated，真实 idle 容量由 maxActive
            // 限制；保留配置字段仅用于兼容读取，不能改变 putLast 语义。
            if queue.len() >= self.config.max_open {
                Err(holder)
            } else {
                // 锁内完成复用决定并移除本次通知，后续租约不会继承旧等待者。
                return_completion = holder.take_return_completion();
                // 对应 Java: DruidDataSource#putLast 追加到数组尾部；借用侧 pop_back，
                // 共同构成“后归还者先复用”的 LIFO 语义（SEM-POOL-001）。
                queue.push_back(holder);
                Ok(queue.len())
            }
        };

        // Java 在 putLast 尝试完成后递增 recycleCount，即使池满导致 putLast=false。
        self.recycle_count.fetch_add(1, Ordering::Relaxed);
        match returned {
            Err(holder) => self.discard_recycled_holder_with_overflow(holder),
            Ok(pooling_count) => {
                self.record_pooling_count(pooling_count);
                self.notify.notify_one();
                if let Some(completion) = return_completion {
                    // 完成通知在所有归还统计之后发出，跨线程等待者看到完整结果。
                    let _ = completion.send(());
                }
                (false, None)
            }
        }
    }

    /// Records one fatal discard before the holder can be transferred by its logical owner.
    pub(crate) fn signal_owner_discard(&self, disposition: &ConnectionRecycleDisposition) {
        // 先保留关闭 worker 的存活义务，再减少 active，避免 shutdown 将
        // “计数已释放”误认为底层 holder 已经完成转交。
        self.pending_owner_discards.fetch_add(1, Ordering::AcqRel);
        let was_active = self
            .active_count
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                (current > 0).then_some(current - 1)
            })
            .is_ok();
        if !was_active {
            self.pending_owner_discards.fetch_sub(1, Ordering::AcqRel);
            return;
        }
        self.close_count.fetch_add(1, Ordering::Relaxed);
        self.discard_count.fetch_add(1, Ordering::Relaxed);
        if disposition.has_recycle_error() {
            self.recycle_error_count.fetch_add(1, Ordering::Relaxed);
        }
        self.record_destroy();
        self.request_refill(self.config.min_idle);
    }

    /// Transfers and closes a holder whose pool accounting was completed by `signal_owner_discard`.
    pub(crate) fn finish_signaled_discard(&self, holder: DruidConnectionHolder) -> bool {
        drop(self.finish_signaled_discard_with_overflow(holder));
        false
    }

    /// 已记录致命丢弃的 holder 入队失败时返还所有权，供显式关闭路径处理。
    pub(crate) fn finish_signaled_discard_with_overflow(
        &self,
        mut holder: DruidConnectionHolder,
    ) -> Option<DruidConnectionHolder> {
        let _termination = CloseWorkerTerminationGuard { inner: self };
        let connection_id = holder.connection_id();
        let connection_age = holder.connection_age();
        holder.mark_discarded();
        let overflow = self.enqueue_close_holder(holder, connection_id, connection_age);
        // 真正的关闭命令已入队后，才允许最后一个租约触发 worker 退出。
        self.pending_owner_discards.fetch_sub(1, Ordering::AcqRel);
        overflow
    }

    pub(crate) fn discard_recycled_holder_with_overflow(
        &self,
        holder: DruidConnectionHolder,
    ) -> (bool, Option<DruidConnectionHolder>) {
        let overflow = self.destroy_holder_with_overflow(holder);
        (self.request_refill(self.config.min_idle), overflow)
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
        self.refill_target.store(target, Ordering::Release);
        self.create_sender
            .read()
            .clone()
            .is_some_and(|sender| sender.send(Some(target)).is_ok())
    }

    /// 请求 creator 在此前补池命令处理完成后退出。
    pub(crate) fn request_create_worker_shutdown(&self) {
        self.refill_target.store(0, Ordering::Release);
        if let Some(sender) = self.create_sender.read().clone() {
            let _ = sender.send(None);
        }
    }

    /// 销毁 canonical holder 中的 driver connection。
    pub fn destroy_holder(&self, holder: DruidConnectionHolder) {
        drop(self.destroy_holder_with_overflow(holder));
    }

    pub(crate) fn destroy_holder_with_overflow(
        &self,
        mut holder: DruidConnectionHolder,
    ) -> Option<DruidConnectionHolder> {
        let connection_id = holder.connection_id();
        let connection_age = holder.connection_age();
        holder.mark_discarded();
        self.record_destroy();
        self.enqueue_close_holder(holder, connection_id, connection_age)
    }

    /// 同步路径仅尝试有界入队；满队列时释放 owned holder，不派生后台任务。
    pub(crate) fn enqueue_close_holder(
        &self,
        holder: DruidConnectionHolder,
        connection_id: u64,
        connection_age: Duration,
    ) -> Option<DruidConnectionHolder> {
        let Some(sender) = self.close_sender.read().clone() else {
            self.close_queue_rejected_total
                .fetch_add(1, Ordering::Relaxed);
            return Some(holder);
        };
        let Ok(queue_permit) = Arc::clone(&self.close_queue_permits).try_acquire_owned() else {
            self.close_queue_rejected_total
                .fetch_add(1, Ordering::Relaxed);
            return Some(holder);
        };
        let command = ConnectionCloseCommand {
            connection_id,
            connection_age,
            holder: Box::new(holder),
            queue_permit,
        };
        if let Err(error) = sender.try_send(command) {
            self.close_queue_rejected_total
                .fetch_add(1, Ordering::Relaxed);
            return Some(*error.into_inner().holder);
        }
        None
    }

    /// 队列溢出时，在显式异步关闭调用者内尝试有时限的物理关闭。
    pub(crate) async fn close_overflow_holder_now(
        &self,
        mut holder: DruidConnectionHolder,
    ) -> Result<(), RdbcError> {
        let connection_id = holder.connection_id();
        match tokio::time::timeout(
            self.config.close_timeout,
            holder.close_standard_connection(),
        )
        .await
        {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => {
                tracing::warn!(%error, connection_id, "overflow connection close failed");
                Err(error.into())
            }
            Err(_) => {
                self.close_timeout_total.fetch_add(1, Ordering::Relaxed);
                tracing::warn!(connection_id, "overflow connection close timed out");
                holder.complete_return();
                Err(RdbcError::Other(
                    "physical connection close timed out".to_owned(),
                ))
            }
        }
    }

    /// 当池已关闭且最后一个活跃租约已归还时请求关闭 worker。
    pub(crate) fn request_close_worker_shutdown_if_idle(&self) {
        if !self.closed.load(Ordering::Acquire) || !self.close_worker_is_idle() {
            return;
        }
        // 关闭最后一个 sender 后，worker 会先排空已有命令再自然退出。
        self.close_sender.write().take();
    }

    /// 返回关闭 worker 是否已无待归还租约；结果用于决定能否发送退出命令。
    pub(crate) fn close_worker_is_idle(&self) -> bool {
        self.active_count.load(Ordering::Acquire) == 0
            && self.pending_owner_discards.load(Ordering::Acquire) == 0
    }

    pub(crate) fn record_destroy(&self) {
        let _ = self
            .total_count
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                (current > 0).then_some(current - 1)
            });
        self.destroy_count.fetch_add(1, Ordering::Relaxed);
    }

    /// 等待 holder 中的 driver connection 完成关闭。
    pub async fn destroy_holder_now(&self, mut holder: DruidConnectionHolder) {
        let connection_id = holder.connection_id();
        holder.mark_discarded();
        self.record_destroy();
        match tokio::time::timeout(
            self.config.close_timeout,
            holder.close_standard_connection(),
        )
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                tracing::warn!(%error, connection_id, "close canonical connection immediately failed");
            }
            Err(_) => {
                self.close_timeout_total.fetch_add(1, Ordering::Relaxed);
                tracing::warn!(
                    connection_id,
                    "close canonical connection immediately timed out"
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

                let driver_timeout_expired = self
                    .config
                    .effective_driver_connection_timeout()
                    .is_some_and(|timeout| !timeout.is_zero() && holder.connection_age() > timeout);
                if driver_timeout_expired {
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
                self.test_holder_internal(holder).await
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
