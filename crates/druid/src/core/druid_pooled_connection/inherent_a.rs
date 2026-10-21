use super::*;

impl DruidPooledConnection {
    /// RDBC 不支持事务隔离。
    pub const TRANSACTION_NONE: i32 = 0;
    /// RDBC `READ_UNCOMMITTED`。
    pub const TRANSACTION_READ_UNCOMMITTED: i32 = 1;
    /// RDBC `READ_COMMITTED`。
    pub const TRANSACTION_READ_COMMITTED: i32 = 2;
    /// RDBC `REPEATABLE_READ`。
    pub const TRANSACTION_REPEATABLE_READ: i32 = 4;
    /// RDBC `SERIALIZABLE`。
    pub const TRANSACTION_SERIALIZABLE: i32 = 8;

    /// Creates a logical connection from one canonical holder.
    pub fn with_holder(
        holder: DruidConnectionHolder,
        data_source: String,
        filter_chain: Option<Arc<FilterChain>>,
        keep_underlying_transaction_isolation: bool,
        return_connection: DruidConnectionReturnCallback,
    ) -> Self {
        Self::with_holder_and_observer(
            holder,
            data_source,
            filter_chain,
            keep_underlying_transaction_isolation,
            Box::new(|_| {}),
            return_connection,
        )
    }

    pub(crate) fn with_holder_and_observer(
        mut holder: DruidConnectionHolder,
        data_source: String,
        filter_chain: Option<Arc<FilterChain>>,
        keep_underlying_transaction_isolation: bool,
        disposition_observer: DruidConnectionDispositionObserver,
        return_connection: DruidConnectionReturnCallback,
    ) -> Self {
        let id = holder.connection_id();
        let lease_active = Arc::new(AtomicBool::new(true));
        let execution_running = Arc::new(AtomicBool::new(false));
        let resource_generation = if let Ok(generation) = holder.begin_standard_lease() {
            generation
        } else {
            holder.mark_discarded();
            holder.standard_resource_registry().current_generation()
        };
        let owner_disposition = Arc::new(OwnerDispositionControl::new(disposition_observer));
        let standard_policy = Some(StandardLogicalPolicy::new(
            id,
            data_source.clone(),
            Arc::clone(&lease_active),
            Arc::clone(&execution_running),
            filter_chain.clone(),
            holder.defaults().standard_auto_commit(),
            resource_generation,
            Arc::clone(&owner_disposition),
        ));
        if let Some(policy) = standard_policy.as_ref() {
            policy.set_statement_event_listeners(holder.statement_event_listeners_handle());
        }
        let connected_time_millis = now_millis().saturating_sub(
            holder
                .created_at
                .elapsed()
                .as_millis()
                .try_into()
                .unwrap_or(u64::MAX),
        );
        Self {
            holder: Some(holder),
            id,
            data_source,
            filter_chain,
            keep_underlying_transaction_isolation,
            standard_test_on_return: false,
            native_return_validation: None,
            pending_return_completion: None,
            exception_sorter: None,
            fatal_error_handler: None,
            return_connection: Some(return_connection),
            native_explicit_return: None,
            owner_disposition,
            lease_active,
            execution_running,
            standard_policy,
            owner_managed: false,
            borrowed_at: Instant::now(),
            recycled: false,
            connection_closed_notified: false,
            stats_collector: None,
            transaction_info: None,
            query_timeout: 0,
            transaction_query_timeout: 0,
            statement_id_seed: Arc::new(AtomicU64::new(20_000)),
            result_set_id_seed: Arc::new(AtomicU64::new(50_000)),
            metadata_id_seed: Arc::new(AtomicU64::new(80_000)),
            connection_properties: Arc::new(HashMap::new()),
            connected_time_millis,
            close_count: 0,
            last_validate_time_millis: 0,
            attributes: ProxyAttributes::default(),
            initial_client_info: None,
            client_info_touched: false,
        }
    }

    /// 使用 canonical standard holder 创建 native-pool logical connection。
    #[allow(clippy::too_many_arguments)]
    pub fn with_standard_holder(
        holder: DruidConnectionHolder,
        data_source: String,
        filter_chain: Option<Arc<FilterChain>>,
        keep_underlying_transaction_isolation: bool,
        test_on_return: bool,
        return_connection: DruidConnectionReturnCallback,
    ) -> Self {
        let mut connection = Self::with_holder(
            holder,
            data_source,
            filter_chain,
            keep_underlying_transaction_isolation,
            return_connection,
        );
        connection.standard_test_on_return = test_on_return;
        connection
    }

    /// Creates a logical connection with an immediate one-shot discard observer.
    ///
    /// The observer updates pool accounting or external-pool poison synchronously; the real holder
    /// remains owned by this connection and is transferred exactly once during close or Drop.
    #[allow(clippy::too_many_arguments)]
    pub fn with_standard_holder_and_disposition_observer(
        holder: DruidConnectionHolder,
        data_source: String,
        filter_chain: Option<Arc<FilterChain>>,
        keep_underlying_transaction_isolation: bool,
        test_on_return: bool,
        disposition_observer: DruidConnectionDispositionObserver,
        return_connection: DruidConnectionReturnCallback,
    ) -> Self {
        let mut connection = Self::with_holder_and_observer(
            holder,
            data_source,
            filter_chain,
            keep_underlying_transaction_isolation,
            disposition_observer,
            return_connection,
        );
        connection.standard_test_on_return = test_on_return;
        connection
    }

    /// 从 bb8/deadpool 等 external standard lease 创建 logical connection。
    ///
    /// external owner 的 holder 永远不会进入 native pool callback。
    #[allow(clippy::too_many_arguments)]
    pub async fn with_external_standard_connection(
        connection: Box<dyn druid_rdbc::connection::Connection>,
        id: u64,
        data_source: String,
        filter_chain: Option<Arc<FilterChain>>,
        external_return: StandardConnectionReturnCallback,
        native_return: DruidConnectionReturnCallback,
    ) -> Result<Self, DruidError> {
        let holder =
            DruidConnectionHolder::with_external_connection(connection, id, Duration::ZERO, 0)
                .await?;
        Ok(Self::with_standard_holder(
            holder,
            data_source,
            filter_chain,
            false,
            false,
            Box::new(move |mut holder, disposition| {
                if holder.is_external_lease() {
                    if let Some(connection) = holder.take_standard_connection() {
                        external_return(connection, disposition);
                    }
                    false
                } else {
                    native_return(holder, disposition)
                }
            }),
        ))
    }

    /// 为 native pool 归还安装与创建、借出和保活相同的校验策略。
    /// 参数为实际池状态；不改变 external lease 的驱动校验合同。
    pub(crate) fn set_native_return_validation(&mut self, pool: Arc<crate::pool::PoolInner>) {
        self.native_return_validation = Some(pool);
    }

    /// 安装原生池显式归还回调，保留关闭队列溢出的 holder 所有权。
    pub(crate) fn set_native_explicit_return(
        &mut self,
        return_connection: NativeExplicitReturnCallback,
    ) {
        self.native_explicit_return = Some(return_connection);
    }

    /// 返回连接 ID。
    pub fn id(&self) -> u64 {
        self.id
    }

    /// 重置池化连接持有时间起点。
    ///
    /// 对应 Java：`DruidPooledConnection#setConnectedTimeNano()`，由
    /// `StatFilter#dataSource_getConnection` 在下游成功返回后调用。
    pub fn set_connected_time_nano(&mut self) {
        self.borrowed_at = Instant::now();
    }

    /// 返回从 `StatFilter` 记录的连接获取时刻起经过的时间。
    #[must_use]
    pub fn connection_hold_duration(&self) -> Duration {
        self.borrowed_at.elapsed()
    }

    /// 返回 Connection proxy attribute 数量。
    #[must_use]
    pub fn attributes_size(&self) -> usize {
        self.attributes.len()
    }

    /// 清空 Connection proxy attributes。
    pub fn clear_attributes(&self) {
        self.attributes.clear();
    }

    /// 返回 Connection proxy attributes 快照。
    #[must_use]
    pub fn attributes(&self) -> HashMap<String, ProxyAttributeValue> {
        self.attributes.snapshot()
    }

    /// 返回指定 Connection proxy attribute。
    #[must_use]
    pub fn attribute(&self, key: &str) -> Option<ProxyAttributeValue> {
        self.attributes.get(key)
    }

    /// 保存或覆盖 Connection proxy attribute。
    pub fn put_attribute(
        &self,
        key: impl Into<String>,
        value: ProxyAttributeValue,
    ) -> Option<ProxyAttributeValue> {
        self.attributes.put(key, value)
    }

    pub(crate) fn set_proxy_id_seeds(
        &mut self,
        statement_id_seed: Arc<AtomicU64>,
        result_set_id_seed: Arc<AtomicU64>,
        metadata_id_seed: Arc<AtomicU64>,
    ) {
        self.statement_id_seed = statement_id_seed;
        self.result_set_id_seed = result_set_id_seed;
        self.metadata_id_seed = metadata_id_seed;
    }

    /// 设置创建物理连接时使用的属性快照。
    pub(crate) fn set_connection_properties(
        &mut self,
        connection_properties: Arc<HashMap<String, String>>,
    ) {
        self.connection_properties = connection_properties;
    }

    pub(crate) fn next_statement_id(&self) -> u64 {
        self.statement_id_seed.fetch_add(1, Ordering::AcqRel)
    }

    /// 返回底层物理连接创建时间的 Unix epoch 毫秒值。
    #[must_use]
    pub const fn connected_time_millis(&self) -> u64 {
        self.connected_time_millis
    }

    /// 返回物理连接创建边界的逻辑驱动属性。
    ///
    /// 对应 Java：`ConnectionProxy#getProperties()`。Rust 暴露共享只读快照，
    /// 保留键和值但不允许某个租约修改数据源后续连接的驱动参数。
    #[must_use]
    pub fn properties(&self) -> &HashMap<String, String> {
        self.connection_properties.as_ref()
    }

    /// 返回本逻辑连接成功关闭的次数。
    #[must_use]
    pub const fn close_count(&self) -> u64 {
        self.close_count
    }

    /// 返回最近一次成功验证连接的 Unix epoch 毫秒值。
    #[must_use]
    pub const fn last_validate_time_millis(&self) -> u64 {
        self.last_validate_time_millis
    }

    /// 显式设置最近验证时间。
    pub fn set_last_validate_time_millis(&mut self, last_validate_time_millis: u64) {
        self.last_validate_time_millis = last_validate_time_millis;
    }

    /// 返回当前活动事务信息。
    #[must_use]
    pub fn transaction_info(&self) -> Option<Arc<TransactionInfo>> {
        self.transaction_info.clone()
    }

    pub(crate) fn end_transaction_info(&mut self) {
        if let Some(transaction_info) = self.transaction_info.take() {
            transaction_info.set_end_time_millis_now();
        }
    }

    /// 返回数据源名称。
    pub fn data_source(&self) -> &str {
        &self.data_source
    }

    /// 返回连接是否已经归还。
    pub fn is_recycled(&self) -> bool {
        self.recycled
    }

    /// 返回池管理器使用的租约存活令牌。
    ///
    /// Rust-only 内部适配：`removeAbandoned` 仅使令牌失效，不跨线程取得
    /// canonical connection 独占视图，从而避免 Java 强制关闭方式在 Rust 中造成别名。
    pub(crate) fn lease_active_token(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.lease_active)
    }

    /// 返回池管理器使用的执行中令牌。
    pub(crate) fn execution_running_token(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.execution_running)
    }

    /// 将 native pool checkout 转为标准 owner 管理的 holder。
    pub(crate) fn prepare_for_pooled_owner(&mut self) {
        self.owner_managed = true;
        self.lease_active.store(false, Ordering::Release);
        self.execution_running.store(false, Ordering::Release);
        self.standard_policy = None;
    }

    /// 将 owner 保留的已复位 holder 通过一次性 native callback 归还池。
    pub(crate) fn return_from_pooled_owner(&mut self) {
        if self.client_info_touched {
            self.discard_from_pooled_owner();
            return;
        }
        self.owner_managed = false;
        self.recycle_once(ConnectionRecycleDisposition::Reusable);
    }

    /// 复位失败时丢弃 owner 持有的 holder，避免污染 native idle pool。
    pub(crate) fn discard_from_pooled_owner(&mut self) {
        self.owner_managed = false;
        if let Some(holder) = self.holder.as_mut() {
            holder.mark_discarded();
        }
        self.recycle_once(ConnectionRecycleDisposition::discard());
    }

    /// 为标准 owner 开启一个新的逻辑租约；旧租约的 policy generation 保持失效。
    pub(crate) fn activate_owner_logical_lease(&mut self) -> Result<(), RdbcError> {
        if !self.owner_managed
            || self.recycled
            || self.owner_disposition.is_signaled()
            || self.client_info_touched
        {
            return Err(RdbcError::ConnectionDiscarded);
        }
        let holder = self.holder.as_ref().ok_or(RdbcError::ConnectionDiscarded)?;
        let resource_generation = holder.begin_standard_lease()?;
        self.lease_active.store(true, Ordering::Release);
        self.execution_running.store(false, Ordering::Release);
        let policy = StandardLogicalPolicy::new(
            self.id,
            self.data_source.clone(),
            Arc::clone(&self.lease_active),
            Arc::clone(&self.execution_running),
            self.filter_chain.clone(),
            holder.defaults().standard_auto_commit(),
            resource_generation,
            Arc::clone(&self.owner_disposition),
        );
        policy.set_statement_event_listeners(holder.statement_event_listeners_handle());
        if let Some(stats_collector) = self.stats_collector.as_ref() {
            policy.set_stats_collector(Arc::clone(stats_collector));
        }
        if let Some(exception_sorter) = self.exception_sorter.as_ref() {
            policy.set_exception_sorter(Arc::clone(exception_sorter));
        }
        if let Some(fatal_error_handler) = self.fatal_error_handler.as_ref() {
            policy.set_fatal_error_handler(Arc::clone(fatal_error_handler));
        }
        self.standard_policy = Some(policy);
        self.connection_closed_notified = false;
        self.borrowed_at = Instant::now();
        Ok(())
    }

    /// 完成 owner 管理的逻辑租约关闭，但保留池 holder 供 owner 再次借出。
    pub(crate) fn finish_owner_logical_lease(&mut self) {
        self.lease_active.store(false, Ordering::Release);
        self.execution_running.store(false, Ordering::Release);
        self.standard_policy = None;
        self.connection_closed_notified = false;
        self.end_transaction_info();
    }

    /// 装配数据源的异常分类器。
    ///
    /// 对应 Java：`DruidAbstractDataSource#setExceptionSorter(ExceptionSorter)`。
    /// 分类器命中 fatal SQL 异常时，当前 holder 与物理 Adapter 会立即标记为
    /// discard，之后的 close/Drop 只能进入销毁分支。
    pub fn set_exception_sorter(&mut self, exception_sorter: Arc<dyn ExceptionSorter>) {
        if let Some(policy) = &self.standard_policy {
            policy.set_exception_sorter(Arc::clone(&exception_sorter));
        }
        self.exception_sorter = Some(exception_sorter);
    }

    /// 使用链式形式装配数据源异常分类器。
    #[must_use]
    pub fn with_exception_sorter(mut self, exception_sorter: Arc<dyn ExceptionSorter>) -> Self {
        self.set_exception_sorter(exception_sorter);
        self
    }

    /// 装配数据源级 fatal-error 状态处理器。
    pub(crate) fn set_fatal_error_handler(
        &mut self,
        fatal_error_handler: Arc<dyn FatalErrorHandler>,
    ) {
        if let Some(policy) = &self.standard_policy {
            policy.set_fatal_error_handler(Arc::clone(&fatal_error_handler));
        }
        self.fatal_error_handler = Some(fatal_error_handler);
    }

    /// 装配数据源级运行统计；native pool 对外连接必须共享同一实例。
    pub(crate) fn set_stats_collector(&mut self, stats_collector: Arc<StatsCollector>) {
        if let Some(policy) = &self.standard_policy {
            policy.set_stats_collector(Arc::clone(&stats_collector));
        }
        self.stats_collector = Some(stats_collector);
    }

    /// 装配数据源级普通/事务查询超时。
    pub(crate) fn set_query_timeouts(
        &mut self,
        query_timeout: i32,
        transaction_query_timeout: i32,
    ) {
        self.query_timeout = query_timeout;
        self.transaction_query_timeout = transaction_query_timeout;
    }

    pub(crate) fn effective_query_timeout(&self) -> i32 {
        if !self.current_auto_commit() && self.transaction_query_timeout > 0 {
            self.transaction_query_timeout
        } else {
            self.query_timeout
        }
    }

    pub(crate) fn current_auto_commit(&self) -> bool {
        self.standard_ref()
            .and_then(druid_rdbc::connection::Connection::auto_commit)
            .unwrap_or(true)
    }

    /// 处理驱动错误并返回它是否使连接不可复用。
    ///
    /// 对应 Java：
    /// `DruidDataSource#handleConnectionException` → `handleFatalError`。
    /// 非 SQL 错误和未命中 sorter 的 SQL 错误保持连接可复用。
    pub fn handle_exception(&mut self, error: &DruidError) -> bool {
        let error = RdbcError::from(error.clone());
        self.handle_exception_with_sql(&error, None)
    }

    /// 处理标准 RDBC 错误，返回该错误是否使连接不可复用。
    /// 参数 `error` 为标准错误；返回值表示是否已触发连接丢弃。
    pub fn handle_rdbc_exception(&mut self, error: &RdbcError) -> bool {
        self.handle_exception_with_sql(error, None)
    }

    pub(crate) fn handle_exception_with_sql(
        &mut self,
        error: &RdbcError,
        sql: Option<&str>,
    ) -> bool {
        let fatal = self
            .exception_sorter
            .as_ref()
            .zip(error.sql_exception())
            .is_some_and(|(exception_sorter, exception)| {
                exception_sorter.is_exception_fatal(exception)
            });
        if fatal {
            if let Some(policy) = &self.standard_policy {
                policy.remember_fatal_error(error);
            }
            let fatal_error_handler = self.fatal_error_handler.clone();
            let on_fatal_error = fatal_error_handler.as_ref().is_some_and(|handler| {
                let product_error = DruidError::from(error.clone());
                handler.handle_fatal_error(&product_error, sql)
            });
            // Pool accounting/poison is synchronous, but ownership of the real
            // holder stays with this logical connection until close/Drop. The
            // later transfer observes the one-shot signal and cannot account a
            // second discard.
            self.signal_owner_discard(&ConnectionRecycleDisposition::discard());
            if on_fatal_error {
                if let Some(handler) = fatal_error_handler {
                    handler.request_fatal_error_refill();
                }
            }
        }
        fatal
    }

    /// 对数据库操作结果执行统一异常分类，并原样保留成功值或错误。
    ///
    /// 对应 Java：`DruidPooledConnection#handleException(Throwable, String)`。
    /// 所有可能产生 `SQLException` 的物理连接和过滤器调用都必须经过本入口，
    /// 使 fatal 判断不会只覆盖 SQL execute/query。
    pub(crate) fn classify_result<T, E>(&mut self, result: Result<T, E>) -> Result<T, RdbcError>
    where
        E: Into<RdbcError>,
    {
        self.classify_result_with_sql(result, None)
    }

    /// 对数据库操作结果执行异常分类，并保留触发错误的 SQL。
    pub(crate) fn classify_result_with_sql<T, E>(
        &mut self,
        result: Result<T, E>,
        sql: Option<&str>,
    ) -> Result<T, RdbcError>
    where
        E: Into<RdbcError>,
    {
        let result = result.map_err(Into::into);
        if let Err(error) = &result {
            if error.sql_exception().is_some() {
                self.notify_connection_error(error);
            }
            self.handle_exception_with_sql(error, sql);
        }
        result
    }

    /// Completes a result already classified by [`StandardLogicalPolicy`].
    ///
    /// The shared policy owns sorter/filter/stat classification for direct canonical execution.
    /// When it marks the lease fatal, this owner records the shared disposition while retaining
    /// the real holder until close/Drop; re-running the sorter here would duplicate fatal-handler
    /// notifications.
    pub(crate) fn finish_standard_policy_result<T>(
        &mut self,
        policy: &StandardLogicalPolicy,
        result: Result<T, RdbcError>,
    ) -> Result<T, RdbcError> {
        if let Err(error) = &result {
            if error.sql_exception().is_some() {
                self.notify_connection_error(error);
            }
        }
        if policy.is_discarded() {
            self.signal_owner_discard(&ConnectionRecycleDisposition::discard());
        }
        result
    }
}
