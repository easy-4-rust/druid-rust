//! 对外池化连接。

use crate::connection_defaults::ConnectionDefaults;
use crate::connection_recycle_disposition::ConnectionRecycleDisposition;
use crate::druid_connection_holder::DruidConnectionHolder;
use crate::druid_pooled_callable_statement::DruidPooledCallableStatement;
use crate::druid_pooled_prepared_statement::DruidPooledPreparedStatement;
use crate::error::DruidError;
use crate::exec_result::ExecResult;
use crate::filter::{ConnectionEvent, ExecContext};
use crate::filter_chain::FilterChain;
use crate::physical_connection::PhysicalConnection;
use crate::physical_connection_capabilities::PhysicalConnectionCapabilities;
use crate::physical_connection_factory::PhysicalConnectionFactory;
use crate::physical_prepared_statement::PhysicalPreparedStatement;
use crate::prepared_statement_holder::PreparedStatementHolder;
use crate::prepared_statement_key::{PreparedStatementKey, PreparedStatementMethodType};
use crate::row::Row;
use crate::savepoint::Savepoint;
use crate::value::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Native pool 回收完整 holder 的一次性回调。
pub type DruidConnectionReturnCallback =
    Box<dyn FnOnce(DruidConnectionHolder, ConnectionRecycleDisposition) + Send>;

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
    return_connection: Option<DruidConnectionReturnCallback>,
    lease_active: Arc<AtomicBool>,
    recycled: bool,
}

impl std::fmt::Debug for DruidPooledConnection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DruidPooledConnection")
            .field("id", &self.id)
            .field("data_source", &self.data_source)
            .field(
                "has_physical_connection",
                &self
                    .holder
                    .as_ref()
                    .is_some_and(DruidConnectionHolder::has_physical_connection),
            )
            .field("recycled", &self.recycled)
            .finish()
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
            return_connection: Some(return_connection),
            lease_active: Arc::new(AtomicBool::new(true)),
            recycled: false,
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
        self.prepare_from_key(key, false).await
    }

    async fn prepare_call_from_key(
        &mut self,
        key: PreparedStatementKey,
    ) -> Result<DruidPooledCallableStatement, DruidError> {
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
        Ok(DruidPooledCallableStatement::new(prepared_statement))
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
                let statement = if callable {
                    self.physical_mut()?.prepare_physical_call(&key).await?
                } else {
                    self.physical_mut()?
                        .prepare_physical_statement(&key)
                        .await?
                };
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
            return_connection(holder, disposition);
        }
    }

    fn drop_disposition(&mut self) -> ConnectionRecycleDisposition {
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

    async fn exec_with_filters(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<ExecResult, DruidError> {
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let data_source = self.data_source.clone();
        let mut context = ExecContext {
            sql,
            params: &params,
            data_source: &data_source,
            start,
            fingerprint: None,
        };

        if let Some(filter_chain) = &filter_chain {
            filter_chain.before_execute(&mut context).await?;
        }

        // 过滤器需要在 after 阶段观察同一组参数，因此只克隆驱动调用所需所有权。
        let result = self.physical_mut()?.exec(sql, params.clone()).await;
        if let Some(holder) = self.holder.as_ref() {
            holder.record_execute();
        }
        if let Some(filter_chain) = &filter_chain {
            filter_chain
                .after_execute(&context, &result, start.elapsed())
                .await;
        }
        result
    }

    async fn fetch_with_filters(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<Vec<Row>, DruidError> {
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let data_source = self.data_source.clone();
        let mut context = ExecContext {
            sql,
            params: &params,
            data_source: &data_source,
            start,
            fingerprint: None,
        };

        if let Some(filter_chain) = &filter_chain {
            filter_chain.before_execute(&mut context).await?;
        }

        let result = self.physical_mut()?.fetch(sql, params.clone()).await;
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
                .await;
        }
        result
    }

    pub(crate) async fn exec_prepared_with_filters(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
    ) -> Result<ExecResult, DruidError> {
        let sql = statement.sql().to_string();
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let data_source = self.data_source.clone();
        let mut context = ExecContext {
            sql: &sql,
            params: &params,
            data_source: &data_source,
            start,
            fingerprint: None,
        };
        if let Some(filter_chain) = &filter_chain {
            filter_chain.before_execute(&mut context).await?;
        }
        let result = self
            .physical_mut()?
            .exec_prepared(statement, params.clone())
            .await;
        if let Some(holder) = self.holder.as_ref() {
            holder.record_execute();
        }
        if let Some(filter_chain) = &filter_chain {
            filter_chain
                .after_execute(&context, &result, start.elapsed())
                .await;
        }
        result
    }

    pub(crate) async fn fetch_prepared_with_filters(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
    ) -> Result<Vec<Row>, DruidError> {
        let sql = statement.sql().to_string();
        let start = Instant::now();
        let filter_chain = self.filter_chain.clone();
        let data_source = self.data_source.clone();
        let mut context = ExecContext {
            sql: &sql,
            params: &params,
            data_source: &data_source,
            start,
            fingerprint: None,
        };
        if let Some(filter_chain) = &filter_chain {
            filter_chain.before_execute(&mut context).await?;
        }
        let result = self
            .physical_mut()?
            .fetch_prepared(statement, params.clone())
            .await;
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
                .await;
        }
        result
    }
}

#[async_trait::async_trait]
impl PhysicalConnection for DruidPooledConnection {
    async fn exec(&mut self, sql: &str, params: Vec<Value>) -> Result<ExecResult, DruidError> {
        self.exec_with_filters(sql, params).await
    }

    async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        self.fetch_with_filters(sql, params).await
    }

    async fn prepare_physical_statement(
        &mut self,
        key: &PreparedStatementKey,
    ) -> Result<Arc<dyn PhysicalPreparedStatement>, DruidError> {
        self.physical_mut()?.prepare_physical_statement(key).await
    }

    async fn prepare_physical_call(
        &mut self,
        key: &PreparedStatementKey,
    ) -> Result<Arc<dyn PhysicalPreparedStatement>, DruidError> {
        self.physical_mut()?.prepare_physical_call(key).await
    }

    async fn exec_prepared(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
    ) -> Result<ExecResult, DruidError> {
        self.exec_prepared_with_filters(statement, params).await
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
        self.physical_mut()?
            .close_prepared_statement(statement)
            .await
    }

    async fn begin(&mut self) -> Result<(), DruidError> {
        self.physical_mut()?.begin().await
    }

    async fn commit(&mut self) -> Result<(), DruidError> {
        self.before_connection_event(&ConnectionEvent::Commit)
            .await?;
        self.physical_mut()?.commit().await
    }

    async fn rollback(&mut self) -> Result<(), DruidError> {
        self.before_connection_event(&ConnectionEvent::Rollback)
            .await?;
        self.physical_mut()?.rollback().await
    }

    async fn rollback_to(&mut self, savepoint: &Savepoint) -> Result<(), DruidError> {
        self.physical_mut()?.rollback_to(savepoint).await
    }

    async fn set_savepoint(&mut self) -> Result<Savepoint, DruidError> {
        self.physical_mut()?.set_savepoint().await
    }

    async fn set_savepoint_named(&mut self, name: &str) -> Result<Savepoint, DruidError> {
        self.physical_mut()?.set_savepoint_named(name).await
    }

    async fn release_savepoint(&mut self, savepoint: &Savepoint) -> Result<(), DruidError> {
        self.physical_mut()?.release_savepoint(savepoint).await
    }

    async fn abort(&mut self) -> Result<(), DruidError> {
        self.before_connection_event(&ConnectionEvent::Abort)
            .await?;
        let result = self.physical_mut()?.abort().await;
        if let Some(holder) = self.holder.as_mut() {
            holder.mark_discarded();
        }
        self.recycle_once(ConnectionRecycleDisposition::discard());
        result
    }

    async fn ping(&mut self) -> Result<(), DruidError> {
        self.before_connection_event(&ConnectionEvent::IsValid)
            .await?;
        self.physical_mut()?.ping().await
    }

    async fn close(&mut self) -> Result<(), DruidError> {
        if self.recycled {
            return Ok(());
        }

        self.before_connection_event(&ConnectionEvent::Close)
            .await?;
        let filter_chain = self.filter_chain.clone();
        let disposition = self.prepare_for_recycle().await;
        self.recycle_once(disposition);
        if let Some(filter_chain) = filter_chain {
            filter_chain.after_connection_close().await;
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
        self.before_connection_event(&ConnectionEvent::SetAutoCommit(auto_commit))
            .await?;
        self.physical_mut()?.set_auto_commit(auto_commit).await
    }

    fn read_only(&self) -> bool {
        self.holder
            .as_ref()
            .and_then(DruidConnectionHolder::physical_connection)
            .is_some_and(|connection| connection.read_only())
    }

    async fn set_read_only(&mut self, read_only: bool) -> Result<(), DruidError> {
        self.before_connection_event(&ConnectionEvent::SetReadOnly(read_only))
            .await?;
        self.physical_mut()?.set_read_only(read_only).await
    }

    fn transaction_isolation(&self) -> u8 {
        self.holder
            .as_ref()
            .and_then(DruidConnectionHolder::physical_connection)
            .map_or(2, |connection| connection.transaction_isolation())
    }

    async fn set_transaction_isolation(&mut self, level: u8) -> Result<(), DruidError> {
        self.before_connection_event(&ConnectionEvent::SetTransactionIsolation(level))
            .await?;
        self.physical_mut()?.set_transaction_isolation(level).await
    }

    fn holdability(&self) -> i32 {
        self.holder
            .as_ref()
            .and_then(DruidConnectionHolder::physical_connection)
            .map_or(0, |connection| connection.holdability())
    }

    async fn set_holdability(&mut self, holdability: i32) -> Result<(), DruidError> {
        self.physical_mut()?.set_holdability(holdability).await
    }

    async fn clear_warnings(&mut self) -> Result<(), DruidError> {
        self.physical_mut()?.clear_warnings().await
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
        self.before_connection_event(&ConnectionEvent::SetCatalog(catalog.to_string()))
            .await?;
        self.physical_mut()?.set_catalog(catalog).await
    }

    fn schema(&self) -> Option<&str> {
        self.holder
            .as_ref()
            .and_then(DruidConnectionHolder::physical_connection)
            .and_then(|connection| connection.schema())
    }

    async fn set_schema(&mut self, schema: &str) -> Result<(), DruidError> {
        self.before_connection_event(&ConnectionEvent::SetSchema(schema.to_string()))
            .await?;
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
        self.physical_mut()?.set_schema(schema).await?;
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
        let disposition = self.drop_disposition();
        self.recycle_once(disposition);
    }
}
