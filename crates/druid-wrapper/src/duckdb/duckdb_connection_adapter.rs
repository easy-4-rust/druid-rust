//! DuckDB 原生物理连接适配器。

use super::{DuckDbDatabaseMetaData, DuckDbPreparedStatement};
use bigdecimal::{num_bigint::BigInt, BigDecimal};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid::core::{
    DruidError, ExecResult, PhysicalConnection, PhysicalConnectionCapabilities,
    PhysicalDatabaseMetaData, PhysicalPreparedStatement, PhysicalResultSet, PreparedStatementKey,
    Row, RowSetResultSet, SqlException, SqlWarning, StatementExecuteResult, StatementGeneratedKeys,
    Value,
};
use duckdb::types::{
    Decimal as DuckDbDecimal, FromSql as DuckDbFromSql, Null, ValueRef as DuckDbValueRef,
};
use duckdb::{Connection, InterruptHandle, Statement};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static NEXT_CONNECTION_ID: AtomicU64 = AtomicU64::new(1);

/// DuckDB 原生物理连接适配器。
///
/// 对应 Java 平台依赖：具体 JDBC driver 的 `java.sql.Connection`。本对象只
/// 持有一个 duckdb-rs `Connection`，不持有外部连接池。所有同步 FFI 调用都在
/// Tokio blocking worker 上执行，避免阻塞 Druid 异步调度线程。
pub struct DuckDbConnectionAdapter {
    connection_id: u64,
    connection: Option<Arc<Mutex<Connection>>>,
    interrupt_handle: Arc<InterruptHandle>,
    url: String,
    auto_commit: bool,
    discarded: bool,
}

impl DuckDbConnectionAdapter {
    /// 打开一个 DuckDB 原生未池化连接。
    ///
    /// 参数 `url` 使用 `duckdb:` scheme；`duckdb::memory:` 创建独立内存库。
    pub async fn connect(url: &str) -> Result<Self, DruidError> {
        let target = Self::parse_target(url)?;
        let connection = tokio::task::spawn_blocking(move || match target {
            None => Connection::open_in_memory(),
            Some(path) => Connection::open(path),
        })
        .await
        .map_err(Self::worker_error)?
        .map_err(Self::driver_error)?;
        let interrupt_handle = connection.interrupt_handle();
        Ok(Self {
            connection_id: NEXT_CONNECTION_ID.fetch_add(1, Ordering::Relaxed),
            connection: Some(Arc::new(Mutex::new(connection))),
            interrupt_handle,
            url: url.to_owned(),
            auto_commit: true,
            discarded: false,
        })
    }

    fn parse_target(url: &str) -> Result<Option<PathBuf>, DruidError> {
        if url == "duckdb::memory:" || url == "duckdb:///:memory:" {
            return Ok(None);
        }
        let value = url.strip_prefix("duckdb:").ok_or_else(|| {
            DruidError::InvalidArgument("DuckDB URL must start with `duckdb:`".to_string())
        })?;
        let value = value.strip_prefix("//").unwrap_or(value);
        if value.is_empty()
            || value.contains('\0')
            || value.contains('?')
            || value.contains('#')
            || value.contains('%')
        {
            return Err(DruidError::InvalidArgument(
                "DuckDB file URL must contain a non-empty literal path without query, fragment, NUL or percent encoding"
                    .to_string(),
            ));
        }
        Ok(Some(PathBuf::from(value)))
    }

    fn connection_ref(&self) -> Result<Arc<Mutex<Connection>>, DruidError> {
        if self.discarded {
            return Err(DruidError::ConnectionDiscarded);
        }
        self.connection
            .as_ref()
            .map(Arc::clone)
            .ok_or(DruidError::ConnectionDiscarded)
    }

    async fn run_blocking<T, F>(&self, operation: F) -> Result<T, DruidError>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> Result<T, DruidError> + Send + 'static,
    {
        let connection = self.connection_ref()?;
        tokio::task::spawn_blocking(move || operation(&connection.lock()))
            .await
            .map_err(Self::worker_error)?
    }

    fn worker_error(error: tokio::task::JoinError) -> DruidError {
        DruidError::DriverError(format!("DuckDB blocking worker failed: {error}"))
    }

    pub(super) fn driver_error(error: duckdb::Error) -> DruidError {
        match error {
            duckdb::Error::DuckDBFailure(error, message) => {
                let error_code = i32::try_from(error.extended_code).unwrap_or_default();
                DruidError::SqlException(Box::new(
                    SqlException::new(
                        error_code,
                        None,
                        message.or_else(|| Some(error.to_string())),
                    )
                    .with_class_name("duckdb::Error::DuckDBFailure"),
                ))
            }
            error => DruidError::SqlException(Box::new(
                SqlException::driver(0, error.to_string()).with_class_name("duckdb::Error"),
            )),
        }
    }

    fn decimal_parameter(value: &BigDecimal) -> Result<DuckDbDecimal, DruidError> {
        let (coefficient, exponent) = value.as_bigint_and_exponent();
        let mut scaled = i128::try_from(coefficient).map_err(|_| {
            DruidError::DriverError(format!(
                "DuckDB DECIMAL cannot represent coefficient of `{value}`"
            ))
        })?;
        let scale = if exponent < 0 {
            let power = u32::try_from(-exponent).map_err(|error| {
                DruidError::DriverError(format!("invalid decimal exponent: {error}"))
            })?;
            let multiplier = 10_i128.checked_pow(power).ok_or_else(|| {
                DruidError::DriverError(format!("DuckDB DECIMAL exponent is too large: {exponent}"))
            })?;
            scaled = scaled.checked_mul(multiplier).ok_or_else(|| {
                DruidError::DriverError(format!("DuckDB DECIMAL value is too large: {value}"))
            })?;
            0
        } else {
            u8::try_from(exponent).map_err(|_| {
                DruidError::DriverError(format!("DuckDB DECIMAL scale is too large: {exponent}"))
            })?
        };
        let digits = scaled
            .unsigned_abs()
            .checked_ilog10()
            .map_or(1, |value| value.saturating_add(1));
        let width = u8::try_from(digits).unwrap_or(u8::MAX).max(scale).max(1);
        DuckDbDecimal::new(width, scale, scaled)
            .map_err(|error| DruidError::DriverError(error.to_string()))
    }

    fn bind_parameters(statement: &mut Statement<'_>, params: &[Value]) -> Result<(), DruidError> {
        for (index, value) in params.iter().enumerate() {
            let index = index + 1;
            let result = match value {
                Value::Null => statement.raw_bind_parameter(index, Null),
                Value::Bool(value) => statement.raw_bind_parameter(index, *value),
                Value::Int(value) => statement.raw_bind_parameter(index, *value),
                Value::Float(value) => statement.raw_bind_parameter(index, *value),
                Value::Decimal(value) => {
                    statement.raw_bind_parameter(index, Self::decimal_parameter(value)?)
                }
                Value::Date(value) => statement.raw_bind_parameter(index, *value),
                Value::Time(value) => statement.raw_bind_parameter(index, *value),
                Value::Timestamp(value) => statement.raw_bind_parameter(index, *value),
                Value::String(value) => statement.raw_bind_parameter(index, value.as_str()),
                Value::Bytes(value) => statement.raw_bind_parameter(index, value.as_slice()),
            };
            result.map_err(Self::driver_error)?;
        }
        Ok(())
    }

    fn checked_i64<T>(value: T, type_name: &str) -> Result<i64, DruidError>
    where
        i64: TryFrom<T>,
        <i64 as TryFrom<T>>::Error: std::fmt::Display,
    {
        i64::try_from(value).map_err(|error| {
            DruidError::DriverError(format!(
                "DuckDB {type_name} value cannot fit JDBC signed BIGINT: {error}"
            ))
        })
    }

    fn row_value(value: DuckDbValueRef<'_>) -> Result<Value, DruidError> {
        match value {
            DuckDbValueRef::Null => Ok(Value::Null),
            DuckDbValueRef::Boolean(value) => Ok(Value::Bool(value)),
            DuckDbValueRef::TinyInt(value) => Ok(Value::Int(i64::from(value))),
            DuckDbValueRef::SmallInt(value) => Ok(Value::Int(i64::from(value))),
            DuckDbValueRef::Int(value) => Ok(Value::Int(i64::from(value))),
            DuckDbValueRef::BigInt(value) => Ok(Value::Int(value)),
            DuckDbValueRef::HugeInt(value) => Self::checked_i64(value, "HUGEINT").map(Value::Int),
            DuckDbValueRef::UHugeInt(value) => Self::checked_i64(value, "UHUGEINT").map(Value::Int),
            DuckDbValueRef::UTinyInt(value) => Ok(Value::Int(i64::from(value))),
            DuckDbValueRef::USmallInt(value) => Ok(Value::Int(i64::from(value))),
            DuckDbValueRef::UInt(value) => Ok(Value::Int(i64::from(value))),
            DuckDbValueRef::UBigInt(value) => Self::checked_i64(value, "UBIGINT").map(Value::Int),
            DuckDbValueRef::Float(value) => Ok(Value::Float(f64::from(value))),
            DuckDbValueRef::Double(value) => Ok(Value::Float(value)),
            DuckDbValueRef::Decimal(value) => Ok(Value::Decimal(BigDecimal::new(
                BigInt::from(value.value()),
                i64::from(value.scale()),
            ))),
            value @ DuckDbValueRef::Timestamp(..) => NaiveDateTime::column_result(value)
                .map(Value::Timestamp)
                .map_err(|error| DruidError::DriverError(error.to_string())),
            DuckDbValueRef::Text(value) => std::str::from_utf8(value)
                .map(|value| Value::String(value.to_owned()))
                .map_err(|error| DruidError::DriverError(error.to_string())),
            DuckDbValueRef::Blob(value) | DuckDbValueRef::Geometry(value) => {
                Ok(Value::Bytes(value.to_vec()))
            }
            value @ DuckDbValueRef::Date32(_) => NaiveDate::column_result(value)
                .map(Value::Date)
                .map_err(|error| DruidError::DriverError(error.to_string())),
            value @ DuckDbValueRef::Time64(..) => NaiveTime::column_result(value)
                .map(Value::Time)
                .map_err(|error| DruidError::DriverError(error.to_string())),
            DuckDbValueRef::Enum(value, index) => DuckDbValueRef::Enum(value, index)
                .as_str()
                .map(|value| Value::String(value.to_owned()))
                .map_err(|error| DruidError::DriverError(error.to_string())),
            DuckDbValueRef::Interval { .. }
            | DuckDbValueRef::List(..)
            | DuckDbValueRef::Struct(..)
            | DuckDbValueRef::Array(..)
            | DuckDbValueRef::Map(..)
            | DuckDbValueRef::Union(..) => Err(DruidError::UnsupportedOperation {
                operation: "duckdb_complex_result_value",
            }),
            _ => Err(DruidError::UnsupportedOperation {
                operation: "duckdb_unknown_result_value",
            }),
        }
    }

    fn query(
        connection: &Connection,
        sql: &str,
        params: &[Value],
    ) -> Result<(Vec<Row>, Vec<String>), DruidError> {
        let mut statement = connection.prepare_cached(sql).map_err(Self::driver_error)?;
        Self::bind_parameters(&mut statement, params)?;
        statement.raw_execute().map_err(Self::driver_error)?;
        Self::collect_executed_rows(&statement)
    }

    fn collect_executed_rows(
        statement: &Statement<'_>,
    ) -> Result<(Vec<Row>, Vec<String>), DruidError> {
        let column_count = statement.column_count();
        let labels = (0..column_count)
            .map(|index| {
                statement
                    .column_name(index)
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|_| format!("column_{}", index + 1))
            })
            .collect();
        let mut result = statement.raw_query();
        let mut rows = Vec::new();
        while let Some(row) = result.next().map_err(Self::driver_error)? {
            let values = (0..column_count)
                .map(|index| {
                    row.get_ref(index)
                        .map_err(Self::driver_error)
                        .and_then(Self::row_value)
                })
                .collect::<Result<Vec<_>, _>>()?;
            rows.push(Row::new(values));
        }
        Ok((rows, labels))
    }

    fn prepared_statement<'statement>(
        &self,
        statement: &'statement dyn PhysicalPreparedStatement,
    ) -> Result<&'statement DuckDbPreparedStatement, DruidError> {
        let statement = statement
            .as_any()
            .downcast_ref::<DuckDbPreparedStatement>()
            .ok_or_else(|| {
                DruidError::DriverError(
                    "prepared statement was not created by DuckDbConnectionAdapter".to_string(),
                )
            })?;
        if statement.is_closed() {
            return Err(DruidError::ConnectionDiscarded);
        }
        if statement.connection_id() != self.connection_id {
            return Err(DruidError::DriverError(
                "DuckDB prepared statement belongs to another physical connection".to_string(),
            ));
        }
        Ok(statement)
    }

    async fn control_statement(&self, sql: &'static str) -> Result<(), DruidError> {
        self.run_blocking(move |connection| {
            connection.execute_batch(sql).map_err(Self::driver_error)
        })
        .await
    }
}

#[async_trait::async_trait]
impl PhysicalConnection for DuckDbConnectionAdapter {
    async fn exec(&mut self, sql: &str, params: Vec<Value>) -> Result<ExecResult, DruidError> {
        let sql = sql.to_owned();
        self.run_blocking(move |connection| {
            let mut statement = connection
                .prepare_cached(&sql)
                .map_err(Self::driver_error)?;
            Self::bind_parameters(&mut statement, &params)?;
            let rows_affected = statement.raw_execute().map_err(Self::driver_error)?;
            Ok(ExecResult {
                rows_affected: u64::try_from(rows_affected).unwrap_or(u64::MAX),
                last_insert_id: None,
                row_count: None,
            })
        })
        .await
    }

    async fn execute(
        &mut self,
        sql: &str,
        params: Vec<Value>,
        generated_keys: StatementGeneratedKeys,
    ) -> Result<Vec<StatementExecuteResult>, DruidError> {
        if !matches!(generated_keys, StatementGeneratedKeys::None) {
            return Err(DruidError::UnsupportedOperation {
                operation: "duckdb_generated_keys",
            });
        }
        let sql = sql.to_owned();
        self.run_blocking(move |connection| {
            let mut statement = connection
                .prepare_cached(&sql)
                .map_err(Self::driver_error)?;
            Self::bind_parameters(&mut statement, &params)?;
            let rows_affected = statement.raw_execute().map_err(Self::driver_error)?;
            if statement.column_count() > 0 {
                Self::collect_executed_rows(&statement)
                    .map(|(rows, _)| vec![StatementExecuteResult::ResultSet(rows)])
            } else {
                Ok(vec![StatementExecuteResult::Update(ExecResult {
                    rows_affected: u64::try_from(rows_affected).unwrap_or(u64::MAX),
                    last_insert_id: None,
                    row_count: None,
                })])
            }
        })
        .await
    }

    async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        let sql = sql.to_owned();
        self.run_blocking(move |connection| {
            Self::query(connection, &sql, &params).map(|value| value.0)
        })
        .await
    }

    async fn fetch_result_set(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let sql = sql.to_owned();
        let (rows, labels) = self
            .run_blocking(move |connection| Self::query(connection, &sql, &params))
            .await?;
        Ok(Arc::new(RowSetResultSet::with_column_labels(rows, labels)))
    }

    async fn prepare_physical_statement(
        &mut self,
        key: &PreparedStatementKey,
    ) -> Result<Arc<dyn PhysicalPreparedStatement>, DruidError> {
        let sql = key.sql().to_owned();
        let prepare_sql = sql.clone();
        self.run_blocking(move |connection| {
            connection
                .prepare_cached(&prepare_sql)
                .map(drop)
                .map_err(Self::driver_error)
        })
        .await?;
        Ok(Arc::new(DuckDbPreparedStatement::new(
            self.connection_id,
            sql,
            Arc::clone(&self.interrupt_handle),
        )))
    }

    async fn exec_prepared(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
    ) -> Result<ExecResult, DruidError> {
        let sql = self.prepared_statement(statement)?.sql().to_owned();
        self.exec(&sql, params).await
    }

    async fn execute_prepared(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
        generated_keys: StatementGeneratedKeys,
    ) -> Result<Vec<StatementExecuteResult>, DruidError> {
        let sql = self.prepared_statement(statement)?.sql().to_owned();
        self.execute(&sql, params, generated_keys).await
    }

    async fn fetch_prepared(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
    ) -> Result<Vec<Row>, DruidError> {
        let sql = self.prepared_statement(statement)?.sql().to_owned();
        self.fetch(&sql, params).await
    }

    async fn begin(&mut self) -> Result<(), DruidError> {
        if !self.auto_commit {
            return Err(DruidError::DriverError(
                "DuckDB transaction is already active".to_string(),
            ));
        }
        self.control_statement("BEGIN TRANSACTION").await?;
        self.auto_commit = false;
        Ok(())
    }

    async fn commit(&mut self) -> Result<(), DruidError> {
        if self.auto_commit {
            return Err(DruidError::DriverError(
                "DuckDB has no active transaction to commit".to_string(),
            ));
        }
        self.control_statement("COMMIT").await?;
        self.auto_commit = true;
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), DruidError> {
        if self.auto_commit {
            return Err(DruidError::DriverError(
                "DuckDB has no active transaction to roll back".to_string(),
            ));
        }
        self.control_statement("ROLLBACK").await?;
        self.auto_commit = true;
        Ok(())
    }

    async fn abort(&mut self) -> Result<(), DruidError> {
        self.interrupt_handle.interrupt();
        self.close().await
    }

    async fn ping(&mut self) -> Result<(), DruidError> {
        self.run_blocking(|connection| {
            connection
                .query_row("SELECT 1", [], |_row| Ok(()))
                .map_err(Self::driver_error)
        })
        .await
    }

    async fn close(&mut self) -> Result<(), DruidError> {
        self.connection.take();
        self.auto_commit = true;
        Ok(())
    }

    fn is_closed(&self) -> bool {
        self.connection.is_none()
    }

    fn capabilities(&self) -> PhysicalConnectionCapabilities {
        PhysicalConnectionCapabilities {
            transactions: true,
            savepoints: false,
            auto_commit: true,
            read_only: false,
            transaction_isolation: false,
            holdability: false,
            clear_warnings: true,
            catalog: false,
            schema: false,
        }
    }

    fn database_meta_data(&mut self) -> Result<Box<dyn PhysicalDatabaseMetaData + '_>, DruidError> {
        Ok(Box::new(DuckDbDatabaseMetaData::new(
            self.connection_ref()?,
            self.url.clone(),
        )))
    }

    fn auto_commit(&self) -> bool {
        self.auto_commit
    }

    async fn set_auto_commit(&mut self, auto_commit: bool) -> Result<(), DruidError> {
        match (self.auto_commit, auto_commit) {
            (true, false) => self.begin().await,
            (false, true) => self.commit().await,
            _ => Ok(()),
        }
    }

    async fn warnings(&mut self) -> Result<Option<SqlWarning>, DruidError> {
        self.connection_ref()?;
        Ok(None)
    }

    async fn clear_warnings(&mut self) -> Result<(), DruidError> {
        self.connection_ref()?;
        Ok(())
    }

    fn mark_discarded(&mut self) {
        self.discarded = true;
    }

    fn is_discarded(&self) -> bool {
        self.discarded
    }

    fn driver_name(&self) -> &str {
        if self.is_closed() {
            "duckdb-rs-closed"
        } else {
            "duckdb-rs"
        }
    }
}
