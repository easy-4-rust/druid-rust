//! `SQLx` canonical 连接适配器。

use super::sqlx_connection_actor::SqlxConnectionActor;
use super::sqlx_prepared_statement::SqlxPreparedStatement;
use crate::standard_adapter_executor::AdapterUpdateResult;
use crate::task4_runtime_trace::RuntimeTraceEmitter;
use bigdecimal::{BigDecimal, FromPrimitive};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid_rdbc::error::RdbcError;
use druid_rdbc::types::{Row, Value};
use sqlx::any::{AnyRow, AnyTransactionManager, AnyTypeInfoKind};
use sqlx::mysql::{MySqlArguments, MySqlConnection, MySqlRow, MySqlTransactionManager};
use sqlx::postgres::{PgArguments, PgConnection, PgRow, PgTransactionManager};
use sqlx::sqlite::{SqliteArguments, SqliteRow, SqliteTransactionManager};
use sqlx::{
    Any, AnyConnection, Column, Connection as SqlxConnection, Executor, MySql, Postgres,
    Row as SqlxRow, Sqlite, SqliteConnection, Statement, TransactionManager, TypeInfo, ValueRef,
};
use std::sync::Arc;

type SqlxExecResult = AdapterUpdateResult;

fn any_bind_unsupported(value_type: &'static str) -> RdbcError {
    RdbcError::UnsupportedOperation {
        operation: match value_type {
            "decimal" => "sqlx_any_bind_decimal",
            "date" => "sqlx_any_bind_date",
            "time" => "sqlx_any_bind_time",
            "timestamp" => "sqlx_any_bind_timestamp",
            _ => "sqlx_any_bind_strong_value",
        },
    }
}

#[cfg(test)]
fn read_only_session_sql(backend: &str, read_only: bool) -> Option<&'static str> {
    match (backend, read_only) {
        ("postgresql", true) => Some("SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY"),
        ("postgresql", false) => Some("SET SESSION CHARACTERISTICS AS TRANSACTION READ WRITE"),
        ("mysql", true) => Some("SET SESSION TRANSACTION READ ONLY"),
        ("mysql", false) => Some("SET SESSION TRANSACTION READ WRITE"),
        _ => None,
    }
}

enum SqlxConnectionBackend {
    Any(AnyConnection),
    MySql(MySqlConnection),
    PostgreSql(PgConnection),
    Sqlite(SqliteConnection),
}

/// `SQLx` 物理连接适配器。
///
/// 对应 Java 平台依赖: `java.sql.Connection` 的驱动实现。
/// 本对象只包装一个 `SQLx` Connection，不包含 `SQLx` Pool、bb8 或 deadpool，
/// 因而不会形成 pool-in-pool。SQLite、MySQL、PostgreSQL 使用各自原生连接，
/// 以保留 Decimal 和日期时间等强类型；未知 `SQLx` scheme 才回退到 `Any`。
pub struct SqlxConnectionAdapter {
    connection: Option<SqlxConnectionBackend>,
    url: String,
    savepoint_sequence: u64,
    auto_commit: bool,
    read_only: bool,
    discarded: bool,
    standard_actor: Option<Arc<SqlxConnectionActor>>,
    runtime_trace: Option<RuntimeTraceEmitter>,
}

impl SqlxConnectionAdapter {
    /// 直接连接数据库并创建 Adapter。
    ///
    /// 参数 `url` 为 `SQLx` 数据库 URL；返回未池化的物理连接 Adapter。
    pub async fn connect(url: &str) -> Result<Self, RdbcError> {
        let connection = if url.starts_with("sqlite:") {
            SqlxConnectionBackend::Sqlite(
                SqliteConnection::connect(url)
                    .await
                    .map_err(Self::driver_error)?,
            )
        } else if url.starts_with("mysql:") {
            SqlxConnectionBackend::MySql(
                MySqlConnection::connect(url)
                    .await
                    .map_err(Self::driver_error)?,
            )
        } else if url.starts_with("postgres:") || url.starts_with("postgresql:") {
            SqlxConnectionBackend::PostgreSql(
                PgConnection::connect(url)
                    .await
                    .map_err(Self::driver_error)?,
            )
        } else {
            sqlx::any::install_default_drivers();
            SqlxConnectionBackend::Any(
                AnyConnection::connect(url)
                    .await
                    .map_err(Self::driver_error)?,
            )
        };
        Ok(Self {
            connection: Some(connection),
            url: url.to_owned(),
            savepoint_sequence: 0,
            auto_commit: true,
            read_only: false,
            discarded: false,
            standard_actor: None,
            runtime_trace: None,
        })
    }

    pub(crate) fn install_runtime_trace(&mut self, runtime_trace: RuntimeTraceEmitter) {
        self.runtime_trace = Some(runtime_trace);
    }

    pub(super) fn take_runtime_trace(&mut self) -> Option<RuntimeTraceEmitter> {
        self.runtime_trace.take()
    }

    /// 把当前 raw adapter 转成 canonical owned actor，并在 connection handle 中保留 clone。
    fn standard_actor_core(&mut self) -> Result<Arc<SqlxConnectionActor>, RdbcError> {
        if let Some(actor) = &self.standard_actor {
            return Ok(Arc::clone(actor));
        }
        if self.connection.is_none() || self.discarded {
            return Err(RdbcError::ConnectionDiscarded);
        }
        let detached = Self {
            connection: None,
            url: self.url.clone(),
            savepoint_sequence: self.savepoint_sequence,
            auto_commit: self.auto_commit,
            read_only: self.read_only,
            discarded: self.discarded,
            standard_actor: None,
            runtime_trace: None,
        };
        let owned = std::mem::replace(self, detached);
        let actor = SqlxConnectionActor::new(owned);
        self.standard_actor = Some(Arc::clone(&actor));
        Ok(actor)
    }

    pub(super) fn standard_auto_commit_core(&self) -> bool {
        self.auto_commit
    }

    pub(super) fn standard_product_name_core(&self) -> Result<String, RdbcError> {
        match self.backend_key() {
            "sqlite" => Ok("SQLite".to_owned()),
            "mysql" => Ok("MySQL".to_owned()),
            "postgresql" => Ok("PostgreSQL".to_owned()),
            _ => Err(RdbcError::UnsupportedOperation {
                operation: "sqlx_database_metadata_product_name",
            }),
        }
    }

    pub(super) async fn close_connection_core(&mut self) -> Result<(), RdbcError> {
        match self.connection.take() {
            Some(SqlxConnectionBackend::Any(connection)) => {
                SqlxConnection::close(connection)
                    .await
                    .map_err(Self::driver_error)?;
            }
            Some(SqlxConnectionBackend::MySql(connection)) => {
                SqlxConnection::close(connection)
                    .await
                    .map_err(Self::driver_error)?;
            }
            Some(SqlxConnectionBackend::PostgreSql(connection)) => {
                SqlxConnection::close(connection)
                    .await
                    .map_err(Self::driver_error)?;
            }
            Some(SqlxConnectionBackend::Sqlite(connection)) => {
                SqlxConnection::close(connection)
                    .await
                    .map_err(Self::driver_error)?;
            }
            None => {}
        }
        Ok(())
    }

    pub(super) async fn ping_connection_core(&mut self) -> Result<(), RdbcError> {
        if let Some(actor) = &self.standard_actor {
            return actor.ping().await;
        }
        self.ping_raw_core().await
    }

    pub(super) async fn ping_raw_core(&mut self) -> Result<(), RdbcError> {
        match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => SqlxConnection::ping(connection)
                .await
                .map_err(Self::driver_error),
            SqlxConnectionBackend::MySql(connection) => SqlxConnection::ping(connection)
                .await
                .map_err(Self::driver_error),
            SqlxConnectionBackend::PostgreSql(connection) => SqlxConnection::ping(connection)
                .await
                .map_err(Self::driver_error),
            SqlxConnectionBackend::Sqlite(connection) => SqlxConnection::ping(connection)
                .await
                .map_err(Self::driver_error),
        }
    }

    pub(super) async fn begin_raw_core(&mut self) -> Result<(), RdbcError> {
        if !self.auto_commit {
            return Err(RdbcError::DriverError(
                "SQLx transaction is already active".to_owned(),
            ));
        }
        match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => {
                AnyTransactionManager::begin(connection, None)
                    .await
                    .map_err(Self::driver_error)
            }
            SqlxConnectionBackend::MySql(connection) => {
                MySqlTransactionManager::begin(connection, None)
                    .await
                    .map_err(Self::driver_error)
            }
            SqlxConnectionBackend::PostgreSql(connection) => {
                PgTransactionManager::begin(connection, None)
                    .await
                    .map_err(Self::driver_error)
            }
            SqlxConnectionBackend::Sqlite(connection) => {
                SqliteTransactionManager::begin(connection, None)
                    .await
                    .map_err(Self::driver_error)
            }
        }?;
        self.auto_commit = false;
        Ok(())
    }

    pub(super) async fn commit_raw_core(&mut self) -> Result<(), RdbcError> {
        match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => AnyTransactionManager::commit(connection)
                .await
                .map_err(Self::driver_error),
            SqlxConnectionBackend::MySql(connection) => MySqlTransactionManager::commit(connection)
                .await
                .map_err(Self::driver_error),
            SqlxConnectionBackend::PostgreSql(connection) => {
                PgTransactionManager::commit(connection)
                    .await
                    .map_err(Self::driver_error)
            }
            SqlxConnectionBackend::Sqlite(connection) => {
                SqliteTransactionManager::commit(connection)
                    .await
                    .map_err(Self::driver_error)
            }
        }?;
        self.auto_commit = true;
        Ok(())
    }

    pub(super) async fn rollback_raw_core(&mut self) -> Result<(), RdbcError> {
        match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => AnyTransactionManager::rollback(connection)
                .await
                .map_err(Self::driver_error),
            SqlxConnectionBackend::MySql(connection) => {
                MySqlTransactionManager::rollback(connection)
                    .await
                    .map_err(Self::driver_error)
            }
            SqlxConnectionBackend::PostgreSql(connection) => {
                PgTransactionManager::rollback(connection)
                    .await
                    .map_err(Self::driver_error)
            }
            SqlxConnectionBackend::Sqlite(connection) => {
                SqliteTransactionManager::rollback(connection)
                    .await
                    .map_err(Self::driver_error)
            }
        }?;
        self.auto_commit = true;
        Ok(())
    }

    async fn set_auto_commit_core(&mut self, enabled: bool) -> Result<(), RdbcError> {
        if let Some(actor) = &self.standard_actor {
            return actor.set_auto_commit(enabled).await;
        }
        match (self.auto_commit, enabled) {
            (true, false) => self.begin_raw_core().await,
            (false, true) => self.commit_raw_core().await,
            _ => Ok(()),
        }
    }

    async fn commit_core(&mut self) -> Result<(), RdbcError> {
        if let Some(actor) = &self.standard_actor {
            actor.commit().await
        } else {
            self.commit_raw_core().await
        }
    }

    async fn rollback_core(&mut self) -> Result<(), RdbcError> {
        if let Some(actor) = &self.standard_actor {
            actor.rollback().await
        } else {
            self.rollback_raw_core().await
        }
    }

    fn connection_mut(&mut self) -> Result<&mut SqlxConnectionBackend, RdbcError> {
        if self.discarded {
            return Err(RdbcError::ConnectionDiscarded);
        }
        self.connection
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)
    }

    pub(super) fn backend_key(&self) -> &'static str {
        match self.connection {
            Some(SqlxConnectionBackend::Any(_)) => "any",
            Some(SqlxConnectionBackend::MySql(_)) => "mysql",
            Some(SqlxConnectionBackend::PostgreSql(_)) => "postgresql",
            Some(SqlxConnectionBackend::Sqlite(_)) => "sqlite",
            None => "closed",
        }
    }

    pub(super) fn driver_error(error: sqlx::Error) -> RdbcError {
        match error {
            sqlx::Error::Database(database_error) => {
                let sql_state = database_error.code().map(std::borrow::Cow::into_owned);
                let error_code = sql_state
                    .as_deref()
                    .and_then(|code| code.parse::<i32>().ok())
                    .unwrap_or_default();
                RdbcError::SqlException(Box::new(
                    druid_rdbc::error::SqlException::new(
                        error_code,
                        sql_state,
                        Some(database_error.message().to_string()),
                    )
                    .with_class_name("sqlx::error::DatabaseError"),
                ))
            }
            sqlx::Error::Io(error) => RdbcError::SqlException(Box::new(
                druid_rdbc::error::SqlException::new(
                    0,
                    Some("08006".to_owned()),
                    Some(error.to_string()),
                )
                .with_class_name("std::io::Error")
                .recoverable(),
            )),
            sqlx::Error::Tls(error) => RdbcError::SqlException(Box::new(
                druid_rdbc::error::SqlException::new(
                    0,
                    Some("08006".to_owned()),
                    Some(error.to_string()),
                )
                .with_class_name("sqlx::error::TlsError")
                .recoverable(),
            )),
            sqlx::Error::Protocol(message) => RdbcError::SqlException(Box::new(
                druid_rdbc::error::SqlException::new(0, Some("08006".to_owned()), Some(message))
                    .with_class_name("sqlx::error::ProtocolError")
                    .recoverable(),
            )),
            error @ (sqlx::Error::PoolTimedOut
            | sqlx::Error::PoolClosed
            | sqlx::Error::WorkerCrashed) => RdbcError::SqlException(Box::new(
                druid_rdbc::error::SqlException::new(
                    0,
                    Some("08006".to_owned()),
                    Some(error.to_string()),
                )
                .with_class_name("sqlx::Error")
                .recoverable(),
            )),
            error => RdbcError::DriverError(error.to_string()),
        }
    }

    fn bind_any_values(
        sql: &str,
        params: Vec<Value>,
    ) -> Result<sqlx::query::Query<'_, Any, sqlx::any::AnyArguments<'_>>, RdbcError> {
        let mut query = sqlx::query(sql);
        for value in params {
            query = match value {
                Value::Null => query.bind(Option::<String>::None),
                Value::Bool(value) => query.bind(value),
                Value::Int(value) => query.bind(value),
                Value::Float(value) => query.bind(value),
                Value::String(value) => query.bind(value),
                Value::Bytes(value) => query.bind(value),
                Value::Decimal(_) => return Err(any_bind_unsupported("decimal")),
                Value::Date(_) => return Err(any_bind_unsupported("date")),
                Value::Time(_) => return Err(any_bind_unsupported("time")),
                Value::Timestamp(_) => return Err(any_bind_unsupported("timestamp")),
            };
        }
        Ok(query)
    }

    fn bind_sqlite_values(
        sql: &str,
        params: Vec<Value>,
    ) -> sqlx::query::Query<'_, Sqlite, SqliteArguments<'_>> {
        let mut query = sqlx::query(sql);
        for value in params {
            query = match value {
                Value::Null => query.bind(Option::<String>::None),
                Value::Bool(value) => query.bind(value),
                Value::Int(value) => query.bind(value),
                Value::Float(value) => query.bind(value),
                // SQLite 没有 DECIMAL storage class。绑定阶段使用十进制文本，
                // 但 NUMERIC affinity 仍可能在存储时转为 INTEGER/REAL；读取端
                // 必须按真实 runtime storage class 上报，不能伪造 Decimal。
                Value::Decimal(value) => query.bind(value.to_string()),
                Value::Date(value) => query.bind(value),
                Value::Time(value) => query.bind(value),
                Value::Timestamp(value) => query.bind(value),
                Value::String(value) => query.bind(value),
                Value::Bytes(value) => query.bind(value),
            };
        }
        query
    }

    fn bind_mysql_values(
        sql: &str,
        params: Vec<Value>,
    ) -> sqlx::query::Query<'_, MySql, MySqlArguments> {
        let mut query = sqlx::query(sql);
        for value in params {
            query = match value {
                Value::Null => query.bind(Option::<String>::None),
                Value::Bool(value) => query.bind(value),
                Value::Int(value) => query.bind(value),
                Value::Float(value) => query.bind(value),
                Value::Decimal(value) => query.bind(value),
                Value::Date(value) => query.bind(value),
                Value::Time(value) => query.bind(value),
                Value::Timestamp(value) => query.bind(value),
                Value::String(value) => query.bind(value),
                Value::Bytes(value) => query.bind(value),
            };
        }
        query
    }

    fn bind_postgresql_values(
        sql: &str,
        params: Vec<Value>,
    ) -> sqlx::query::Query<'_, Postgres, PgArguments> {
        let mut query = sqlx::query(sql);
        for value in params {
            query = match value {
                Value::Null => query.bind(Option::<String>::None),
                Value::Bool(value) => query.bind(value),
                Value::Int(value) => query.bind(value),
                Value::Float(value) => query.bind(value),
                Value::Decimal(value) => query.bind(value),
                Value::Date(value) => query.bind(value),
                Value::Time(value) => query.bind(value),
                Value::Timestamp(value) => query.bind(value),
                Value::String(value) => query.bind(value),
                Value::Bytes(value) => query.bind(value),
            };
        }
        query
    }

    fn bind_any_prepared_values<'query>(
        statement: &'query sqlx::any::AnyStatement<'query>,
        params: Vec<Value>,
    ) -> Result<sqlx::query::Query<'query, Any, sqlx::any::AnyArguments<'query>>, RdbcError> {
        let mut query = statement.query();
        for value in params {
            query = match value {
                Value::Null => query.bind(Option::<String>::None),
                Value::Bool(value) => query.bind(value),
                Value::Int(value) => query.bind(value),
                Value::Float(value) => query.bind(value),
                Value::String(value) => query.bind(value),
                Value::Bytes(value) => query.bind(value),
                Value::Decimal(_) => return Err(any_bind_unsupported("decimal")),
                Value::Date(_) => return Err(any_bind_unsupported("date")),
                Value::Time(_) => return Err(any_bind_unsupported("time")),
                Value::Timestamp(_) => return Err(any_bind_unsupported("timestamp")),
            };
        }
        Ok(query)
    }

    fn bind_sqlite_prepared_values<'query>(
        statement: &'query sqlx::sqlite::SqliteStatement<'query>,
        params: Vec<Value>,
    ) -> sqlx::query::Query<'query, Sqlite, SqliteArguments<'query>> {
        let mut query = statement.query();
        for value in params {
            query = match value {
                Value::Null => query.bind(Option::<String>::None),
                Value::Bool(value) => query.bind(value),
                Value::Int(value) => query.bind(value),
                Value::Float(value) => query.bind(value),
                Value::Decimal(value) => query.bind(value.to_string()),
                Value::Date(value) => query.bind(value),
                Value::Time(value) => query.bind(value),
                Value::Timestamp(value) => query.bind(value),
                Value::String(value) => query.bind(value),
                Value::Bytes(value) => query.bind(value),
            };
        }
        query
    }

    fn bind_mysql_prepared_values<'query>(
        statement: &'query sqlx::mysql::MySqlStatement<'query>,
        params: Vec<Value>,
    ) -> sqlx::query::Query<'query, MySql, MySqlArguments> {
        let mut query = statement.query();
        for value in params {
            query = match value {
                Value::Null => query.bind(Option::<String>::None),
                Value::Bool(value) => query.bind(value),
                Value::Int(value) => query.bind(value),
                Value::Float(value) => query.bind(value),
                Value::Decimal(value) => query.bind(value),
                Value::Date(value) => query.bind(value),
                Value::Time(value) => query.bind(value),
                Value::Timestamp(value) => query.bind(value),
                Value::String(value) => query.bind(value),
                Value::Bytes(value) => query.bind(value),
            };
        }
        query
    }

    fn bind_postgresql_prepared_values<'query>(
        statement: &'query sqlx::postgres::PgStatement<'query>,
        params: Vec<Value>,
    ) -> sqlx::query::Query<'query, Postgres, PgArguments> {
        let mut query = statement.query();
        for value in params {
            query = match value {
                Value::Null => query.bind(Option::<String>::None),
                Value::Bool(value) => query.bind(value),
                Value::Int(value) => query.bind(value),
                Value::Float(value) => query.bind(value),
                Value::Decimal(value) => query.bind(value),
                Value::Date(value) => query.bind(value),
                Value::Time(value) => query.bind(value),
                Value::Timestamp(value) => query.bind(value),
                Value::String(value) => query.bind(value),
                Value::Bytes(value) => query.bind(value),
            };
        }
        query
    }

    fn decode_any_row(row: AnyRow) -> Result<Row, RdbcError> {
        let mut values = Vec::with_capacity(row.columns().len());
        for (index, column) in row.columns().iter().enumerate() {
            let raw = row.try_get_raw(index).map_err(Self::driver_error)?;
            if raw.is_null() {
                values.push(Value::Null);
                continue;
            }

            let value = match column.type_info().kind() {
                AnyTypeInfoKind::Null => Value::Null,
                AnyTypeInfoKind::Bool => {
                    Value::Bool(row.try_get(index).map_err(Self::driver_error)?)
                }
                AnyTypeInfoKind::SmallInt => {
                    let value: i16 = row.try_get(index).map_err(Self::driver_error)?;
                    Value::Int(i64::from(value))
                }
                AnyTypeInfoKind::Integer => {
                    let value: i32 = row.try_get(index).map_err(Self::driver_error)?;
                    Value::Int(i64::from(value))
                }
                AnyTypeInfoKind::BigInt => {
                    Value::Int(row.try_get(index).map_err(Self::driver_error)?)
                }
                AnyTypeInfoKind::Real => {
                    let value: f32 = row.try_get(index).map_err(Self::driver_error)?;
                    Value::Float(f64::from(value))
                }
                AnyTypeInfoKind::Double => {
                    Value::Float(row.try_get(index).map_err(Self::driver_error)?)
                }
                AnyTypeInfoKind::Text => {
                    Value::String(row.try_get(index).map_err(Self::driver_error)?)
                }
                AnyTypeInfoKind::Blob => {
                    Value::Bytes(row.try_get(index).map_err(Self::driver_error)?)
                }
            };
            values.push(value);
        }
        Ok(Row::new(values))
    }

    fn decode_mysql_row(row: MySqlRow) -> Result<Row, RdbcError> {
        let mut values = Vec::with_capacity(row.columns().len());
        for (index, column) in row.columns().iter().enumerate() {
            let raw = row.try_get_raw(index).map_err(Self::driver_error)?;
            if raw.is_null() {
                values.push(Value::Null);
                continue;
            }
            let type_name = column.type_info().name().to_ascii_uppercase();
            let value = match type_name.as_str() {
                "BOOLEAN" | "BOOL" => Value::Bool(row.try_get(index).map_err(Self::driver_error)?),
                "TINYINT" => {
                    let value: i8 = row.try_get(index).map_err(Self::driver_error)?;
                    Value::Int(i64::from(value))
                }
                "SMALLINT" => {
                    let value: i16 = row.try_get(index).map_err(Self::driver_error)?;
                    Value::Int(i64::from(value))
                }
                "MEDIUMINT" | "INT" | "INTEGER" => {
                    let value: i32 = row.try_get(index).map_err(Self::driver_error)?;
                    Value::Int(i64::from(value))
                }
                "BIGINT" => Value::Int(row.try_get(index).map_err(Self::driver_error)?),
                "FLOAT" => {
                    let value: f32 = row.try_get(index).map_err(Self::driver_error)?;
                    Value::Float(f64::from(value))
                }
                "DOUBLE" | "REAL" => Value::Float(row.try_get(index).map_err(Self::driver_error)?),
                "DECIMAL" | "NUMERIC" | "NEWDECIMAL" => {
                    Value::Decimal(row.try_get(index).map_err(Self::driver_error)?)
                }
                "DATE" => Value::Date(row.try_get(index).map_err(Self::driver_error)?),
                "TIME" => Value::Time(row.try_get(index).map_err(Self::driver_error)?),
                "DATETIME" | "TIMESTAMP" => {
                    Value::Timestamp(row.try_get(index).map_err(Self::driver_error)?)
                }
                name if name.contains("BLOB") || name.contains("BINARY") => {
                    Value::Bytes(row.try_get(index).map_err(Self::driver_error)?)
                }
                name if name.contains("CHAR")
                    || name.contains("TEXT")
                    || matches!(name, "ENUM" | "SET" | "JSON") =>
                {
                    Value::String(row.try_get(index).map_err(Self::driver_error)?)
                }
                unsupported => {
                    return Err(RdbcError::DriverError(format!(
                        "MySQL type {unsupported} is not represented by druid::core::Value"
                    )));
                }
            };
            values.push(value);
        }
        Ok(Row::new(values))
    }

    fn decode_postgresql_row(row: PgRow) -> Result<Row, RdbcError> {
        let mut values = Vec::with_capacity(row.columns().len());
        for (index, column) in row.columns().iter().enumerate() {
            let raw = row.try_get_raw(index).map_err(Self::driver_error)?;
            if raw.is_null() {
                values.push(Value::Null);
                continue;
            }
            let type_name = column.type_info().name().to_ascii_uppercase();
            let value = match type_name.as_str() {
                "BOOL" | "BOOLEAN" => Value::Bool(row.try_get(index).map_err(Self::driver_error)?),
                "INT2" | "SMALLINT" => {
                    let value: i16 = row.try_get(index).map_err(Self::driver_error)?;
                    Value::Int(i64::from(value))
                }
                "INT4" | "INTEGER" => {
                    let value: i32 = row.try_get(index).map_err(Self::driver_error)?;
                    Value::Int(i64::from(value))
                }
                "INT8" | "BIGINT" => Value::Int(row.try_get(index).map_err(Self::driver_error)?),
                "FLOAT4" | "REAL" => {
                    let value: f32 = row.try_get(index).map_err(Self::driver_error)?;
                    Value::Float(f64::from(value))
                }
                "FLOAT8" | "DOUBLE PRECISION" => {
                    Value::Float(row.try_get(index).map_err(Self::driver_error)?)
                }
                "NUMERIC" | "DECIMAL" => {
                    Value::Decimal(row.try_get(index).map_err(Self::driver_error)?)
                }
                "DATE" => Value::Date(row.try_get(index).map_err(Self::driver_error)?),
                "TIME" => Value::Time(row.try_get(index).map_err(Self::driver_error)?),
                "TIMESTAMP" => Value::Timestamp(row.try_get(index).map_err(Self::driver_error)?),
                "BYTEA" => Value::Bytes(row.try_get(index).map_err(Self::driver_error)?),
                "TEXT" | "VARCHAR" | "CHAR" | "BPCHAR" | "NAME" => {
                    Value::String(row.try_get(index).map_err(Self::driver_error)?)
                }
                unsupported => {
                    return Err(RdbcError::DriverError(format!(
                        "PostgreSQL type {unsupported} is not represented by druid::core::Value"
                    )));
                }
            };
            values.push(value);
        }
        Ok(Row::new(values))
    }

    fn decode_sqlite_row(row: SqliteRow) -> Result<Row, RdbcError> {
        let mut values = Vec::with_capacity(row.columns().len());
        for (index, column) in row.columns().iter().enumerate() {
            let raw = row.try_get_raw(index).map_err(Self::driver_error)?;
            if raw.is_null() {
                values.push(Value::Null);
                continue;
            }

            // SQLite 的表达式列（如 COUNT(*)）没有声明类型，SQLx 会把
            // column type 标为 NULL，但运行时 value type 仍是 INTEGER/REAL 等。
            // 普通 BOOLEAN 列仍优先使用声明类型，保留 Druid 的布尔值语义。
            let runtime_type = raw.type_info();
            let type_name = match column.type_info().name() {
                "NULL" => runtime_type.name(),
                declared => declared,
            };
            let value = match type_name {
                "BOOLEAN" => Value::Bool(row.try_get(index).map_err(Self::driver_error)?),
                "INTEGER" => Value::Int(row.try_get(index).map_err(Self::driver_error)?),
                "REAL" => Value::Float(row.try_get(index).map_err(Self::driver_error)?),
                "NUMERIC" | "DECIMAL" => {
                    let value = match runtime_type.name() {
                        "INTEGER" => BigDecimal::from(
                            row.try_get::<i64, _>(index).map_err(Self::driver_error)?,
                        ),
                        "REAL" => BigDecimal::from_f64(
                            row.try_get::<f64, _>(index).map_err(Self::driver_error)?,
                        )
                        .ok_or_else(|| {
                            RdbcError::DriverError(
                                "SQLite REAL cannot be represented as BigDecimal".to_string(),
                            )
                        })?,
                        _ => row
                            .try_get::<String, _>(index)
                            .map_err(Self::driver_error)?
                            .parse::<BigDecimal>()
                            .map_err(|error| RdbcError::DriverError(error.to_string()))?,
                    };
                    Value::Decimal(value)
                }
                "DATE" => Value::Date(
                    row.try_get::<NaiveDate, _>(index)
                        .map_err(Self::driver_error)?,
                ),
                "TIME" => Value::Time(
                    row.try_get::<NaiveTime, _>(index)
                        .map_err(Self::driver_error)?,
                ),
                "DATETIME" | "TIMESTAMP" => Value::Timestamp(
                    row.try_get::<NaiveDateTime, _>(index)
                        .map_err(Self::driver_error)?,
                ),
                "TEXT" => Value::String(row.try_get(index).map_err(Self::driver_error)?),
                "BLOB" => Value::Bytes(row.try_get(index).map_err(Self::driver_error)?),
                "NULL" => Value::Null,
                unsupported => {
                    return Err(RdbcError::DriverError(format!(
                        "SQLite type {unsupported} is not represented by druid::core::Value"
                    )));
                }
            };
            values.push(value);
        }
        Ok(Row::new(values))
    }

    pub(super) async fn fetch_rows_with_labels(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<(Vec<Row>, Vec<String>), RdbcError> {
        match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => {
                let statement = {
                    let statement = (&mut *connection)
                        .prepare(sql)
                        .await
                        .map_err(Self::driver_error)?;
                    Statement::to_owned(&statement)
                };
                let labels = statement
                    .columns()
                    .iter()
                    .map(|column| column.name().to_string())
                    .collect();
                let rows = Self::bind_any_prepared_values(&statement, params)?
                    .fetch_all(connection)
                    .await
                    .map_err(Self::driver_error)?;
                let rows = rows
                    .into_iter()
                    .map(Self::decode_any_row)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((rows, labels))
            }
            SqlxConnectionBackend::MySql(connection) => {
                let statement = {
                    let statement = (&mut *connection)
                        .prepare(sql)
                        .await
                        .map_err(Self::driver_error)?;
                    Statement::to_owned(&statement)
                };
                let labels = statement
                    .columns()
                    .iter()
                    .map(|column| column.name().to_owned())
                    .collect();
                let rows = Self::bind_mysql_prepared_values(&statement, params)
                    .fetch_all(connection)
                    .await
                    .map_err(Self::driver_error)?;
                let rows = rows
                    .into_iter()
                    .map(Self::decode_mysql_row)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((rows, labels))
            }
            SqlxConnectionBackend::PostgreSql(connection) => {
                let statement = {
                    let statement = (&mut *connection)
                        .prepare(sql)
                        .await
                        .map_err(Self::driver_error)?;
                    Statement::to_owned(&statement)
                };
                let labels = statement
                    .columns()
                    .iter()
                    .map(|column| column.name().to_owned())
                    .collect();
                let rows = Self::bind_postgresql_prepared_values(&statement, params)
                    .fetch_all(connection)
                    .await
                    .map_err(Self::driver_error)?;
                let rows = rows
                    .into_iter()
                    .map(Self::decode_postgresql_row)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((rows, labels))
            }
            SqlxConnectionBackend::Sqlite(connection) => {
                let statement = {
                    let statement = (&mut *connection)
                        .prepare(sql)
                        .await
                        .map_err(Self::driver_error)?;
                    Statement::to_owned(&statement)
                };
                let labels = statement
                    .columns()
                    .iter()
                    .map(|column| column.name().to_string())
                    .collect();
                let rows = Self::bind_sqlite_prepared_values(&statement, params)
                    .fetch_all(connection)
                    .await
                    .map_err(Self::driver_error)?;
                let rows = rows
                    .into_iter()
                    .map(Self::decode_sqlite_row)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((rows, labels))
            }
        }
    }

    pub(super) async fn standard_parameter_count_core(
        &mut self,
        sql: &str,
    ) -> Result<usize, RdbcError> {
        fn count<'query, S: Statement<'query>>(statement: &S) -> Result<usize, RdbcError> {
            match statement.parameters() {
                Some(sqlx::Either::Left(types)) => Ok(types.len()),
                Some(sqlx::Either::Right(count)) => Ok(count),
                None => Err(RdbcError::UnsupportedOperation {
                    operation: "sqlx_parameter_metadata_unavailable",
                }),
            }
        }
        match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => {
                let statement = (&mut *connection)
                    .prepare(sql)
                    .await
                    .map_err(Self::driver_error)?;
                count(&statement)
            }
            SqlxConnectionBackend::MySql(connection) => {
                let statement = (&mut *connection)
                    .prepare(sql)
                    .await
                    .map_err(Self::driver_error)?;
                count(&statement)
            }
            SqlxConnectionBackend::PostgreSql(connection) => {
                let statement = (&mut *connection)
                    .prepare(sql)
                    .await
                    .map_err(Self::driver_error)?;
                count(&statement)
            }
            SqlxConnectionBackend::Sqlite(connection) => {
                let statement = (&mut *connection)
                    .prepare(sql)
                    .await
                    .map_err(Self::driver_error)?;
                count(&statement)
            }
        }
    }

    pub(super) async fn execute_update_core(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<SqlxExecResult, RdbcError> {
        match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => {
                let result = Self::bind_any_values(sql, params)?
                    .execute(connection)
                    .await
                    .map_err(Self::driver_error)?;
                Ok(AdapterUpdateResult {
                    rows_affected: result.rows_affected(),
                    last_insert_id: result.last_insert_id(),
                    row_count: None,
                })
            }
            SqlxConnectionBackend::MySql(connection) => {
                let result = Self::bind_mysql_values(sql, params)
                    .execute(connection)
                    .await
                    .map_err(Self::driver_error)?;
                Ok(AdapterUpdateResult {
                    rows_affected: result.rows_affected(),
                    last_insert_id: (result.last_insert_id() != 0)
                        .then(|| i64::try_from(result.last_insert_id()).unwrap_or(i64::MAX)),
                    row_count: None,
                })
            }
            SqlxConnectionBackend::PostgreSql(connection) => {
                let result = Self::bind_postgresql_values(sql, params)
                    .execute(connection)
                    .await
                    .map_err(Self::driver_error)?;
                Ok(AdapterUpdateResult {
                    rows_affected: result.rows_affected(),
                    last_insert_id: None,
                    row_count: None,
                })
            }
            SqlxConnectionBackend::Sqlite(connection) => {
                let result = Self::bind_sqlite_values(sql, params)
                    .execute(connection)
                    .await
                    .map_err(Self::driver_error)?;
                let last_insert_id =
                    (result.last_insert_rowid() != 0).then_some(result.last_insert_rowid());
                Ok(AdapterUpdateResult {
                    rows_affected: result.rows_affected(),
                    last_insert_id,
                    row_count: None,
                })
            }
        }
    }
}

druid_rdbc::impl_standard_wrapper!(SqlxConnectionAdapter => Connection);

#[async_trait::async_trait]
impl druid_rdbc::connection::Connection for SqlxConnectionAdapter {
    async fn create_statement(
        &mut self,
        options: druid_rdbc::statement::StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::Statement>, RdbcError> {
        let actor = self.standard_actor_core()?;
        Ok(Box::new(SqlxPreparedStatement::owned_statement(
            String::new(),
            actor,
            options,
            0,
        )))
    }

    async fn prepare_statement(
        &mut self,
        sql: &str,
        options: druid_rdbc::statement::StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::PreparedStatement>, RdbcError> {
        let actor = self.standard_actor_core()?;
        let parameter_count = actor.parameter_count(sql).await?;
        Ok(Box::new(SqlxPreparedStatement::owned_prepared_statement(
            sql.to_owned(),
            actor,
            options,
            parameter_count,
        )))
    }

    async fn database_meta_data(
        &mut self,
    ) -> Result<Box<dyn druid_rdbc::metadata::DatabaseMetaData>, RdbcError> {
        Ok(Box::new(
            super::sqlx_database_meta_data::SqlxDatabaseMetaData::owned(
                self.standard_actor_core()?,
            ),
        ))
    }

    async fn commit(&mut self) -> Result<(), RdbcError> {
        self.commit_core().await
    }

    async fn rollback(&mut self) -> Result<(), RdbcError> {
        self.rollback_core().await
    }

    async fn set_auto_commit(&mut self, enabled: bool) -> Result<(), RdbcError> {
        self.set_auto_commit_core(enabled).await
    }

    fn auto_commit(&self) -> Result<bool, RdbcError> {
        Ok(self.standard_actor.as_ref().map_or_else(
            || self.standard_auto_commit_core(),
            |actor| actor.auto_commit(),
        ))
    }

    fn capabilities(&self) -> druid_rdbc::extensions::ConnectionCapabilities {
        druid_rdbc::extensions::ConnectionCapabilities {
            transactions: true,
            savepoints: false,
            catalogs: false,
            schemas: false,
            callable_statements: false,
        }
    }

    async fn is_valid(&mut self, _timeout_seconds: u32) -> Result<bool, RdbcError> {
        match self.ping_connection_core().await {
            Ok(()) => Ok(true),
            Err(RdbcError::ConnectionDiscarded) => Ok(false),
            Err(error) => Err(error),
        }
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        if let Some(actor) = &self.standard_actor {
            actor.close().await
        } else {
            self.close_connection_core().await
        }
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self
            .standard_actor
            .as_ref()
            .map_or_else(|| self.connection.is_none(), |actor| actor.is_closed()))
    }
}

#[cfg(test)]
mod tests {
    use super::read_only_session_sql;

    #[test]
    fn read_only_session_sql_is_backend_specific() {
        assert_eq!(
            read_only_session_sql("postgresql", true),
            Some("SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY")
        );
        assert_eq!(
            read_only_session_sql("mysql", false),
            Some("SET SESSION TRANSACTION READ WRITE")
        );
        assert_eq!(read_only_session_sql("sqlite", true), None);
        assert_eq!(read_only_session_sql("any", true), None);
    }
}
