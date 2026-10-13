//! 对外池化连接。

use super::connection_event_listener::ConnectionEventListener;
use super::connection_recycle_disposition::ConnectionRecycleDisposition;
use super::database_meta_data_proxy_impl::DatabaseMetaDataProxyImpl;
use super::druid_connection_holder::DruidConnectionHolder;
use super::druid_execution_filter_policy::DruidExecutionFilterPolicy;
use super::druid_pooled_callable_statement::DruidPooledCallableStatement;
use super::druid_pooled_prepared_statement::DruidPooledPreparedStatement;
use super::druid_pooled_statement::DruidPooledStatement;
use super::error::{DruidError, RdbcError};
use super::exception_sorter::ExceptionSorter;
use super::fatal_error_handler::FatalErrorHandler;
use super::filter::ConnectionEvent;
use super::filter_chain::FilterChain;
use super::owner_disposition_control::OwnerDispositionControl;
use super::prepared_statement_key::PreparedStatementKey;
use super::prepared_statement_method_type::PreparedStatementMethodType;
use super::proxy_attributes::{ProxyAttributeValue, ProxyAttributes};
use super::sql_warning::SqlWarning;
use super::standard_logical_policy::StandardLogicalPolicy;
use super::statement_event_listener::StatementEventListener;
use super::transaction_info::TransactionInfo;
use crate::stats::StatsCollector;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::time::{SystemTime, UNIX_EPOCH};

/// Native pool 回收完整 holder 的一次性回调。
pub type DruidConnectionReturnCallback =
    Box<dyn FnOnce(DruidConnectionHolder, ConnectionRecycleDisposition) -> bool + Send>;

type NativeExplicitReturnCallback = Box<
    dyn FnOnce(DruidConnectionHolder, ConnectionRecycleDisposition) -> Option<DruidConnectionHolder>
        + Send,
>;

#[cfg(test)]
mod result_set_error_lease_tests;

/// Synchronous one-shot discard notification, separate from the later holder return.
pub type DruidConnectionDispositionObserver =
    Box<dyn FnOnce(ConnectionRecycleDisposition) + Send + 'static>;

/// 外部连接池 canonical standard lease 的一次性归还回调。
pub type StandardConnectionReturnCallback = Box<
    dyn FnOnce(Box<dyn druid_rdbc::connection::Connection>, ConnectionRecycleDisposition) + Send,
>;

/// 对外池化连接。
///
/// 对应 Java: `com.alibaba.druid.pool.DruidPooledConnection`。
/// 该对象拥有一次连接租约，对外暴露连接语义；底层只依赖
/// canonical [`druid_rdbc::connection::Connection`]。显式关闭和 `Drop` 都通过同一条
/// 回收路径，并由 take-once 回调保证 driver connection 最多归还一次。显式异步关闭执行 Java
/// `DruidDataSource#recycle` 的事务回滚、状态复位与 return validation；
/// Drop 只复用无需异步复位的干净连接。
pub struct DruidPooledConnection {
    holder: Option<DruidConnectionHolder>,
    id: u64,
    data_source: String,
    filter_chain: Option<Arc<FilterChain>>,
    keep_underlying_transaction_isolation: bool,
    standard_test_on_return: bool,
    native_return_validation: Option<Arc<crate::pool::PoolInner>>,
    /// 显式归还的完成等待端；取消 future 后仍保留，支持再次关闭继续等待。
    pending_return_completion: Option<tokio::sync::oneshot::Receiver<()>>,
    exception_sorter: Option<Arc<dyn ExceptionSorter>>,
    fatal_error_handler: Option<Arc<dyn FatalErrorHandler>>,
    return_connection: Option<DruidConnectionReturnCallback>,
    native_explicit_return: Option<NativeExplicitReturnCallback>,
    owner_disposition: Arc<OwnerDispositionControl>,
    lease_active: Arc<AtomicBool>,
    execution_running: Arc<AtomicBool>,
    standard_policy: Option<Arc<StandardLogicalPolicy>>,
    borrowed_at: Instant,
    recycled: bool,
    connection_closed_notified: bool,
    stats_collector: Option<Arc<StatsCollector>>,
    transaction_info: Option<Arc<TransactionInfo>>,
    query_timeout: i32,
    transaction_query_timeout: i32,
    statement_id_seed: Arc<AtomicU64>,
    result_set_id_seed: Arc<AtomicU64>,
    metadata_id_seed: Arc<AtomicU64>,
    connection_properties: Arc<HashMap<String, String>>,
    connected_time_millis: u64,
    close_count: u64,
    last_validate_time_millis: u64,
    attributes: ProxyAttributes,
}

#[allow(clippy::missing_fields_in_debug)]
impl std::fmt::Debug for DruidPooledConnection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DruidPooledConnection")
            .field("id", &self.id)
            .field("data_source", &self.data_source)
            .field(
                "holder",
                &self
                    .holder
                    .as_ref()
                    .is_some_and(DruidConnectionHolder::has_standard_connection),
            )
            .field(
                "has_connection",
                &self
                    .holder
                    .as_ref()
                    .is_some_and(DruidConnectionHolder::has_standard_connection),
            )
            .field("filter_chain", &self.filter_chain.is_some())
            .field(
                "keep_underlying_transaction_isolation",
                &self.keep_underlying_transaction_isolation,
            )
            .field("exception_sorter", &self.exception_sorter.is_some())
            .field("fatal_error_handler", &self.fatal_error_handler.is_some())
            .field("return_connection", &self.return_connection.is_some())
            .field(
                "owner_disposition_signaled",
                &self.owner_disposition.is_signaled(),
            )
            .field("lease_active", &self.lease_active.load(Ordering::Acquire))
            .field(
                "execution_running",
                &self.execution_running.load(Ordering::Acquire),
            )
            .field("borrowed_at", &self.borrowed_at)
            .field("recycled", &self.recycled)
            .field(
                "connection_closed_notified",
                &self.connection_closed_notified,
            )
            .finish()
    }
}

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

    fn with_holder_and_observer(
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

    fn next_statement_id(&self) -> u64 {
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

    fn end_transaction_info(&mut self) {
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

    fn effective_query_timeout(&self) -> i32 {
        if !self.current_auto_commit() && self.transaction_query_timeout > 0 {
            self.transaction_query_timeout
        } else {
            self.query_timeout
        }
    }

    fn current_auto_commit(&self) -> bool {
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

    fn handle_exception_with_sql(&mut self, error: &RdbcError, sql: Option<&str>) -> bool {
        let fatal = self
            .exception_sorter
            .as_ref()
            .zip(error.sql_exception())
            .is_some_and(|(exception_sorter, exception)| {
                exception_sorter.is_exception_fatal(exception)
            });
        if fatal {
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
    fn finish_standard_policy_result<T>(
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

    fn standard_mut(
        &mut self,
    ) -> Result<&mut (dyn druid_rdbc::connection::Connection + 'static), RdbcError> {
        if !self.lease_active.load(Ordering::Acquire) {
            if let Some(holder) = self.holder.as_mut() {
                holder.mark_discarded();
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

    fn standard_ref(
        &self,
    ) -> Result<&(dyn druid_rdbc::connection::Connection + 'static), RdbcError> {
        if !self.lease_active.load(Ordering::Acquire) {
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

    fn current_catalog(&self) -> Option<String> {
        self.holder
            .as_ref()
            .and_then(DruidConnectionHolder::standard_connection)
            .and_then(|connection| connection.catalog().ok().flatten())
    }

    async fn invalidate_prepared_statement_cache(&mut self) -> Result<(), RdbcError> {
        let result = match self.holder.as_mut() {
            Some(holder) => holder.clear_standard_statement_pool().await,
            None => Err(RdbcError::ConnectionDiscarded),
        };
        self.classify_result(result)
    }

    fn recycle_once(&mut self, disposition: ConnectionRecycleDisposition) -> bool {
        let holder = self.take_recycled_holder(&disposition);
        self.native_explicit_return.take();
        if let (Some(holder), Some(return_connection)) = (holder, self.return_connection.take()) {
            return return_connection(holder, disposition);
        }
        false
    }

    fn take_recycled_holder(
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
    fn signal_owner_discard(&mut self, disposition: &ConnectionRecycleDisposition) -> bool {
        if disposition.is_reusable() {
            return false;
        }
        if let Some(holder) = self.holder.as_mut() {
            holder.mark_discarded();
        }
        self.lease_active.store(false, Ordering::Release);
        self.owner_disposition.signal_discard(disposition.clone())
    }

    fn notify_connection_closed(&mut self) {
        if self.connection_closed_notified {
            return;
        }
        self.connection_closed_notified = true;
        if let Some(holder) = self.holder.as_ref() {
            for listener in holder.connection_event_listeners() {
                listener.connection_closed(self.id);
            }
        }
    }

    fn notify_connection_error(&self, error: &RdbcError) {
        let product_error = DruidError::from(error.clone());
        if let Some(holder) = self.holder.as_ref() {
            for listener in holder.connection_event_listeners() {
                listener.connection_error_occurred(self.id, &product_error);
            }
        }
    }

    fn drop_disposition(&mut self) -> ConnectionRecycleDisposition {
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

    async fn prepare_for_recycle(&mut self) -> ConnectionRecycleDisposition {
        if !self.lease_active.load(Ordering::Acquire) {
            if let Some(holder) = self.holder.as_mut() {
                holder.mark_discarded();
            }
            return ConnectionRecycleDisposition::discard();
        }
        self.prepare_standard_for_recycle().await
    }

    async fn prepare_standard_for_recycle(&mut self) -> ConnectionRecycleDisposition {
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
    async fn await_return_completion(&mut self) -> Result<(), RdbcError> {
        let Some(completion) = self.pending_return_completion.as_mut() else {
            return Ok(());
        };
        let result = completion.await.map_err(|_| {
            RdbcError::Other("native connection return completion was lost".to_owned())
        });
        self.pending_return_completion = None;
        result
    }

    fn discard_for_recycle_error(&mut self, error: RdbcError) -> ConnectionRecycleDisposition {
        if let Some(holder) = self.holder.as_mut() {
            holder.mark_discarded();
        }
        ConnectionRecycleDisposition::recycle_error(error.into())
    }

    async fn before_connection_event(&mut self, event: &ConnectionEvent) -> Result<(), RdbcError> {
        match &self.filter_chain {
            Some(filter_chain) => filter_chain
                .before_connection_event_with_identity(self.id, event)
                .await
                .map_err(RdbcError::from),
            None => Ok(()),
        }
    }

    async fn after_connection_event(
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

    async fn close_logical_once(&mut self) -> Result<(), RdbcError> {
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
        if !self.recycled {
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

druid_rdbc::impl_standard_wrapper!(DruidPooledConnection => Connection);

#[async_trait::async_trait]
impl druid_rdbc::connection::Connection for DruidPooledConnection {
    async fn exec(
        &mut self,
        sql: &str,
        params: Vec<druid_rdbc::types::Value>,
    ) -> Result<druid_rdbc::statement::ExecResult, RdbcError> {
        let policy = self
            .standard_policy
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clone();
        let driver_params = params.clone();
        let connection = self.standard_mut()?;
        let result =
            policy
                .execute(
                    sql,
                    params,
                    0,
                    super::ExecOperation::Update,
                    move |effective_sql| async move {
                        connection.exec(&effective_sql, driver_params).await
                    },
                    |result| *result,
                )
                .await;
        self.finish_standard_policy_result(policy.as_ref(), result)
    }

    async fn fetch(
        &mut self,
        sql: &str,
        params: Vec<druid_rdbc::types::Value>,
    ) -> Result<Vec<druid_rdbc::types::Row>, RdbcError> {
        let policy = self
            .standard_policy
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clone();
        let driver_params = params.clone();
        let connection = self.standard_mut()?;
        let result = policy
            .execute(
                sql,
                params,
                0,
                super::ExecOperation::Query,
                move |effective_sql| async move {
                    connection.fetch(&effective_sql, driver_params).await
                },
                |rows| druid_rdbc::statement::ExecResult {
                    row_count: Some(u64::try_from(rows.len()).unwrap_or(u64::MAX)),
                    ..druid_rdbc::statement::ExecResult::default()
                },
            )
            .await;
        self.finish_standard_policy_result(policy.as_ref(), result)
    }

    async fn exec_batch(
        &mut self,
        batch: Vec<(String, Vec<druid_rdbc::types::Value>)>,
    ) -> Result<Vec<i64>, RdbcError> {
        let result = self.standard_mut()?.exec_batch(batch).await;
        self.classify_result(result)
    }

    async fn create_statement(
        &mut self,
        options: druid_rdbc::statement::StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::Statement>, RdbcError> {
        let resources = self
            .holder
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .standard_resource_registry();
        let policy = self
            .standard_policy
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clone();
        let admission = resources.begin_admission(policy.resource_generation())?;
        let result = self.standard_mut()?.create_statement(options).await;
        let statement = self.classify_result(result)?;
        let statement_id = self.next_statement_id();
        let statement = DruidPooledStatement::new_standard(
            statement,
            options,
            Arc::clone(&resources),
            policy,
            self.id,
            statement_id,
            Arc::clone(&self.result_set_id_seed),
            Arc::clone(&self.metadata_id_seed),
            Arc::clone(&self.lease_active),
            self.filter_chain.clone(),
            self.stats_collector.clone(),
        );
        resources
            .register(&admission, Arc::clone(&statement.inner))
            .await?;
        let query_timeout = self.effective_query_timeout();
        if query_timeout > 0 {
            let result = statement.configure_standard_query_timeout(query_timeout);
            self.classify_result(result)?;
        }
        let event_result = DruidExecutionFilterPolicy::complete_statement_creation(
            self.filter_chain.as_deref(),
            self.stats_collector.as_ref(),
            self.id,
            statement_id,
            &super::StatementEvent::CreateStatement,
        )
        .await;
        self.classify_result(event_result)?;
        Ok(Box::new(statement))
    }

    async fn prepare_statement(
        &mut self,
        sql: &str,
        options: druid_rdbc::statement::StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::PreparedStatement>, RdbcError> {
        self.prepare_statement_with_generated_keys(
            sql,
            options,
            druid_rdbc::statement::GeneratedKeys::None,
        )
        .await
    }

    async fn prepare_statement_with_generated_keys(
        &mut self,
        sql: &str,
        options: druid_rdbc::statement::StatementOptions,
        generated_keys: druid_rdbc::statement::GeneratedKeys,
    ) -> Result<Box<dyn druid_rdbc::statement::PreparedStatement>, RdbcError> {
        let effective_sql =
            DruidExecutionFilterPolicy::prepare_statement_sql(self.filter_chain.as_deref(), sql)?;
        // 对应 Java prepareStatement 重载：请求模式与列顺序均参与私有缓存身份。
        let method_type = match &generated_keys {
            druid_rdbc::statement::GeneratedKeys::None => PreparedStatementMethodType::M1,
            druid_rdbc::statement::GeneratedKeys::ReturnGeneratedKeys => {
                PreparedStatementMethodType::M6
            }
            druid_rdbc::statement::GeneratedKeys::ColumnIndexes(_) => {
                PreparedStatementMethodType::M4
            }
            druid_rdbc::statement::GeneratedKeys::ColumnNames(_) => PreparedStatementMethodType::M5,
            _ => {
                return Err(RdbcError::UnsupportedOperation {
                    operation: "connection_prepare_statement_generated_keys",
                })
            }
        };
        let key = PreparedStatementKey::standard(
            &effective_sql,
            self.current_catalog(),
            method_type,
            options,
            generated_keys.clone(),
        )?;
        let cache = self
            .holder
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .standard_statement_pool();
        let cache_stats = Arc::clone(
            self.holder
                .as_ref()
                .ok_or(RdbcError::ConnectionDiscarded)?
                .prepared_statement_stats(),
        );
        let resources = self
            .holder
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .standard_resource_registry();
        let policy = self
            .standard_policy
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clone();
        let admission = resources.begin_admission(policy.resource_generation())?;
        let (cached, cache_enabled) = {
            let mut cache = cache.lock().await;
            let enabled = cache.is_enabled();
            (cache.take(&key), enabled)
        };
        let cache_hit = cached.is_some();
        let statement = if let Some(statement) = cached {
            cache_stats.record_cache_checkout();
            cache_stats.record_hit();
            statement
        } else {
            let result = self
                .standard_mut()?
                .prepare_statement_with_generated_keys(&effective_sql, options, generated_keys)
                .await;
            let statement = self.classify_result_with_sql(result, Some(&effective_sql))?;
            cache_stats.record_prepare();
            if cache_enabled {
                cache_stats.record_miss();
            }
            statement
        };
        let statement_id = self.next_statement_id();
        let statement = DruidPooledPreparedStatement::new_standard(
            statement,
            &effective_sql,
            options,
            key,
            cache,
            cache_stats,
            cache_hit,
            resources,
            &admission,
            policy,
            self.id,
            statement_id,
            Arc::clone(&self.result_set_id_seed),
            Arc::clone(&self.metadata_id_seed),
            Arc::clone(&self.lease_active),
            self.filter_chain.clone(),
            self.stats_collector.clone(),
        )
        .await?;
        let query_timeout = self.effective_query_timeout();
        if query_timeout > 0 {
            let result = statement.configure_standard_query_timeout(query_timeout);
            self.classify_result_with_sql(result, Some(&effective_sql))?;
        }
        let event_result = DruidExecutionFilterPolicy::complete_statement_creation(
            self.filter_chain.as_deref(),
            self.stats_collector.as_ref(),
            self.id,
            statement_id,
            &super::StatementEvent::PrepareStatement(effective_sql),
        )
        .await;
        self.classify_result(event_result)?;
        Ok(Box::new(statement))
    }

    async fn prepare_call(
        &mut self,
        sql: &str,
        options: druid_rdbc::statement::StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::CallableStatement>, RdbcError> {
        let effective_sql =
            DruidExecutionFilterPolicy::prepare_statement_sql(self.filter_chain.as_deref(), sql)?;
        let method_type = if options == druid_rdbc::statement::StatementOptions::default() {
            PreparedStatementMethodType::Precall1
        } else {
            PreparedStatementMethodType::Precall2
        };
        let key = PreparedStatementKey::standard(
            &effective_sql,
            self.current_catalog(),
            method_type,
            options,
            druid_rdbc::statement::GeneratedKeys::None,
        )?;
        let holder = self.holder.as_ref().ok_or(RdbcError::ConnectionDiscarded)?;
        let cache = holder.standard_statement_pool();
        let cache_stats = Arc::clone(holder.prepared_statement_stats());
        let resources = self
            .holder
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .standard_resource_registry();
        let policy = self
            .standard_policy
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clone();
        let admission = resources.begin_admission(policy.resource_generation())?;
        let (cached, cache_enabled) = {
            let mut cache = cache.lock().await;
            (cache.take_callable(&key), cache.is_enabled())
        };
        let cache_hit = cached.is_some();
        let statement = if let Some(statement) = cached {
            cache_stats.record_cache_checkout();
            cache_stats.record_hit();
            statement
        } else {
            let result = self
                .standard_mut()?
                .prepare_call(&effective_sql, options)
                .await;
            let statement = self.classify_result_with_sql(result, Some(&effective_sql))?;
            cache_stats.record_prepare();
            if cache_enabled {
                cache_stats.record_miss();
            }
            statement
        };
        let statement_id = self.next_statement_id();
        let statement = DruidPooledCallableStatement::new_standard(
            statement,
            &effective_sql,
            options,
            key,
            cache,
            cache_stats,
            cache_hit,
            resources,
            &admission,
            policy,
            self.id,
            statement_id,
            Arc::clone(&self.result_set_id_seed),
            Arc::clone(&self.metadata_id_seed),
            Arc::clone(&self.lease_active),
            self.filter_chain.clone(),
            self.stats_collector.clone(),
        )
        .await?;
        let query_timeout = self.effective_query_timeout();
        if query_timeout > 0 {
            let result = statement.configure_standard_query_timeout(query_timeout);
            self.classify_result_with_sql(result, Some(&effective_sql))?;
        }
        let event_result = DruidExecutionFilterPolicy::complete_statement_creation(
            self.filter_chain.as_deref(),
            self.stats_collector.as_ref(),
            self.id,
            statement_id,
            &super::StatementEvent::PrepareCall(effective_sql),
        )
        .await;
        self.classify_result(event_result)?;
        Ok(Box::new(statement))
    }

    async fn database_meta_data(
        &mut self,
    ) -> Result<Box<dyn druid_rdbc::metadata::DatabaseMetaData>, RdbcError> {
        let filter_chain = self.filter_chain.clone();
        if let Some(filter_chain) = &filter_chain {
            let result = filter_chain
                .standard_connection_meta_data_before(self.id)
                .await;
            self.classify_result(result)?;
        }
        let raw_result = self.standard_mut()?.database_meta_data().await;
        let filter_result = raw_result
            .as_ref()
            .map(|_| ())
            .map_err(|error| DruidError::from(error.clone()));
        let raw = self.classify_result(raw_result);
        if let Some(filter_chain) = &filter_chain {
            let after = filter_chain
                .standard_connection_meta_data_after(self.id, &filter_result)
                .await;
            self.classify_result(after)?;
        }
        let raw = raw?;
        let resources = self
            .holder
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .standard_resource_registry();
        let policy = self
            .standard_policy
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clone();
        Ok(Box::new(DatabaseMetaDataProxyImpl::new_standard(
            raw,
            self.id,
            Arc::clone(&self.lease_active),
            Arc::clone(&self.result_set_id_seed),
            resources,
            policy,
            self.filter_chain.clone(),
            self.stats_collector.clone(),
        )))
    }

    /// 创建受当前租约监管的标准 Blob，返回独占资源或驱动错误。
    /// 对应 Java: DruidPooledConnection#createBlob。
    async fn create_blob(&mut self) -> Result<Box<dyn druid_rdbc::resource::Blob>, RdbcError> {
        self.standard_ref()?;
        let registry = self
            .holder
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .standard_resource_registry();
        let policy = self
            .standard_policy
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clone();
        policy.ensure_open()?;
        let admission = registry.begin_admission(policy.resource_generation())?;
        let raw_result = self.standard_mut()?.create_blob().await;
        let raw = self.classify_result(raw_result)?;
        // 驱动返回后直到 registry 接管前不再 await，取消不会丢失已创建资源。
        let id = super::resource_id::ResourceId::local();
        let owner = Arc::new(super::druid_resource_owner::DruidResourceOwner::new(
            id.clone(),
            super::resource_kind::ResourceKind::Blob,
            policy,
        ));
        let context = Arc::new(super::resource_context::ResourceContext::new(
            id,
            super::resource_kind::ResourceKind::Blob,
            super::resource_runtime_capabilities::ResourceRuntimeCapabilities::large_object(),
            owner,
        ));
        let resource =
            super::druid_blob::DruidBlob::register(raw, context, registry, &admission).await?;
        Ok(Box::new(resource))
    }

    /// 创建受当前租约监管的标准 Clob，返回独占资源或驱动错误。
    /// 对应 Java: DruidPooledConnection#createClob。
    async fn create_clob(&mut self) -> Result<Box<dyn druid_rdbc::resource::Clob>, RdbcError> {
        self.standard_ref()?;
        let registry = self
            .holder
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .standard_resource_registry();
        let policy = self
            .standard_policy
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clone();
        policy.ensure_open()?;
        let admission = registry.begin_admission(policy.resource_generation())?;
        let raw_result = self.standard_mut()?.create_clob().await;
        let raw = self.classify_result(raw_result)?;
        // 驱动返回后直到 registry 接管前不再 await，取消不会丢失已创建资源。
        let id = super::resource_id::ResourceId::local();
        let owner = Arc::new(super::druid_resource_owner::DruidResourceOwner::new(
            id.clone(),
            super::resource_kind::ResourceKind::Clob,
            policy,
        ));
        let context = Arc::new(super::resource_context::ResourceContext::new(
            id,
            super::resource_kind::ResourceKind::Clob,
            super::resource_runtime_capabilities::ResourceRuntimeCapabilities::large_object(),
            owner,
        ));
        let resource =
            super::druid_clob::DruidClob::register(raw, context, registry, &admission).await?;
        Ok(Box::new(resource))
    }

    /// 创建受当前租约监管的标准 NClob，返回独占资源或驱动错误。
    /// 对应 Java: DruidPooledConnection#createNClob。
    async fn create_n_clob(&mut self) -> Result<Box<dyn druid_rdbc::resource::NClob>, RdbcError> {
        self.standard_ref()?;
        let registry = self
            .holder
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .standard_resource_registry();
        let policy = self
            .standard_policy
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clone();
        policy.ensure_open()?;
        let admission = registry.begin_admission(policy.resource_generation())?;
        let raw_result = self.standard_mut()?.create_n_clob().await;
        let raw = self.classify_result(raw_result)?;
        // 驱动返回后直到 registry 接管前不再 await，取消不会丢失已创建资源。
        let id = super::resource_id::ResourceId::local();
        let owner = Arc::new(super::druid_resource_owner::DruidResourceOwner::new(
            id.clone(),
            super::resource_kind::ResourceKind::NClob,
            policy,
        ));
        let context = Arc::new(super::resource_context::ResourceContext::new(
            id,
            super::resource_kind::ResourceKind::NClob,
            super::resource_runtime_capabilities::ResourceRuntimeCapabilities::large_object(),
            owner,
        ));
        let resource =
            super::druid_n_clob::DruidNClob::register(raw, context, registry, &admission).await?;
        Ok(Box::new(resource))
    }

    /// 创建受当前租约监管的标准 SqlXml，返回独占资源或驱动错误。
    /// 对应 Java: DruidPooledConnection#createSqlXml。
    async fn create_sql_xml(&mut self) -> Result<Box<dyn druid_rdbc::resource::SqlXml>, RdbcError> {
        self.standard_ref()?;
        let registry = self
            .holder
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .standard_resource_registry();
        let policy = self
            .standard_policy
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clone();
        policy.ensure_open()?;
        let admission = registry.begin_admission(policy.resource_generation())?;
        let raw_result = self.standard_mut()?.create_sql_xml().await;
        let raw = self.classify_result(raw_result)?;
        // 驱动返回后直到 registry 接管前不再 await，取消不会丢失已创建资源。
        let id = super::resource_id::ResourceId::local();
        let owner = Arc::new(super::druid_resource_owner::DruidResourceOwner::new(
            id.clone(),
            super::resource_kind::ResourceKind::SqlXml,
            policy,
        ));
        let context = Arc::new(super::resource_context::ResourceContext::new(
            id,
            super::resource_kind::ResourceKind::SqlXml,
            super::resource_runtime_capabilities::ResourceRuntimeCapabilities::READ
                | super::resource_runtime_capabilities::ResourceRuntimeCapabilities::WRITE
                | super::resource_runtime_capabilities::ResourceRuntimeCapabilities::STREAM
                | super::resource_runtime_capabilities::ResourceRuntimeCapabilities::FREE,
            owner,
        ));
        let resource =
            super::druid_sql_xml::DruidSqlXml::register(raw, context, registry, &admission).await?;
        Ok(Box::new(resource))
    }

    /// 创建独立拥有的SQL结构对象，保留nullable名称与有序属性；返回原驱动对象或分类错误。
    /// 对应 Java: DruidPooledConnection#createStruct。Struct无free且默认随应用引用有效，不能加入LOB释放状态机。
    async fn create_struct(
        &mut self,
        type_name: Option<&str>,
        attributes: Option<Vec<druid_rdbc::types::RdbcObject>>,
    ) -> Result<Box<dyn druid_rdbc::resource::Struct>, RdbcError> {
        self.standard_ref()?;
        let result = self
            .standard_mut()?
            .create_struct(type_name, attributes)
            .await;
        self.classify_result(result)
    }

    /// 创建当前租约监管的 SQL 数组，保留可空类型名和有序元素；返回资源或驱动错误。
    /// 对应 Java: DruidPooledConnection#createArrayOf。
    async fn create_array_of(
        &mut self,
        type_name: Option<&str>,
        elements: Option<Vec<druid_rdbc::types::RdbcObject>>,
    ) -> Result<Box<dyn druid_rdbc::resource::Array>, RdbcError> {
        self.standard_ref()?;
        let registry = self
            .holder
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .standard_resource_registry();
        let policy = self
            .standard_policy
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)?
            .clone();
        policy.ensure_open()?;
        let admission = registry.begin_admission(policy.resource_generation())?;
        let raw_result = self
            .standard_mut()?
            .create_array_of(type_name, elements)
            .await;
        let raw = self.classify_result(raw_result)?;
        let id = super::resource_id::ResourceId::local();
        let owner = Arc::new(super::druid_resource_owner::DruidResourceOwner::new(
            id.clone(),
            super::resource_kind::ResourceKind::Array,
            policy.clone(),
        ));
        let context = Arc::new(super::resource_context::ResourceContext::new(
            id,
            super::resource_kind::ResourceKind::Array,
            super::resource_runtime_capabilities::ResourceRuntimeCapabilities::READ
                | super::resource_runtime_capabilities::ResourceRuntimeCapabilities::RANGE
                | super::resource_runtime_capabilities::ResourceRuntimeCapabilities::TYPE_MAP
                | super::resource_runtime_capabilities::ResourceRuntimeCapabilities::RESULT_SET
                | super::resource_runtime_capabilities::ResourceRuntimeCapabilities::FREE,
            owner,
        ));
        let resource = super::druid_array::DruidArray::register(
            raw,
            context,
            registry,
            &admission,
            self.id,
            Arc::clone(&self.result_set_id_seed),
            Arc::clone(&self.lease_active),
            self.filter_chain.clone(),
            self.stats_collector.clone(),
            policy,
        )
        .await?;
        Ok(Box::new(resource))
    }

    async fn commit(&mut self) -> Result<(), RdbcError> {
        let event = ConnectionEvent::Commit;
        let started_at = Instant::now();
        self.before_connection_event(&event).await?;
        if let Some(policy) = &self.standard_policy {
            policy.record_transaction_boundary(true);
        }
        let result = self.standard_mut()?.commit().await;
        self.classify_result(result)?;
        self.after_connection_event(&event, started_at).await
    }

    async fn rollback(&mut self) -> Result<(), RdbcError> {
        let event = ConnectionEvent::Rollback;
        let started_at = Instant::now();
        self.before_connection_event(&event).await?;
        if let Some(policy) = &self.standard_policy {
            policy.record_transaction_boundary(false);
        }
        let result = self.standard_mut()?.rollback().await;
        self.classify_result(result)?;
        self.after_connection_event(&event, started_at).await
    }

    async fn rollback_to(
        &mut self,
        savepoint: &druid_rdbc::types::Savepoint,
    ) -> Result<(), RdbcError> {
        let result = self.standard_mut()?.rollback_to(savepoint).await;
        self.classify_result(result)
    }

    async fn set_savepoint(&mut self) -> Result<druid_rdbc::types::Savepoint, RdbcError> {
        let result = self.standard_mut()?.set_savepoint().await;
        self.classify_result(result)
    }

    async fn set_savepoint_named(
        &mut self,
        name: &str,
    ) -> Result<druid_rdbc::types::Savepoint, RdbcError> {
        let result = self.standard_mut()?.set_savepoint_named(name).await;
        self.classify_result(result)
    }

    async fn release_savepoint(
        &mut self,
        savepoint: &druid_rdbc::types::Savepoint,
    ) -> Result<(), RdbcError> {
        let result = self.standard_mut()?.release_savepoint(savepoint).await;
        self.classify_result(result)
    }

    async fn set_auto_commit(&mut self, enabled: bool) -> Result<(), RdbcError> {
        let event = ConnectionEvent::SetAutoCommit(enabled);
        let started_at = Instant::now();
        self.before_connection_event(&event).await?;
        let result = self.standard_mut()?.set_auto_commit(enabled).await;
        self.classify_result(result)?;
        if let Some(policy) = &self.standard_policy {
            policy.set_auto_commit_state(enabled);
        }
        self.after_connection_event(&event, started_at).await
    }

    fn auto_commit(&self) -> Result<bool, RdbcError> {
        self.standard_ref()?.auto_commit()
    }

    fn read_only(&self) -> Result<bool, RdbcError> {
        self.standard_ref()?.read_only()
    }

    async fn set_read_only(&mut self, read_only: bool) -> Result<(), RdbcError> {
        let event = ConnectionEvent::SetReadOnly(read_only);
        let started_at = Instant::now();
        self.before_connection_event(&event).await?;
        self.invalidate_prepared_statement_cache().await?;
        let result = self.standard_mut()?.set_read_only(read_only).await;
        self.classify_result(result)?;
        self.after_connection_event(&event, started_at).await
    }

    fn transaction_isolation(
        &self,
    ) -> Result<druid_rdbc::connection::TransactionIsolation, RdbcError> {
        self.standard_ref()?.transaction_isolation()
    }

    async fn set_transaction_isolation(
        &mut self,
        level: druid_rdbc::connection::TransactionIsolation,
    ) -> Result<(), RdbcError> {
        self.invalidate_prepared_statement_cache().await?;
        let result = self.standard_mut()?.set_transaction_isolation(level).await;
        self.classify_result(result)
    }

    fn holdability(&self) -> Result<druid_rdbc::statement::ResultSetHoldability, RdbcError> {
        self.standard_ref()?.holdability()
    }

    async fn set_holdability(
        &mut self,
        holdability: druid_rdbc::statement::ResultSetHoldability,
    ) -> Result<(), RdbcError> {
        self.invalidate_prepared_statement_cache().await?;
        let result = self.standard_mut()?.set_holdability(holdability).await;
        self.classify_result(result)
    }

    async fn set_client_info(&mut self, name: &str, value: Option<&str>) -> Result<(), RdbcError> {
        let result = self.standard_mut()?.set_client_info(name, value).await;
        self.classify_result(result)
    }

    fn client_info(&self, name: &str) -> Result<Option<String>, RdbcError> {
        self.standard_ref()?.client_info(name)
    }

    async fn native_sql(&mut self, sql: &str) -> Result<String, RdbcError> {
        self.standard_mut()?.native_sql(sql).await
    }

    async fn set_network_timeout(&mut self, timeout: std::time::Duration) -> Result<(), RdbcError> {
        let result = self.standard_mut()?.set_network_timeout(timeout).await;
        self.classify_result(result)
    }

    fn network_timeout(&self) -> Result<std::time::Duration, RdbcError> {
        self.standard_ref()?.network_timeout()
    }

    fn type_map(&self) -> Result<Option<druid_rdbc::types::RdbcTypeMap>, RdbcError> {
        self.standard_ref()?.type_map()
    }

    async fn set_type_map(
        &mut self,
        type_map: druid_rdbc::types::RdbcTypeMap,
    ) -> Result<(), RdbcError> {
        self.invalidate_prepared_statement_cache().await?;
        let result = self.standard_mut()?.set_type_map(type_map).await;
        self.classify_result(result)
    }

    fn catalog(&self) -> Result<Option<String>, RdbcError> {
        self.standard_ref()?.catalog()
    }

    async fn set_catalog(&mut self, catalog: Option<&str>) -> Result<(), RdbcError> {
        let event = ConnectionEvent::SetCatalog(catalog.unwrap_or_default().to_string());
        let started_at = Instant::now();
        self.before_connection_event(&event).await?;
        self.invalidate_prepared_statement_cache().await?;
        let result = self.standard_mut()?.set_catalog(catalog).await;
        self.classify_result(result)?;
        self.after_connection_event(&event, started_at).await
    }

    fn schema(&self) -> Result<Option<String>, RdbcError> {
        self.standard_ref()?.schema()
    }

    async fn set_schema(&mut self, schema: Option<&str>) -> Result<(), RdbcError> {
        let event = ConnectionEvent::SetSchema(schema.unwrap_or_default().to_string());
        let started_at = Instant::now();
        self.before_connection_event(&event).await?;
        let initial_schema = self.standard_ref()?.schema()?;
        if let Some(holder) = self.holder.as_ref() {
            holder.remember_initial_schema(initial_schema);
        }
        self.invalidate_prepared_statement_cache().await?;
        let result = self.standard_mut()?.set_schema(schema).await;
        self.classify_result(result)?;
        self.after_connection_event(&event, started_at).await
    }

    async fn warnings(&mut self) -> Result<Option<SqlWarning>, RdbcError> {
        DruidPooledConnection::warnings(self).await
    }

    async fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        let event = ConnectionEvent::ClearWarnings;
        let started_at = Instant::now();
        self.before_connection_event(&event).await?;
        DruidPooledConnection::clear_warnings(self).await?;
        self.after_connection_event(&event, started_at).await
    }

    fn capabilities(&self) -> druid_rdbc::extensions::ConnectionCapabilities {
        self.standard_ref()
            .map(druid_rdbc::connection::Connection::capabilities)
            .unwrap_or_default()
    }

    async fn is_valid(&mut self, timeout_seconds: u32) -> Result<bool, RdbcError> {
        let event = ConnectionEvent::IsValid;
        let started_at = Instant::now();
        self.before_connection_event(&event).await?;
        let result = self.standard_mut()?.is_valid(timeout_seconds).await;
        let valid = self.classify_result(result)?;
        self.after_connection_event(&event, started_at).await?;
        Ok(valid)
    }

    async fn ping(&mut self) -> Result<(), RdbcError> {
        let result = self.standard_mut()?.ping().await;
        self.classify_result(result)
    }

    async fn abort(&mut self) -> Result<(), RdbcError> {
        let result = self.standard_mut()?.abort().await;
        self.classify_result(result)
    }

    fn driver_name(&self) -> Result<String, RdbcError> {
        self.standard_ref()?.driver_name()
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.close_logical_once().await
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.recycled || !self.lease_active.load(Ordering::Acquire))
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

impl Drop for DruidPooledConnection {
    fn drop(&mut self) {
        if self.recycled {
            return;
        }
        self.notify_connection_closed();
        let disposition = self.drop_disposition();
        self.recycle_once(disposition);
    }
}
