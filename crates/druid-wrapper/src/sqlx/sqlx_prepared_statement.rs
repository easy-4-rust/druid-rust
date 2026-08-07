//! `SQLx` 预编译语句适配器。

use crate::prepared_parameter_materializer::PreparedParameterMaterializer;
use crate::prepared_parameter_state::PreparedParameterState;
use druid::core::{
    DruidError, PhysicalPreparedStatement, PhysicalStatement, PhysicalStatementOptions,
    PreparedInputParameter, SqlTextStatement, Value,
};
use sqlx::any::AnyStatement;
use sqlx::mysql::MySqlStatement;
use sqlx::postgres::PgStatement;
use sqlx::sqlite::SqliteStatement;
use sqlx::{Column, Statement};
use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};

enum SqlxPreparedStatementBackend {
    Any(AnyStatement<'static>),
    MySql(MySqlStatement<'static>),
    PostgreSql(PgStatement<'static>),
    Sqlite(SqliteStatement<'static>),
}

/// `SQLx` 驱动 prepare 结果。
///
/// 对应 Java 平台依赖：具体 JDBC driver 的 `PreparedStatement`。`SQLx` statement
/// 可跨同类连接复用，并由 `SQLx` connection 自身维护 server-side statement cache。
pub struct SqlxPreparedStatement {
    sql: String,
    backend: SqlxPreparedStatementBackend,
    closed: AtomicBool,
    statement: SqlTextStatement,
    parameter_state: PreparedParameterState,
}

impl SqlxPreparedStatement {
    pub(crate) fn any(sql: impl Into<String>, statement: AnyStatement<'static>) -> Self {
        Self {
            sql: sql.into(),
            backend: SqlxPreparedStatementBackend::Any(statement),
            closed: AtomicBool::new(false),
            statement: SqlTextStatement::new(PhysicalStatementOptions::default()),
            parameter_state: PreparedParameterState::new(),
        }
    }

    pub(crate) fn sqlite(sql: impl Into<String>, statement: SqliteStatement<'static>) -> Self {
        Self {
            sql: sql.into(),
            backend: SqlxPreparedStatementBackend::Sqlite(statement),
            closed: AtomicBool::new(false),
            statement: SqlTextStatement::new(PhysicalStatementOptions::default()),
            parameter_state: PreparedParameterState::new(),
        }
    }

    pub(crate) fn mysql(sql: impl Into<String>, statement: MySqlStatement<'static>) -> Self {
        Self {
            sql: sql.into(),
            backend: SqlxPreparedStatementBackend::MySql(statement),
            closed: AtomicBool::new(false),
            statement: SqlTextStatement::new(PhysicalStatementOptions::default()),
            parameter_state: PreparedParameterState::new(),
        }
    }

    pub(crate) fn postgresql(sql: impl Into<String>, statement: PgStatement<'static>) -> Self {
        Self {
            sql: sql.into(),
            backend: SqlxPreparedStatementBackend::PostgreSql(statement),
            closed: AtomicBool::new(false),
            statement: SqlTextStatement::new(PhysicalStatementOptions::default()),
            parameter_state: PreparedParameterState::new(),
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
            | SqlxPreparedStatementBackend::Sqlite(_) => None,
        }
    }

    /// 返回 `SQLx MySQL` 的真实预编译语句。
    pub(crate) fn mysql_statement(&self) -> Option<&MySqlStatement<'static>> {
        match &self.backend {
            SqlxPreparedStatementBackend::MySql(statement) => Some(statement),
            SqlxPreparedStatementBackend::Any(_)
            | SqlxPreparedStatementBackend::PostgreSql(_)
            | SqlxPreparedStatementBackend::Sqlite(_) => None,
        }
    }

    /// 返回 `SQLx PostgreSQL` 的真实预编译语句。
    pub(crate) fn postgresql_statement(&self) -> Option<&PgStatement<'static>> {
        match &self.backend {
            SqlxPreparedStatementBackend::PostgreSql(statement) => Some(statement),
            SqlxPreparedStatementBackend::Any(_)
            | SqlxPreparedStatementBackend::MySql(_)
            | SqlxPreparedStatementBackend::Sqlite(_) => None,
        }
    }

    /// 返回 `SQLx SQLite` 的真实预编译语句。
    pub(crate) fn sqlite_statement(&self) -> Option<&SqliteStatement<'static>> {
        match &self.backend {
            SqlxPreparedStatementBackend::Sqlite(statement) => Some(statement),
            SqlxPreparedStatementBackend::Any(_)
            | SqlxPreparedStatementBackend::MySql(_)
            | SqlxPreparedStatementBackend::PostgreSql(_) => None,
        }
    }

    pub(crate) fn returns_rows(&self) -> bool {
        match &self.backend {
            SqlxPreparedStatementBackend::Any(statement) => !statement.columns().is_empty(),
            SqlxPreparedStatementBackend::MySql(statement) => !statement.columns().is_empty(),
            SqlxPreparedStatementBackend::PostgreSql(statement) => !statement.columns().is_empty(),
            SqlxPreparedStatementBackend::Sqlite(statement) => !statement.columns().is_empty(),
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
        }
    }

    /// 在 `SQLx` 物理 setter 边界物化一个完整 JDBC 参数描述符。
    pub(crate) fn materialize_parameter(
        parameter: &PreparedInputParameter,
    ) -> Result<Value, DruidError> {
        PreparedParameterMaterializer::materialize(parameter)
    }

    /// 返回物理 setter 已经保存的参数值。
    pub(crate) fn materialized_parameters(
        &self,
        parameter_count: usize,
    ) -> Result<Vec<Value>, DruidError> {
        self.parameter_state.values(parameter_count)
    }

    /// 消费物理句柄保存的有序参数批次。
    pub(crate) fn take_batches(
        &self,
        expected_count: usize,
    ) -> Result<Option<Vec<Vec<Value>>>, DruidError> {
        self.parameter_state.take_batches(expected_count)
    }
}

impl PhysicalPreparedStatement for SqlxPreparedStatement {
    fn sql(&self) -> &str {
        &self.sql
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn max_field_size(&self) -> Result<i32, DruidError> {
        self.statement.max_field_size()
    }

    fn set_max_field_size(&self, max: i32) -> Result<(), DruidError> {
        self.statement.set_max_field_size(max)
    }

    fn max_rows(&self) -> Result<i32, DruidError> {
        self.statement.max_rows()
    }

    fn set_max_rows(&self, max: i32) -> Result<(), DruidError> {
        self.statement.set_max_rows(max)
    }

    fn set_escape_processing(&self, enabled: bool) -> Result<(), DruidError> {
        self.statement.set_escape_processing(enabled)
    }

    fn query_timeout(&self) -> Result<i32, DruidError> {
        self.statement.query_timeout()
    }

    fn set_query_timeout(&self, seconds: i32) -> Result<(), DruidError> {
        self.statement.set_query_timeout(seconds)
    }

    fn cancel(&self) -> Result<(), DruidError> {
        self.statement.cancel()
    }

    fn warnings(&self) -> Result<Option<druid::core::SqlWarning>, DruidError> {
        self.statement.warnings()
    }

    fn clear_warnings(&self) -> Result<(), DruidError> {
        self.statement.clear_warnings()
    }

    fn set_cursor_name(&self, name: &str) -> Result<(), DruidError> {
        self.statement.set_cursor_name(name)
    }

    fn set_fetch_direction(&self, direction: i32) -> Result<(), DruidError> {
        self.statement.set_fetch_direction(direction)
    }

    fn fetch_direction(&self) -> Result<i32, DruidError> {
        self.statement.fetch_direction()
    }

    fn set_fetch_size(&self, rows: i32) -> Result<(), DruidError> {
        self.statement.set_fetch_size(rows)
    }

    fn fetch_size(&self) -> Result<i32, DruidError> {
        self.statement.fetch_size()
    }

    fn close_on_completion(&self) -> Result<(), DruidError> {
        self.statement.close_on_completion()
    }

    fn is_close_on_completion(&self) -> Result<bool, DruidError> {
        self.statement.is_close_on_completion()
    }

    fn set_parameter(
        &self,
        parameter_index: usize,
        parameter: &PreparedInputParameter,
    ) -> Result<(), DruidError> {
        self.parameter_state.set(parameter_index, parameter)
    }

    fn clear_parameters(&self) -> Result<(), DruidError> {
        self.parameter_state.clear_parameters();
        Ok(())
    }

    fn add_batch(&self, params: &[Value]) -> Result<(), DruidError> {
        self.parameter_state.add_values(params);
        Ok(())
    }

    fn add_parameter_batch(&self, params: &[PreparedInputParameter]) -> Result<(), DruidError> {
        self.parameter_state.add_parameters(params)
    }

    fn clear_batch(&self) -> Result<(), DruidError> {
        self.parameter_state.clear_batches();
        Ok(())
    }

    fn close(&self) -> Result<(), DruidError> {
        self.closed.store(true, Ordering::Release);
        self.statement.close()?;
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
}
