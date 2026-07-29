//! SQLx 物理连接适配器。

use super::sqlx_prepared_statement::SqlxPreparedStatement;
use bigdecimal::{BigDecimal, FromPrimitive};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid::core::{
    DruidError, ExecResult, PhysicalConnection, PhysicalConnectionCapabilities,
    PhysicalPreparedStatement, PhysicalResultSet, PreparedInputParameter, PreparedStatementKey,
    Row, RowSetResultSet, Savepoint, SqlWarning, StatementExecuteResult, StatementGeneratedKeys,
    Value,
};
use sqlx::any::{AnyRow, AnyTransactionManager, AnyTypeInfoKind};
use sqlx::sqlite::{SqliteArguments, SqliteRow, SqliteTransactionManager};
use sqlx::{
    Any, AnyConnection, Column, Connection as SqlxConnection, Executor, Row as SqlxRow, Sqlite,
    SqliteConnection, Statement, TransactionManager, TypeInfo, ValueRef,
};
use std::sync::Arc;

fn any_bind_unsupported(value_type: &'static str) -> DruidError {
    DruidError::UnsupportedOperation {
        operation: match value_type {
            "decimal" => "sqlx_any_bind_decimal",
            "date" => "sqlx_any_bind_date",
            "time" => "sqlx_any_bind_time",
            "timestamp" => "sqlx_any_bind_timestamp",
            _ => "sqlx_any_bind_strong_value",
        },
    }
}

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
        match error {
            sqlx::Error::Database(database_error) => {
                let sql_state = database_error.code().map(std::borrow::Cow::into_owned);
                let error_code = sql_state
                    .as_deref()
                    .and_then(|code| code.parse::<i32>().ok())
                    .unwrap_or_default();
                DruidError::SqlException(Box::new(
                    druid::core::SqlException::new(
                        error_code,
                        sql_state,
                        Some(database_error.message().to_string()),
                    )
                    .with_class_name("sqlx::error::DatabaseError"),
                ))
            }
            error => DruidError::DriverError(error.to_string()),
        }
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

    fn prepared_statement(
        statement: &dyn PhysicalPreparedStatement,
    ) -> Result<&SqlxPreparedStatement, DruidError> {
        statement
            .as_any()
            .downcast_ref::<SqlxPreparedStatement>()
            .ok_or_else(|| {
                DruidError::DriverError(
                    "prepared statement was not created by SqlxConnectionAdapter".to_string(),
                )
            })
    }

    fn materialized_parameters(
        statement: &dyn PhysicalPreparedStatement,
        parameters: &[PreparedInputParameter],
    ) -> Result<Vec<Value>, DruidError> {
        Self::prepared_statement(statement)?.materialized_parameters(parameters.len())
    }

    fn bind_any_values<'query>(
        sql: &'query str,
        params: Vec<Value>,
    ) -> Result<sqlx::query::Query<'query, Any, sqlx::any::AnyArguments<'query>>, DruidError> {
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

    fn bind_any_prepared_values<'query>(
        statement: &'query sqlx::any::AnyStatement<'query>,
        params: Vec<Value>,
    ) -> Result<sqlx::query::Query<'query, Any, sqlx::any::AnyArguments<'query>>, DruidError> {
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
                "NUMERIC" | "DECIMAL" => {
                    let value = match runtime_type.name() {
                        "INTEGER" => BigDecimal::from(
                            row.try_get::<i64, _>(index).map_err(Self::driver_error)?,
                        ),
                        "REAL" => BigDecimal::from_f64(
                            row.try_get::<f64, _>(index).map_err(Self::driver_error)?,
                        )
                        .ok_or_else(|| {
                            DruidError::DriverError(
                                "SQLite REAL cannot be represented as BigDecimal".to_string(),
                            )
                        })?,
                        _ => row
                            .try_get::<String, _>(index)
                            .map_err(Self::driver_error)?
                            .parse::<BigDecimal>()
                            .map_err(|error| DruidError::DriverError(error.to_string()))?,
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
                    return Err(DruidError::DriverError(format!(
                        "SQLite type {unsupported} is not represented by druid::core::Value"
                    )));
                }
            };
            values.push(value);
        }
        Ok(Row::new(values))
    }

    async fn fetch_rows_with_labels(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<(Vec<Row>, Vec<String>), DruidError> {
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
                let result = Self::bind_any_values(sql, params)?
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
        self.fetch_rows_with_labels(sql, params)
            .await
            .map(|(rows, _)| rows)
    }

    async fn fetch_result_set(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let (rows, labels) = self.fetch_rows_with_labels(sql, params).await?;
        Ok(Arc::new(RowSetResultSet::with_column_labels(rows, labels)))
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
                let result = Self::bind_any_prepared_values(statement, params)?
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

    async fn exec_prepared_parameters(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameters: Vec<PreparedInputParameter>,
    ) -> Result<ExecResult, DruidError> {
        let params = Self::materialized_parameters(statement, &parameters)?;
        self.exec_prepared(statement, params).await
    }

    async fn execute_prepared(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
        _generated_keys: StatementGeneratedKeys,
    ) -> Result<Vec<StatementExecuteResult>, DruidError> {
        let statement_ref = Self::prepared_statement(statement)?;
        let returns_rows = statement_ref
            .sqlite_statement()
            .map(|statement| !statement.columns().is_empty())
            .or_else(|| {
                statement_ref
                    .any_statement()
                    .map(|statement| !statement.columns().is_empty())
            })
            .unwrap_or(false);
        if returns_rows {
            self.fetch_prepared(statement, params)
                .await
                .map(|rows| vec![StatementExecuteResult::ResultSet(rows)])
        } else {
            self.exec_prepared(statement, params)
                .await
                .map(|result| vec![StatementExecuteResult::Update(result)])
        }
    }

    async fn execute_prepared_parameters(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameters: Vec<PreparedInputParameter>,
        generated_keys: StatementGeneratedKeys,
    ) -> Result<Vec<StatementExecuteResult>, DruidError> {
        let params = Self::materialized_parameters(statement, &parameters)?;
        self.execute_prepared(statement, params, generated_keys)
            .await
    }

    async fn exec_prepared_parameter_batch(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameter_sets: Vec<Vec<PreparedInputParameter>>,
    ) -> Result<Vec<i32>, DruidError> {
        let statement_ref = Self::prepared_statement(statement)?;
        let parameter_sets =
            if let Some(parameter_sets) = statement_ref.take_batches(parameter_sets.len())? {
                parameter_sets
            } else {
                parameter_sets
                    .iter()
                    .map(|parameters| {
                        parameters
                            .iter()
                            .map(SqlxPreparedStatement::materialize_parameter)
                            .collect::<Result<Vec<_>, _>>()
                    })
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| DruidError::BatchUpdateException {
                        update_counts: Vec::new(),
                        cause: Box::new(error),
                    })?
            };

        let mut update_counts = Vec::with_capacity(parameter_sets.len());
        for params in parameter_sets {
            match self.exec_prepared(statement, params).await {
                Ok(result) => {
                    update_counts.push(i32::try_from(result.rows_affected).unwrap_or(i32::MAX));
                }
                Err(error) => {
                    return Err(DruidError::BatchUpdateException {
                        update_counts,
                        cause: Box::new(error),
                    });
                }
            }
        }
        Ok(update_counts)
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
                let rows = Self::bind_any_prepared_values(statement, params)?
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

    async fn fetch_prepared_parameters(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameters: Vec<PreparedInputParameter>,
    ) -> Result<Vec<Row>, DruidError> {
        let params = Self::materialized_parameters(statement, &parameters)?;
        self.fetch_prepared(statement, params).await
    }

    async fn fetch_prepared_result_set(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let sqlx_statement = statement
            .as_any()
            .downcast_ref::<SqlxPreparedStatement>()
            .ok_or_else(|| {
                DruidError::DriverError(
                    "prepared statement was not created by SqlxConnectionAdapter".to_string(),
                )
            })?;
        let labels = sqlx_statement
            .any_statement()
            .map(|statement| {
                statement
                    .columns()
                    .iter()
                    .map(|column| column.name().to_string())
                    .collect::<Vec<_>>()
            })
            .or_else(|| {
                sqlx_statement.sqlite_statement().map(|statement| {
                    statement
                        .columns()
                        .iter()
                        .map(|column| column.name().to_string())
                        .collect::<Vec<_>>()
                })
            })
            .unwrap_or_default();
        let rows = self.fetch_prepared(statement, params).await?;
        Ok(Arc::new(RowSetResultSet::with_column_labels(rows, labels)))
    }

    async fn fetch_prepared_parameters_result_set(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        parameters: Vec<PreparedInputParameter>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let params = Self::materialized_parameters(statement, &parameters)?;
        self.fetch_prepared_result_set(statement, params).await
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
            clear_warnings: true,
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

    /// 返回 SQLx 连接的 SQLWarning 链。
    ///
    /// 对应 Java：`java.sql.Connection#getWarnings()`。SQLx 的公开 Connection
    /// SPI 不暴露 JDBC warning 链，因此存活连接返回 `None`；关闭或已丢弃连接
    /// 仍按 Druid 连接状态语义返回 `ConnectionDiscarded`。
    async fn warnings(&mut self) -> Result<Option<SqlWarning>, DruidError> {
        self.connection_mut()?;
        Ok(None)
    }

    /// 清除 SQLx 连接的 SQLWarning。
    ///
    /// 对应 Java：`java.sql.Connection#clearWarnings()`。SQLx 不保留可清理的
    /// warning 状态，存活连接无操作成功，关闭或已丢弃连接返回状态错误。
    async fn clear_warnings(&mut self) -> Result<(), DruidError> {
        self.connection_mut()?;
        Ok(())
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
