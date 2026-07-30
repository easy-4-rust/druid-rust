//! 对外池化连接。

use super::clob_proxy_impl::ClobProxyImpl;
use super::connection_defaults::ConnectionDefaults;
use super::connection_event_listener::ConnectionEventListener;
use super::connection_recycle_disposition::ConnectionRecycleDisposition;
use super::database_meta_data_proxy_impl::DatabaseMetaDataProxyImpl;
use super::druid_connection_holder::{DruidConnectionHolder, StatementTrace};
use super::druid_pooled_callable_statement::DruidPooledCallableStatement;
use super::druid_pooled_prepared_statement::DruidPooledPreparedStatement;
use super::druid_pooled_statement::DruidPooledStatement;
use super::error::DruidError;
use super::exception_sorter::ExceptionSorter;
use super::exec_result::ExecResult;
use super::fatal_error_handler::FatalErrorHandler;
use super::filter::{ConnectionEvent, ExecContext};
use super::filter_chain::FilterChain;
use super::jdbc_blob::JdbcBlob;
use super::jdbc_result_set::PhysicalResultSet;
use super::n_clob_proxy_impl::NClobProxyImpl;
use super::physical_connection::PhysicalConnection;
use super::physical_connection_capabilities::PhysicalConnectionCapabilities;
use super::physical_connection_factory::PhysicalConnectionFactory;
use super::physical_prepared_statement::PhysicalPreparedStatement;
use super::physical_statement::{
    PhysicalStatementOptions, StatementExecuteResult, StatementGeneratedKeys,
};
use super::prepared_input_parameter::PreparedInputParameter;
use super::prepared_statement_holder::PreparedStatementHolder;
use super::prepared_statement_key::{PreparedStatementKey, PreparedStatementMethodType};
use super::proxy_attributes::{ProxyAttributeValue, ProxyAttributes};
use super::row::Row;
use super::savepoint::Savepoint;
use super::sql_warning::SqlWarning;
use super::statement_event_listener::StatementEventListener;
use super::transaction_info::TransactionInfo;
use super::value::Value;
use super::wrapper::{Unwrapped, Wrapper};
use crate::stats::{JdbcStatementStat, StatsCollector};
use serde_json::Value as JsonValue;
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::time::{SystemTime, UNIX_EPOCH};

/// Native pool 回收完整 holder 的一次性回调。
pub type DruidConnectionReturnCallback =
    Box<dyn FnOnce(DruidConnectionHolder, ConnectionRecycleDisposition) -> bool + Send>;

/// 一次数据库执行的运行中标记，离开作用域时无条件复位。
struct ExecutionRunningGuard {
    execution_running: Arc<AtomicBool>,
    statement_stat: Option<Arc<JdbcStatementStat>>,
    started_at: Instant,
}

impl ExecutionRunningGuard {
    fn new(
        execution_running: Arc<AtomicBool>,
        statement_stat: Option<Arc<JdbcStatementStat>>,
    ) -> Self {
        execution_running.store(true, Ordering::Release);
        if let Some(stat) = statement_stat.as_ref() {
            stat.before_execute();
        }
        Self {
            execution_running,
            statement_stat,
            started_at: Instant::now(),
        }
    }
}

impl Drop for ExecutionRunningGuard {
    fn drop(&mut self) {
        if let Some(stat) = self.statement_stat.as_ref() {
            stat.after_execute(self.started_at.elapsed());
        }
        self.execution_running.store(false, Ordering::Release);
    }
}

/// 对外池化连接。
///
/// 对应 Java: `com.alibaba.druid.pool.DruidPooledConnection`。
/// 该对象拥有一次连接租约，对外暴露连接语义；底层只依赖
/// `PhysicalConnection`。显式关闭和 `Drop` 都通过同一条回收路径，
/// 并由 take-once 回调保证物理连接最多归还一次。显式异步关闭执行 Java
/// `DruidDataSource#recycle` 的事务回滚、状态复位与 return validation；
/// Drop 只复用无需异步复位的干净连接。
pub struct DruidPooledConnection {
    holder: Option<DruidConnectionHolder>,
    id: u64,
    data_source: String,
    filter_chain: Option<Arc<FilterChain>>,
    keep_underlying_transaction_isolation: bool,
    recycle_validator: Option<Arc<dyn PhysicalConnectionFactory>>,
    exception_sorter: Option<Arc<dyn ExceptionSorter>>,
    fatal_error_handler: Option<Arc<dyn FatalErrorHandler>>,
    return_connection: Option<DruidConnectionReturnCallback>,
    lease_active: Arc<AtomicBool>,
    execution_running: Arc<AtomicBool>,
    borrowed_at: Instant,
    recycled: bool,
    connection_closed_notified: bool,
    stats_collector: Option<Arc<StatsCollector>>,
    transaction_started_at: Option<Instant>,
    transaction_info: Option<Arc<TransactionInfo>>,
    query_timeout: i32,
    transaction_query_timeout: i32,
    statement_id_seed: Arc<AtomicU64>,
    result_set_id_seed: Arc<AtomicU64>,
    metadata_id_seed: Arc<AtomicU64>,
    transaction_id_seed: Arc<AtomicU64>,
    connection_properties: Arc<HashMap<String, String>>,
    connected_time_millis: u64,
    close_count: u64,
    last_validate_time_millis: u64,
    last_execute_sql: Option<String>,
    attributes: ProxyAttributes,
}

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
                    .is_some_and(DruidConnectionHolder::has_physical_connection),
            )
            .field(
                "has_physical_connection",
                &self
                    .holder
                    .as_ref()
                    .is_some_and(DruidConnectionHolder::has_physical_connection),
            )
            .field("filter_chain", &self.filter_chain.is_some())
            .field(
                "keep_underlying_transaction_isolation",
                &self.keep_underlying_transaction_isolation,
            )
            .field("recycle_validator", &self.recycle_validator.is_some())
            .field("exception_sorter", &self.exception_sorter.is_some())
            .field("fatal_error_handler", &self.fatal_error_handler.is_some())
            .field("return_connection", &self.return_connection.is_some())
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

impl Wrapper for DruidPooledConnection {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn is_wrapper_for(&self, iface: Option<TypeId>) -> bool {
        let Some(iface) = iface else {
            return false;
        };
        if Wrapper::as_any(self).type_id() == iface {
            return true;
        }
        if iface == TypeId::of::<dyn PhysicalConnection>() {
            return self
                .holder
                .as_ref()
                .is_some_and(DruidConnectionHolder::has_physical_connection);
        }
        self.holder
            .as_ref()
            .and_then(DruidConnectionHolder::physical_connection)
            .is_some_and(|connection| {
                let connection: &dyn Any = connection;
                connection.type_id() == iface
            })
    }

    fn unwrap(&self, iface: Option<TypeId>) -> Option<Unwrapped<'_>> {
        let iface = iface?;
        if Wrapper::as_any(self).type_id() == iface {
            return Some(Unwrapped::Object(Wrapper::as_any(self)));
        }
        let connection = self
            .holder
            .as_ref()
            .and_then(DruidConnectionHolder::physical_connection)?;
        if iface == TypeId::of::<dyn PhysicalConnection>() {
            return Some(Unwrapped::PhysicalConnection(connection));
        }
        let connection: &dyn Any = connection;
        (connection.type_id() == iface).then_some(Unwrapped::Object(connection))
    }
}

impl DruidPooledConnection {
    /// 创建不带过滤上下文的池化连接。
    ///
    /// 参数 `physical_connection` 为底层物理连接，`id` 为连接 ID，
    /// `return_connection` 在关闭或析构时接收并归还连接。
    pub fn new(
        physical_connection: Box<dyn PhysicalConnection>,
        id: u64,
        return_connection: Box<dyn FnOnce(Box<dyn PhysicalConnection>, u64) + Send>,
    ) -> Self {
        Self::with_context(
            physical_connection,
            id,
            String::new(),
            None,
            return_connection,
        )
    }

    /// 创建带数据源和过滤链上下文的池化连接。
    ///
    /// 参数 `data_source` 会原样进入前置与后置过滤器，
    /// `filter_chain` 为该数据源装配的过滤链。
    pub fn with_context(
        physical_connection: Box<dyn PhysicalConnection>,
        id: u64,
        data_source: String,
        filter_chain: Option<Arc<FilterChain>>,
        return_connection: Box<dyn FnOnce(Box<dyn PhysicalConnection>, u64) + Send>,
    ) -> Self {
        Self::with_recycle_policy(
            physical_connection,
            id,
            data_source,
            filter_chain,
            false,
            None,
            Box::new(move |connection, connection_id, _disposition| {
                return_connection(connection, connection_id);
            }),
        )
    }

    /// 创建具有完整回收策略的池化连接。
    ///
    /// 对应 Java：`DruidPooledConnection` 构造及
    /// `DruidDataSource#recycle(DruidPooledConnection)`。
    ///
    /// # 参数
    /// - `physical_connection`：本次租约独占的物理连接。
    /// - `id`：物理连接 ID。
    /// - `data_source`：数据源名称。
    /// - `filter_chain`：连接与 SQL 过滤链。
    /// - `keep_underlying_transaction_isolation`：归还时是否保留隔离级别。
    /// - `recycle_validator`：启用 `testOnReturn` 时使用的物理连接工厂。
    /// - `return_connection`：接收唯一物理连接所有权及最终回收处置。
    pub fn with_recycle_policy(
        physical_connection: Box<dyn PhysicalConnection>,
        id: u64,
        data_source: String,
        filter_chain: Option<Arc<FilterChain>>,
        keep_underlying_transaction_isolation: bool,
        recycle_validator: Option<Arc<dyn PhysicalConnectionFactory>>,
        return_connection: Box<
            dyn FnOnce(Box<dyn PhysicalConnection>, u64, ConnectionRecycleDisposition) + Send,
        >,
    ) -> Self {
        let connection_defaults = ConnectionDefaults::capture(physical_connection.as_ref());
        Self::with_recycle_policy_and_defaults(
            physical_connection,
            id,
            data_source,
            filter_chain,
            connection_defaults,
            keep_underlying_transaction_isolation,
            recycle_validator,
            return_connection,
        )
    }

    /// 使用物理连接首次入池时保存的默认状态创建池化连接。
    ///
    /// Native Pool 在多次借出同一物理连接时必须调用本方法，避免把上一次借用者
    /// 可能保留的状态重新捕获为“默认值”。
    #[allow(clippy::too_many_arguments)]
    pub fn with_recycle_policy_and_defaults(
        physical_connection: Box<dyn PhysicalConnection>,
        id: u64,
        data_source: String,
        filter_chain: Option<Arc<FilterChain>>,
        connection_defaults: ConnectionDefaults,
        keep_underlying_transaction_isolation: bool,
        recycle_validator: Option<Arc<dyn PhysicalConnectionFactory>>,
        return_connection: Box<
            dyn FnOnce(Box<dyn PhysicalConnection>, u64, ConnectionRecycleDisposition) + Send,
        >,
    ) -> Self {
        let holder = DruidConnectionHolder::with_connection_and_defaults(
            physical_connection,
            id,
            Duration::ZERO,
            0,
            connection_defaults,
        );
        Self::with_holder(
            holder,
            data_source,
            filter_chain,
            keep_underlying_transaction_isolation,
            recycle_validator,
            Box::new(move |mut holder, disposition| {
                let connection_id = holder.connection_id();
                if let Some(connection) = holder.take_physical_connection() {
                    return_connection(connection, connection_id, disposition);
                }
                false
            }),
        )
    }

    /// 使用 canonical `DruidConnectionHolder` 创建池化连接。
    ///
    /// Native pool 应使用该入口，使 holder 在空闲队列、借出连接和回收回调
    /// 之间移动而不拆散物理连接与生命周期状态。
    ///
    /// # 参数
    /// - `holder`：本次租约独占的 holder。
    /// - `data_source`：数据源名称。
    /// - `filter_chain`：连接与 SQL 过滤链。
    /// - `keep_underlying_transaction_isolation`：归还时是否保留隔离级别。
    /// - `recycle_validator`：启用 `testOnReturn` 时的校验器。
    /// - `return_connection`：接收完整 holder 与最终回收处置。
    pub fn with_holder(
        holder: DruidConnectionHolder,
        data_source: String,
        filter_chain: Option<Arc<FilterChain>>,
        keep_underlying_transaction_isolation: bool,
        recycle_validator: Option<Arc<dyn PhysicalConnectionFactory>>,
        return_connection: DruidConnectionReturnCallback,
    ) -> Self {
        let id = holder.connection_id();
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
            recycle_validator,
            exception_sorter: None,
            fatal_error_handler: None,
            return_connection: Some(return_connection),
            lease_active: Arc::new(AtomicBool::new(true)),
            execution_running: Arc::new(AtomicBool::new(false)),
            borrowed_at: Instant::now(),
            recycled: false,
            connection_closed_notified: false,
            stats_collector: None,
            transaction_started_at: None,
            transaction_info: None,
            query_timeout: 0,
            transaction_query_timeout: 0,
            statement_id_seed: Arc::new(AtomicU64::new(20_000)),
            result_set_id_seed: Arc::new(AtomicU64::new(50_000)),
            metadata_id_seed: Arc::new(AtomicU64::new(80_000)),
            transaction_id_seed: Arc::new(AtomicU64::new(60_000)),
            connection_properties: Arc::new(HashMap::new()),
            connected_time_millis,
            close_count: 0,
            last_validate_time_millis: 0,
            last_execute_sql: None,
            attributes: ProxyAttributes::default(),
        }
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

    /// 返回从 StatFilter 记录的连接获取时刻起经过的时间。
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
        transaction_id_seed: Arc<AtomicU64>,
    ) {
        self.statement_id_seed = statement_id_seed;
        self.result_set_id_seed = result_set_id_seed;
        self.metadata_id_seed = metadata_id_seed;
        self.transaction_id_seed = transaction_id_seed;
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

    fn ensure_transaction_info(&mut self) -> Arc<TransactionInfo> {
        if let Some(transaction_info) = self.transaction_info.as_ref() {
            return Arc::clone(transaction_info);
        }
        let transaction_info = Arc::new(TransactionInfo::new(
            self.transaction_id_seed.fetch_add(1, Ordering::AcqRel),
        ));
        self.attributes.put(
            "stat.tx",
            ProxyAttributeValue::Value(Arc::clone(&transaction_info) as Arc<dyn Any + Send + Sync>),
        );
        self.transaction_info = Some(Arc::clone(&transaction_info));
        transaction_info
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
    /// `&mut PhysicalConnection`，从而避免 Java 强制关闭方式在 Rust 中造成别名。
    pub(crate) fn lease_active_token(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.lease_active)
    }

    /// 返回池管理器使用的执行中令牌。
    pub(crate) fn execution_running_token(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.execution_running)
    }

    fn begin_execution(&mut self) -> Result<ExecutionRunningGuard, DruidError> {
        // before Filter 可能短路；先清除上一条物理 SQL，避免 Statement 把前一次
        // 执行文本误认成本次改写结果。
        self.last_execute_sql = None;
        if !self.lease_active.load(Ordering::Acquire) {
            return Err(DruidError::ConnectionLeaked {
                id: self.id,
                held_for: self.borrowed_at.elapsed(),
            });
        }
        Ok(ExecutionRunningGuard::new(
            Arc::clone(&self.execution_running),
            self.stats_collector
                .as_ref()
                .map(|collector| Arc::clone(collector.statement_stat())),
        ))
    }

    /// 装配数据源的异常分类器。
    ///
    /// 对应 Java：`DruidAbstractDataSource#setExceptionSorter(ExceptionSorter)`。
    /// 分类器命中 fatal SQL 异常时，当前 holder 与物理 Adapter 会立即标记为
    /// discard，之后的 close/Drop 只能进入销毁分支。
    pub fn set_exception_sorter(&mut self, exception_sorter: Arc<dyn ExceptionSorter>) {
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
        self.fatal_error_handler = Some(fatal_error_handler);
    }

    /// 装配数据源级运行统计；native pool 对外连接必须共享同一实例。
    pub(crate) fn set_stats_collector(&mut self, stats_collector: Arc<StatsCollector>) {
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
        if !self.auto_commit() && self.transaction_query_timeout > 0 {
            self.transaction_query_timeout
        } else {
            self.query_timeout
        }
    }

    fn record_execution<T>(&mut self, sql: &str, result: &Result<T, DruidError>) {
        self.last_execute_sql = Some(sql.to_owned());
        if !self.auto_commit() {
            if self.transaction_started_at.is_none() {
                self.transaction_started_at = Some(Instant::now());
                if let Some(stats) = self.stats_collector.as_ref() {
                    stats.record_start_transaction();
                }
            }
            self.ensure_transaction_info().record_sql(sql, 10);
        }
        if let Some(stats) = self.stats_collector.as_ref() {
            stats.record_execute_result(result.is_ok());
            if let Some(entry) = stats.connection_stat().entry(self.id) {
                entry.set_last_sql(Some(sql.to_owned()));
                if let Err(error) = result {
                    entry.error(error);
                }
            }
            if let Err(error) = result {
                stats.statement_stat().error(error);
            }
        }
    }

    /// 返回本租约最近一次实际交给物理驱动的 SQL。
    ///
    /// Filter 改写发生后，该值与调用方原始参数可能不同；Statement proxy 必须
    /// 用它更新 `lastExecuteSql`。
    #[must_use]
    pub(crate) fn last_execute_sql(&self) -> Option<&str> {
        self.last_execute_sql.as_deref()
    }

    /// 处理驱动错误并返回它是否使连接不可复用。
    ///
    /// 对应 Java：
    /// `DruidDataSource#handleConnectionException` → `handleFatalError`。
    /// 非 SQL 错误和未命中 sorter 的 SQL 错误保持连接可复用。
    pub fn handle_exception(&mut self, error: &DruidError) -> bool {
        self.handle_exception_with_sql(error, None)
    }

    fn handle_exception_with_sql(&mut self, error: &DruidError, sql: Option<&str>) -> bool {
        let fatal = self
            .exception_sorter
            .as_ref()
            .zip(error.sql_exception())
            .is_some_and(|(exception_sorter, exception)| {
                exception_sorter.is_exception_fatal(exception)
            });
        if fatal {
            if let Some(holder) = self.holder.as_mut() {
                holder.mark_discarded();
            }
            let fatal_error_handler = self.fatal_error_handler.clone();
            let on_fatal_error = fatal_error_handler
                .as_ref()
                .is_some_and(|handler| handler.handle_fatal_error(error, sql));
            // Java `handleFatalError` 当场执行 discardConnection，而不是等用户
            // 随后 close；这样 activeCount 会在错误返回前释放。
            let refill_requested = self.discard_connection();
            if on_fatal_error && !refill_requested {
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
    pub(crate) fn classify_result<T>(
        &mut self,
        result: Result<T, DruidError>,
    ) -> Result<T, DruidError> {
        self.classify_result_with_sql(result, None)
    }

    /// 对数据库操作结果执行异常分类，并保留触发错误的 SQL。
    pub(crate) fn classify_result_with_sql<T>(
        &mut self,
        result: Result<T, DruidError>,
        sql: Option<&str>,
    ) -> Result<T, DruidError> {
        if let Err(error) = &result {
            if error.sql_exception().is_some() {
                self.notify_connection_error(error);
            }
            self.handle_exception_with_sql(error, sql);
        }
        result
    }

    /// 返回底层物理连接的可变引用；连接已归还时返回 `None`。
    pub fn physical_connection_mut(&mut self) -> Option<&mut (dyn PhysicalConnection + 'static)> {
        self.holder
            .as_mut()
            .and_then(DruidConnectionHolder::physical_connection_mut)
    }

    /// 返回借用当前物理连接的数据库元数据代理。
    ///
    /// 对应 Java：`DruidPooledConnection#getMetaData()`。代理公开完整
    /// `DatabaseMetaDataProxyImpl` 方法面，生命周期不得超过当前连接的可变
    /// 借用；连接已归还或 Adapter 不支持时返回精确错误。
    pub fn database_meta_data(&mut self) -> Result<DatabaseMetaDataProxyImpl<'_>, DruidError> {
        if !self.auto_commit() {
            self.ensure_transaction_info();
        }
        let connection_id = self.id;
        let filter_chain = self.filter_chain.clone();
        let physical = self
            .physical_connection_mut()
            .ok_or(DruidError::ConnectionDiscarded)?;
        let raw = match filter_chain {
            Some(filter_chain) => {
                filter_chain.connection_database_meta_data(physical, connection_id)?
            }
            None => physical.database_meta_data()?,
        };
        Ok(DatabaseMetaDataProxyImpl::new(raw, connection_id))
    }

    /// 返回当前连接 holder；连接归还后返回 `None`。
    ///
    /// 对应 Java：`DruidPooledConnection#getConnectionHolder()`。
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
    /// `initVariants` 时返回 `None`，启用但数据库不是 MySQL 协议族时返回空表。
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
    ) -> Result<(), DruidError> {
        let holder = self
            .holder
            .as_ref()
            .ok_or(DruidError::ConnectionDiscarded)?;
        holder.add_connection_event_listener(listener);
        Ok(())
    }

    /// 按对象身份移除连接监听器。
    ///
    /// 对应 Java: `DruidPooledConnection#removeConnectionEventListener`。
    pub fn remove_connection_event_listener(
        &self,
        listener: &Arc<dyn ConnectionEventListener>,
    ) -> Result<bool, DruidError> {
        let holder = self
            .holder
            .as_ref()
            .ok_or(DruidError::ConnectionDiscarded)?;
        Ok(holder.remove_connection_event_listener(listener))
    }

    /// 添加 Statement 生命周期监听器。
    ///
    /// 对应 Java：`DruidPooledConnection#addStatementEventListener`。
    pub fn add_statement_event_listener(
        &self,
        listener: Arc<dyn StatementEventListener>,
    ) -> Result<(), DruidError> {
        let holder = self
            .holder
            .as_ref()
            .ok_or(DruidError::ConnectionDiscarded)?;
        holder.add_statement_event_listener(listener);
        Ok(())
    }

    /// 按对象身份移除 Statement 生命周期监听器。
    ///
    /// 对应 Java：`DruidPooledConnection#removeStatementEventListener`。
    pub fn remove_statement_event_listener(
        &self,
        listener: &Arc<dyn StatementEventListener>,
    ) -> Result<bool, DruidError> {
        let holder = self
            .holder
            .as_ref()
            .ok_or(DruidError::ConnectionDiscarded)?;
        Ok(holder.remove_statement_event_listener(listener))
    }

    /// 从当前 holder 移除一个已关闭的逻辑 Statement。
    ///
    /// 对应 Java：`DruidPooledConnection`/`DruidPooledStatement#close()` 完成后
    /// 调用 `DruidConnectionHolder#removeTrace`。
    pub(crate) fn remove_statement_trace(&self, identity: usize) -> bool {
        self.holder
            .as_ref()
            .is_some_and(|holder| holder.remove_statement_trace(identity))
    }

    /// 创建驱动 Blob。
    ///
    /// 对应 Java：`DruidPooledConnection#createBlob()`。Java Druid 对 Blob
    /// 只做连接状态检查和 FilterChain 转发，不创建 `BlobProxy`。
    pub async fn create_blob(&mut self) -> Result<JdbcBlob, DruidError> {
        let filter_chain = self
            .filter_chain
            .clone()
            .unwrap_or_else(|| Arc::new(FilterChain::new()));
        let result = filter_chain
            .connection_create_blob(self.physical_mut()?)
            .await;
        self.classify_result(result)
    }

    /// 创建经 Druid FilterChain 包装的 Clob。
    ///
    /// 对应 Java：`ConnectionProxyImpl#createClob()` 与
    /// `FilterChainImpl#wrap(ConnectionProxy, Clob)`。
    pub async fn create_clob(&mut self) -> Result<ClobProxyImpl, DruidError> {
        let filter_chain = self
            .filter_chain
            .clone()
            .unwrap_or_else(|| Arc::new(FilterChain::new()));
        let result = filter_chain
            .connection_create_clob(self.physical_mut()?)
            .await;
        let clob = self.classify_result(result)?;
        Ok(ClobProxyImpl::new(self.id, clob, filter_chain))
    }

    /// 创建经 Druid FilterChain 包装并保持类型身份的 NClob。
    ///
    /// 对应 Java：`ConnectionProxyImpl#createNClob()`。
    pub async fn create_n_clob(&mut self) -> Result<NClobProxyImpl, DruidError> {
        let filter_chain = self
            .filter_chain
            .clone()
            .unwrap_or_else(|| Arc::new(FilterChain::new()));
        let result = filter_chain
            .connection_create_n_clob(self.physical_mut()?)
            .await;
        let n_clob = self.classify_result(result)?;
        Ok(NClobProxyImpl::new(self.id, n_clob, filter_chain))
    }

    /// 创建默认类型、只读并发和当前保持性的普通池化语句。
    ///
    /// 对应 Java：`DruidPooledConnection#createStatement()`。
    pub async fn create_statement(&mut self) -> Result<DruidPooledStatement, DruidError> {
        self.create_statement_with_options(PhysicalStatementOptions::default())
            .await
    }

    /// 使用指定结果集类型与并发模式创建普通池化语句。
    ///
    /// 对应 Java：`DruidPooledConnection#createStatement(int, int)`。
    pub async fn create_statement_with_result_set(
        &mut self,
        result_set_type: i32,
        result_set_concurrency: i32,
    ) -> Result<DruidPooledStatement, DruidError> {
        self.create_statement_with_options(PhysicalStatementOptions {
            result_set_type,
            result_set_concurrency,
            result_set_holdability: self
                .holder
                .as_ref()
                .and_then(DruidConnectionHolder::physical_connection)
                .map_or(1, PhysicalConnection::holdability),
        })
        .await
    }

    /// 使用完整结果集创建参数创建普通池化语句。
    ///
    /// 对应 Java：`DruidPooledConnection#createStatement(int, int, int)`。
    pub async fn create_statement_with_holdability(
        &mut self,
        result_set_type: i32,
        result_set_concurrency: i32,
        result_set_holdability: i32,
    ) -> Result<DruidPooledStatement, DruidError> {
        self.create_statement_with_options(PhysicalStatementOptions {
            result_set_type,
            result_set_concurrency,
            result_set_holdability,
        })
        .await
    }

    async fn create_statement_with_options(
        &mut self,
        options: PhysicalStatementOptions,
    ) -> Result<DruidPooledStatement, DruidError> {
        let result = self
            .physical_mut()?
            .create_physical_statement(options)
            .await;
        let statement = self.classify_result(result)?;
        let query_timeout = self.effective_query_timeout();
        if query_timeout > 0 {
            let timeout_result = statement.set_query_timeout(query_timeout);
            self.classify_result(timeout_result)?;
        }
        let statement_id = self.next_statement_id();
        if let Some(filter_chain) = &self.filter_chain {
            let event_result = filter_chain
                .after_statement_event_with_identity(
                    self.id,
                    statement_id,
                    &super::StatementEvent::CreateStatement,
                )
                .await;
            self.classify_result(event_result)?;
        }
        if let Some(stats) = self.stats_collector.as_ref() {
            stats.statement_stat().increment_create_counter();
        }
        let statement = DruidPooledStatement::new(
            statement,
            self.id,
            statement_id,
            Arc::clone(&self.result_set_id_seed),
            Arc::clone(&self.metadata_id_seed),
            self.lease_active.clone(),
            self.filter_chain.clone(),
            self.stats_collector.clone(),
        );
        let holder = self
            .holder
            .as_ref()
            .ok_or(DruidError::ConnectionDiscarded)?;
        holder.add_statement_trace(StatementTrace::Statement(statement.statement_trace_inner()));
        Ok(statement)
    }

    /// 创建 `prepareStatement(String)` 语义的池化语句。
    pub async fn prepare_statement(
        &mut self,
        sql: &str,
    ) -> Result<DruidPooledPreparedStatement, DruidError> {
        let key = PreparedStatementKey::new(
            Some(sql.to_string()),
            self.current_catalog(),
            PreparedStatementMethodType::M1,
        )?;
        self.prepare_statement_from_key(key).await
    }

    /// 创建 `prepareStatement(String, int, int)` 语义的池化语句。
    pub async fn prepare_statement_with_result_set(
        &mut self,
        sql: &str,
        result_set_type: i32,
        result_set_concurrency: i32,
    ) -> Result<DruidPooledPreparedStatement, DruidError> {
        let key = PreparedStatementKey::with_result_set(
            Some(sql.to_string()),
            self.current_catalog(),
            PreparedStatementMethodType::M2,
            result_set_type,
            result_set_concurrency,
        )?;
        self.prepare_statement_from_key(key).await
    }

    /// 创建 `prepareStatement(String, int, int, int)` 语义的池化语句。
    pub async fn prepare_statement_with_holdability(
        &mut self,
        sql: &str,
        result_set_type: i32,
        result_set_concurrency: i32,
        result_set_holdability: i32,
    ) -> Result<DruidPooledPreparedStatement, DruidError> {
        let key = PreparedStatementKey::with_result_set_holdability(
            Some(sql.to_string()),
            self.current_catalog(),
            PreparedStatementMethodType::M3,
            result_set_type,
            result_set_concurrency,
            result_set_holdability,
        )?;
        self.prepare_statement_from_key(key).await
    }

    /// 创建 `prepareStatement(String, int[])` 语义的池化语句。
    pub async fn prepare_statement_with_column_indexes(
        &mut self,
        sql: &str,
        column_indexes: Vec<i32>,
    ) -> Result<DruidPooledPreparedStatement, DruidError> {
        let key = PreparedStatementKey::with_column_indexes(
            Some(sql.to_string()),
            self.current_catalog(),
            PreparedStatementMethodType::M4,
            Some(column_indexes),
        )?;
        self.prepare_statement_from_key(key).await
    }

    /// 创建 `prepareStatement(String, String[])` 语义的池化语句。
    pub async fn prepare_statement_with_column_names(
        &mut self,
        sql: &str,
        column_names: Vec<String>,
    ) -> Result<DruidPooledPreparedStatement, DruidError> {
        let key = PreparedStatementKey::with_column_names(
            Some(sql.to_string()),
            self.current_catalog(),
            PreparedStatementMethodType::M5,
            Some(column_names),
        )?;
        self.prepare_statement_from_key(key).await
    }

    /// 创建 `prepareStatement(String, int autoGeneratedKeys)` 语义的池化语句。
    pub async fn prepare_statement_with_auto_generated_keys(
        &mut self,
        sql: &str,
        auto_generated_keys: i32,
    ) -> Result<DruidPooledPreparedStatement, DruidError> {
        let key = PreparedStatementKey::with_auto_generated_keys(
            Some(sql.to_string()),
            self.current_catalog(),
            PreparedStatementMethodType::M6,
            auto_generated_keys,
        )?;
        self.prepare_statement_from_key(key).await
    }

    /// 创建 `prepareCall(String)` 语义的池化调用语句。
    ///
    /// 对应 Java：`DruidPooledConnection#prepareCall(String)`。
    pub async fn prepare_call(
        &mut self,
        sql: &str,
    ) -> Result<DruidPooledCallableStatement, DruidError> {
        let key = PreparedStatementKey::new(
            Some(sql.to_string()),
            self.current_catalog(),
            PreparedStatementMethodType::Precall1,
        )?;
        self.prepare_call_from_key(key).await
    }

    /// 创建 `prepareCall(String, int, int, int)` 语义的池化调用语句。
    ///
    /// 对应 Java：
    /// `DruidPooledConnection#prepareCall(String, int, int, int)`。
    pub async fn prepare_call_with_holdability(
        &mut self,
        sql: &str,
        result_set_type: i32,
        result_set_concurrency: i32,
        result_set_holdability: i32,
    ) -> Result<DruidPooledCallableStatement, DruidError> {
        let key = PreparedStatementKey::with_result_set_holdability(
            Some(sql.to_string()),
            self.current_catalog(),
            PreparedStatementMethodType::Precall2,
            result_set_type,
            result_set_concurrency,
            result_set_holdability,
        )?;
        self.prepare_call_from_key(key).await
    }

    /// 创建 `prepareCall(String, int, int)` 语义的池化调用语句。
    ///
    /// 对应 Java：`DruidPooledConnection#prepareCall(String, int, int)`。
    pub async fn prepare_call_with_result_set(
        &mut self,
        sql: &str,
        result_set_type: i32,
        result_set_concurrency: i32,
    ) -> Result<DruidPooledCallableStatement, DruidError> {
        let key = PreparedStatementKey::with_result_set(
            Some(sql.to_string()),
            self.current_catalog(),
            PreparedStatementMethodType::Precall3,
            result_set_type,
            result_set_concurrency,
        )?;
        self.prepare_call_from_key(key).await
    }

    /// 显式归还连接。
    ///
    /// 该同步兼容入口采用与 Drop 相同的安全策略：仅复用无需异步复位的连接；
    /// 需要完整 Java recycle 语义时应调用异步 `close()`。
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

    fn physical_mut(&mut self) -> Result<&mut (dyn PhysicalConnection + 'static), DruidError> {
        if !self.lease_active.load(Ordering::Acquire) {
            if let Some(holder) = self.holder.as_mut() {
                holder.mark_discarded();
            }
            return Err(DruidError::ConnectionLeaked {
                id: self.id,
                held_for: self.borrowed_at.elapsed(),
            });
        }
        self.holder
            .as_mut()
            .and_then(DruidConnectionHolder::physical_connection_mut)
            .ok_or(DruidError::ConnectionDiscarded)
    }

    fn current_catalog(&self) -> Option<String> {
        self.holder
            .as_ref()
            .and_then(DruidConnectionHolder::physical_connection)
            .and_then(PhysicalConnection::catalog)
            .map(ToOwned::to_owned)
    }

    async fn prepare_statement_from_key(
        &mut self,
        key: PreparedStatementKey,
    ) -> Result<DruidPooledPreparedStatement, DruidError> {
        let statement = self.prepare_from_key(key, false).await?;
        let sql = statement.prepared_statement_holder().key().sql().to_owned();
        if let Some(filter_chain) = &self.filter_chain {
            let event_result = filter_chain
                .after_statement_event_with_identity(
                    self.id,
                    statement.id(),
                    &super::StatementEvent::PrepareStatement(sql),
                )
                .await;
            self.classify_result(event_result)?;
        }
        if let Some(stats) = self.stats_collector.as_ref() {
            stats.statement_stat().increment_prepare_counter();
        }
        Ok(statement)
    }

    async fn prepare_call_from_key(
        &mut self,
        key: PreparedStatementKey,
    ) -> Result<DruidPooledCallableStatement, DruidError> {
        let mut prepared_statement = self.prepare_from_key(key, true).await?;
        let sql = prepared_statement
            .prepared_statement_holder()
            .key()
            .sql()
            .to_owned();
        if prepared_statement
            .prepared_statement_holder()
            .statement()
            .as_callable()
            .is_none()
        {
            prepared_statement.record_exception();
            return Err(DruidError::UnsupportedOperation {
                operation: "prepare_physical_call",
            });
        }
        let statement = DruidPooledCallableStatement::new(prepared_statement);
        if let Some(filter_chain) = &self.filter_chain {
            let event_result = filter_chain
                .after_statement_event_with_identity(
                    self.id,
                    statement.id(),
                    &super::StatementEvent::PrepareCall(sql),
                )
                .await;
            self.classify_result(event_result)?;
        }
        if let Some(stats) = self.stats_collector.as_ref() {
            stats.statement_stat().increment_prepare_call_count();
        }
        Ok(statement)
    }

    async fn prepare_from_key(
        &mut self,
        key: PreparedStatementKey,
        callable: bool,
    ) -> Result<DruidPooledPreparedStatement, DruidError> {
        let key = if let Some(filter_chain) = &self.filter_chain {
            let rewritten_sql = filter_chain.prepare_statement_sql(key.sql())?;
            key.with_sql(rewritten_sql)
        } else {
            key
        };
        let pooled = self
            .holder
            .as_ref()
            .is_some_and(DruidConnectionHolder::is_pool_prepared_statements);
        let statement_pool = if pooled {
            self.holder
                .as_mut()
                .map(DruidConnectionHolder::statement_pool)
        } else {
            None
        };
        let cached = statement_pool.as_ref().and_then(|pool| {
            pool.lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(&key)
        });

        let statement_holder = match cached {
            Some(statement_holder) => statement_holder,
            None => {
                let result = if callable {
                    self.physical_mut()?.prepare_physical_call(&key).await
                } else {
                    self.physical_mut()?.prepare_physical_statement(&key).await
                };
                let statement = self.classify_result(result)?;
                let query_timeout = self.effective_query_timeout();
                if query_timeout > 0 {
                    let timeout_result = statement.set_query_timeout(query_timeout);
                    self.classify_result(timeout_result)?;
                }
                let stats = self
                    .holder
                    .as_ref()
                    .ok_or(DruidError::ConnectionDiscarded)?
                    .prepared_statement_stats()
                    .clone();
                stats.record_prepare();
                Arc::new(PreparedStatementHolder::new(key, statement))
            }
        };
        statement_holder.increment_in_use_count();
        let stats = self
            .holder
            .as_ref()
            .ok_or(DruidError::ConnectionDiscarded)?
            .prepared_statement_stats()
            .clone();
        let statement = DruidPooledPreparedStatement::new(
            statement_holder,
            pooled,
            statement_pool,
            stats,
            self.id,
            self.next_statement_id(),
            Arc::clone(&self.result_set_id_seed),
            Arc::clone(&self.metadata_id_seed),
            self.lease_active.clone(),
            self.filter_chain.clone(),
            self.stats_collector.clone(),
        );
        let holder = self
            .holder
            .as_ref()
            .ok_or(DruidError::ConnectionDiscarded)?;
        holder.add_statement_trace(StatementTrace::Prepared(statement.statement_trace_shared()));
        Ok(statement)
    }

    pub(crate) fn is_same_open_lease(&self, lease_active: &Arc<AtomicBool>) -> bool {
        !self.recycled
            && Arc::ptr_eq(&self.lease_active, lease_active)
            && lease_active.load(Ordering::Acquire)
    }

    fn recycle_once(&mut self, disposition: ConnectionRecycleDisposition) -> bool {
        if self.recycled {
            return false;
        }

        // Java recycle 会先关闭 statementTrace 中仍打开的语句。Prepared /
        // Callable Statement 必须通过共享关闭状态机归还或删除 cache holder，
        // 普通 Statement 则直接关闭物理资源。
        if let Some(holder) = self.holder.as_ref() {
            holder.close_statement_trace();
        }
        // 理论上 trace 关闭后不应再有 active holder；保留防御分支，避免异常
        // 状态进入下一次租约。
        if self
            .holder
            .as_ref()
            .is_some_and(DruidConnectionHolder::has_in_use_prepared_statement)
        {
            if let Some(holder) = self.holder.as_mut() {
                holder.clear_statement_cache();
            }
        }
        self.lease_active.store(false, Ordering::Release);
        self.recycled = true;
        if let (Some(holder), Some(return_connection)) =
            (self.holder.take(), self.return_connection.take())
        {
            holder.clear_connection_event_listeners();
            holder.clear_statement_event_listeners();
            return return_connection(holder, disposition);
        }
        false
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

    fn notify_connection_error(&self, error: &DruidError) {
        if let Some(holder) = self.holder.as_ref() {
            for listener in holder.connection_event_listeners() {
                listener.connection_error_occurred(self.id, error);
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
        let Some(connection) = holder.physical_connection_mut() else {
            return ConnectionRecycleDisposition::discard();
        };

        let requires_async_recycle = self.recycle_validator.is_some()
            || defaults.needs_reset(connection, self.keep_underlying_transaction_isolation);
        if connection.is_closed() || connection.is_discarded() || requires_async_recycle {
            // Drop 不能跨越 await。脏连接不得未经 rollback/reset 重新进入空闲队列。
            holder.mark_discarded();
            ConnectionRecycleDisposition::discard()
        } else {
            ConnectionRecycleDisposition::Reusable
        }
    }

    async fn prepare_for_recycle(&mut self) -> ConnectionRecycleDisposition {
        if !self.lease_active.load(Ordering::Acquire) {
            if let Some(holder) = self.holder.as_mut() {
                holder.mark_discarded();
            }
            return ConnectionRecycleDisposition::discard();
        }
        let already_unusable = self
            .holder
            .as_ref()
            .and_then(DruidConnectionHolder::physical_connection)
            .is_none_or(|connection| connection.is_closed() || connection.is_discarded());
        if already_unusable {
            return ConnectionRecycleDisposition::discard();
        }

        let reset_result = async {
            let (supports_transactions, auto_commit, read_only) = {
                let connection = self
                    .holder
                    .as_ref()
                    .and_then(DruidConnectionHolder::physical_connection)
                    .ok_or(DruidError::ConnectionDiscarded)?;
                (
                    connection.capabilities().transactions,
                    connection.auto_commit(),
                    connection.read_only(),
                )
            };

            // 对应 Java DruidDataSource#recycle：非自动提交且非只读时先回滚。
            if supports_transactions && !auto_commit && !read_only {
                self.before_connection_event(&ConnectionEvent::Rollback)
                    .await?;
                self.physical_mut()?.rollback().await?;
            }

            let keep_isolation = self.keep_underlying_transaction_isolation;
            self.holder
                .as_mut()
                .ok_or(DruidError::ConnectionDiscarded)?
                .reset(keep_isolation)
                .await?;

            Ok::<(), DruidError>(())
        }
        .await;

        if let Err(error) = reset_result {
            // Java recycle 捕获 rollback/reset 异常、丢弃物理连接并记录
            // recycleError，不把回收异常重新抛给 close 调用者。
            return self.discard_for_recycle_error(error).await;
        }

        // Java testConnectionInternal 将 checker 异常折叠为 false；验证失败会销毁
        // 连接，但不计入 recycleErrorCount。
        if let Some(validator) = self.recycle_validator.clone() {
            let validation_failed = match self
                .holder
                .as_mut()
                .and_then(DruidConnectionHolder::physical_connection_box_mut)
            {
                Some(connection) => validator.validate(connection).await.is_err(),
                None => true,
            };
            if validation_failed {
                if let Some(holder) = self.holder.as_mut() {
                    holder.mark_discarded();
                    if let Some(connection) = holder.physical_connection_mut() {
                        let _ = connection.close().await;
                    }
                }
                return ConnectionRecycleDisposition::discard();
            }
            if let Some(holder) = self.holder.as_ref() {
                holder.record_valid();
            }
        }

        // Java 在 testOnReturn 之后恢复 MySQL-family 的 initSchema；失败进入
        // recycle error 分支并丢弃连接。
        let restore_result = match self.holder.as_mut() {
            Some(holder) if holder.should_restore_schema_on_recycle() => {
                holder.restore_initial_schema().await
            }
            _ => Ok(()),
        };
        if let Err(error) = restore_result {
            return self.discard_for_recycle_error(error).await;
        }

        ConnectionRecycleDisposition::Reusable
    }

    /// `dataSource_releaseConnection` around-chain 的末端回收动作。
    ///
    /// 该方法不再次进入 Filter，避免递归；回收错误按 Java
    /// `DruidDataSource#recycle` 规则折叠为 discard disposition。
    pub(crate) async fn recycle_from_data_source_filter(&mut self) -> Result<(), DruidError> {
        if self.recycled {
            return Ok(());
        }
        let disposition = self.prepare_for_recycle().await;
        self.recycle_once(disposition);
        self.close_count = self.close_count.saturating_add(1);
        self.end_transaction_info();
        Ok(())
    }

    async fn discard_for_recycle_error(
        &mut self,
        error: DruidError,
    ) -> ConnectionRecycleDisposition {
        if let Some(holder) = self.holder.as_mut() {
            holder.mark_discarded();
            if let Some(connection) = holder.physical_connection_mut() {
                let _ = connection.close().await;
            }
        }
        ConnectionRecycleDisposition::recycle_error(error)
    }

    async fn before_connection_event(&mut self, event: &ConnectionEvent) -> Result<(), DruidError> {
        match &self.filter_chain {
            Some(filter_chain) => {
                filter_chain
                    .before_connection_event_with_identity(self.id, event)
                    .await
            }
            None => Ok(()),
        }
    }

    /// 返回连接警告链。
    ///
    /// 对应 Java：`DruidPooledConnection#getWarnings()`。调用穿过完整 Filter
    /// around-chain，但 Java 此 getter 不捕获底层 `SQLException`，因此错误不会
    /// 进入连接 `ExceptionSorter`。
    pub async fn warnings(&mut self) -> Result<Option<SqlWarning>, DruidError> {
        let filter_chain = self.filter_chain.clone();
        let physical = self.physical_mut()?;
        match filter_chain {
            Some(filter_chain) => filter_chain.connection_warnings(physical).await,
            None => physical.warnings().await,
        }
    }

    /// 清除连接警告链。
    ///
    /// 对应 Java：`DruidPooledConnection#clearWarnings()`。与 getter 不同，
    /// Java 捕获底层 `SQLException` 并交给 `handleException`，Rust 因此必须
    /// 执行 fatal sorter。
    pub async fn clear_warnings(&mut self) -> Result<(), DruidError> {
        let filter_chain = self.filter_chain.clone();
        let result = {
            let physical = self.physical_mut()?;
            match filter_chain {
                Some(filter_chain) => filter_chain.connection_clear_warnings(physical).await,
                None => physical.clear_warnings().await,
            }
        };
        self.classify_result(result)
    }

    pub(crate) async fn exec_with_filters(
        &mut self,
        sql: &str,
        params: Vec<Value>,
        statement_id: Option<u64>,
    ) -> Result<ExecResult, DruidError> {
        let _execution_running = self.begin_execution()?;
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let data_source = self.data_source.clone();
        let mut context = ExecContext {
            connection_id: self.id,
            statement_id,
            sql: sql.to_owned(),
            params: &params,
            prepared_parameters: None,
            data_source: &data_source,
            start,
            fingerprint: None,
            in_transaction: !self.auto_commit(),
            operation: super::ExecOperation::Update,
        };

        if let Some(filter_chain) = &filter_chain {
            let result = filter_chain.before_execute(&mut context).await;
            self.classify_result_with_sql(result, Some(&context.sql))?;
        }

        // 过滤器需要在 after 阶段观察同一组参数，因此只克隆驱动调用所需所有权。
        let result = self
            .physical_mut()?
            .exec(&context.sql, params.clone())
            .await;
        let result = self.classify_result_with_sql(result, Some(&context.sql));
        self.record_execution(&context.sql, &result);
        if let Some(holder) = self.holder.as_ref() {
            holder.record_execute();
        }
        if let Some(filter_chain) = &filter_chain {
            let after_result = filter_chain
                .after_execute(&context, &result, start.elapsed())
                .await;
            self.classify_result_with_sql(after_result, Some(&context.sql))?;
        }
        result
    }

    /// 以 Java `statement_execute` 边界执行 generic Statement。
    ///
    /// 驱动返回有序 JDBC 结果；Filter before/after 对整个 `execute` 调用各执行
    /// 一次，查询首结果通过 `row_count` 暴露给 after hook，更新首结果保留
    /// `ExecResult` 的更新计数与生成键。
    pub(crate) async fn execute_with_filters(
        &mut self,
        sql: &str,
        generated_keys: StatementGeneratedKeys,
        statement_id: Option<u64>,
    ) -> Result<Vec<StatementExecuteResult>, DruidError> {
        let _execution_running = self.begin_execution()?;
        let params = Vec::<Value>::new();
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let data_source = self.data_source.clone();
        let mut context = ExecContext {
            connection_id: self.id,
            statement_id,
            sql: sql.to_owned(),
            params: &params,
            prepared_parameters: None,
            data_source: &data_source,
            start,
            fingerprint: None,
            in_transaction: !self.auto_commit(),
            operation: super::ExecOperation::Execute,
        };

        if let Some(filter_chain) = &filter_chain {
            let before_result = filter_chain.before_execute(&mut context).await;
            self.classify_result_with_sql(before_result, Some(&context.sql))?;
        }

        let result = self
            .physical_mut()?
            .execute(&context.sql, params.clone(), generated_keys)
            .await;
        let result = self.classify_result_with_sql(result, Some(&context.sql));
        self.record_execution(&context.sql, &result);
        if let Some(holder) = self.holder.as_ref() {
            holder.record_execute();
        }
        if let Some(filter_chain) = &filter_chain {
            let filter_result = match &result {
                Ok(results) => match results.first() {
                    Some(StatementExecuteResult::ResultSet(rows)) => Ok(ExecResult {
                        rows_affected: 0,
                        last_insert_id: None,
                        row_count: Some(rows.len() as u64),
                    }),
                    Some(StatementExecuteResult::Update(execution)) => Ok(execution.clone()),
                    None => Ok(ExecResult::default()),
                },
                Err(error) => Err(error.clone()),
            };
            let after_result = filter_chain
                .after_execute(&context, &filter_result, start.elapsed())
                .await;
            self.classify_result_with_sql(after_result, Some(&context.sql))?;
        }
        result
    }

    /// 以 Java `preparedStatement_execute` 边界执行 generic PreparedStatement。
    ///
    /// 固定预编译 SQL 与本次参数快照只进入一次 Filter before/after；物理
    /// Adapter 返回 query/update 有序结果，不能由 Druid 层分析 SQL 前缀。
    pub(crate) async fn execute_prepared_with_filters(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
        generated_keys: StatementGeneratedKeys,
        statement_id: Option<u64>,
    ) -> Result<Vec<StatementExecuteResult>, DruidError> {
        let _execution_running = self.begin_execution()?;
        let sql = statement.sql().to_string();
        let prepared_parameters = params
            .iter()
            .cloned()
            .map(PreparedInputParameter::RustValue)
            .collect::<Vec<_>>();
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let data_source = self.data_source.clone();
        let mut context = ExecContext {
            connection_id: self.id,
            statement_id,
            sql: sql.clone(),
            params: &params,
            prepared_parameters: Some(&prepared_parameters),
            data_source: &data_source,
            start,
            fingerprint: None,
            in_transaction: !self.auto_commit(),
            operation: super::ExecOperation::Execute,
        };

        if let Some(filter_chain) = &filter_chain {
            let before_result = filter_chain.before_execute(&mut context).await;
            self.classify_result_with_sql(before_result, Some(&sql))?;
        }

        let result = self
            .physical_mut()?
            .execute_prepared(statement, params.clone(), generated_keys)
            .await;
        let result = self.classify_result_with_sql(result, Some(&sql));
        self.record_execution(&sql, &result);
        if let Some(holder) = self.holder.as_ref() {
            holder.record_execute();
        }
        if let Some(filter_chain) = &filter_chain {
            let filter_result = match &result {
                Ok(results) => match results.first() {
                    Some(StatementExecuteResult::ResultSet(rows)) => Ok(ExecResult {
                        rows_affected: 0,
                        last_insert_id: None,
                        row_count: Some(rows.len() as u64),
                    }),
                    Some(StatementExecuteResult::Update(execution)) => Ok(execution.clone()),
                    None => Ok(ExecResult::default()),
                },
                Err(error) => Err(error.clone()),
            };
            let after_result = filter_chain
                .after_execute(&context, &filter_result, start.elapsed())
                .await;
            self.classify_result_with_sql(after_result, Some(&sql))?;
        }
        result
    }

    /// 以完整 setter 描述符执行 generic PreparedStatement。
    pub(crate) async fn execute_prepared_parameters_with_filters(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameters: Vec<PreparedInputParameter>,
        generated_keys: StatementGeneratedKeys,
        statement_id: Option<u64>,
    ) -> Result<Vec<StatementExecuteResult>, DruidError> {
        let _execution_running = self.begin_execution()?;
        let sql = statement.sql().to_string();
        let scalar_params = parameters
            .iter()
            .map(PreparedInputParameter::scalar_value)
            .collect::<Result<Vec<_>, _>>()
            .unwrap_or_default();
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let data_source = self.data_source.clone();
        let mut context = ExecContext {
            connection_id: self.id,
            statement_id,
            sql: sql.clone(),
            params: &scalar_params,
            prepared_parameters: Some(&parameters),
            data_source: &data_source,
            start,
            fingerprint: None,
            in_transaction: !self.auto_commit(),
            operation: super::ExecOperation::Execute,
        };

        if let Some(filter_chain) = &filter_chain {
            let before_result = filter_chain.before_execute(&mut context).await;
            self.classify_result_with_sql(before_result, Some(&sql))?;
        }

        let result = self
            .physical_mut()?
            .execute_prepared_parameters(statement, parameters.clone(), generated_keys)
            .await;
        let result = self.classify_result_with_sql(result, Some(&sql));
        self.record_execution(&sql, &result);
        if let Some(holder) = self.holder.as_ref() {
            holder.record_execute();
        }
        if let Some(filter_chain) = &filter_chain {
            let filter_result = match &result {
                Ok(results) => match results.first() {
                    Some(StatementExecuteResult::ResultSet(rows)) => Ok(ExecResult {
                        rows_affected: 0,
                        last_insert_id: None,
                        row_count: Some(rows.len() as u64),
                    }),
                    Some(StatementExecuteResult::Update(execution)) => Ok(execution.clone()),
                    None => Ok(ExecResult::default()),
                },
                Err(error) => Err(error.clone()),
            };
            let after_result = filter_chain
                .after_execute(&context, &filter_result, start.elapsed())
                .await;
            self.classify_result_with_sql(after_result, Some(&sql))?;
        }
        result
    }

    /// 以单次 Java `statement_executeBatch` Filter 边界执行整个批次。
    ///
    /// `sql` 必须是 `StatementProxy#getBatchSql()` 的 `"\n;\n"` 合并结果；
    /// 物理层返回完整 JDBC 更新计数数组，失败时保留部分计数。
    pub(crate) async fn exec_batch_with_filters(
        &mut self,
        sql: &str,
        statements: &[String],
        statement_id: Option<u64>,
    ) -> Result<Vec<i32>, DruidError> {
        let _execution_running = self.begin_execution()?;
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let data_source = self.data_source.clone();
        let mut context = super::BatchExecContext {
            connection_id: self.id,
            statement_id,
            sql,
            statements,
            parameter_sets: &[],
            prepared_parameter_sets: None,
            kind: super::BatchExecKind::Statement,
            data_source: &data_source,
            start,
            fingerprint: None,
            in_transaction: !self.auto_commit(),
        };

        if let Some(filter_chain) = &filter_chain {
            let before_result = filter_chain.before_batch(&mut context).await;
            self.classify_result_with_sql(before_result, Some(sql))?;
        }

        let batch = statements
            .iter()
            .cloned()
            .map(|statement| (statement, Vec::<Value>::new()))
            .collect();
        let result = self.physical_mut()?.exec_batch(batch).await;
        let result = self.classify_result_with_sql(result, Some(sql));
        self.record_execution(sql, &result);
        if let Some(holder) = self.holder.as_ref() {
            holder.record_execute();
        }
        if let Some(filter_chain) = &filter_chain {
            let after_result = filter_chain
                .after_batch(&context, &result, start.elapsed())
                .await;
            self.classify_result_with_sql(after_result, Some(sql))?;
        }
        result
    }

    /// 以单次 Java `statement_executeBatch` Filter 边界执行 PreparedStatement 批次。
    ///
    /// PreparedStatement 的 `getBatchSql()` 始终为原始预编译 SQL，而继承的
    /// `getBatchSqlList()` 为空；参数批次由物理驱动消费。只有进入物理执行后才
    /// 清空调用方批次，因而 before Filter 短路时仍可重试。
    pub(crate) async fn exec_prepared_batch_with_filters(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameter_sets: &mut Vec<Vec<PreparedInputParameter>>,
        statement_id: Option<u64>,
    ) -> Result<Vec<i32>, DruidError> {
        let _execution_running = self.begin_execution()?;
        let sql = statement.sql().to_string();
        let snapshot = parameter_sets.clone();
        let scalar_snapshot = snapshot
            .iter()
            .map(|parameters| {
                parameters
                    .iter()
                    .map(PreparedInputParameter::scalar_value)
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()
            .unwrap_or_default();
        let statements = Vec::<String>::new();
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let data_source = self.data_source.clone();
        let mut context = super::BatchExecContext {
            connection_id: self.id,
            statement_id,
            sql: &sql,
            statements: &statements,
            parameter_sets: &scalar_snapshot,
            prepared_parameter_sets: Some(&snapshot),
            kind: super::BatchExecKind::PreparedStatement,
            data_source: &data_source,
            start,
            fingerprint: None,
            in_transaction: !self.auto_commit(),
        };

        if let Some(filter_chain) = &filter_chain {
            let before_result = filter_chain.before_batch(&mut context).await;
            self.classify_result_with_sql(before_result, Some(&sql))?;
        }

        let physical = self.physical_mut()?;
        let result = physical
            .exec_prepared_parameter_batch(statement, snapshot.clone())
            .await;
        // JDBC 驱动在 executeBatch 调用后消费参数批次；物理调用前的短路不清空。
        parameter_sets.clear();
        let result = self.classify_result_with_sql(result, Some(&sql));
        self.record_execution(&sql, &result);
        if let Some(holder) = self.holder.as_ref() {
            holder.record_execute();
        }
        if let Some(filter_chain) = &filter_chain {
            let after_result = filter_chain
                .after_batch(&context, &result, start.elapsed())
                .await;
            self.classify_result_with_sql(after_result, Some(&sql))?;
        }
        result
    }

    pub(crate) async fn fetch_with_filters(
        &mut self,
        sql: &str,
        params: Vec<Value>,
        statement_id: Option<u64>,
    ) -> Result<Vec<Row>, DruidError> {
        let _execution_running = self.begin_execution()?;
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let data_source = self.data_source.clone();
        let mut context = ExecContext {
            connection_id: self.id,
            statement_id,
            sql: sql.to_owned(),
            params: &params,
            prepared_parameters: None,
            data_source: &data_source,
            start,
            fingerprint: None,
            in_transaction: !self.auto_commit(),
            operation: super::ExecOperation::Query,
        };

        if let Some(filter_chain) = &filter_chain {
            let result = filter_chain.before_execute(&mut context).await;
            self.classify_result_with_sql(result, Some(&context.sql))?;
        }

        let result = self
            .physical_mut()?
            .fetch(&context.sql, params.clone())
            .await;
        let result = self.classify_result_with_sql(result, Some(&context.sql));
        self.record_execution(&context.sql, &result);
        if let Some(holder) = self.holder.as_ref() {
            holder.record_execute();
        }
        if let Some(filter_chain) = &filter_chain {
            let filter_result = match &result {
                Ok(rows) => Ok(ExecResult {
                    rows_affected: 0,
                    last_insert_id: None,
                    row_count: Some(rows.len() as u64),
                }),
                Err(error) => Err(error.clone()),
            };
            let after_result = filter_chain
                .after_execute(&context, &filter_result, start.elapsed())
                .await;
            self.classify_result_with_sql(after_result, Some(&context.sql))?;
        }
        result
    }

    /// 以 Java `Statement#executeQuery` 边界返回物理 `ResultSet`。
    ///
    /// Filter before/after 仍各执行一次；物理结果集的行数只有在游标抓取时才能
    /// 确定，因此 SQL after 事件不伪造 eager `row_count`。
    pub(crate) async fn fetch_result_set_with_filters(
        &mut self,
        sql: &str,
        params: Vec<Value>,
        statement_id: Option<u64>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let _execution_running = self.begin_execution()?;
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let data_source = self.data_source.clone();
        let mut context = ExecContext {
            connection_id: self.id,
            statement_id,
            sql: sql.to_owned(),
            params: &params,
            prepared_parameters: None,
            data_source: &data_source,
            start,
            fingerprint: None,
            in_transaction: !self.auto_commit(),
            operation: super::ExecOperation::Query,
        };

        if let Some(filter_chain) = &filter_chain {
            let result = filter_chain.before_execute(&mut context).await;
            self.classify_result_with_sql(result, Some(&context.sql))?;
        }

        let result = self
            .physical_mut()?
            .fetch_result_set(&context.sql, params.clone())
            .await;
        let result = self.classify_result_with_sql(result, Some(&context.sql));
        self.record_execution(&context.sql, &result);
        if let Some(holder) = self.holder.as_ref() {
            holder.record_execute();
        }
        if let Some(filter_chain) = &filter_chain {
            let filter_result = match &result {
                Ok(_) => Ok(ExecResult {
                    rows_affected: 0,
                    last_insert_id: None,
                    row_count: None,
                }),
                Err(error) => Err(error.clone()),
            };
            let after_result = filter_chain
                .after_execute(&context, &filter_result, start.elapsed())
                .await;
            self.classify_result_with_sql(after_result, Some(&context.sql))?;
        }
        result
    }

    pub(crate) async fn exec_prepared_with_filters(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
        statement_id: Option<u64>,
    ) -> Result<ExecResult, DruidError> {
        let _execution_running = self.begin_execution()?;
        let sql = statement.sql().to_string();
        let prepared_parameters = params
            .iter()
            .cloned()
            .map(PreparedInputParameter::RustValue)
            .collect::<Vec<_>>();
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let data_source = self.data_source.clone();
        let mut context = ExecContext {
            connection_id: self.id,
            statement_id,
            sql: sql.clone(),
            params: &params,
            prepared_parameters: Some(&prepared_parameters),
            data_source: &data_source,
            start,
            fingerprint: None,
            in_transaction: !self.auto_commit(),
            operation: super::ExecOperation::Update,
        };
        if let Some(filter_chain) = &filter_chain {
            let result = filter_chain.before_execute(&mut context).await;
            self.classify_result_with_sql(result, Some(&sql))?;
        }
        let result = self
            .physical_mut()?
            .exec_prepared(statement, params.clone())
            .await;
        let result = self.classify_result_with_sql(result, Some(statement.sql()));
        self.record_execution(statement.sql(), &result);
        if let Some(holder) = self.holder.as_ref() {
            holder.record_execute();
        }
        if let Some(filter_chain) = &filter_chain {
            let after_result = filter_chain
                .after_execute(&context, &result, start.elapsed())
                .await;
            self.classify_result_with_sql(after_result, Some(&sql))?;
        }
        result
    }

    /// 以完整 setter 描述符执行更新 PreparedStatement。
    pub(crate) async fn exec_prepared_parameters_with_filters(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameters: Vec<PreparedInputParameter>,
        statement_id: Option<u64>,
    ) -> Result<ExecResult, DruidError> {
        let _execution_running = self.begin_execution()?;
        let sql = statement.sql().to_string();
        let scalar_params = parameters
            .iter()
            .map(PreparedInputParameter::scalar_value)
            .collect::<Result<Vec<_>, _>>()
            .unwrap_or_default();
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let data_source = self.data_source.clone();
        let mut context = ExecContext {
            connection_id: self.id,
            statement_id,
            sql: sql.clone(),
            params: &scalar_params,
            prepared_parameters: Some(&parameters),
            data_source: &data_source,
            start,
            fingerprint: None,
            in_transaction: !self.auto_commit(),
            operation: super::ExecOperation::Update,
        };
        if let Some(filter_chain) = &filter_chain {
            let result = filter_chain.before_execute(&mut context).await;
            self.classify_result_with_sql(result, Some(&sql))?;
        }
        let result = self
            .physical_mut()?
            .exec_prepared_parameters(statement, parameters.clone())
            .await;
        let result = self.classify_result_with_sql(result, Some(statement.sql()));
        self.record_execution(statement.sql(), &result);
        if let Some(holder) = self.holder.as_ref() {
            holder.record_execute();
        }
        if let Some(filter_chain) = &filter_chain {
            let after_result = filter_chain
                .after_execute(&context, &result, start.elapsed())
                .await;
            self.classify_result_with_sql(after_result, Some(&sql))?;
        }
        result
    }

    pub(crate) async fn fetch_prepared_with_filters(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
        statement_id: Option<u64>,
    ) -> Result<Vec<Row>, DruidError> {
        let _execution_running = self.begin_execution()?;
        let sql = statement.sql().to_string();
        let prepared_parameters = params
            .iter()
            .cloned()
            .map(PreparedInputParameter::RustValue)
            .collect::<Vec<_>>();
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let data_source = self.data_source.clone();
        let mut context = ExecContext {
            connection_id: self.id,
            statement_id,
            sql: sql.clone(),
            params: &params,
            prepared_parameters: Some(&prepared_parameters),
            data_source: &data_source,
            start,
            fingerprint: None,
            in_transaction: !self.auto_commit(),
            operation: super::ExecOperation::Query,
        };
        if let Some(filter_chain) = &filter_chain {
            let result = filter_chain.before_execute(&mut context).await;
            self.classify_result_with_sql(result, Some(&sql))?;
        }
        let result = self
            .physical_mut()?
            .fetch_prepared(statement, params.clone())
            .await;
        let result = self.classify_result_with_sql(result, Some(statement.sql()));
        self.record_execution(statement.sql(), &result);
        if let Some(holder) = self.holder.as_ref() {
            holder.record_execute();
        }
        if let Some(filter_chain) = &filter_chain {
            let filter_result = match &result {
                Ok(rows) => Ok(ExecResult {
                    rows_affected: 0,
                    last_insert_id: None,
                    row_count: Some(rows.len() as u64),
                }),
                Err(error) => Err(error.clone()),
            };
            let after_result = filter_chain
                .after_execute(&context, &filter_result, start.elapsed())
                .await;
            self.classify_result_with_sql(after_result, Some(&sql))?;
        }
        result
    }

    /// 执行物理预编译查询，同时保留驱动级 `ResultSet` 身份和列元数据。
    pub(crate) async fn fetch_prepared_result_set_with_filters(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
        statement_id: Option<u64>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let _execution_running = self.begin_execution()?;
        let sql = statement.sql().to_string();
        let prepared_parameters = params
            .iter()
            .cloned()
            .map(PreparedInputParameter::RustValue)
            .collect::<Vec<_>>();
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let data_source = self.data_source.clone();
        let mut context = ExecContext {
            connection_id: self.id,
            statement_id,
            sql: sql.clone(),
            params: &params,
            prepared_parameters: Some(&prepared_parameters),
            data_source: &data_source,
            start,
            fingerprint: None,
            in_transaction: !self.auto_commit(),
            operation: super::ExecOperation::Query,
        };
        if let Some(filter_chain) = &filter_chain {
            let result = filter_chain.before_execute(&mut context).await;
            self.classify_result_with_sql(result, Some(&sql))?;
        }
        let result = self
            .physical_mut()?
            .fetch_prepared_result_set(statement, params.clone())
            .await;
        let result = self.classify_result_with_sql(result, Some(statement.sql()));
        self.record_execution(statement.sql(), &result);
        if let Some(holder) = self.holder.as_ref() {
            holder.record_execute();
        }
        if let Some(filter_chain) = &filter_chain {
            let filter_result = result.as_ref().map(|_| ExecResult {
                rows_affected: 0,
                last_insert_id: None,
                row_count: None,
            });
            let filter_result = filter_result.map_err(Clone::clone);
            let after_result = filter_chain
                .after_execute(&context, &filter_result, start.elapsed())
                .await;
            self.classify_result_with_sql(after_result, Some(&sql))?;
        }
        result
    }

    /// 以完整 setter 描述符执行查询 PreparedStatement。
    pub(crate) async fn fetch_prepared_parameters_with_filters(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameters: Vec<PreparedInputParameter>,
        statement_id: Option<u64>,
    ) -> Result<Vec<Row>, DruidError> {
        let _execution_running = self.begin_execution()?;
        let sql = statement.sql().to_string();
        let scalar_params = parameters
            .iter()
            .map(PreparedInputParameter::scalar_value)
            .collect::<Result<Vec<_>, _>>()
            .unwrap_or_default();
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let data_source = self.data_source.clone();
        let mut context = ExecContext {
            connection_id: self.id,
            statement_id,
            sql: sql.clone(),
            params: &scalar_params,
            prepared_parameters: Some(&parameters),
            data_source: &data_source,
            start,
            fingerprint: None,
            in_transaction: !self.auto_commit(),
            operation: super::ExecOperation::Query,
        };
        if let Some(filter_chain) = &filter_chain {
            let result = filter_chain.before_execute(&mut context).await;
            self.classify_result_with_sql(result, Some(&sql))?;
        }
        let result = self
            .physical_mut()?
            .fetch_prepared_parameters(statement, parameters.clone())
            .await;
        let result = self.classify_result_with_sql(result, Some(statement.sql()));
        self.record_execution(statement.sql(), &result);
        if let Some(holder) = self.holder.as_ref() {
            holder.record_execute();
        }
        if let Some(filter_chain) = &filter_chain {
            let filter_result = match &result {
                Ok(rows) => Ok(ExecResult {
                    rows_affected: 0,
                    last_insert_id: None,
                    row_count: Some(rows.len() as u64),
                }),
                Err(error) => Err(error.clone()),
            };
            let after_result = filter_chain
                .after_execute(&context, &filter_result, start.elapsed())
                .await;
            self.classify_result_with_sql(after_result, Some(&sql))?;
        }
        result
    }

    /// 以完整 setter 描述符执行查询，并保留驱动级 `ResultSet` 身份和列元数据。
    pub(crate) async fn fetch_prepared_parameters_result_set_with_filters(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameters: Vec<PreparedInputParameter>,
        statement_id: Option<u64>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let _execution_running = self.begin_execution()?;
        let sql = statement.sql().to_string();
        let scalar_params = parameters
            .iter()
            .map(PreparedInputParameter::scalar_value)
            .collect::<Result<Vec<_>, _>>()
            .unwrap_or_default();
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let data_source = self.data_source.clone();
        let mut context = ExecContext {
            connection_id: self.id,
            statement_id,
            sql: sql.clone(),
            params: &scalar_params,
            prepared_parameters: Some(&parameters),
            data_source: &data_source,
            start,
            fingerprint: None,
            in_transaction: !self.auto_commit(),
            operation: super::ExecOperation::Query,
        };
        if let Some(filter_chain) = &filter_chain {
            let result = filter_chain.before_execute(&mut context).await;
            self.classify_result_with_sql(result, Some(&sql))?;
        }
        let result = self
            .physical_mut()?
            .fetch_prepared_parameters_result_set(statement, parameters.clone())
            .await;
        let result = self.classify_result_with_sql(result, Some(statement.sql()));
        self.record_execution(statement.sql(), &result);
        if let Some(holder) = self.holder.as_ref() {
            holder.record_execute();
        }
        if let Some(filter_chain) = &filter_chain {
            let filter_result = result.as_ref().map(|_| ExecResult {
                rows_affected: 0,
                last_insert_id: None,
                row_count: None,
            });
            let filter_result = filter_result.map_err(Clone::clone);
            let after_result = filter_chain
                .after_execute(&context, &filter_result, start.elapsed())
                .await;
            self.classify_result_with_sql(after_result, Some(&sql))?;
        }
        result
    }
}

#[async_trait::async_trait]
impl PhysicalConnection for DruidPooledConnection {
    async fn exec(&mut self, sql: &str, params: Vec<Value>) -> Result<ExecResult, DruidError> {
        self.exec_with_filters(sql, params, None).await
    }

    async fn execute(
        &mut self,
        sql: &str,
        params: Vec<Value>,
        generated_keys: StatementGeneratedKeys,
    ) -> Result<Vec<StatementExecuteResult>, DruidError> {
        if params.is_empty() {
            self.execute_with_filters(sql, generated_keys, None).await
        } else {
            Err(DruidError::InvalidArgument(
                "DruidPooledStatement generic execute does not accept bind parameters".to_string(),
            ))
        }
    }

    async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        self.fetch_with_filters(sql, params, None).await
    }

    async fn prepare_physical_statement(
        &mut self,
        key: &PreparedStatementKey,
    ) -> Result<Arc<dyn PhysicalPreparedStatement>, DruidError> {
        let result = self.physical_mut()?.prepare_physical_statement(key).await;
        self.classify_result(result)
    }

    async fn prepare_physical_call(
        &mut self,
        key: &PreparedStatementKey,
    ) -> Result<Arc<dyn PhysicalPreparedStatement>, DruidError> {
        let result = self.physical_mut()?.prepare_physical_call(key).await;
        self.classify_result(result)
    }

    async fn exec_prepared(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
    ) -> Result<ExecResult, DruidError> {
        self.exec_prepared_with_filters(statement, params, None)
            .await
    }

    async fn exec_prepared_batch(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameter_sets: Vec<Vec<Value>>,
    ) -> Result<Vec<i32>, DruidError> {
        let mut parameter_sets = parameter_sets
            .into_iter()
            .map(|parameters| {
                parameters
                    .into_iter()
                    .map(PreparedInputParameter::RustValue)
                    .collect()
            })
            .collect();
        self.exec_prepared_batch_with_filters(statement, &mut parameter_sets, None)
            .await
    }

    async fn exec_prepared_parameter_batch(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        mut parameter_sets: Vec<Vec<PreparedInputParameter>>,
    ) -> Result<Vec<i32>, DruidError> {
        self.exec_prepared_batch_with_filters(statement, &mut parameter_sets, None)
            .await
    }

    async fn fetch_prepared(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
    ) -> Result<Vec<Row>, DruidError> {
        self.fetch_prepared_with_filters(statement, params, None)
            .await
    }

    async fn close_prepared_statement(
        &mut self,
        statement: Arc<dyn PhysicalPreparedStatement>,
    ) -> Result<(), DruidError> {
        let result = self
            .physical_mut()?
            .close_prepared_statement(statement)
            .await;
        self.classify_result(result)
    }

    async fn begin(&mut self) -> Result<(), DruidError> {
        let result = self.physical_mut()?.begin().await;
        let result = self.classify_result(result);
        if result.is_ok() && self.transaction_started_at.is_none() {
            self.transaction_started_at = Some(Instant::now());
            self.ensure_transaction_info();
            if let Some(stats) = self.stats_collector.as_ref() {
                stats.record_start_transaction();
            }
        }
        result
    }

    async fn commit(&mut self) -> Result<(), DruidError> {
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let result = self.before_connection_event(&ConnectionEvent::Commit).await;
        self.classify_result(result)?;
        let transaction_elapsed = self
            .transaction_started_at
            .take()
            .map(|started_at| started_at.elapsed());
        if let Some(stats) = self.stats_collector.as_ref() {
            stats.record_commit(transaction_elapsed);
        }
        let result = self.physical_mut()?.commit().await;
        let result = self.classify_result(result);
        self.end_transaction_info();
        result?;
        if let Some(filter_chain) = filter_chain {
            filter_chain
                .after_connection_event_with_identity(
                    self.id,
                    &ConnectionEvent::Commit,
                    start.elapsed(),
                )
                .await?;
        }
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), DruidError> {
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let result = self
            .before_connection_event(&ConnectionEvent::Rollback)
            .await;
        self.classify_result(result)?;
        let transaction_elapsed = self
            .transaction_started_at
            .take()
            .map(|started_at| started_at.elapsed());
        if let Some(stats) = self.stats_collector.as_ref() {
            stats.record_rollback(transaction_elapsed);
        }
        let result = self.physical_mut()?.rollback().await;
        let result = self.classify_result(result);
        self.end_transaction_info();
        result?;
        if let Some(filter_chain) = filter_chain {
            filter_chain
                .after_connection_event_with_identity(
                    self.id,
                    &ConnectionEvent::Rollback,
                    start.elapsed(),
                )
                .await?;
        }
        Ok(())
    }

    async fn rollback_to(&mut self, savepoint: &Savepoint) -> Result<(), DruidError> {
        if let Some(stats) = self.stats_collector.as_ref() {
            stats.record_rollback(None);
        }
        let result = self.physical_mut()?.rollback_to(savepoint).await;
        self.classify_result(result)
    }

    async fn set_savepoint(&mut self) -> Result<Savepoint, DruidError> {
        let result = self.physical_mut()?.set_savepoint().await;
        self.classify_result(result)
    }

    async fn set_savepoint_named(&mut self, name: &str) -> Result<Savepoint, DruidError> {
        let result = self.physical_mut()?.set_savepoint_named(name).await;
        self.classify_result(result)
    }

    async fn release_savepoint(&mut self, savepoint: &Savepoint) -> Result<(), DruidError> {
        let result = self.physical_mut()?.release_savepoint(savepoint).await;
        self.classify_result(result)
    }

    async fn abort(&mut self) -> Result<(), DruidError> {
        let before_result = self.before_connection_event(&ConnectionEvent::Abort).await;
        let before_result = self.classify_result(before_result);
        let result = match before_result {
            Ok(()) => {
                let result = self.physical_mut()?.abort().await;
                self.classify_result(result)
            }
            Err(error) => Err(error),
        };
        if let Some(holder) = self.holder.as_mut() {
            holder.mark_discarded();
        }
        self.recycle_once(ConnectionRecycleDisposition::discard());
        result
    }

    async fn ping(&mut self) -> Result<(), DruidError> {
        let result = self
            .before_connection_event(&ConnectionEvent::IsValid)
            .await;
        self.classify_result(result)?;
        let result = self.physical_mut()?.ping().await;
        let result = self.classify_result(result);
        if result.is_ok() {
            self.last_validate_time_millis = now_millis();
            if let Some(handler) = self.fatal_error_handler.as_ref() {
                handler.clear_on_fatal_error();
            }
        }
        result
    }

    async fn close(&mut self) -> Result<(), DruidError> {
        if self.recycled {
            return Ok(());
        }

        self.notify_connection_closed();
        let filter_chain = self.filter_chain.clone();
        let release_result = match filter_chain {
            Some(filter_chain) if !filter_chain.is_empty() => {
                filter_chain.data_source_release_connection(self).await
            }
            _ => self.recycle_from_data_source_filter().await,
        };
        if let Err(error) = release_result {
            if !self.recycled {
                // Java 在 Filter 到达 recycle 末端前报错时不会设置 disable；
                // 下一次 close 会重新通知 ConnectionEventListener 并重试整条链。
                self.connection_closed_notified = false;
            }
            return Err(error);
        }
        if !self.recycled {
            // Java Filter 成功短路后 close 仍设置 disable=true，但 holder 不会
            // 被数据源回收。标记逻辑租约关闭并丢弃 return callback，保留相同
            // active-count 泄漏责任，不由 Drop 悄悄修复自定义 Filter。
            self.lease_active.store(false, Ordering::Release);
            self.recycled = true;
            self.return_connection.take();
        }
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.recycled
    }

    fn capabilities(&self) -> PhysicalConnectionCapabilities {
        self.holder
            .as_ref()
            .and_then(DruidConnectionHolder::physical_connection)
            .map_or_else(PhysicalConnectionCapabilities::default, |connection| {
                connection.capabilities()
            })
    }

    fn auto_commit(&self) -> bool {
        self.holder
            .as_ref()
            .and_then(DruidConnectionHolder::physical_connection)
            .map_or(true, |connection| connection.auto_commit())
    }

    async fn set_auto_commit(&mut self, auto_commit: bool) -> Result<(), DruidError> {
        let result = self
            .before_connection_event(&ConnectionEvent::SetAutoCommit(auto_commit))
            .await;
        self.classify_result(result)?;
        let result = self.physical_mut()?.set_auto_commit(auto_commit).await;
        let result = self.classify_result(result);
        if result.is_ok() && auto_commit {
            self.end_transaction_info();
        }
        result
    }

    fn read_only(&self) -> bool {
        self.holder
            .as_ref()
            .and_then(DruidConnectionHolder::physical_connection)
            .is_some_and(|connection| connection.read_only())
    }

    async fn set_read_only(&mut self, read_only: bool) -> Result<(), DruidError> {
        let result = self
            .before_connection_event(&ConnectionEvent::SetReadOnly(read_only))
            .await;
        self.classify_result(result)?;
        let result = self.physical_mut()?.set_read_only(read_only).await;
        self.classify_result(result)
    }

    fn transaction_isolation(&self) -> u8 {
        self.holder
            .as_ref()
            .and_then(DruidConnectionHolder::physical_connection)
            .map_or(2, |connection| connection.transaction_isolation())
    }

    async fn set_transaction_isolation(&mut self, level: u8) -> Result<(), DruidError> {
        let result = self
            .before_connection_event(&ConnectionEvent::SetTransactionIsolation(level))
            .await;
        self.classify_result(result)?;
        let result = self.physical_mut()?.set_transaction_isolation(level).await;
        self.classify_result(result)
    }

    fn holdability(&self) -> i32 {
        self.holder
            .as_ref()
            .and_then(DruidConnectionHolder::physical_connection)
            .map_or(0, |connection| connection.holdability())
    }

    async fn set_holdability(&mut self, holdability: i32) -> Result<(), DruidError> {
        let result = self.physical_mut()?.set_holdability(holdability).await;
        self.classify_result(result)
    }

    async fn warnings(&mut self) -> Result<Option<SqlWarning>, DruidError> {
        DruidPooledConnection::warnings(self).await
    }

    async fn clear_warnings(&mut self) -> Result<(), DruidError> {
        DruidPooledConnection::clear_warnings(self).await
    }

    fn mark_discarded(&mut self) {
        if let Some(holder) = self.holder.as_mut() {
            holder.mark_discarded();
        }
    }

    fn is_discarded(&self) -> bool {
        self.holder
            .as_ref()
            .is_none_or(DruidConnectionHolder::is_discard)
    }

    fn catalog(&self) -> Option<&str> {
        self.holder
            .as_ref()
            .and_then(DruidConnectionHolder::physical_connection)
            .and_then(|connection| connection.catalog())
    }

    async fn set_catalog(&mut self, catalog: &str) -> Result<(), DruidError> {
        let result = self
            .before_connection_event(&ConnectionEvent::SetCatalog(catalog.to_string()))
            .await;
        self.classify_result(result)?;
        let result = self.physical_mut()?.set_catalog(catalog).await;
        self.classify_result(result)
    }

    fn schema(&self) -> Option<&str> {
        self.holder
            .as_ref()
            .and_then(DruidConnectionHolder::physical_connection)
            .and_then(|connection| connection.schema())
    }

    async fn set_schema(&mut self, schema: &str) -> Result<(), DruidError> {
        let result = self
            .before_connection_event(&ConnectionEvent::SetSchema(schema.to_string()))
            .await;
        self.classify_result(result)?;
        let initial_schema = self.holder.as_ref().and_then(|holder| {
            holder
                .should_restore_schema_on_recycle()
                .then(|| {
                    holder
                        .physical_connection()
                        .and_then(PhysicalConnection::schema)
                        .map(ToOwned::to_owned)
                })
                .flatten()
        });
        if let Some(holder) = self.holder.as_ref() {
            holder.remember_initial_schema(initial_schema);
        }
        let result = self.physical_mut()?.set_schema(schema).await;
        self.classify_result(result)?;
        if let Some(holder) = self.holder.as_mut() {
            if holder.should_restore_schema_on_recycle() && holder.statement_pool_direct().is_some()
            {
                holder.clear_statement_cache();
            }
        }
        Ok(())
    }

    fn driver_name(&self) -> &str {
        self.holder
            .as_ref()
            .and_then(DruidConnectionHolder::physical_connection)
            .map_or("", |connection| connection.driver_name())
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
