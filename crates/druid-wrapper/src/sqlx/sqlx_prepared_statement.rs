//! `SQLx` 预编译语句适配器。

use super::sqlx_connection_actor::SqlxConnectionActor;
use crate::prepared_parameter_materializer::PreparedParameterMaterializer;
use crate::prepared_parameter_meta_data_snapshot::PreparedParameterMetaDataSnapshot;
use crate::prepared_parameter_state::PreparedParameterState;
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::ParameterMetaData;
use druid_rdbc::spi::{
    PhysicalPreparedStatement, PhysicalStatement, PhysicalStatementOptions, PreparedInputParameter,
    SqlTextStatement,
};
use druid_rdbc::types::Value;
use sqlx::any::AnyStatement;
use sqlx::mysql::MySqlStatement;
use sqlx::postgres::PgStatement;
use sqlx::sqlite::SqliteStatement;
use sqlx::{Column, Statement};
use std::any::Any;
use std::future::Future;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::watch;

type StatementState = SqlTextStatement;

fn count_placeholders(sql: &str) -> usize {
    sql.bytes().filter(|byte| *byte == b'?').count()
}

fn statement_state_core() -> StatementState {
    SqlTextStatement::new(PhysicalStatementOptions::default())
}

/// `SQLx` 语句执行期间由 Druid 控制面产生的中断结果。
pub(crate) enum SqlxStatementExecutionError {
    Driver(RdbcError),
    TimedOut,
    Cancelled,
}

enum SqlxPreparedStatementBackend {
    Any(AnyStatement<'static>),
    MySql(MySqlStatement<'static>),
    PostgreSql(PgStatement<'static>),
    Sqlite(SqliteStatement<'static>),
    OwnedActor(Arc<SqlxConnectionActor>),
}

/// `SQLx` 驱动 prepare 结果。
///
/// 对应 Java 平台依赖：具体 JDBC driver 的 `PreparedStatement`。`SQLx` statement
/// 可跨同类连接复用，并由 `SQLx` connection 自身维护 server-side statement cache。
pub struct SqlxPreparedStatement {
    sql: String,
    backend: SqlxPreparedStatementBackend,
    closed: AtomicBool,
    statement: StatementState,
    parameter_state: PreparedParameterState,
    parameter_count: usize,
    cancel_generation: watch::Sender<u64>,
}

impl SqlxPreparedStatement {
    pub(crate) fn any(sql: impl Into<String>, statement: AnyStatement<'static>) -> Self {
        let sql = sql.into();
        Self {
            parameter_count: count_placeholders(&sql),
            sql,
            backend: SqlxPreparedStatementBackend::Any(statement),
            closed: AtomicBool::new(false),
            statement: SqlTextStatement::new(PhysicalStatementOptions::default()),
            parameter_state: PreparedParameterState::new(),
            cancel_generation: watch::channel(0).0,
        }
    }

    pub(crate) fn sqlite(sql: impl Into<String>, statement: SqliteStatement<'static>) -> Self {
        let sql = sql.into();
        Self {
            parameter_count: count_placeholders(&sql),
            sql,
            backend: SqlxPreparedStatementBackend::Sqlite(statement),
            closed: AtomicBool::new(false),
            statement: statement_state_core(),
            parameter_state: PreparedParameterState::new(),
            cancel_generation: watch::channel(0).0,
        }
    }

    pub(crate) fn mysql(sql: impl Into<String>, statement: MySqlStatement<'static>) -> Self {
        let sql = sql.into();
        Self {
            parameter_count: count_placeholders(&sql),
            sql,
            backend: SqlxPreparedStatementBackend::MySql(statement),
            closed: AtomicBool::new(false),
            statement: SqlTextStatement::new(PhysicalStatementOptions::default()),
            parameter_state: PreparedParameterState::new(),
            cancel_generation: watch::channel(0).0,
        }
    }

    pub(crate) fn postgresql(sql: impl Into<String>, statement: PgStatement<'static>) -> Self {
        let sql = sql.into();
        Self {
            parameter_count: count_placeholders(&sql),
            sql,
            backend: SqlxPreparedStatementBackend::PostgreSql(statement),
            closed: AtomicBool::new(false),
            statement: SqlTextStatement::new(PhysicalStatementOptions::default()),
            parameter_state: PreparedParameterState::new(),
            cancel_generation: watch::channel(0).0,
        }
    }

    /// 创建持有 canonical SQLx actor 的 owned Statement/PreparedStatement。
    pub(crate) fn owned_statement(
        sql: String,
        actor: Arc<SqlxConnectionActor>,
        _options: druid_rdbc::statement::StatementOptions,
    ) -> Self {
        Self {
            parameter_count: count_placeholders(&sql),
            sql,
            backend: SqlxPreparedStatementBackend::OwnedActor(actor),
            closed: AtomicBool::new(false),
            statement: statement_state_core(),
            parameter_state: PreparedParameterState::new(),
            cancel_generation: watch::channel(0).0,
        }
    }

    /// 返回 `SQLx` statement 是否来自当前 Adapter 后端。
    pub(crate) fn matches_backend(&self, backend: &str) -> bool {
        match (&self.backend, backend) {
            (SqlxPreparedStatementBackend::Any(statement), "any") => statement.sql() == self.sql,
            (SqlxPreparedStatementBackend::MySql(statement), "mysql") => {
                statement.sql() == self.sql
            }
            (SqlxPreparedStatementBackend::PostgreSql(statement), "postgresql") => {
                statement.sql() == self.sql
            }
            (SqlxPreparedStatementBackend::Sqlite(statement), "sqlite") => {
                statement.sql() == self.sql
            }
            _ => false,
        }
    }

    /// 返回 `SQLx` Any 的真实预编译语句。
    pub(crate) fn any_statement(&self) -> Option<&AnyStatement<'static>> {
        match &self.backend {
            SqlxPreparedStatementBackend::Any(statement) => Some(statement),
            SqlxPreparedStatementBackend::MySql(_)
            | SqlxPreparedStatementBackend::PostgreSql(_)
            | SqlxPreparedStatementBackend::Sqlite(_)
            | SqlxPreparedStatementBackend::OwnedActor(_) => None,
        }
    }

    /// 返回 `SQLx MySQL` 的真实预编译语句。
    pub(crate) fn mysql_statement(&self) -> Option<&MySqlStatement<'static>> {
        match &self.backend {
            SqlxPreparedStatementBackend::MySql(statement) => Some(statement),
            SqlxPreparedStatementBackend::Any(_)
            | SqlxPreparedStatementBackend::PostgreSql(_)
            | SqlxPreparedStatementBackend::Sqlite(_)
            | SqlxPreparedStatementBackend::OwnedActor(_) => None,
        }
    }

    /// 返回 `SQLx PostgreSQL` 的真实预编译语句。
    pub(crate) fn postgresql_statement(&self) -> Option<&PgStatement<'static>> {
        match &self.backend {
            SqlxPreparedStatementBackend::PostgreSql(statement) => Some(statement),
            SqlxPreparedStatementBackend::Any(_)
            | SqlxPreparedStatementBackend::MySql(_)
            | SqlxPreparedStatementBackend::Sqlite(_)
            | SqlxPreparedStatementBackend::OwnedActor(_) => None,
        }
    }

    /// 返回 `SQLx SQLite` 的真实预编译语句。
    pub(crate) fn sqlite_statement(&self) -> Option<&SqliteStatement<'static>> {
        match &self.backend {
            SqlxPreparedStatementBackend::Sqlite(statement) => Some(statement),
            SqlxPreparedStatementBackend::Any(_)
            | SqlxPreparedStatementBackend::MySql(_)
            | SqlxPreparedStatementBackend::PostgreSql(_)
            | SqlxPreparedStatementBackend::OwnedActor(_) => None,
        }
    }

    pub(crate) fn returns_rows(&self) -> bool {
        match &self.backend {
            SqlxPreparedStatementBackend::Any(statement) => !statement.columns().is_empty(),
            SqlxPreparedStatementBackend::MySql(statement) => !statement.columns().is_empty(),
            SqlxPreparedStatementBackend::PostgreSql(statement) => !statement.columns().is_empty(),
            SqlxPreparedStatementBackend::Sqlite(statement) => !statement.columns().is_empty(),
            SqlxPreparedStatementBackend::OwnedActor(_) => false,
        }
    }

    pub(crate) fn column_labels(&self) -> Vec<String> {
        match &self.backend {
            SqlxPreparedStatementBackend::Any(statement) => statement
                .columns()
                .iter()
                .map(|column| column.name().to_owned())
                .collect(),
            SqlxPreparedStatementBackend::MySql(statement) => statement
                .columns()
                .iter()
                .map(|column| column.name().to_owned())
                .collect(),
            SqlxPreparedStatementBackend::PostgreSql(statement) => statement
                .columns()
                .iter()
                .map(|column| column.name().to_owned())
                .collect(),
            SqlxPreparedStatementBackend::Sqlite(statement) => statement
                .columns()
                .iter()
                .map(|column| column.name().to_owned())
                .collect(),
            SqlxPreparedStatementBackend::OwnedActor(_) => Vec::new(),
        }
    }

    /// 在 `SQLx` 物理 setter 边界物化一个完整 JDBC 参数描述符。
    pub(crate) async fn materialize_parameter(
        parameter: &PreparedInputParameter,
    ) -> Result<Value, RdbcError> {
        PreparedParameterMaterializer::materialize(parameter).await
    }

    /// 返回物理 setter 已经保存的参数值。
    pub(crate) async fn materialized_parameters(
        &self,
        parameter_count: usize,
    ) -> Result<Vec<Value>, RdbcError> {
        self.parameter_state.values(parameter_count).await
    }

    /// 消费物理句柄保存的有序参数批次。
    pub(crate) async fn take_batches(
        &self,
        expected_count: usize,
    ) -> Result<Option<Vec<Vec<Value>>>, RdbcError> {
        self.parameter_state.take_batches(expected_count).await
    }

    fn actor_core(&self) -> Result<&Arc<SqlxConnectionActor>, RdbcError> {
        match &self.backend {
            SqlxPreparedStatementBackend::OwnedActor(actor) => Ok(actor),
            _ => Err(RdbcError::UnsupportedOperation {
                operation: "sqlx_standard_statement_requires_owned_actor",
            }),
        }
    }

    /// 执行一个真实 `SQLx` future，并同时监听 Statement timeout 与显式 cancel。
    ///
    /// `SQLx` 不为所有后端暴露统一的 wire-level cancel handle。中断发生时调用方
    /// 必须丢弃承载该 future 的物理连接，不能把协议状态未知的连接放回 Druid。
    pub(crate) async fn execute_with_controls<T, F>(
        &self,
        execution: F,
    ) -> Result<T, SqlxStatementExecutionError>
    where
        F: Future<Output = Result<T, RdbcError>>,
    {
        let generation = *self.cancel_generation.borrow();
        let mut cancellation = self.cancel_generation.subscribe();
        let cancelled = async move {
            if *cancellation.borrow() != generation {
                return;
            }
            while cancellation.changed().await.is_ok() {
                if *cancellation.borrow() != generation {
                    return;
                }
            }
        };
        let timeout_seconds = self
            .statement
            .query_timeout()
            .map_err(SqlxStatementExecutionError::Driver)?;
        if timeout_seconds > 0 {
            tokio::select! {
                result = execution => result.map_err(SqlxStatementExecutionError::Driver),
                () = cancelled => Err(SqlxStatementExecutionError::Cancelled),
                () = tokio::time::sleep(std::time::Duration::from_secs(timeout_seconds as u64)) => {
                    Err(SqlxStatementExecutionError::TimedOut)
                }
            }
        } else {
            tokio::select! {
                result = execution => result.map_err(SqlxStatementExecutionError::Driver),
                () = cancelled => Err(SqlxStatementExecutionError::Cancelled),
            }
        }
    }
}

impl PhysicalPreparedStatement for SqlxPreparedStatement {
    fn sql(&self) -> &str {
        &self.sql
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn max_field_size(&self) -> Result<i32, RdbcError> {
        self.statement.max_field_size()
    }

    fn set_max_field_size(&self, max: i32) -> Result<(), RdbcError> {
        self.statement.set_max_field_size(max)
    }

    fn max_rows(&self) -> Result<i32, RdbcError> {
        self.statement.max_rows()
    }

    fn set_max_rows(&self, max: i32) -> Result<(), RdbcError> {
        self.statement.set_max_rows(max)
    }

    fn set_escape_processing(&self, enabled: bool) -> Result<(), RdbcError> {
        self.statement.set_escape_processing(enabled)
    }

    fn query_timeout(&self) -> Result<i32, RdbcError> {
        self.statement.query_timeout()
    }

    fn set_query_timeout(&self, seconds: i32) -> Result<(), RdbcError> {
        self.statement.set_query_timeout(seconds)
    }

    fn cancel(&self) -> Result<(), RdbcError> {
        self.statement.cancel()?;
        let next = (*self.cancel_generation.borrow()).wrapping_add(1);
        self.cancel_generation.send_replace(next);
        Ok(())
    }

    fn warnings(&self) -> Result<Option<druid_rdbc::error::SqlWarning>, RdbcError> {
        self.statement.warnings()
    }

    fn clear_warnings(&self) -> Result<(), RdbcError> {
        self.statement.clear_warnings()
    }

    fn set_cursor_name(&self, name: &str) -> Result<(), RdbcError> {
        self.statement.set_cursor_name(name)
    }

    fn set_fetch_direction(&self, direction: i32) -> Result<(), RdbcError> {
        self.statement.set_fetch_direction(direction)
    }

    fn fetch_direction(&self) -> Result<i32, RdbcError> {
        self.statement.fetch_direction()
    }

    fn set_fetch_size(&self, rows: i32) -> Result<(), RdbcError> {
        self.statement.set_fetch_size(rows)
    }

    fn fetch_size(&self) -> Result<i32, RdbcError> {
        self.statement.fetch_size()
    }

    fn close_on_completion(&self) -> Result<(), RdbcError> {
        self.statement.close_on_completion()
    }

    fn is_close_on_completion(&self) -> Result<bool, RdbcError> {
        self.statement.is_close_on_completion()
    }

    fn set_parameter(
        &self,
        parameter_index: usize,
        parameter: &PreparedInputParameter,
    ) -> Result<(), RdbcError> {
        self.parameter_state.set(parameter_index, parameter)
    }

    fn clear_parameters(&self) -> Result<(), RdbcError> {
        self.parameter_state.clear_parameters();
        Ok(())
    }

    fn add_batch(&self, params: &[Value]) -> Result<(), RdbcError> {
        self.parameter_state.add_values(params);
        Ok(())
    }

    fn add_parameter_batch(&self, params: &[PreparedInputParameter]) -> Result<(), RdbcError> {
        self.parameter_state.add_parameters(params)
    }

    fn clear_batch(&self) -> Result<(), RdbcError> {
        self.parameter_state.clear_batches();
        Ok(())
    }

    fn close(&self) -> Result<(), RdbcError> {
        self.closed.store(true, Ordering::Release);
        self.statement.close()?;
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
}

druid_rdbc::impl_standard_wrapper!(SqlxPreparedStatement => PreparedStatement);

#[async_trait::async_trait]
impl druid_rdbc::statement::Statement for SqlxPreparedStatement {
    async fn execute_query(
        &mut self,
        sql: &str,
    ) -> Result<Box<dyn druid_rdbc::result_set::ResultSet>, RdbcError> {
        self.actor_core()?.execute_query(sql, Vec::new()).await
    }

    async fn execute_update(
        &mut self,
        sql: &str,
        _generated_keys: druid_rdbc::statement::GeneratedKeys,
    ) -> Result<i64, RdbcError> {
        self.actor_core()?.execute_update(sql, Vec::new()).await
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed.store(true, Ordering::Release);
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed.load(Ordering::Acquire))
    }

    async fn execute_values(
        &mut self,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<Box<dyn druid_rdbc::result_set::ResultSet>, RdbcError> {
        self.actor_core()?.execute_query(sql, values).await
    }
}

#[async_trait::async_trait]
impl druid_rdbc::statement::PreparedStatement for SqlxPreparedStatement {
    fn set_parameter(
        &mut self,
        parameter: druid_rdbc::statement::PreparedParameter,
    ) -> Result<(), RdbcError> {
        self.parameter_state.set_standard(parameter)
    }

    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
        self.parameter_state.clear_parameters();
        Ok(())
    }

    async fn execute_prepared_query(
        &mut self,
    ) -> Result<Box<dyn druid_rdbc::result_set::ResultSet>, RdbcError> {
        let values = self.parameter_state.values(self.parameter_count).await?;
        self.actor_core()?.execute_query(&self.sql, values).await
    }

    async fn execute_prepared_update(&mut self) -> Result<i64, RdbcError> {
        let values = self.parameter_state.values(self.parameter_count).await?;
        self.actor_core()?.execute_update(&self.sql, values).await
    }

    fn parameter_meta_data(&self) -> Result<Arc<dyn ParameterMetaData>, RdbcError> {
        Ok(Arc::new(PreparedParameterMetaDataSnapshot::unknown_inputs(
            self.parameter_count,
        )))
    }
}
