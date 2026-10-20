//! Druid filters over the canonical RDBC standard surface.

use super::{
    ClobFilterChain, ConnectionCloseFilterChain, ConnectionOpenFilterChain, ConnectionOpenResult,
    DataSourceGetConnectionFilterChain, DataSourceReleaseConnectionFilterChain, DruidError,
    DruidPooledConnection, ExecResult, PreparedInputParameter, RdbcClob, RdbcInputStream,
    RdbcOutputStream, RdbcReader, RdbcString, RdbcWriter, RegisteredLob, SqlWarning,
    StandardConnectionLobFilterChain, StandardConnectionMetaDataFilterChain,
    StandardConnectionWarningFilterChain, StandardStatementWarningFilterChain, Value,
};
use druid_rdbc::resource::{Blob, Clob, NClob};
use std::time::{Duration, Instant};

pub mod config;
pub mod encoding;
pub mod mysql8datetime;

/// SQL execution context passed through the Druid filter chain.
#[derive(Debug)]
pub struct ExecContext<'a> {
    pub connection_id: u64,
    pub statement_id: Option<u64>,
    pub sql: String,
    pub params: &'a [Value],
    pub prepared_parameters: Option<&'a [PreparedInputParameter]>,
    pub data_source: &'a str,
    pub start: Instant,
    pub fingerprint: Option<u64>,
    pub in_transaction: bool,
    pub operation: ExecOperation,
}

/// Batch execution context passed through the Druid filter chain.
#[derive(Debug)]
pub struct BatchExecContext<'a> {
    pub connection_id: u64,
    pub statement_id: Option<u64>,
    pub sql: &'a str,
    pub statements: &'a [String],
    pub parameter_sets: &'a [Vec<Value>],
    pub prepared_parameter_sets: Option<&'a [Vec<PreparedInputParameter>]>,
    pub kind: BatchExecKind,
    pub data_source: &'a str,
    pub start: Instant,
    pub fingerprint: Option<u64>,
    pub in_transaction: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BatchExecKind {
    Statement,
    PreparedStatement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecOperation {
    Execute,
    Query,
    Update,
    Batch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionEvent {
    Connect,
    Close,
    SetAutoCommit(bool),
    GetAutoCommit,
    Commit,
    Rollback,
    SetReadOnly(bool),
    GetReadOnly,
    SetCatalog(String),
    GetCatalog,
    SetTransactionIsolation(u8),
    GetTransactionIsolation,
    ClearWarnings,
    SetSchema(String),
    GetSchema,
    Abort,
    IsValid,
    NativeSQL(String),
    SetNetworkTimeout(Duration),
    GetNetworkTimeout,
}

#[derive(Debug, Clone, Copy)]
pub struct ConnectionEventContext<'a> {
    pub connection_id: u64,
    pub event: &'a ConnectionEvent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatementEvent {
    CreateStatement,
    PrepareStatement(String),
    PrepareCall(String),
    Execute(String),
    ExecuteQuery(String),
    ExecuteUpdate(String),
    Close,
    ExecuteBatch,
}

#[derive(Debug, Clone, Copy)]
pub struct StatementEventContext<'a> {
    pub connection_id: u64,
    pub statement_id: u64,
    pub event: &'a StatementEvent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultSetEvent {
    Next,
    Close,
    GetString,
    GetBoolean,
    GetInt,
    First,
    Last,
}

#[async_trait::async_trait]
pub trait BeforeFilter: Send + Sync {
    fn name(&self) -> &str;

    /// 在标准语句属性读取前检查操作；参数为 Java 同名 getter 事件，返回允许读取或 SQL 错误。
    /// 对应 Java: Filter#statement_getMaxRows 等属性读取过滤器入口。
    fn standard_statement_property_read(
        &self,
        _event: &StatementPropertyEvent,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// 包围标准语句批次清除；参数为调用链，返回过滤或驱动结果。
    /// 对应 Java: Filter#statement_clearBatch；默认继续调用以保持已有过滤器兼容。
    fn standard_statement_clear_batch(
        &self,
        chain: &mut super::StandardStatementClearBatchFilterChain<'_>,
    ) -> Result<(), DruidError> {
        chain.statement_clear_batch()
    }

    async fn data_source_get_connection(
        &self,
        chain: &mut DataSourceGetConnectionFilterChain<'_>,
        max_wait: Duration,
    ) -> Result<DruidPooledConnection, DruidError> {
        chain.data_source_get_connection(max_wait).await
    }

    async fn data_source_release_connection(
        &self,
        chain: &mut DataSourceReleaseConnectionFilterChain<'_>,
        connection: &mut DruidPooledConnection,
    ) -> Result<(), DruidError> {
        chain.data_source_recycle(connection).await
    }

    async fn connection_connect(
        &self,
        chain: &mut ConnectionOpenFilterChain<'_>,
        properties: &mut std::collections::HashMap<String, String>,
    ) -> Result<ConnectionOpenResult, DruidError> {
        chain.connection_connect(properties).await
    }

    async fn connection_close(
        &self,
        chain: &mut ConnectionCloseFilterChain<'_>,
    ) -> Result<(), DruidError> {
        chain.connection_close().await
    }

    /// 读取连接警告；参数为标准连接调用链，返回可由过滤器改写的警告。
    /// 对应 Java: Filter#connection_getWarnings。
    async fn connection_get_warnings(
        &self,
        chain: &mut StandardConnectionWarningFilterChain<'_>,
    ) -> Result<Option<SqlWarning>, DruidError> {
        chain.connection_get_warnings().await
    }

    /// 清除连接警告；参数为标准连接调用链，返回过滤或底层操作结果。
    /// 对应 Java: Filter#connection_clearWarnings。
    async fn connection_clear_warnings(
        &self,
        chain: &mut StandardConnectionWarningFilterChain<'_>,
    ) -> Result<(), DruidError> {
        chain.connection_clear_warnings().await
    }

    /// 包围标准语句的警告读取；参数为语句链，返回可改写的完整警告。
    /// 对应 Java: Filter#statement_getWarnings；标准 RDBC 方法是同步的。
    fn standard_statement_get_warnings(
        &self,
        chain: &mut StandardStatementWarningFilterChain<'_>,
    ) -> Result<Option<SqlWarning>, DruidError> {
        chain.statement_get_warnings()
    }

    /// 包围标准语句的警告清除；参数为语句链，返回过滤或驱动结果。
    /// 对应 Java: Filter#statement_clearWarnings；标准 RDBC 方法是同步的。
    fn standard_statement_clear_warnings(
        &self,
        chain: &mut StandardStatementWarningFilterChain<'_>,
    ) -> Result<(), DruidError> {
        chain.statement_clear_warnings()
    }

    /// 创建 Blob；参数为标准连接链，返回已登记的受管资源。
    async fn connection_create_blob(
        &self,
        chain: &mut StandardConnectionLobFilterChain<'_>,
    ) -> Result<RegisteredLob<dyn Blob>, DruidError> {
        chain.connection_create_blob().await
    }

    /// 创建 Clob；参数为标准连接链，返回已登记的受管资源。
    async fn connection_create_clob(
        &self,
        chain: &mut StandardConnectionLobFilterChain<'_>,
    ) -> Result<RegisteredLob<dyn Clob>, DruidError> {
        chain.connection_create_clob().await
    }

    /// 创建 NClob；参数为标准连接链，返回已登记的受管资源。
    async fn connection_create_n_clob(
        &self,
        chain: &mut StandardConnectionLobFilterChain<'_>,
    ) -> Result<RegisteredLob<dyn NClob>, DruidError> {
        chain.connection_create_n_clob().await
    }

    async fn before(&self, context: &mut ExecContext<'_>) -> Result<(), DruidError>;

    fn prepare_statement_sql(&self, sql: &str) -> Result<String, DruidError> {
        Ok(sql.to_owned())
    }

    fn statement_add_batch_sql(&self, sql: &str) -> Result<String, DruidError> {
        Ok(sql.to_owned())
    }

    async fn before_execute_error(
        &self,
        _context: &ExecContext<'_>,
        _error: &DruidError,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    fn config_from_properties(
        &self,
        _properties: &std::collections::HashMap<String, String>,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    fn config_from_system_properties(
        &self,
        _properties: &std::collections::HashMap<String, String>,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    async fn before_batch(&self, context: &mut BatchExecContext<'_>) -> Result<(), DruidError> {
        let params = context
            .parameter_sets
            .last()
            .map(Vec::as_slice)
            .unwrap_or_default();
        let mut execute_context = ExecContext {
            connection_id: context.connection_id,
            statement_id: context.statement_id,
            sql: context.sql.to_owned(),
            params,
            prepared_parameters: context
                .prepared_parameter_sets
                .and_then(|sets| sets.last())
                .map(Vec::as_slice),
            data_source: context.data_source,
            start: context.start,
            fingerprint: context.fingerprint,
            in_transaction: context.in_transaction,
            operation: ExecOperation::Batch,
        };
        self.before(&mut execute_context).await
    }

    async fn before_batch_error(
        &self,
        context: &BatchExecContext<'_>,
        error: &DruidError,
    ) -> Result<(), DruidError> {
        let params = context
            .parameter_sets
            .last()
            .map(Vec::as_slice)
            .unwrap_or_default();
        let execute_context = ExecContext {
            connection_id: context.connection_id,
            statement_id: context.statement_id,
            sql: context.sql.to_owned(),
            params,
            prepared_parameters: context
                .prepared_parameter_sets
                .and_then(|sets| sets.last())
                .map(Vec::as_slice),
            data_source: context.data_source,
            start: context.start,
            fingerprint: context.fingerprint,
            in_transaction: context.in_transaction,
            operation: ExecOperation::Batch,
        };
        self.before_execute_error(&execute_context, error).await
    }

    async fn on_connection_event(&self, _event: &ConnectionEvent) -> Result<(), DruidError> {
        Ok(())
    }

    async fn on_connection_event_context(
        &self,
        context: &ConnectionEventContext<'_>,
    ) -> Result<(), DruidError> {
        self.on_connection_event(context.event).await
    }

    async fn standard_connection_get_meta_data_before(
        &self,
        _connection_id: u64,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    async fn standard_connection_get_meta_data_after(
        &self,
        _connection_id: u64,
        _result: &Result<(), DruidError>,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    /// 包围完整标准 metadata 获取；参数为连接链，返回可替换的受管原始对象。
    /// 对应 Java: Filter#connection_getMetaData。默认兼容既有前置/后置通知。
    async fn standard_connection_get_meta_data(
        &self,
        chain: &mut StandardConnectionMetaDataFilterChain<'_>,
    ) -> Result<Box<dyn druid_rdbc::metadata::DatabaseMetaData>, DruidError> {
        let connection_id = chain.connection_id();
        self.standard_connection_get_meta_data_before(connection_id)
            .await?;
        let result = chain.connection_get_meta_data().await;
        let status = result.as_ref().map(|_| ()).map_err(Clone::clone);
        self.standard_connection_get_meta_data_after(connection_id, &status)
            .await?;
        result
    }

    async fn on_statement_event(&self, _event: &StatementEvent) -> Result<(), DruidError> {
        Ok(())
    }

    async fn on_statement_event_context(
        &self,
        context: &StatementEventContext<'_>,
    ) -> Result<(), DruidError> {
        self.on_statement_event(context.event).await
    }

    fn on_statement_close_context(
        &self,
        _context: &StatementEventContext<'_>,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    async fn on_result_set_event(&self, _event: &ResultSetEvent) -> Result<(), DruidError> {
        Ok(())
    }

    async fn clob_length(&self, chain: &mut ClobFilterChain<'_>) -> Result<i64, DruidError> {
        chain.clob_length().await
    }

    async fn clob_get_sub_string(
        &self,
        chain: &mut ClobFilterChain<'_>,
        position: i64,
        length: i32,
    ) -> Result<RdbcString, DruidError> {
        chain.clob_get_sub_string(position, length).await
    }

    async fn clob_get_character_stream(
        &self,
        chain: &mut ClobFilterChain<'_>,
    ) -> Result<RdbcReader, DruidError> {
        chain.clob_get_character_stream().await
    }

    async fn clob_get_ascii_stream(
        &self,
        chain: &mut ClobFilterChain<'_>,
    ) -> Result<RdbcInputStream, DruidError> {
        chain.clob_get_ascii_stream().await
    }

    async fn clob_position_string(
        &self,
        chain: &mut ClobFilterChain<'_>,
        pattern: &RdbcString,
        start: i64,
    ) -> Result<Option<i64>, DruidError> {
        chain.clob_position_string(pattern, start).await
    }

    async fn clob_position_clob(
        &self,
        chain: &mut ClobFilterChain<'_>,
        pattern: &RdbcClob,
        start: i64,
    ) -> Result<Option<i64>, DruidError> {
        chain.clob_position_clob(pattern, start).await
    }

    async fn clob_set_string(
        &self,
        chain: &mut ClobFilterChain<'_>,
        position: i64,
        value: &RdbcString,
    ) -> Result<i32, DruidError> {
        chain.clob_set_string(position, value).await
    }

    async fn clob_set_string_range(
        &self,
        chain: &mut ClobFilterChain<'_>,
        position: i64,
        value: &RdbcString,
        offset: i32,
        length: i32,
    ) -> Result<i32, DruidError> {
        chain
            .clob_set_string_range(position, value, offset, length)
            .await
    }

    async fn clob_set_ascii_stream(
        &self,
        chain: &mut ClobFilterChain<'_>,
        position: i64,
    ) -> Result<RdbcOutputStream, DruidError> {
        chain.clob_set_ascii_stream(position).await
    }

    async fn clob_set_character_stream(
        &self,
        chain: &mut ClobFilterChain<'_>,
        position: i64,
    ) -> Result<RdbcWriter, DruidError> {
        chain.clob_set_character_stream(position).await
    }

    async fn clob_truncate(
        &self,
        chain: &mut ClobFilterChain<'_>,
        length: i64,
    ) -> Result<(), DruidError> {
        chain.clob_truncate(length).await
    }

    async fn clob_free(&self, chain: &mut ClobFilterChain<'_>) -> Result<(), DruidError> {
        chain.clob_free().await
    }

    async fn clob_get_character_stream_range(
        &self,
        chain: &mut ClobFilterChain<'_>,
        position: i64,
        length: i64,
    ) -> Result<RdbcReader, DruidError> {
        chain
            .clob_get_character_stream_range(position, length)
            .await
    }

    async fn init(&self) -> Result<(), DruidError> {
        Ok(())
    }

    async fn destroy(&self) -> Result<(), DruidError> {
        Ok(())
    }
}

#[async_trait::async_trait]
pub trait AfterFilter: Send + Sync {
    fn name(&self) -> &str;

    async fn after(
        &self,
        context: &ExecContext<'_>,
        result: &Result<ExecResult, DruidError>,
        elapsed: Duration,
    ) -> Result<(), DruidError>;

    async fn after_batch(
        &self,
        context: &BatchExecContext<'_>,
        result: &Result<Vec<i32>, DruidError>,
        elapsed: Duration,
    ) -> Result<(), DruidError> {
        let execute_result = match result {
            Ok(update_counts) => Ok(ExecResult {
                rows_affected: update_counts
                    .iter()
                    .filter_map(|count| u64::try_from(*count).ok())
                    .sum(),
                last_insert_id: None,
                row_count: None,
            }),
            Err(error) => Err(error.clone()),
        };
        let params = context
            .parameter_sets
            .last()
            .map(Vec::as_slice)
            .unwrap_or_default();
        let execute_context = ExecContext {
            connection_id: context.connection_id,
            statement_id: context.statement_id,
            sql: context.sql.to_owned(),
            params,
            prepared_parameters: context
                .prepared_parameter_sets
                .and_then(|sets| sets.last())
                .map(Vec::as_slice),
            data_source: context.data_source,
            start: context.start,
            fingerprint: None,
            in_transaction: context.in_transaction,
            operation: ExecOperation::Batch,
        };
        self.after(&execute_context, &execute_result, elapsed).await
    }

    async fn after_connection_event(
        &self,
        _event: &ConnectionEvent,
        _elapsed: Duration,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    async fn after_connection_event_context(
        &self,
        context: &ConnectionEventContext<'_>,
        elapsed: Duration,
    ) -> Result<(), DruidError> {
        self.after_connection_event(context.event, elapsed).await
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatementPropertyEvent {
    SetQueryTimeout(i32),
    GetQueryTimeout,
    GetUpdateCount,
    SetMaxRows(i32),
    GetMaxRows,
    SetMaxFieldSize(i32),
    GetMaxFieldSize,
    SetFetchDirection(i32),
    GetFetchDirection,
    SetFetchSize(i32),
    GetFetchSize,
    IsPoolable,
    IsClosed,
    GetMoreResults,
    GetResultSetConcurrency,
    GetResultSetType,
    GetResultSetHoldability,
    GetGeneratedKeys,
    ClearWarnings,
    SetCursorName(String),
    AddBatch(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClobEvent {
    Length,
    GetSubString(i64, i32),
    SetString(i64, String),
    Truncate(i64),
    Free,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataSourceEvent {
    GetConnection,
    GetConnectionWithAuth(String, String),
    ReleaseConnection,
    Log(String),
}

#[async_trait::async_trait]
pub trait ExtendedFilter: Send + Sync {
    async fn on_statement_property_event(
        &self,
        _event: &StatementPropertyEvent,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    async fn on_clob_event(&self, _event: &ClobEvent) -> Result<(), DruidError> {
        Ok(())
    }

    async fn on_datasource_event(&self, _event: &DataSourceEvent) -> Result<(), DruidError> {
        Ok(())
    }

    async fn config_from_properties(
        &mut self,
        _properties: &std::collections::HashMap<String, String>,
    ) -> Result<(), DruidError> {
        Ok(())
    }

    fn is_wrapper_for(&self, _type_name: &str) -> bool {
        false
    }
}
