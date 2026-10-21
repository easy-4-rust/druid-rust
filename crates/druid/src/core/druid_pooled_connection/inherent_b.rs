use super::*;

impl DruidPooledConnection {
    /// 返回底层物理连接的可变引用；连接已归还时返回 `None`。
    pub fn connection_holder(&self) -> Option<&DruidConnectionHolder> {
        self.holder.as_ref()
    }

    /// 返回当前连接 holder 的可变引用；连接归还后返回 `None`。
    ///
    /// 对应 Java 可通过 `getConnectionHolder()` 访问 holder 的可变对象语义。
    pub fn connection_holder_mut(&mut self) -> Option<&mut DruidConnectionHolder> {
        self.holder.as_mut()
    }

    /// 返回创建物理连接时采集的会话变量。
    ///
    /// 对应 Java：`DruidPooledConnection#getVariables()`。未启用
    /// `initVariants` 时返回 `None`，启用但数据库不是 `MySQL` 协议族时返回空表。
    #[must_use]
    pub fn variables(&self) -> Option<&HashMap<String, JsonValue>> {
        self.holder
            .as_ref()
            .and_then(DruidConnectionHolder::variables)
    }

    /// 返回创建物理连接时采集的全局变量。
    ///
    /// 对应 Java 历史拼写：`DruidPooledConnection#getGloablVariables()`。
    #[must_use]
    pub fn global_variables(&self) -> Option<&HashMap<String, JsonValue>> {
        self.holder
            .as_ref()
            .and_then(DruidConnectionHolder::global_variables)
    }

    /// Java `getGloablVariables` 拼写兼容别名。
    #[deprecated(note = "use global_variables")]
    #[must_use]
    pub fn gloabl_variables(&self) -> Option<&HashMap<String, JsonValue>> {
        self.global_variables()
    }

    /// 添加连接关闭/错误监听器。
    ///
    /// 对应 Java: `DruidPooledConnection#addConnectionEventListener`。
    pub fn add_connection_event_listener(
        &self,
        listener: Arc<dyn ConnectionEventListener>,
    ) -> Result<(), RdbcError> {
        let holder = self.holder.as_ref().ok_or(RdbcError::ConnectionDiscarded)?;
        holder.add_connection_event_listener(listener);
        Ok(())
    }

    /// 按对象身份移除连接监听器。
    ///
    /// 对应 Java: `DruidPooledConnection#removeConnectionEventListener`。
    pub fn remove_connection_event_listener(
        &self,
        listener: &Arc<dyn ConnectionEventListener>,
    ) -> Result<bool, RdbcError> {
        let holder = self.holder.as_ref().ok_or(RdbcError::ConnectionDiscarded)?;
        Ok(holder.remove_connection_event_listener(listener))
    }

    /// 添加 Statement 生命周期监听器。
    ///
    /// 对应 Java：`DruidPooledConnection#addStatementEventListener`。
    pub fn add_statement_event_listener(
        &self,
        listener: Arc<dyn StatementEventListener>,
    ) -> Result<(), RdbcError> {
        let holder = self.holder.as_ref().ok_or(RdbcError::ConnectionDiscarded)?;
        holder.add_statement_event_listener(listener);
        Ok(())
    }

    /// 按对象身份移除 Statement 生命周期监听器。
    ///
    /// 对应 Java：`DruidPooledConnection#removeStatementEventListener`。
    pub fn remove_statement_event_listener(
        &self,
        listener: &Arc<dyn StatementEventListener>,
    ) -> Result<bool, RdbcError> {
        let holder = self.holder.as_ref().ok_or(RdbcError::ConnectionDiscarded)?;
        Ok(holder.remove_statement_event_listener(listener))
    }

    /// 从当前 holder 移除一个已关闭的逻辑 Statement。
    ///
    /// 对应 Java：`DruidPooledConnection`/`DruidPooledStatement#close()` 完成后
    /// 调用 `DruidConnectionHolder#removeTrace`。
    pub fn recycle(mut self) {
        let disposition = self.drop_disposition();
        self.recycle_once(disposition);
    }

    /// 强制丢弃当前租约并返回是否已请求 creator 补池。
    ///
    /// 对应 Java `DruidDataSource#discardConnection(Connection/
    /// DruidConnectionHolder)` 的最终处置语义。该入口幂等；第一次调用关闭
    /// statement trace、清理 listener、递减 active、增加 discard，并把 holder
    /// 交给受监管 close worker。容量低于 minIdle 时返回 `true`。
    pub fn discard_connection(&mut self) -> bool {
        if let Some(holder) = self.holder.as_mut() {
            holder.mark_discarded();
        }
        self.recycle_once(ConnectionRecycleDisposition::discard())
    }

    pub(crate) fn standard_mut(
        &mut self,
    ) -> Result<&mut (dyn druid_rdbc::connection::Connection + 'static), RdbcError> {
        if !self.lease_active.load(Ordering::Acquire) {
            if let Some(holder) = self.holder.as_mut() {
                holder.mark_discarded();
            }
            if let Some(error) = self
                .standard_policy
                .as_ref()
                .and_then(|policy| policy.fatal_discard_error())
            {
                return Err(error);
            }
            return Err(RdbcError::ConnectionLeaked {
                id: self.id,
                held_for: self.borrowed_at.elapsed(),
            });
        }
        self.holder
            .as_mut()
            .and_then(DruidConnectionHolder::standard_connection_mut)
            .ok_or(RdbcError::ConnectionDiscarded)
    }

    pub(crate) fn standard_ref(
        &self,
    ) -> Result<&(dyn druid_rdbc::connection::Connection + 'static), RdbcError> {
        if !self.lease_active.load(Ordering::Acquire) {
            if let Some(error) = self
                .standard_policy
                .as_ref()
                .and_then(|policy| policy.fatal_discard_error())
            {
                return Err(error);
            }
            return Err(RdbcError::ConnectionLeaked {
                id: self.id,
                held_for: self.borrowed_at.elapsed(),
            });
        }
        self.holder
            .as_ref()
            .and_then(DruidConnectionHolder::standard_connection)
            .ok_or(RdbcError::ConnectionDiscarded)
    }

    pub(crate) fn current_catalog(&self) -> Option<String> {
        self.holder
            .as_ref()
            .and_then(DruidConnectionHolder::standard_connection)
            .and_then(|connection| connection.catalog().ok().flatten())
    }

    pub(crate) async fn invalidate_prepared_statement_cache(&mut self) -> Result<(), RdbcError> {
        let result = match self.holder.as_mut() {
            Some(holder) => holder.clear_standard_statement_pool().await,
            None => Err(RdbcError::ConnectionDiscarded),
        };
        self.classify_result(result)
    }

    pub(crate) fn recycle_once(&mut self, disposition: ConnectionRecycleDisposition) -> bool {
        let holder = self.take_recycled_holder(&disposition);
        self.native_explicit_return.take();
        if let (Some(holder), Some(return_connection)) = (holder, self.return_connection.take()) {
            return return_connection(holder, disposition);
        }
        false
    }

    pub(crate) fn take_recycled_holder(
        &mut self,
        disposition: &ConnectionRecycleDisposition,
    ) -> Option<DruidConnectionHolder> {
        if self.recycled {
            return None;
        }
        // 致命处置须先通知 owner，再移交真实 holder；一次性控制避免重复记账。
        self.signal_owner_discard(disposition);
        self.lease_active.store(false, Ordering::Release);
        self.recycled = true;
        let holder = self.holder.take()?;
        holder.clear_connection_event_listeners();
        holder.clear_statement_event_listeners();
        Some(holder)
    }

    /// Signals a non-reusable owner disposition without transferring the holder.
    ///
    /// Fatal operation paths call this before returning their error; close/Drop
    /// later call `recycle_once`, where the same one-shot control prevents pool
    /// accounting and external poison from being emitted twice.
    pub(crate) fn signal_owner_discard(
        &mut self,
        disposition: &ConnectionRecycleDisposition,
    ) -> bool {
        if disposition.is_reusable() {
            return false;
        }
        if let Some(holder) = self.holder.as_mut() {
            holder.mark_discarded();
        }
        self.lease_active.store(false, Ordering::Release);
        self.owner_disposition.signal_discard(disposition.clone())
    }

    pub(crate) fn notify_connection_closed(&mut self) {
        if self.connection_closed_notified {
            return;
        }
        self.connection_closed_notified = true;
        if let Some(holder) = self.holder.as_ref() {
            for listener in holder.connection_event_listeners() {
                if catch_unwind(AssertUnwindSafe(|| listener.connection_closed(self.id))).is_err() {
                    tracing::error!(
                        connection_id = self.id,
                        "connection-closed listener panicked; continuing connection cleanup"
                    );
                }
            }
        }
    }

    pub(crate) fn notify_connection_error(&self, error: &RdbcError) {
        let product_error = DruidError::from(error.clone());
        if let Some(holder) = self.holder.as_ref() {
            for listener in holder.connection_event_listeners() {
                if catch_unwind(AssertUnwindSafe(|| {
                    listener.connection_error_occurred(self.id, &product_error);
                }))
                .is_err()
                {
                    tracing::error!(
                        connection_id = self.id,
                        "connection-error listener panicked; preserving database error handling"
                    );
                }
            }
        }
    }

    pub(crate) fn drop_disposition(&mut self) -> ConnectionRecycleDisposition {
        if !self.lease_active.load(Ordering::Acquire) {
            if let Some(holder) = self.holder.as_mut() {
                holder.mark_discarded();
            }
            return ConnectionRecycleDisposition::discard();
        }
        let Some(holder) = self.holder.as_mut() else {
            return ConnectionRecycleDisposition::discard();
        };
        let defaults = holder.defaults().clone();
        if self
            .standard_policy
            .as_ref()
            .is_some_and(|policy| policy.is_discarded())
            || self.client_info_touched
            || holder.has_open_standard_resources()
            || holder.standard_connection().is_none_or(|connection| {
                connection.is_closed().unwrap_or(true)
                    || defaults
                        .standard_needs_reset(
                            connection,
                            self.keep_underlying_transaction_isolation,
                        )
                        .unwrap_or(true)
                    || self.standard_test_on_return
            })
        {
            holder.mark_discarded();
            return ConnectionRecycleDisposition::discard();
        }
        ConnectionRecycleDisposition::Reusable
    }

    pub(crate) async fn prepare_for_recycle(&mut self) -> ConnectionRecycleDisposition {
        if !self.lease_active.load(Ordering::Acquire) {
            if let Some(holder) = self.holder.as_mut() {
                holder.mark_discarded();
            }
            return ConnectionRecycleDisposition::discard();
        }
        self.prepare_standard_for_recycle().await
    }

    /// 首次修改前捕获客户端属性；旧驱动不提供快照时允许原操作，但归还只能丢弃。
    pub(crate) async fn prepare_client_info_mutation(&mut self) -> Result<(), RdbcError> {
        if self.client_info_touched {
            return Ok(());
        }
        let result = self.standard_mut()?.client_info_properties().await;
        self.initial_client_info = match result {
            Ok(properties) => Some(properties),
            Err(error)
                if error.kind() == druid_rdbc::error::RdbcErrorKind::SqlFeatureNotSupported
                    || matches!(error, RdbcError::UnsupportedOperation { .. }) =>
            {
                None
            }
            Err(error) => return self.classify_result::<(), _>(Err(error)),
        };
        // 必须先标记再进入驱动；Future 被取消或驱动部分成功也不能遗漏复位。
        self.client_info_touched = true;
        Ok(())
    }

    pub(crate) async fn reset_client_info_for_return(&mut self) -> Result<(), RdbcError> {
        if !self.client_info_touched {
            return Ok(());
        }
        let original = self
            .initial_client_info
            .clone()
            .ok_or(RdbcError::ConnectionDiscarded)?;
        self.standard_mut()?
            .set_client_info_properties(Some(&original))
            .await?;
        self.client_info_touched = false;
        self.initial_client_info = None;
        Ok(())
    }

    pub(crate) async fn prepare_standard_for_recycle(&mut self) -> ConnectionRecycleDisposition {
        let reset_result = async {
            let resources = self
                .holder
                .as_ref()
                .ok_or(RdbcError::ConnectionDiscarded)?
                .standard_resource_registry();
            resources.close_all().await?;
            if self
                .standard_policy
                .as_ref()
                .is_some_and(|policy| policy.is_discarded())
            {
                return Err(RdbcError::ConnectionDiscarded);
            }
            let defaults = self
                .holder
                .as_ref()
                .ok_or(RdbcError::ConnectionDiscarded)?
                .defaults()
                .clone();
            let (supports_transactions, auto_commit, read_only) = {
                let connection = self.standard_ref()?;
                (
                    connection.capabilities().transactions,
                    if defaults.standard_auto_commit().is_some() {
                        Some(connection.auto_commit()?)
                    } else {
                        None
                    },
                    if defaults.standard_read_only().is_some() {
                        Some(connection.read_only()?)
                    } else {
                        None
                    },
                )
            };
            if supports_transactions && auto_commit.is_none() {
                return Err(RdbcError::UnsupportedOperation {
                    operation: "connection_auto_commit",
                });
            }
            if supports_transactions && auto_commit == Some(false) && read_only != Some(true) {
                self.before_connection_event(&ConnectionEvent::Rollback)
                    .await?;
                self.standard_mut()?.rollback().await?;
            }
            self.reset_client_info_for_return().await?;
            self.holder
                .as_mut()
                .ok_or(RdbcError::ConnectionDiscarded)?
                .reset(self.keep_underlying_transaction_isolation)
                .await?;
            Ok::<(), RdbcError>(())
        }
        .await;
        if let Err(error) = reset_result {
            return self.discard_for_recycle_error(error);
        }

        if self.standard_test_on_return {
            let validation = match self.native_return_validation.clone() {
                Some(pool)
                    if pool.config.valid_connection_checker.is_some()
                        || pool.config.validation_query.is_some() =>
                {
                    match self
                        .holder
                        .as_mut()
                        .and_then(DruidConnectionHolder::standard_connection_box_mut)
                    {
                        Some(connection) => pool
                            .validate_connection(connection)
                            .await
                            .map(|()| true)
                            .map_err(RdbcError::from),
                        None => Err(RdbcError::ConnectionDiscarded),
                    }
                }
                // 未配置 checker/query 时仍执行标准驱动校验，不能把配置路径的空操作当成有效。
                Some(_) | None => match self.standard_mut() {
                    Ok(connection) => connection.is_valid(1).await,
                    Err(error) => Err(error),
                },
            };
            if !matches!(validation, Ok(true)) {
                if let Some(holder) = self.holder.as_mut() {
                    holder.mark_discarded();
                }
                return ConnectionRecycleDisposition::discard();
            }
            if let Some(holder) = self.holder.as_ref() {
                holder.record_valid();
            }
        }

        let restore_result = match self.holder.as_mut() {
            Some(holder) if holder.should_restore_schema_on_recycle() => {
                holder.restore_initial_schema().await
            }
            _ => Ok(()),
        };
        if let Err(error) = restore_result {
            return self.discard_for_recycle_error(error.into());
        }
        ConnectionRecycleDisposition::Reusable
    }

    /// `dataSource_releaseConnection` around-chain 的末端回收动作。
    ///
    /// 该方法不再次进入 Filter，避免递归；回收错误按 Java
    /// `DruidDataSource#recycle` 规则折叠为 discard disposition。
    pub(crate) async fn recycle_from_data_source_filter(&mut self) -> Result<(), RdbcError> {
        if self.recycled {
            return self.await_return_completion().await;
        }
        let disposition = self.prepare_for_recycle().await;
        if self.owner_managed && disposition.is_reusable() {
            // 逻辑句柄的 filter release 已完成；holder 仍由标准 owner 持有，
            // 不更新 native pool 的 active/idle 记账，也不触发 native return callback。
            self.finish_owner_logical_lease();
            return Ok(());
        }
        if self.native_return_validation.is_some() {
            if let Some(holder) = self.holder.as_mut() {
                self.pending_return_completion = Some(holder.begin_return_completion());
            }
        }
        // 注册通知到转移 holder 之间没有 await，取消不会留下无主的连接。
        let close_result = if let Some(return_connection) = self.native_explicit_return.take() {
            let holder = self.take_recycled_holder(&disposition);
            self.return_connection.take();
            let overflow = holder.and_then(|holder| return_connection(holder, disposition));
            if let (Some(pool), Some(overflow)) = (self.native_return_validation.clone(), overflow)
            {
                pool.close_overflow_holder_now(overflow).await
            } else {
                Ok(())
            }
        } else {
            self.recycle_once(disposition);
            Ok(())
        };
        self.close_count = self.close_count.saturating_add(1);
        self.end_transaction_info();
        self.await_return_completion().await?;
        close_result
    }

    /// 等待原生 holder 归还或关闭完成；取消只结束等待，不接管驱动所有权。
    pub(crate) async fn await_return_completion(&mut self) -> Result<(), RdbcError> {
        let Some(completion) = self.pending_return_completion.as_mut() else {
            return Ok(());
        };
        let result = completion.await.map_err(|_| {
            RdbcError::Other("native connection return completion was lost".to_owned())
        });
        self.pending_return_completion = None;
        result
    }

    pub(crate) fn discard_for_recycle_error(
        &mut self,
        error: RdbcError,
    ) -> ConnectionRecycleDisposition {
        if let Some(holder) = self.holder.as_mut() {
            holder.mark_discarded();
        }
        ConnectionRecycleDisposition::recycle_error(error.into())
    }

    pub(crate) async fn before_connection_event(
        &mut self,
        event: &ConnectionEvent,
    ) -> Result<(), RdbcError> {
        match &self.filter_chain {
            Some(filter_chain) => filter_chain
                .before_connection_event_with_identity(self.id, event)
                .await
                .map_err(RdbcError::from),
            None => Ok(()),
        }
    }

    pub(crate) async fn after_connection_event(
        &mut self,
        event: &ConnectionEvent,
        started_at: Instant,
    ) -> Result<(), RdbcError> {
        let Some(filter_chain) = self.filter_chain.clone() else {
            return Ok(());
        };
        let result = filter_chain
            .after_connection_event_with_identity(self.id, event, started_at.elapsed())
            .await
            .map_err(RdbcError::from);
        self.classify_result(result)
    }

    pub(crate) async fn close_logical_once(&mut self) -> Result<(), RdbcError> {
        if self.recycled {
            return self.await_return_completion().await;
        }

        self.notify_connection_closed();
        let filter_chain = self.filter_chain.clone();
        let release_result = match filter_chain {
            Some(filter_chain) if !filter_chain.is_empty() => filter_chain
                .data_source_release_connection(self)
                .await
                .map_err(RdbcError::from),
            _ => self.recycle_from_data_source_filter().await,
        };
        if let Err(error) = release_result {
            if !self.recycled {
                self.connection_closed_notified = false;
            }
            return Err(error);
        }
        if !self.recycled && self.owner_managed {
            self.finish_owner_logical_lease();
        } else if !self.recycled {
            self.lease_active.store(false, Ordering::Release);
            self.recycled = true;
            self.return_connection.take();
        }
        Ok(())
    }

    /// 返回 canonical connection warning chain。
    pub async fn warnings(&mut self) -> Result<Option<SqlWarning>, RdbcError> {
        let chain = self.filter_chain.clone();
        match chain {
            Some(chain) if chain.before_count() > 0 => chain
                .connection_get_warnings(self.standard_mut()?)
                .await
                .map_err(RdbcError::from),
            _ => self.standard_mut()?.warnings().await,
        }
    }

    /// 清除连接警告链。
    ///
    /// 对应 Java：`DruidPooledConnection#clearWarnings()`。与 getter 不同，
    /// Java 捕获底层 `SQLException` 并交给 `handleException`，Rust 因此必须
    /// 执行 fatal sorter。
    pub async fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        let chain = self.filter_chain.clone();
        let result = match chain {
            Some(chain) if chain.before_count() > 0 => chain
                .connection_clear_warnings(self.standard_mut()?)
                .await
                .map_err(RdbcError::from),
            _ => self.standard_mut()?.clear_warnings().await,
        };
        self.classify_result(result)
    }
}
