//! `SQLx` canonical 连接适配器。

use super::sqlx_connection_actor::SqlxConnectionActor;
use super::sqlx_prepared_statement::SqlxPreparedStatement;
use crate::standard_adapter_executor::AdapterUpdateResult;
use crate::task4_runtime_trace::RuntimeTraceEmitter;
use bigdecimal::{BigDecimal, FromPrimitive};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{ColumnDescriptor, ColumnNullability, ResultSetColumnType};
use druid_rdbc::types::{Row, Types, Value};
use sqlx::any::{AnyRow, AnyTransactionManager, AnyTypeInfoKind};
use sqlx::mysql::{MySqlArguments, MySqlConnection, MySqlRow, MySqlTransactionManager};
use sqlx::postgres::{PgArguments, PgConnection, PgRow, PgTransactionManager};
use sqlx::sqlite::{SqliteArguments, SqliteRow, SqliteTransactionManager};
use sqlx::{
    Any, AnyConnection, Column, Connection as SqlxConnection, Executor, MySql, Postgres,
    Row as SqlxRow, Sqlite, SqliteConnection, Statement, TransactionManager, TypeInfo, ValueRef,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

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
    actor_discarded: Option<Arc<AtomicBool>>,
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
            actor_discarded: None,
            standard_actor: None,
            runtime_trace: None,
        })
    }

    pub(crate) fn install_runtime_trace(&mut self, runtime_trace: RuntimeTraceEmitter) {
        self.runtime_trace = Some(runtime_trace);
    }

    pub(super) fn attach_actor_discard_flag(&mut self, flag: Arc<AtomicBool>) {
        self.actor_discarded = Some(flag);
    }

    pub(super) fn take_runtime_trace(&mut self) -> Option<RuntimeTraceEmitter> {
        self.runtime_trace.take()
    }

    fn validate_savepoint_name(name: &str) -> Result<(), RdbcError> {
        let valid = !name.is_empty()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
        if valid {
            Ok(())
        } else {
            Err(RdbcError::InvalidArgument(
                "savepoint name must contain only ASCII letters, digits, or '_'".to_owned(),
            ))
        }
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
            actor_discarded: None,
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

    pub(super) fn standard_read_only_core(&self) -> bool {
        self.read_only
    }

    pub(super) const fn standard_savepoint_sequence_core(&self) -> u64 {
        self.savepoint_sequence
    }

    pub(super) fn standard_url_core(&self) -> &str {
        &self.url
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

    pub(super) async fn standard_product_version_core(&mut self) -> Result<String, RdbcError> {
        match self.connection_mut()? {
            SqlxConnectionBackend::Sqlite(connection) => {
                sqlx::query_scalar::<Sqlite, String>("select sqlite_version()")
                    .fetch_one(connection)
                    .await
                    .map_err(Self::driver_error)
            }
            SqlxConnectionBackend::MySql(connection) => {
                sqlx::query_scalar::<MySql, String>("select version()")
                    .fetch_one(connection)
                    .await
                    .map_err(Self::driver_error)
            }
            SqlxConnectionBackend::PostgreSql(connection) => {
                sqlx::query_scalar::<Postgres, String>("show server_version")
                    .fetch_one(connection)
                    .await
                    .map_err(Self::driver_error)
            }
            SqlxConnectionBackend::Any(connection) => {
                sqlx::query_scalar::<Any, String>("select version()")
                    .fetch_one(connection)
                    .await
                    .map_err(Self::driver_error)
            }
        }
    }

    pub(super) async fn standard_user_name_core(&mut self) -> Result<Option<String>, RdbcError> {
        match self.connection_mut()? {
            SqlxConnectionBackend::Sqlite(_) => Ok(None),
            SqlxConnectionBackend::MySql(connection) => {
                sqlx::query_scalar::<MySql, String>("select current_user()")
                    .fetch_one(connection)
                    .await
                    .map(Some)
                    .map_err(Self::driver_error)
            }
            SqlxConnectionBackend::PostgreSql(connection) => {
                sqlx::query_scalar::<Postgres, String>("select current_user")
                    .fetch_one(connection)
                    .await
                    .map(Some)
                    .map_err(Self::driver_error)
            }
            SqlxConnectionBackend::Any(_) => Err(RdbcError::UnsupportedOperation {
                operation: "sqlx_database_metadata_user_name",
            }),
        }
    }

    pub(super) async fn standard_read_only_metadata_core(&mut self) -> Result<bool, RdbcError> {
        if self.backend_key() == "sqlite" {
            if let SqlxConnectionBackend::Sqlite(connection) = self.connection_mut()? {
                return sqlx::query_scalar::<Sqlite, i64>("PRAGMA query_only")
                    .fetch_one(connection)
                    .await
                    .map(|value| value != 0)
                    .map_err(Self::driver_error);
            }
        }
        Ok(self.read_only)
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

    pub(super) async fn execute_control_statement(&mut self, sql: &str) -> Result<(), RdbcError> {
        match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => {
                sqlx::query(sql)
                    .execute(connection)
                    .await
                    .map_err(Self::driver_error)?;
            }
            SqlxConnectionBackend::MySql(connection) => {
                // MySQL 的 SAVEPOINT 等控制命令不支持 prepared protocol。
                // 此入口只接收已校验的无绑定控制 SQL，使用原连接的 text protocol。
                connection.execute(sql).await.map_err(Self::driver_error)?;
            }
            SqlxConnectionBackend::PostgreSql(connection) => {
                sqlx::query(sql)
                    .execute(connection)
                    .await
                    .map_err(Self::driver_error)?;
            }
            SqlxConnectionBackend::Sqlite(connection) => {
                sqlx::query(sql)
                    .execute(connection)
                    .await
                    .map_err(Self::driver_error)?;
            }
        }
        Ok(())
    }

    pub(super) async fn set_read_only_raw_core(
        &mut self,
        read_only: bool,
    ) -> Result<(), RdbcError> {
        if !self.auto_commit {
            return Err(RdbcError::InvalidArgument(
                "read_only cannot change while a transaction is active".to_owned(),
            ));
        }
        self.connection_mut()?;
        let backend = self.backend_key();
        let Some(sql) = read_only_session_sql(backend, read_only) else {
            if read_only {
                return Err(RdbcError::UnsupportedOperation {
                    operation: if backend == "sqlite" {
                        "sqlx_sqlite_read_only_transaction"
                    } else {
                        "sqlx_driver_read_only_transaction"
                    },
                });
            }
            self.read_only = false;
            return Ok(());
        };
        self.execute_control_statement(sql).await?;
        self.read_only = read_only;
        Ok(())
    }

    async fn standard_control_core(&mut self, sql: &str) -> Result<(), RdbcError> {
        if let Some(actor) = &self.standard_actor {
            actor.execute_control_statement(sql).await
        } else {
            self.execute_control_statement(sql).await
        }
    }

    fn next_savepoint_id_core(&mut self) -> u64 {
        if let Some(actor) = &self.standard_actor {
            actor.next_savepoint_id()
        } else {
            self.savepoint_sequence = self.savepoint_sequence.saturating_add(1);
            self.savepoint_sequence
        }
    }

    fn standard_backend_key_core(&self) -> &'static str {
        self.standard_actor
            .as_ref()
            .map_or_else(|| self.backend_key(), |actor| actor.backend_key())
    }

    fn connection_mut(&mut self) -> Result<&mut SqlxConnectionBackend, RdbcError> {
        if self.discarded {
            return Err(RdbcError::ConnectionDiscarded);
        }
        self.connection
            .as_mut()
            .ok_or(RdbcError::ConnectionDiscarded)
    }

    /// 让 SQLite worker 在执行被取消或截止时间到达时自行中断当前 VM。
    /// Tokio future 被取消时，底层 SQLite worker 仍可能继续运行；因此必须把截止时间
    /// 安装到物理连接，而不能只在调用方等待 future 的地方计时。
    pub(super) async fn install_sqlite_progress_control(
        &mut self,
        deadline: Option<Instant>,
        interrupted: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<(), RdbcError> {
        if let SqlxConnectionBackend::Sqlite(connection) = self.connection_mut()? {
            let mut handle = connection.lock_handle().await.map_err(Self::driver_error)?;
            handle.set_progress_handler(1_000, move || {
                !interrupted.load(std::sync::atomic::Ordering::Acquire)
                    && deadline.is_none_or(|deadline| Instant::now() < deadline)
            });
        }
        Ok(())
    }

    /// 等待 SQLite worker 释放物理句柄，并清除本次执行的中断回调。
    pub(super) async fn clear_sqlite_progress_deadline(&mut self) -> Result<(), RdbcError> {
        if let SqlxConnectionBackend::Sqlite(connection) = self.connection_mut()? {
            connection
                .lock_handle()
                .await
                .map_err(Self::driver_error)?
                .remove_progress_handler();
        }
        Ok(())
    }

    /// 无法确认 worker 已释放时，禁止将物理连接重新借给池。
    pub(super) fn discard_after_unconfirmed_interrupt(&mut self) {
        self.discarded = true;
        if let Some(flag) = &self.actor_discarded {
            flag.store(true, Ordering::Release);
        }
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
                "DATETIME" => Value::Timestamp(row.try_get(index).map_err(Self::driver_error)?),
                "TIMESTAMP" => {
                    // 仅在 MySQL 列类型已确认为 TIMESTAMP 后使用其日期时间解码器。
                    // SQLx 的 NaiveDateTime 类型检查只接受 DATETIME，但两者共享该解码器；
                    // 保留会话返回的年月日时分秒，不额外转换时区或绕过其他列的类型检查。
                    Value::Timestamp(
                        row.try_get_unchecked::<NaiveDateTime, _>(index)
                            .map_err(Self::driver_error)?,
                    )
                }
                name if name.contains("BLOB") || name.contains("BINARY") => {
                    Value::Bytes(row.try_get(index).map_err(Self::driver_error)?)
                }
                name if name.contains("CHAR")
                    || name.contains("TEXT")
                    || std::matches!(name, "ENUM" | "SET" | "JSON") =>
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

    fn column_descriptor(name: &str, type_name: &str, nullable: Option<bool>) -> ColumnDescriptor {
        let uppercase = type_name.to_ascii_uppercase();
        let (column_type, signed, case_sensitive) = match uppercase.as_str() {
            "BOOL" | "BOOLEAN" => (ResultSetColumnType::Jdbc(Types::BOOLEAN), false, false),
            "TINYINT" => (ResultSetColumnType::Jdbc(Types::TINYINT), true, false),
            "SMALLINT" | "INT2" => (ResultSetColumnType::Jdbc(Types::SMALLINT), true, false),
            "INTEGER" | "INT" | "INT4" | "MEDIUMINT" => {
                (ResultSetColumnType::Jdbc(Types::INTEGER), true, false)
            }
            "BIGINT" | "INT8" => (ResultSetColumnType::Jdbc(Types::BIGINT), true, false),
            "REAL" | "FLOAT4" => (ResultSetColumnType::Jdbc(Types::REAL), true, false),
            "FLOAT" => (ResultSetColumnType::Jdbc(Types::FLOAT), true, false),
            "DOUBLE" | "FLOAT8" | "DOUBLE PRECISION" => {
                (ResultSetColumnType::Jdbc(Types::DOUBLE), true, false)
            }
            "NUMERIC" | "DECIMAL" | "NEWDECIMAL" => {
                (ResultSetColumnType::Jdbc(Types::DECIMAL), true, false)
            }
            "DATE" => (ResultSetColumnType::Jdbc(Types::DATE), false, false),
            "TIME" => (ResultSetColumnType::Jdbc(Types::TIME), false, false),
            "DATETIME" | "TIMESTAMP" => (ResultSetColumnType::Jdbc(Types::TIMESTAMP), false, false),
            value if value.contains("BLOB") || value.contains("BINARY") || value == "BYTEA" => {
                (ResultSetColumnType::Jdbc(Types::VARBINARY), false, false)
            }
            value
                if value.contains("CHAR")
                    || value.contains("TEXT")
                    || std::matches!(value, "NAME" | "ENUM" | "SET" | "JSON") =>
            {
                (ResultSetColumnType::Jdbc(Types::VARCHAR), false, true)
            }
            "NULL" => (ResultSetColumnType::Jdbc(Types::NULL), false, false),
            other => (ResultSetColumnType::Named(other.to_owned()), false, true),
        };
        let nullability = match nullable {
            Some(true) => ColumnNullability::Nullable,
            Some(false) => ColumnNullability::NoNulls,
            None => ColumnNullability::Unknown,
        };
        ColumnDescriptor::new(name, type_name, nullability)
            .with_driver_type(name, column_type)
            .with_flags(false, case_sensitive, true, false, signed)
    }

    pub(super) async fn describe_columns_core(
        &mut self,
        sql: &str,
    ) -> Result<Vec<ColumnDescriptor>, RdbcError> {
        match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => {
                let describe = (&mut *connection)
                    .describe(sql)
                    .await
                    .map_err(Self::driver_error)?;
                Ok(describe
                    .columns()
                    .iter()
                    .enumerate()
                    .map(|(index, column)| {
                        Self::column_descriptor(
                            column.name(),
                            column.type_info().name(),
                            describe.nullable(index),
                        )
                    })
                    .collect())
            }
            SqlxConnectionBackend::MySql(connection) => {
                let describe = (&mut *connection)
                    .describe(sql)
                    .await
                    .map_err(Self::driver_error)?;
                Ok(describe
                    .columns()
                    .iter()
                    .enumerate()
                    .map(|(index, column)| {
                        Self::column_descriptor(
                            column.name(),
                            column.type_info().name(),
                            describe.nullable(index),
                        )
                    })
                    .collect())
            }
            SqlxConnectionBackend::PostgreSql(connection) => {
                let describe = (&mut *connection)
                    .describe(sql)
                    .await
                    .map_err(Self::driver_error)?;
                Ok(describe
                    .columns()
                    .iter()
                    .enumerate()
                    .map(|(index, column)| {
                        Self::column_descriptor(
                            column.name(),
                            column.type_info().name(),
                            describe.nullable(index),
                        )
                    })
                    .collect())
            }
            SqlxConnectionBackend::Sqlite(connection) => {
                let describe = (&mut *connection)
                    .describe(sql)
                    .await
                    .map_err(Self::driver_error)?;
                Ok(describe
                    .columns()
                    .iter()
                    .enumerate()
                    .map(|(index, column)| {
                        Self::column_descriptor(
                            column.name(),
                            column.type_info().name(),
                            describe.nullable(index),
                        )
                    })
                    .collect())
            }
        }
    }

    pub(super) async fn fetch_rows_with_labels(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<(Vec<Row>, Vec<ColumnDescriptor>), RdbcError> {
        match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => {
                let describe = (&mut *connection)
                    .describe(sql)
                    .await
                    .map_err(Self::driver_error)?;
                let statement = {
                    let statement = (&mut *connection)
                        .prepare(sql)
                        .await
                        .map_err(Self::driver_error)?;
                    Statement::to_owned(&statement)
                };
                let columns = describe
                    .columns()
                    .iter()
                    .enumerate()
                    .map(|(index, column)| {
                        Self::column_descriptor(
                            column.name(),
                            column.type_info().name(),
                            describe.nullable(index),
                        )
                    })
                    .collect();
                let rows = Self::bind_any_prepared_values(&statement, params)?
                    .fetch_all(connection)
                    .await
                    .map_err(Self::driver_error)?;
                let rows = rows
                    .into_iter()
                    .map(Self::decode_any_row)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((rows, columns))
            }
            SqlxConnectionBackend::MySql(connection) => {
                let describe = (&mut *connection)
                    .describe(sql)
                    .await
                    .map_err(Self::driver_error)?;
                let statement = {
                    let statement = (&mut *connection)
                        .prepare(sql)
                        .await
                        .map_err(Self::driver_error)?;
                    Statement::to_owned(&statement)
                };
                let columns = describe
                    .columns()
                    .iter()
                    .enumerate()
                    .map(|(index, column)| {
                        Self::column_descriptor(
                            column.name(),
                            column.type_info().name(),
                            describe.nullable(index),
                        )
                    })
                    .collect();
                let rows = Self::bind_mysql_prepared_values(&statement, params)
                    .fetch_all(connection)
                    .await
                    .map_err(Self::driver_error)?;
                let rows = rows
                    .into_iter()
                    .map(Self::decode_mysql_row)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((rows, columns))
            }
            SqlxConnectionBackend::PostgreSql(connection) => {
                let describe = (&mut *connection)
                    .describe(sql)
                    .await
                    .map_err(Self::driver_error)?;
                let statement = {
                    let statement = (&mut *connection)
                        .prepare(sql)
                        .await
                        .map_err(Self::driver_error)?;
                    Statement::to_owned(&statement)
                };
                let columns = describe
                    .columns()
                    .iter()
                    .enumerate()
                    .map(|(index, column)| {
                        Self::column_descriptor(
                            column.name(),
                            column.type_info().name(),
                            describe.nullable(index),
                        )
                    })
                    .collect();
                let rows = Self::bind_postgresql_prepared_values(&statement, params)
                    .fetch_all(connection)
                    .await
                    .map_err(Self::driver_error)?;
                let rows = rows
                    .into_iter()
                    .map(Self::decode_postgresql_row)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((rows, columns))
            }
            SqlxConnectionBackend::Sqlite(connection) => {
                let describe = (&mut *connection)
                    .describe(sql)
                    .await
                    .map_err(Self::driver_error)?;
                let statement = {
                    let statement = (&mut *connection)
                        .prepare(sql)
                        .await
                        .map_err(Self::driver_error)?;
                    Statement::to_owned(&statement)
                };
                let columns = describe
                    .columns()
                    .iter()
                    .enumerate()
                    .map(|(index, column)| {
                        Self::column_descriptor(
                            column.name(),
                            column.type_info().name(),
                            describe.nullable(index),
                        )
                    })
                    .collect();
                let rows = Self::bind_sqlite_prepared_values(&statement, params)
                    .fetch_all(connection)
                    .await
                    .map_err(Self::driver_error)?;
                let rows = rows
                    .into_iter()
                    .map(Self::decode_sqlite_row)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((rows, columns))
            }
        }
    }

    /// 在原物理连接上执行按需读取；驱动自有缓冲与 wrapper 请求容量分别受各自预算约束。
    pub(super) async fn serve_cursor_core(
        &mut self,
        sql: &str,
        params: Vec<Value>,
        startup: &mut Option<
            tokio::sync::oneshot::Sender<Result<Vec<ColumnDescriptor>, RdbcError>>,
        >,
        requests: &mut tokio::sync::mpsc::Receiver<super::sqlx_cursor_request::SqlxCursorRequest>,
        stop: &mut tokio::sync::watch::Receiver<bool>,
        parent_stop: &mut tokio::sync::watch::Receiver<bool>,
        statement_stop: &mut tokio::sync::watch::Receiver<bool>,
        interrupted: Arc<std::sync::atomic::AtomicBool>,
    ) -> Result<(), RdbcError> {
        match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => {
                let describe = (&mut *connection)
                    .describe(sql)
                    .await
                    .map_err(Self::driver_error)?;
                let statement = {
                    let statement = (&mut *connection)
                        .prepare(sql)
                        .await
                        .map_err(Self::driver_error)?;
                    Statement::to_owned(&statement)
                };
                let columns = describe
                    .columns()
                    .iter()
                    .enumerate()
                    .map(|(index, column)| {
                        Self::column_descriptor(
                            column.name(),
                            column.type_info().name(),
                            describe.nullable(index),
                        )
                    })
                    .collect();
                let rows = Self::bind_any_prepared_values(&statement, params)?.fetch(connection);
                let rows = futures::StreamExt::map(rows, |row| {
                    row.map_err(Self::driver_error)
                        .and_then(Self::decode_any_row)
                });
                super::sqlx_cursor_request::SqlxCursorRequest::serve(
                    columns,
                    startup,
                    requests,
                    stop,
                    parent_stop,
                    statement_stop,
                    Arc::clone(&interrupted),
                    rows,
                )
                .await
            }
            SqlxConnectionBackend::MySql(connection) => {
                let describe = (&mut *connection)
                    .describe(sql)
                    .await
                    .map_err(Self::driver_error)?;
                let statement = {
                    let statement = (&mut *connection)
                        .prepare(sql)
                        .await
                        .map_err(Self::driver_error)?;
                    Statement::to_owned(&statement)
                };
                let columns = describe
                    .columns()
                    .iter()
                    .enumerate()
                    .map(|(index, column)| {
                        Self::column_descriptor(
                            column.name(),
                            column.type_info().name(),
                            describe.nullable(index),
                        )
                    })
                    .collect();
                let rows = Self::bind_mysql_prepared_values(&statement, params).fetch(connection);
                let rows = futures::StreamExt::map(rows, |row| {
                    row.map_err(Self::driver_error)
                        .and_then(Self::decode_mysql_row)
                });
                super::sqlx_cursor_request::SqlxCursorRequest::serve(
                    columns,
                    startup,
                    requests,
                    stop,
                    parent_stop,
                    statement_stop,
                    Arc::clone(&interrupted),
                    rows,
                )
                .await
            }
            SqlxConnectionBackend::PostgreSql(connection) => {
                let describe = (&mut *connection)
                    .describe(sql)
                    .await
                    .map_err(Self::driver_error)?;
                let statement = {
                    let statement = (&mut *connection)
                        .prepare(sql)
                        .await
                        .map_err(Self::driver_error)?;
                    Statement::to_owned(&statement)
                };
                let columns = describe
                    .columns()
                    .iter()
                    .enumerate()
                    .map(|(index, column)| {
                        Self::column_descriptor(
                            column.name(),
                            column.type_info().name(),
                            describe.nullable(index),
                        )
                    })
                    .collect();
                let rows =
                    Self::bind_postgresql_prepared_values(&statement, params).fetch(connection);
                let rows = futures::StreamExt::map(rows, |row| {
                    row.map_err(Self::driver_error)
                        .and_then(Self::decode_postgresql_row)
                });
                super::sqlx_cursor_request::SqlxCursorRequest::serve(
                    columns,
                    startup,
                    requests,
                    stop,
                    parent_stop,
                    statement_stop,
                    Arc::clone(&interrupted),
                    rows,
                )
                .await
            }
            SqlxConnectionBackend::Sqlite(connection) => {
                let describe = (&mut *connection)
                    .describe(sql)
                    .await
                    .map_err(Self::driver_error)?;
                let statement = {
                    let statement = (&mut *connection)
                        .prepare(sql)
                        .await
                        .map_err(Self::driver_error)?;
                    Statement::to_owned(&statement)
                };
                let columns = describe
                    .columns()
                    .iter()
                    .enumerate()
                    .map(|(index, column)| {
                        Self::column_descriptor(
                            column.name(),
                            column.type_info().name(),
                            describe.nullable(index),
                        )
                    })
                    .collect();
                let rows = Self::bind_sqlite_prepared_values(&statement, params).fetch(connection);
                let rows = futures::StreamExt::map(rows, |row| {
                    row.map_err(Self::driver_error)
                        .and_then(Self::decode_sqlite_row)
                });
                super::sqlx_cursor_request::SqlxCursorRequest::serve(
                    columns,
                    startup,
                    requests,
                    stop,
                    parent_stop,
                    statement_stop,
                    Arc::clone(&interrupted),
                    rows,
                )
                .await
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
                // 驱动在每次操作后都报告连接级 rowid；只有本次非零 INSERT 才能发布它。
                let inserts_rows = matches!(
                    sqlparser::parser::Parser::parse_sql(
                        &sqlparser::dialect::SQLiteDialect {},
                        sql
                    )
                    .as_deref(),
                    Ok([sqlparser::ast::Statement::Insert(_)])
                );
                let last_insert_id =
                    (inserts_rows && result.rows_affected() > 0 && result.last_insert_rowid() != 0)
                        .then_some(result.last_insert_rowid());
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
    async fn exec(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<druid_rdbc::statement::ExecResult, RdbcError> {
        let result = if let Some(actor) = &self.standard_actor {
            actor
                .execute_update_result(
                    sql,
                    params,
                    0,
                    Arc::new(std::sync::atomic::AtomicBool::new(false)),
                )
                .await?
        } else {
            self.execute_update_core(sql, params).await?
        };
        Ok(druid_rdbc::statement::ExecResult {
            rows_affected: result.rows_affected,
            last_insert_id: result.last_insert_id,
            row_count: result.row_count,
        })
    }

    async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, RdbcError> {
        if let Some(actor) = &self.standard_actor {
            actor.fetch_rows(sql, params).await
        } else {
            self.fetch_rows_with_labels(sql, params)
                .await
                .map(|(rows, _)| rows)
        }
    }

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
        self.prepare_statement_with_generated_keys(
            sql,
            options,
            druid_rdbc::statement::GeneratedKeys::None,
        )
        .await
    }

    /// SQLite auto 准备将请求存入 owned statement；未验证的模式在建句柄前拒绝。
    async fn prepare_statement_with_generated_keys(
        &mut self,
        sql: &str,
        options: druid_rdbc::statement::StatementOptions,
        generated_keys: druid_rdbc::statement::GeneratedKeys,
    ) -> Result<Box<dyn druid_rdbc::statement::PreparedStatement>, RdbcError> {
        match &generated_keys {
            druid_rdbc::statement::GeneratedKeys::None => {}
            druid_rdbc::statement::GeneratedKeys::ReturnGeneratedKeys
                if self.standard_backend_key_core() == "sqlite" => {}
            _ => {
                return Err(RdbcError::feature_not_supported(
                    "connection_prepare_statement_generated_keys",
                ));
            }
        }
        let actor = self.standard_actor_core()?;
        let parameter_count = actor.parameter_count(sql).await?;
        let result_set_meta_data = druid_rdbc::metadata::ResultSetMetaDataSnapshot::new(
            actor.describe_columns(sql).await?,
        );
        Ok(Box::new(
            SqlxPreparedStatement::owned_prepared_statement(
                sql.to_owned(),
                actor,
                options,
                parameter_count,
                result_set_meta_data,
            )
            .with_generated_keys_request(generated_keys),
        ))
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

    async fn rollback_to(
        &mut self,
        savepoint: &druid_rdbc::types::Savepoint,
    ) -> Result<(), RdbcError> {
        let name = savepoint
            .name
            .clone()
            .unwrap_or_else(|| format!("druid_sp_{}", savepoint.id));
        Self::validate_savepoint_name(&name)?;
        self.standard_control_core(&format!("ROLLBACK TO SAVEPOINT {name}"))
            .await
    }

    async fn set_savepoint(&mut self) -> Result<druid_rdbc::types::Savepoint, RdbcError> {
        let id = self.next_savepoint_id_core();
        self.standard_control_core(&format!("SAVEPOINT druid_sp_{id}"))
            .await?;
        Ok(druid_rdbc::types::Savepoint { id, name: None })
    }

    async fn set_savepoint_named(
        &mut self,
        name: &str,
    ) -> Result<druid_rdbc::types::Savepoint, RdbcError> {
        Self::validate_savepoint_name(name)?;
        let id = self.next_savepoint_id_core();
        self.standard_control_core(&format!("SAVEPOINT {name}"))
            .await?;
        Ok(druid_rdbc::types::Savepoint {
            id,
            name: Some(name.to_owned()),
        })
    }

    async fn release_savepoint(
        &mut self,
        savepoint: &druid_rdbc::types::Savepoint,
    ) -> Result<(), RdbcError> {
        let name = savepoint
            .name
            .clone()
            .unwrap_or_else(|| format!("druid_sp_{}", savepoint.id));
        Self::validate_savepoint_name(&name)?;
        self.standard_control_core(&format!("RELEASE SAVEPOINT {name}"))
            .await
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

    fn read_only(&self) -> Result<bool, RdbcError> {
        Ok(self
            .standard_actor
            .as_ref()
            .map_or_else(|| self.standard_read_only_core(), |actor| actor.read_only()))
    }

    async fn set_read_only(&mut self, read_only: bool) -> Result<(), RdbcError> {
        if let Some(actor) = &self.standard_actor {
            actor.set_read_only(read_only).await
        } else {
            self.set_read_only_raw_core(read_only).await
        }
    }

    fn capabilities(&self) -> druid_rdbc::extensions::ConnectionCapabilities {
        druid_rdbc::extensions::ConnectionCapabilities {
            transactions: true,
            savepoints: true,
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

    async fn ping(&mut self) -> Result<(), RdbcError> {
        self.ping_connection_core().await
    }

    async fn warnings(&mut self) -> Result<Option<druid_rdbc::error::SqlWarning>, RdbcError> {
        if let Some(actor) = &self.standard_actor {
            actor.warnings()
        } else {
            self.connection_mut()?;
            Ok(None)
        }
    }

    async fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        if let Some(actor) = &self.standard_actor {
            actor.clear_warnings()
        } else {
            self.connection_mut()?;
            Ok(())
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

    fn driver_name(&self) -> Result<String, RdbcError> {
        Ok(match self.standard_backend_key_core() {
            "mysql" => "MySQL",
            "postgresql" => "PostgreSQL",
            "sqlite" => "SQLite",
            "any" => "SQLx Any",
            _ => "sqlx-closed",
        }
        .to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::{read_only_session_sql, SqlxConnectionAdapter, SqlxConnectionBackend};
    use druid_rdbc::connection::Connection;
    use druid_rdbc::error::RdbcError;
    use druid_rdbc::result_set::ColumnIndex;
    use druid_rdbc::statement::StatementOptions;
    use druid_rdbc::types::{RdbcObject, Value};
    use sqlx::any::AnyConnectOptions;
    use sqlx::ConnectOptions;

    fn assert_feature_not_supported<T>(result: Result<T, RdbcError>, operation: &str) {
        match result {
            Err(RdbcError::SqlException(error)) => {
                assert_eq!(error.sql_state(), Some("0A000"), "{operation}");
            }
            Err(error) => panic!("{operation} returned the wrong error: {error:?}"),
            Ok(_) => panic!("{operation} unexpectedly succeeded"),
        }
    }

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

    #[tokio::test]
    async fn any_backend_cancellation_is_exactly_not_supported() {
        sqlx::any::install_default_drivers();
        let options = "sqlite::memory:"
            .parse::<AnyConnectOptions>()
            .expect("Any connection URL should parse");
        let connection = options
            .connect()
            .await
            .expect("SQLx Any SQLite driver should open for fallback capability testing");
        let mut adapter = SqlxConnectionAdapter {
            connection: Some(SqlxConnectionBackend::Any(connection)),
            url: "sqlite::memory:".to_owned(),
            savepoint_sequence: 0,
            auto_commit: true,
            read_only: false,
            discarded: false,
            actor_discarded: None,
            standard_actor: None,
            runtime_trace: None,
        };
        let mut statement = adapter
            .create_statement(StatementOptions::default())
            .await
            .expect("Any-backed statement should be created");
        let error = match statement.cancellation_control() {
            Ok(_) => panic!("Any fallback must not advertise SQLite-native cancellation"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            RdbcError::SqlException(ref sql_error) if sql_error.sql_state() == Some("0A000")
        ));
        statement.close().await.unwrap();
        adapter.close().await.unwrap();
    }

    #[tokio::test]
    async fn sqlite_live_resources_capabilities_and_async_result_set_are_truthful() {
        let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
            .await
            .expect("real SQLx SQLite connection should open");

        assert_eq!(
            connection.capabilities(),
            druid_rdbc::extensions::ConnectionCapabilities {
                transactions: true,
                savepoints: true,
                catalogs: false,
                schemas: false,
                callable_statements: false,
            }
        );

        assert_feature_not_supported(
            connection
                .create_array_of(Some("INTEGER"), Some(Vec::new()))
                .await,
            "Connection#createArrayOf",
        );
        assert_feature_not_supported(connection.create_blob().await, "Connection#createBlob");
        assert_feature_not_supported(connection.create_clob().await, "Connection#createClob");
        assert_feature_not_supported(connection.create_n_clob().await, "Connection#createNClob");
        assert_feature_not_supported(connection.create_sql_xml().await, "Connection#createSQLXML");
        assert_feature_not_supported(
            connection
                .create_struct(Some("address"), Some(Vec::<RdbcObject>::new()))
                .await,
            "Connection#createStruct",
        );

        let mut statement = connection
            .create_statement(StatementOptions::default())
            .await
            .expect("standard statement should be created");
        let mut result_set = statement
            .execute_query("SELECT 10 UNION ALL SELECT 20 UNION ALL SELECT 30")
            .await
            .expect("SQLite should execute the real multi-row query");
        let column = ColumnIndex::new(1).expect("one-based column index should be valid");
        for expected in [10, 20, 30] {
            assert!(result_set.next().await.expect("row fetch should succeed"));
            assert_eq!(
                result_set.value(column).expect("current row exists"),
                Value::Int(expected)
            );
        }
        assert!(!result_set.next().await.expect("EOF should be observable"));
        result_set
            .close()
            .await
            .expect("result set should close explicitly");
        statement.close().await.expect("statement should close");
        connection
            .close()
            .await
            .expect("SQLite connection should close");
    }
}
