//! SQLx 物理连接适配器。

use crate::sqlx_prepared_statement::SqlxPreparedStatement;
use druid_core::{
    DruidError, ExecResult, PhysicalConnection, PhysicalConnectionCapabilities,
    PhysicalPreparedStatement, PreparedStatementKey, Row, Savepoint, Value,
};
use sqlx::any::{AnyRow, AnyTransactionManager, AnyTypeInfoKind};
use sqlx::sqlite::{SqliteArguments, SqliteRow, SqliteTransactionManager};
use sqlx::{
    Any, AnyConnection, Column, Connection as SqlxConnection, Executor, Row as SqlxRow, Sqlite,
    SqliteConnection, Statement, TransactionManager, TypeInfo, ValueRef,
};
use std::sync::Arc;

enum SqlxConnectionBackend {
    Any(AnyConnection),
    Sqlite(SqliteConnection),
}

/// SQLx 物理连接适配器。
///
/// 对应 Java 平台依赖: `java.sql.Connection` 的驱动实现。
/// 本对象只包装一个 SQLx Connection，不包含 SQLx Pool、bb8 或 deadpool，
/// 因而不会形成 pool-in-pool。SQLite 使用原生连接以保留 BOOLEAN 等类型；
/// MySQL、PostgreSQL 使用 SQLx Any 的统一连接边界。
pub struct SqlxConnectionAdapter {
    connection: Option<SqlxConnectionBackend>,
    savepoint_sequence: u64,
    discarded: bool,
}

impl SqlxConnectionAdapter {
    /// 直接连接数据库并创建 Adapter。
    ///
    /// 参数 `url` 为 SQLx 数据库 URL；返回未池化的物理连接 Adapter。
    pub async fn connect(url: &str) -> Result<Self, DruidError> {
        let connection = if url.starts_with("sqlite:") {
            SqlxConnectionBackend::Sqlite(
                SqliteConnection::connect(url)
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
            savepoint_sequence: 0,
            discarded: false,
        })
    }

    fn connection_mut(&mut self) -> Result<&mut SqlxConnectionBackend, DruidError> {
        if self.discarded {
            return Err(DruidError::ConnectionDiscarded);
        }
        self.connection
            .as_mut()
            .ok_or(DruidError::ConnectionDiscarded)
    }

    fn driver_error(error: sqlx::Error) -> DruidError {
        DruidError::DriverError(error.to_string())
    }

    fn validate_savepoint_name(name: &str) -> Result<(), DruidError> {
        let valid = !name.is_empty()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');
        if valid {
            Ok(())
        } else {
            Err(DruidError::DriverError(
                "savepoint name must contain only ASCII letters, digits, or '_'".to_string(),
            ))
        }
    }

    fn bind_any_values<'query>(
        sql: &'query str,
        params: Vec<Value>,
    ) -> sqlx::query::Query<'query, Any, sqlx::any::AnyArguments<'query>> {
        let mut query = sqlx::query(sql);
        for value in params {
            query = match value {
                Value::Null => query.bind(Option::<String>::None),
                Value::Bool(value) => query.bind(value),
                Value::Int(value) => query.bind(value),
                Value::Float(value) => query.bind(value),
                Value::String(value) => query.bind(value),
                Value::Bytes(value) => query.bind(value),
            };
        }
        query
    }

    fn bind_sqlite_values<'query>(
        sql: &'query str,
        params: Vec<Value>,
    ) -> sqlx::query::Query<'query, Sqlite, SqliteArguments<'query>> {
        let mut query = sqlx::query(sql);
        for value in params {
            query = match value {
                Value::Null => query.bind(Option::<String>::None),
                Value::Bool(value) => query.bind(value),
                Value::Int(value) => query.bind(value),
                Value::Float(value) => query.bind(value),
                Value::String(value) => query.bind(value),
                Value::Bytes(value) => query.bind(value),
            };
        }
        query
    }

    fn bind_any_prepared_values<'query>(
        statement: &'query sqlx::any::AnyStatement<'query>,
        params: Vec<Value>,
    ) -> sqlx::query::Query<'query, Any, sqlx::any::AnyArguments<'query>> {
        let mut query = statement.query();
        for value in params {
            query = match value {
                Value::Null => query.bind(Option::<String>::None),
                Value::Bool(value) => query.bind(value),
                Value::Int(value) => query.bind(value),
                Value::Float(value) => query.bind(value),
                Value::String(value) => query.bind(value),
                Value::Bytes(value) => query.bind(value),
            };
        }
        query
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
                Value::String(value) => query.bind(value),
                Value::Bytes(value) => query.bind(value),
            };
        }
        query
    }

    fn decode_any_row(row: AnyRow) -> Result<Row, DruidError> {
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

    fn decode_sqlite_row(row: SqliteRow) -> Result<Row, DruidError> {
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
                "TEXT" => Value::String(row.try_get(index).map_err(Self::driver_error)?),
                "BLOB" => Value::Bytes(row.try_get(index).map_err(Self::driver_error)?),
                "NULL" => Value::Null,
                unsupported => {
                    return Err(DruidError::DriverError(format!(
                        "SQLite type {unsupported} is not represented by druid_core::Value"
                    )));
                }
            };
            values.push(value);
        }
        Ok(Row::new(values))
    }

    async fn execute_control_statement(&mut self, sql: &str) -> Result<(), DruidError> {
        match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => {
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
}

#[async_trait::async_trait]
impl PhysicalConnection for SqlxConnectionAdapter {
    async fn exec(&mut self, sql: &str, params: Vec<Value>) -> Result<ExecResult, DruidError> {
        match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => {
                let result = Self::bind_any_values(sql, params)
                    .execute(connection)
                    .await
                    .map_err(Self::driver_error)?;
                Ok(ExecResult {
                    rows_affected: result.rows_affected(),
                    last_insert_id: result.last_insert_id(),
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
                Ok(ExecResult {
                    rows_affected: result.rows_affected(),
                    last_insert_id,
                    row_count: None,
                })
            }
        }
    }

    async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => {
                let rows = Self::bind_any_values(sql, params)
                    .fetch_all(connection)
                    .await
                    .map_err(Self::driver_error)?;
                rows.into_iter().map(Self::decode_any_row).collect()
            }
            SqlxConnectionBackend::Sqlite(connection) => {
                let rows = Self::bind_sqlite_values(sql, params)
                    .fetch_all(connection)
                    .await
                    .map_err(Self::driver_error)?;
                rows.into_iter().map(Self::decode_sqlite_row).collect()
            }
        }
    }

    async fn prepare_physical_statement(
        &mut self,
        key: &PreparedStatementKey,
    ) -> Result<Arc<dyn PhysicalPreparedStatement>, DruidError> {
        let sql = key.sql().to_string();
        let statement: Arc<dyn PhysicalPreparedStatement> = match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => {
                let statement = (&mut *connection)
                    .prepare(key.sql())
                    .await
                    .map_err(Self::driver_error)?;
                let statement = Statement::to_owned(&statement);
                Arc::new(SqlxPreparedStatement::any(sql, statement))
            }
            SqlxConnectionBackend::Sqlite(connection) => {
                let statement = (&mut *connection)
                    .prepare(key.sql())
                    .await
                    .map_err(Self::driver_error)?;
                let statement = Statement::to_owned(&statement);
                Arc::new(SqlxPreparedStatement::sqlite(sql, statement))
            }
        };
        Ok(statement)
    }

    async fn exec_prepared(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
    ) -> Result<ExecResult, DruidError> {
        let statement = statement
            .as_any()
            .downcast_ref::<SqlxPreparedStatement>()
            .ok_or_else(|| {
                DruidError::DriverError(
                    "prepared statement was not created by SqlxConnectionAdapter".to_string(),
                )
            })?;
        if statement.is_closed() {
            return Err(DruidError::ConnectionDiscarded);
        }
        let sqlite = matches!(self.connection, Some(SqlxConnectionBackend::Sqlite(_)));
        if !statement.matches_backend(sqlite) {
            return Err(DruidError::DriverError(
                "SQLx prepared statement backend does not match connection".to_string(),
            ));
        }
        match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => {
                let statement = statement.any_statement().ok_or_else(|| {
                    DruidError::DriverError(
                        "SQLx prepared statement backend does not match connection".to_string(),
                    )
                })?;
                let result = Self::bind_any_prepared_values(statement, params)
                    .execute(connection)
                    .await
                    .map_err(Self::driver_error)?;
                Ok(ExecResult {
                    rows_affected: result.rows_affected(),
                    last_insert_id: result.last_insert_id(),
                    row_count: None,
                })
            }
            SqlxConnectionBackend::Sqlite(connection) => {
                let statement = statement.sqlite_statement().ok_or_else(|| {
                    DruidError::DriverError(
                        "SQLx prepared statement backend does not match connection".to_string(),
                    )
                })?;
                let result = Self::bind_sqlite_prepared_values(statement, params)
                    .execute(connection)
                    .await
                    .map_err(Self::driver_error)?;
                let last_insert_id =
                    (result.last_insert_rowid() != 0).then_some(result.last_insert_rowid());
                Ok(ExecResult {
                    rows_affected: result.rows_affected(),
                    last_insert_id,
                    row_count: None,
                })
            }
        }
    }

    async fn fetch_prepared(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
    ) -> Result<Vec<Row>, DruidError> {
        let statement = statement
            .as_any()
            .downcast_ref::<SqlxPreparedStatement>()
            .ok_or_else(|| {
                DruidError::DriverError(
                    "prepared statement was not created by SqlxConnectionAdapter".to_string(),
                )
            })?;
        if statement.is_closed() {
            return Err(DruidError::ConnectionDiscarded);
        }
        let sqlite = matches!(self.connection, Some(SqlxConnectionBackend::Sqlite(_)));
        if !statement.matches_backend(sqlite) {
            return Err(DruidError::DriverError(
                "SQLx prepared statement backend does not match connection".to_string(),
            ));
        }
        match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => {
                let statement = statement.any_statement().ok_or_else(|| {
                    DruidError::DriverError(
                        "SQLx prepared statement backend does not match connection".to_string(),
                    )
                })?;
                let rows = Self::bind_any_prepared_values(statement, params)
                    .fetch_all(connection)
                    .await
                    .map_err(Self::driver_error)?;
                rows.into_iter().map(Self::decode_any_row).collect()
            }
            SqlxConnectionBackend::Sqlite(connection) => {
                let statement = statement.sqlite_statement().ok_or_else(|| {
                    DruidError::DriverError(
                        "SQLx prepared statement backend does not match connection".to_string(),
                    )
                })?;
                let rows = Self::bind_sqlite_prepared_values(statement, params)
                    .fetch_all(connection)
                    .await
                    .map_err(Self::driver_error)?;
                rows.into_iter().map(Self::decode_sqlite_row).collect()
            }
        }
    }

    async fn begin(&mut self) -> Result<(), DruidError> {
        match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => {
                AnyTransactionManager::begin(connection, None)
                    .await
                    .map_err(Self::driver_error)
            }
            SqlxConnectionBackend::Sqlite(connection) => {
                SqliteTransactionManager::begin(connection, None)
                    .await
                    .map_err(Self::driver_error)
            }
        }
    }

    async fn commit(&mut self) -> Result<(), DruidError> {
        match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => AnyTransactionManager::commit(connection)
                .await
                .map_err(Self::driver_error),
            SqlxConnectionBackend::Sqlite(connection) => {
                SqliteTransactionManager::commit(connection)
                    .await
                    .map_err(Self::driver_error)
            }
        }
    }

    async fn rollback(&mut self) -> Result<(), DruidError> {
        match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => AnyTransactionManager::rollback(connection)
                .await
                .map_err(Self::driver_error),
            SqlxConnectionBackend::Sqlite(connection) => {
                SqliteTransactionManager::rollback(connection)
                    .await
                    .map_err(Self::driver_error)
            }
        }
    }

    async fn rollback_to(&mut self, savepoint: &Savepoint) -> Result<(), DruidError> {
        let name = savepoint
            .name
            .clone()
            .unwrap_or_else(|| format!("druid_sp_{}", savepoint.id));
        Self::validate_savepoint_name(&name)?;
        self.execute_control_statement(&format!("ROLLBACK TO SAVEPOINT {name}"))
            .await
    }

    async fn set_savepoint(&mut self) -> Result<Savepoint, DruidError> {
        self.savepoint_sequence = self.savepoint_sequence.saturating_add(1);
        let savepoint = Savepoint {
            id: self.savepoint_sequence,
            name: None,
        };
        self.execute_control_statement(&format!("SAVEPOINT druid_sp_{}", savepoint.id))
            .await?;
        Ok(savepoint)
    }

    async fn set_savepoint_named(&mut self, name: &str) -> Result<Savepoint, DruidError> {
        Self::validate_savepoint_name(name)?;
        self.savepoint_sequence = self.savepoint_sequence.saturating_add(1);
        self.execute_control_statement(&format!("SAVEPOINT {name}"))
            .await?;
        Ok(Savepoint {
            id: self.savepoint_sequence,
            name: Some(name.to_string()),
        })
    }

    async fn release_savepoint(&mut self, savepoint: &Savepoint) -> Result<(), DruidError> {
        let name = savepoint
            .name
            .clone()
            .unwrap_or_else(|| format!("druid_sp_{}", savepoint.id));
        Self::validate_savepoint_name(&name)?;
        self.execute_control_statement(&format!("RELEASE SAVEPOINT {name}"))
            .await
    }

    async fn abort(&mut self) -> Result<(), DruidError> {
        self.close().await
    }

    async fn ping(&mut self) -> Result<(), DruidError> {
        match self.connection_mut()? {
            SqlxConnectionBackend::Any(connection) => SqlxConnection::ping(connection)
                .await
                .map_err(Self::driver_error),
            SqlxConnectionBackend::Sqlite(connection) => SqlxConnection::ping(connection)
                .await
                .map_err(Self::driver_error),
        }
    }

    async fn close(&mut self) -> Result<(), DruidError> {
        match self.connection.take() {
            Some(SqlxConnectionBackend::Any(connection)) => {
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

    fn is_closed(&self) -> bool {
        self.connection.is_none()
    }

    fn capabilities(&self) -> PhysicalConnectionCapabilities {
        PhysicalConnectionCapabilities {
            transactions: true,
            savepoints: true,
            auto_commit: true,
            read_only: false,
            transaction_isolation: false,
            holdability: false,
            clear_warnings: false,
            catalog: false,
            schema: false,
        }
    }

    fn auto_commit(&self) -> bool {
        match &self.connection {
            Some(SqlxConnectionBackend::Any(connection)) => {
                AnyTransactionManager::get_transaction_depth(connection) == 0
            }
            Some(SqlxConnectionBackend::Sqlite(connection)) => {
                SqliteTransactionManager::get_transaction_depth(connection) == 0
            }
            None => true,
        }
    }

    async fn set_auto_commit(&mut self, auto_commit: bool) -> Result<(), DruidError> {
        let current = self.auto_commit();
        match (current, auto_commit) {
            (true, false) => self.begin().await,
            (false, true) => {
                while !self.auto_commit() {
                    self.commit().await?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn mark_discarded(&mut self) {
        self.discarded = true;
    }

    fn is_discarded(&self) -> bool {
        self.discarded
    }

    fn driver_name(&self) -> &str {
        match &self.connection {
            Some(SqlxConnectionBackend::Any(connection)) => connection.backend_name(),
            Some(SqlxConnectionBackend::Sqlite(_)) => "SQLite",
            None => "sqlx-closed",
        }
    }
}
