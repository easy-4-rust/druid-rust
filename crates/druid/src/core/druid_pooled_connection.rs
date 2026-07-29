//! 对外池化连接。

use super::connection_defaults::ConnectionDefaults;
use super::connection_event_listener::ConnectionEventListener;
use super::connection_recycle_disposition::ConnectionRecycleDisposition;
use super::druid_connection_holder::DruidConnectionHolder;
use super::druid_pooled_callable_statement::DruidPooledCallableStatement;
use super::druid_pooled_prepared_statement::DruidPooledPreparedStatement;
use super::druid_pooled_statement::DruidPooledStatement;
use super::error::DruidError;
use super::exception_sorter::ExceptionSorter;
use super::exec_result::ExecResult;
use super::filter::{ConnectionEvent, ExecContext};
use super::filter_chain::FilterChain;
use super::jdbc_result_set::PhysicalResultSet;
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
use super::row::Row;
use super::savepoint::Savepoint;
use super::sql_warning::SqlWarning;
use super::value::Value;
use super::wrapper::{Unwrapped, Wrapper};
use std::any::{Any, TypeId};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Native pool 回收完整 holder 的一次性回调。
pub type DruidConnectionReturnCallback =
    Box<dyn FnOnce(DruidConnectionHolder, ConnectionRecycleDisposition) + Send>;

/// 一次数据库执行的运行中标记，离开作用域时无条件复位。
struct ExecutionRunningGuard {
    execution_running: Arc<AtomicBool>,
}

impl ExecutionRunningGuard {
    fn new(execution_running: Arc<AtomicBool>) -> Self {
        execution_running.store(true, Ordering::Release);
        Self { execution_running }
    }
}

impl Drop for ExecutionRunningGuard {
    fn drop(&mut self) {
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
    return_connection: Option<DruidConnectionReturnCallback>,
    lease_active: Arc<AtomicBool>,
    execution_running: Arc<AtomicBool>,
    borrowed_at: Instant,
    recycled: bool,
    connection_closed_notified: bool,
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
        Self {
            holder: Some(holder),
            id,
            data_source,
            filter_chain,
            keep_underlying_transaction_isolation,
            recycle_validator,
            exception_sorter: None,
            return_connection: Some(return_connection),
            lease_active: Arc::new(AtomicBool::new(true)),
            execution_running: Arc::new(AtomicBool::new(false)),
            borrowed_at: Instant::now(),
            recycled: false,
            connection_closed_notified: false,
        }
    }

    /// 返回连接 ID。
    pub fn id(&self) -> u64 {
        self.id
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

    fn begin_execution(&self) -> Result<ExecutionRunningGuard, DruidError> {
        if !self.lease_active.load(Ordering::Acquire) {
            return Err(DruidError::ConnectionLeaked {
                id: self.id,
                held_for: self.borrowed_at.elapsed(),
            });
        }
        Ok(ExecutionRunningGuard::new(Arc::clone(
            &self.execution_running,
        )))
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

    /// 处理驱动错误并返回它是否使连接不可复用。
    ///
    /// 对应 Java：
    /// `DruidDataSource#handleConnectionException` → `handleFatalError`。
    /// 非 SQL 错误和未命中 sorter 的 SQL 错误保持连接可复用。
    pub fn handle_exception(&mut self, error: &DruidError) -> bool {
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
        if let Err(error) = &result {
            if error.sql_exception().is_some() {
                self.notify_connection_error(error);
            }
            self.handle_exception(error);
        }
        result
    }

    /// 返回底层物理连接的可变引用；连接已归还时返回 `None`。
    pub fn physical_connection_mut(&mut self) -> Option<&mut (dyn PhysicalConnection + 'static)> {
        self.holder
            .as_mut()
            .and_then(DruidConnectionHolder::physical_connection_mut)
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
        if let Some(filter_chain) = &self.filter_chain {
            let event_result = filter_chain
                .after_statement_event(&super::StatementEvent::CreateStatement)
                .await;
            self.classify_result(event_result)?;
        }
        Ok(DruidPooledStatement::new(
            statement,
            self.lease_active.clone(),
            self.filter_chain.clone(),
        ))
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
        let sql = key.sql().to_string();
        let statement = self.prepare_from_key(key, false).await?;
        if let Some(filter_chain) = &self.filter_chain {
            let event_result = filter_chain
                .after_statement_event(&super::StatementEvent::PrepareStatement(sql))
                .await;
            self.classify_result(event_result)?;
        }
        Ok(statement)
    }

    async fn prepare_call_from_key(
        &mut self,
        key: PreparedStatementKey,
    ) -> Result<DruidPooledCallableStatement, DruidError> {
        let sql = key.sql().to_string();
        let mut prepared_statement = self.prepare_from_key(key, true).await?;
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
                .after_statement_event(&super::StatementEvent::PrepareCall(sql))
                .await;
            self.classify_result(event_result)?;
        }
        Ok(statement)
    }

    async fn prepare_from_key(
        &mut self,
        key: PreparedStatementKey,
        callable: bool,
    ) -> Result<DruidPooledPreparedStatement, DruidError> {
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
        Ok(DruidPooledPreparedStatement::new(
            statement_holder,
            pooled,
            statement_pool,
            stats,
            self.lease_active.clone(),
            self.filter_chain.clone(),
        ))
    }

    pub(crate) fn is_same_open_lease(&self, lease_active: &Arc<AtomicBool>) -> bool {
        !self.recycled
            && Arc::ptr_eq(&self.lease_active, lease_active)
            && lease_active.load(Ordering::Acquire)
    }

    fn recycle_once(&mut self, disposition: ConnectionRecycleDisposition) {
        if self.recycled {
            return;
        }

        // Java recycle 会关闭 statementTrace 中仍打开的语句。Rust 句柄不长期
        // 借用连接：若缓存里还有 active holder，先清空映射，防止旧租约句柄在
        // 下一次借出后重新污染同一物理连接的缓存。
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
            return_connection(holder, disposition);
        }
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
            Some(filter_chain) => filter_chain.before_connection_event(event).await,
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
    ) -> Result<ExecResult, DruidError> {
        let _execution_running = self.begin_execution()?;
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let data_source = self.data_source.clone();
        let mut context = ExecContext {
            sql,
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
            self.classify_result(result)?;
        }

        // 过滤器需要在 after 阶段观察同一组参数，因此只克隆驱动调用所需所有权。
        let result = self.physical_mut()?.exec(sql, params.clone()).await;
        let result = self.classify_result(result);
        if let Some(holder) = self.holder.as_ref() {
            holder.record_execute();
        }
        if let Some(filter_chain) = &filter_chain {
            filter_chain
                .after_execute(&context, &result, start.elapsed())
                .await?;
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
    ) -> Result<Vec<StatementExecuteResult>, DruidError> {
        let _execution_running = self.begin_execution()?;
        let params = Vec::<Value>::new();
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let data_source = self.data_source.clone();
        let mut context = ExecContext {
            sql,
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
            self.classify_result(before_result)?;
        }

        let result = self
            .physical_mut()?
            .execute(sql, params.clone(), generated_keys)
            .await;
        let result = self.classify_result(result);
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
            self.classify_result(after_result)?;
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
    ) -> Result<Vec<StatementExecuteResult>, DruidError> {
        let _execution_running = self.begin_execution()?;
        let sql = statement.sql().to_string();
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let data_source = self.data_source.clone();
        let mut context = ExecContext {
            sql: &sql,
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
            self.classify_result(before_result)?;
        }

        let result = self
            .physical_mut()?
            .execute_prepared(statement, params.clone(), generated_keys)
            .await;
        let result = self.classify_result(result);
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
            self.classify_result(after_result)?;
        }
        result
    }

    /// 以完整 setter 描述符执行 generic PreparedStatement。
    pub(crate) async fn execute_prepared_parameters_with_filters(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameters: Vec<PreparedInputParameter>,
        generated_keys: StatementGeneratedKeys,
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
            sql: &sql,
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
            self.classify_result(before_result)?;
        }

        let result = self
            .physical_mut()?
            .execute_prepared_parameters(statement, parameters.clone(), generated_keys)
            .await;
        let result = self.classify_result(result);
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
            self.classify_result(after_result)?;
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
    ) -> Result<Vec<i32>, DruidError> {
        let _execution_running = self.begin_execution()?;
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let data_source = self.data_source.clone();
        let mut context = super::BatchExecContext {
            sql,
            statements,
            parameter_sets: &[],
            prepared_parameter_sets: None,
            kind: super::BatchExecKind::Statement,
            data_source: &data_source,
            start,
            in_transaction: !self.auto_commit(),
        };

        if let Some(filter_chain) = &filter_chain {
            let before_result = filter_chain.before_batch(&mut context).await;
            self.classify_result(before_result)?;
        }

        let batch = statements
            .iter()
            .cloned()
            .map(|statement| (statement, Vec::<Value>::new()))
            .collect();
        let result = self.physical_mut()?.exec_batch(batch).await;
        let result = self.classify_result(result);
        if let Some(holder) = self.holder.as_ref() {
            holder.record_execute();
        }
        if let Some(filter_chain) = &filter_chain {
            let after_result = filter_chain
                .after_batch(&context, &result, start.elapsed())
                .await;
            self.classify_result(after_result)?;
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
            sql: &sql,
            statements: &statements,
            parameter_sets: &scalar_snapshot,
            prepared_parameter_sets: Some(&snapshot),
            kind: super::BatchExecKind::PreparedStatement,
            data_source: &data_source,
            start,
            in_transaction: !self.auto_commit(),
        };

        if let Some(filter_chain) = &filter_chain {
            let before_result = filter_chain.before_batch(&mut context).await;
            self.classify_result(before_result)?;
        }

        let physical = self.physical_mut()?;
        let result = physical
            .exec_prepared_parameter_batch(statement, snapshot.clone())
            .await;
        // JDBC 驱动在 executeBatch 调用后消费参数批次；物理调用前的短路不清空。
        parameter_sets.clear();
        let result = self.classify_result(result);
        if let Some(holder) = self.holder.as_ref() {
            holder.record_execute();
        }
        if let Some(filter_chain) = &filter_chain {
            let after_result = filter_chain
                .after_batch(&context, &result, start.elapsed())
                .await;
            self.classify_result(after_result)?;
        }
        result
    }

    pub(crate) async fn fetch_with_filters(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<Vec<Row>, DruidError> {
        let _execution_running = self.begin_execution()?;
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let data_source = self.data_source.clone();
        let mut context = ExecContext {
            sql,
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
            self.classify_result(result)?;
        }

        let result = self.physical_mut()?.fetch(sql, params.clone()).await;
        let result = self.classify_result(result);
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
            filter_chain
                .after_execute(&context, &filter_result, start.elapsed())
                .await?;
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
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let _execution_running = self.begin_execution()?;
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let data_source = self.data_source.clone();
        let mut context = ExecContext {
            sql,
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
            self.classify_result(result)?;
        }

        let result = self
            .physical_mut()?
            .fetch_result_set(sql, params.clone())
            .await;
        let result = self.classify_result(result);
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
            self.classify_result(after_result)?;
        }
        result
    }

    pub(crate) async fn exec_prepared_with_filters(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
    ) -> Result<ExecResult, DruidError> {
        let _execution_running = self.begin_execution()?;
        let sql = statement.sql().to_string();
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let data_source = self.data_source.clone();
        let mut context = ExecContext {
            sql: &sql,
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
            self.classify_result(result)?;
        }
        let result = self
            .physical_mut()?
            .exec_prepared(statement, params.clone())
            .await;
        let result = self.classify_result(result);
        if let Some(holder) = self.holder.as_ref() {
            holder.record_execute();
        }
        if let Some(filter_chain) = &filter_chain {
            filter_chain
                .after_execute(&context, &result, start.elapsed())
                .await?;
        }
        result
    }

    /// 以完整 setter 描述符执行更新 PreparedStatement。
    pub(crate) async fn exec_prepared_parameters_with_filters(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameters: Vec<PreparedInputParameter>,
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
            sql: &sql,
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
            self.classify_result(result)?;
        }
        let result = self
            .physical_mut()?
            .exec_prepared_parameters(statement, parameters.clone())
            .await;
        let result = self.classify_result(result);
        if let Some(holder) = self.holder.as_ref() {
            holder.record_execute();
        }
        if let Some(filter_chain) = &filter_chain {
            filter_chain
                .after_execute(&context, &result, start.elapsed())
                .await?;
        }
        result
    }

    pub(crate) async fn fetch_prepared_with_filters(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
    ) -> Result<Vec<Row>, DruidError> {
        let _execution_running = self.begin_execution()?;
        let sql = statement.sql().to_string();
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let data_source = self.data_source.clone();
        let mut context = ExecContext {
            sql: &sql,
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
            self.classify_result(result)?;
        }
        let result = self
            .physical_mut()?
            .fetch_prepared(statement, params.clone())
            .await;
        let result = self.classify_result(result);
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
            filter_chain
                .after_execute(&context, &filter_result, start.elapsed())
                .await?;
        }
        result
    }

    /// 执行物理预编译查询，同时保留驱动级 `ResultSet` 身份和列元数据。
    pub(crate) async fn fetch_prepared_result_set_with_filters(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let _execution_running = self.begin_execution()?;
        let sql = statement.sql().to_string();
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let data_source = self.data_source.clone();
        let mut context = ExecContext {
            sql: &sql,
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
            self.classify_result(result)?;
        }
        let result = self
            .physical_mut()?
            .fetch_prepared_result_set(statement, params.clone())
            .await;
        let result = self.classify_result(result);
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
            filter_chain
                .after_execute(&context, &filter_result, start.elapsed())
                .await?;
        }
        result
    }

    /// 以完整 setter 描述符执行查询 PreparedStatement。
    pub(crate) async fn fetch_prepared_parameters_with_filters(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameters: Vec<PreparedInputParameter>,
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
            sql: &sql,
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
            self.classify_result(result)?;
        }
        let result = self
            .physical_mut()?
            .fetch_prepared_parameters(statement, parameters.clone())
            .await;
        let result = self.classify_result(result);
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
            filter_chain
                .after_execute(&context, &filter_result, start.elapsed())
                .await?;
        }
        result
    }

    /// 以完整 setter 描述符执行查询，并保留驱动级 `ResultSet` 身份和列元数据。
    pub(crate) async fn fetch_prepared_parameters_result_set_with_filters(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameters: Vec<PreparedInputParameter>,
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
            sql: &sql,
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
            self.classify_result(result)?;
        }
        let result = self
            .physical_mut()?
            .fetch_prepared_parameters_result_set(statement, parameters.clone())
            .await;
        let result = self.classify_result(result);
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
            filter_chain
                .after_execute(&context, &filter_result, start.elapsed())
                .await?;
        }
        result
    }
}

#[async_trait::async_trait]
impl PhysicalConnection for DruidPooledConnection {
    async fn exec(&mut self, sql: &str, params: Vec<Value>) -> Result<ExecResult, DruidError> {
        self.exec_with_filters(sql, params).await
    }

    async fn execute(
        &mut self,
        sql: &str,
        params: Vec<Value>,
        generated_keys: StatementGeneratedKeys,
    ) -> Result<Vec<StatementExecuteResult>, DruidError> {
        if params.is_empty() {
            self.execute_with_filters(sql, generated_keys).await
        } else {
            Err(DruidError::InvalidArgument(
                "DruidPooledStatement generic execute does not accept bind parameters".to_string(),
            ))
        }
    }

    async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        self.fetch_with_filters(sql, params).await
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
        self.exec_prepared_with_filters(statement, params).await
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
        self.exec_prepared_batch_with_filters(statement, &mut parameter_sets)
            .await
    }

    async fn exec_prepared_parameter_batch(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        mut parameter_sets: Vec<Vec<PreparedInputParameter>>,
    ) -> Result<Vec<i32>, DruidError> {
        self.exec_prepared_batch_with_filters(statement, &mut parameter_sets)
            .await
    }

    async fn fetch_prepared(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
    ) -> Result<Vec<Row>, DruidError> {
        self.fetch_prepared_with_filters(statement, params).await
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
        self.classify_result(result)
    }

    async fn commit(&mut self) -> Result<(), DruidError> {
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let result = self.before_connection_event(&ConnectionEvent::Commit).await;
        self.classify_result(result)?;
        let result = self.physical_mut()?.commit().await;
        self.classify_result(result)?;
        if let Some(filter_chain) = filter_chain {
            filter_chain
                .after_connection_event(&ConnectionEvent::Commit, start.elapsed())
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
        let result = self.physical_mut()?.rollback().await;
        self.classify_result(result)?;
        if let Some(filter_chain) = filter_chain {
            filter_chain
                .after_connection_event(&ConnectionEvent::Rollback, start.elapsed())
                .await?;
        }
        Ok(())
    }

    async fn rollback_to(&mut self, savepoint: &Savepoint) -> Result<(), DruidError> {
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
        self.classify_result(result)
    }

    async fn close(&mut self) -> Result<(), DruidError> {
        if self.recycled {
            return Ok(());
        }

        self.notify_connection_closed();
        self.before_connection_event(&ConnectionEvent::Close)
            .await?;
        let filter_chain = self.filter_chain.clone();
        let disposition = self.prepare_for_recycle().await;
        self.recycle_once(disposition);
        if let Some(filter_chain) = filter_chain {
            filter_chain.after_connection_close().await?;
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
        self.classify_result(result)
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
