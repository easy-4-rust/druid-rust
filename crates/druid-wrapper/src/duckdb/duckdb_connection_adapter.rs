//! `DuckDB` 原生 canonical 连接适配器。

use crate::standard_adapter_database_meta_data::StandardAdapterDatabaseMetaData;
use crate::standard_adapter_executor::{
    owned_executor_with_cancel, AdapterExecutionCore, AdapterUpdateResult, StandardExecutor,
};
use crate::standard_adapter_prepared_statement::StandardAdapterPreparedStatement;
use crate::standard_adapter_statement::StandardAdapterStatement;
use crate::standard_query_result::StandardQueryResult;
use crate::task4_runtime_trace::RuntimeTraceEmitter;
use bigdecimal::{num_bigint::BigInt, BigDecimal};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid_rdbc::error::RdbcError;
use druid_rdbc::error::SqlException;
use druid_rdbc::types::{Row, Value};
use duckdb::types::{
    Decimal as DuckDbDecimal, FromSql as DuckDbFromSql, Null, ValueRef as DuckDbValueRef,
};
use duckdb::{Connection, InterruptHandle, Statement};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

static NEXT_CONNECTION_ID: AtomicU64 = AtomicU64::new(1);

/// `DuckDB` 原生 canonical 连接适配器。
///
/// 对应 Java 平台依赖：具体 JDBC driver 的 `java.sql.Connection`。本对象只
/// 持有一个 duckdb-rs `Connection`，不持有外部连接池。所有同步 FFI 调用都在
/// Tokio blocking worker 上执行，避免阻塞 Druid 异步调度线程。
pub struct DuckDbConnectionAdapter {
    connection_id: u64,
    connection: Option<Arc<Mutex<Connection>>>,
    interrupt_handle: Arc<InterruptHandle>,
    operation_active: Arc<AtomicBool>,
    operation_epoch: Arc<AtomicU64>,
    cancellation_requested: Arc<AtomicU64>,
    url: String,
    auto_commit: bool,
    discarded: bool,
    standard_executor: Option<Arc<dyn StandardExecutor>>,
    runtime_trace: Option<RuntimeTraceEmitter>,
}

impl DuckDbConnectionAdapter {
    /// 打开一个 `DuckDB` 原生未池化连接。
    ///
    /// 参数 `url` 使用 `duckdb:` scheme；`duckdb::memory:` 创建独立内存库。
    pub async fn connect(url: &str) -> Result<Self, RdbcError> {
        let target = Self::parse_target(url)?;
        let connection = tokio::task::spawn_blocking(move || match target {
            None => Connection::open_in_memory(),
            Some(path) => Connection::open(path),
        })
        .await
        .map_err(Self::worker_error)?
        .map_err(Self::driver_error)?;
        Ok(Self::from_native_connection(connection, url))
    }

    /// 从同一数据库的现有连接创建独立物理连接；`existing` 为共享数据库句柄，
    /// `url` 保留原连接地址，返回可交给池管理的新连接。
    pub(crate) async fn connect_from_existing(
        existing: Arc<Mutex<Connection>>,
        url: &str,
    ) -> Result<Self, RdbcError> {
        let connection = tokio::task::spawn_blocking(move || existing.lock().try_clone())
            .await
            .map_err(Self::worker_error)?
            .map_err(Self::driver_error)?;
        Ok(Self::from_native_connection(connection, url))
    }

    fn from_native_connection(connection: Connection, url: &str) -> Self {
        let interrupt_handle = connection.interrupt_handle();
        Self {
            connection_id: NEXT_CONNECTION_ID.fetch_add(1, Ordering::Relaxed),
            connection: Some(Arc::new(Mutex::new(connection))),
            interrupt_handle,
            operation_active: Arc::new(AtomicBool::new(false)),
            operation_epoch: Arc::new(AtomicU64::new(0)),
            cancellation_requested: Arc::new(AtomicU64::new(0)),
            url: url.to_owned(),
            auto_commit: true,
            discarded: false,
            standard_executor: None,
            runtime_trace: None,
        }
    }

    pub(crate) fn install_runtime_trace(&mut self, runtime_trace: RuntimeTraceEmitter) {
        self.runtime_trace = Some(runtime_trace);
    }

    fn standard_executor_core(&mut self) -> Result<Arc<dyn StandardExecutor>, RdbcError> {
        if let Some(executor) = &self.standard_executor {
            return Ok(Arc::clone(executor));
        }
        self.connection_ref()?;
        let runtime_trace = self.runtime_trace.take();
        let shell = Self {
            connection_id: self.connection_id,
            connection: None,
            interrupt_handle: Arc::clone(&self.interrupt_handle),
            operation_active: Arc::clone(&self.operation_active),
            operation_epoch: Arc::clone(&self.operation_epoch),
            cancellation_requested: Arc::clone(&self.cancellation_requested),
            url: self.url.clone(),
            auto_commit: self.auto_commit,
            discarded: self.discarded,
            standard_executor: None,
            runtime_trace: None,
        };
        let owned = std::mem::replace(self, shell);
        let interrupt_handle = Arc::clone(&owned.interrupt_handle);
        let operation_active = Arc::clone(&owned.operation_active);
        let operation_epoch = Arc::clone(&owned.operation_epoch);
        let cancellation_requested = Arc::clone(&owned.cancellation_requested);
        let executor = owned_executor_with_cancel(
            owned,
            runtime_trace,
            Arc::new(move || {
                if operation_active.load(Ordering::Acquire) {
                    let cancelled_epoch = operation_epoch.load(Ordering::Acquire);
                    cancellation_requested.store(cancelled_epoch, Ordering::Release);
                    let interrupt_handle = Arc::clone(&interrupt_handle);
                    let operation_active = Arc::clone(&operation_active);
                    let operation_epoch = Arc::clone(&operation_epoch);
                    let _ = std::thread::spawn(move || {
                        // 原生查询可能尚未越过 prepare 边界；在同一执行代内重复通知。
                        for _ in 0..100 {
                            if !operation_active.load(Ordering::Acquire)
                                || operation_epoch.load(Ordering::Acquire) != cancelled_epoch
                            {
                                break;
                            }
                            interrupt_handle.interrupt();
                            std::thread::sleep(std::time::Duration::from_millis(10));
                        }
                    });
                }
            }),
        );
        self.standard_executor = Some(Arc::clone(&executor));
        Ok(executor)
    }

    async fn execute_update_core(
        &self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<AdapterUpdateResult, RdbcError> {
        let sql = sql.to_owned();
        self.run_blocking(move |connection| {
            let mut statement = connection
                .prepare_cached(&sql)
                .map_err(Self::driver_error)?;
            Self::bind_parameters(&mut statement, &params)?;
            let rows_affected = statement.raw_execute().map_err(Self::driver_error)?;
            Ok(AdapterUpdateResult {
                rows_affected: u64::try_from(rows_affected).unwrap_or(u64::MAX),
                last_insert_id: None,
                row_count: None,
            })
        })
        .await
    }

    #[allow(
        clippy::unused_async,
        reason = "uniform async close core across adapter families"
    )]
    async fn close_connection_core(&mut self) -> Result<(), RdbcError> {
        self.connection.take();
        self.auto_commit = true;
        Ok(())
    }

    fn parse_target(url: &str) -> Result<Option<PathBuf>, RdbcError> {
        if url == "duckdb::memory:" || url == "duckdb:///:memory:" {
            return Ok(None);
        }
        let value = url.strip_prefix("duckdb:").ok_or_else(|| {
            RdbcError::InvalidArgument("DuckDB URL must start with `duckdb:`".to_string())
        })?;
        let value = value.strip_prefix("//").unwrap_or(value);
        if value.is_empty()
            || value.contains('\0')
            || value.contains('?')
            || value.contains('#')
            || value.contains('%')
        {
            return Err(RdbcError::InvalidArgument(
                "DuckDB file URL must contain a non-empty literal path without query, fragment, NUL or percent encoding"
                    .to_string(),
            ));
        }
        let path = PathBuf::from(value);
        if path.is_absolute() {
            Ok(Some(path))
        } else {
            std::env::current_dir()
                .map(|current_dir| Some(current_dir.join(path)))
                .map_err(|error| {
                    RdbcError::DriverError(format!(
                        "DuckDB cannot resolve relative database path `{value}`: {error}"
                    ))
                })
        }
    }

    /// 返回当前物理连接的共享引用；连接已丢弃时返回错误。
    pub(crate) fn connection_ref(&self) -> Result<Arc<Mutex<Connection>>, RdbcError> {
        if self.discarded {
            return Err(RdbcError::ConnectionDiscarded);
        }
        self.connection
            .as_ref()
            .map(Arc::clone)
            .ok_or(RdbcError::ConnectionDiscarded)
    }

    async fn run_blocking<T, F>(&self, operation: F) -> Result<T, RdbcError>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> Result<T, RdbcError> + Send + 'static,
    {
        let connection = self.connection_ref()?;
        let epoch = self.operation_epoch.fetch_add(1, Ordering::AcqRel) + 1;
        self.operation_active.store(true, Ordering::Release);
        let active = Arc::clone(&self.operation_active);
        let current_epoch = Arc::clone(&self.operation_epoch);
        let cancellation = Arc::clone(&self.cancellation_requested);
        tokio::task::spawn_blocking(move || {
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if cancellation
                    .compare_exchange(epoch, 0, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    return Err(Self::cancellation_error());
                }
                let connection = connection.lock();
                if cancellation
                    .compare_exchange(epoch, 0, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    return Err(Self::cancellation_error());
                }
                operation(&connection)
            }));
            if current_epoch.load(Ordering::Acquire) == epoch {
                active.store(false, Ordering::Release);
            }
            let _ = cancellation.compare_exchange(epoch, 0, Ordering::AcqRel, Ordering::Acquire);
            match outcome {
                Ok(result) => result,
                Err(payload) => std::panic::resume_unwind(payload),
            }
        })
        .await
        .map_err(Self::worker_error)?
    }

    fn cancellation_error() -> RdbcError {
        RdbcError::SqlException(Box::new(
            SqlException::new(
                0,
                Some("HY008".to_owned()),
                Some("DuckDB operation interrupted".to_owned()),
            )
            .with_class_name("duckdb::Error::OperationInterrupted"),
        ))
    }

    fn worker_error(error: tokio::task::JoinError) -> RdbcError {
        RdbcError::DriverError(format!("DuckDB blocking worker failed: {error}"))
    }

    pub(super) fn driver_error(error: duckdb::Error) -> RdbcError {
        match error {
            duckdb::Error::DuckDBFailure(error, message) => {
                let error_code = i32::try_from(error.extended_code).unwrap_or_default();
                let message = message.unwrap_or_else(|| error.to_string());
                let interrupted = message.to_ascii_lowercase().contains("interrupt");
                RdbcError::SqlException(Box::new(
                    SqlException::new(
                        error_code,
                        Some(if interrupted { "HY008" } else { "HY000" }.to_owned()),
                        Some(message),
                    )
                    .with_class_name(if interrupted {
                        "duckdb::Error::OperationInterrupted"
                    } else {
                        "duckdb::Error::DuckDBFailure"
                    }),
                ))
            }
            error => RdbcError::SqlException(Box::new(
                SqlException::driver(0, error.to_string())
                    .with_sql_state("HY000")
                    .with_class_name("duckdb::Error"),
            )),
        }
    }

    fn decimal_parameter(value: &BigDecimal) -> Result<DuckDbDecimal, RdbcError> {
        let (coefficient, exponent) = value.as_bigint_and_exponent();
        let mut scaled = i128::try_from(coefficient).map_err(|_| {
            RdbcError::DriverError(format!(
                "DuckDB DECIMAL cannot represent coefficient of `{value}`"
            ))
        })?;
        let scale = if exponent < 0 {
            let power = u32::try_from(-exponent).map_err(|error| {
                RdbcError::DriverError(format!("invalid decimal exponent: {error}"))
            })?;
            let multiplier = 10_i128.checked_pow(power).ok_or_else(|| {
                RdbcError::DriverError(format!("DuckDB DECIMAL exponent is too large: {exponent}"))
            })?;
            scaled = scaled.checked_mul(multiplier).ok_or_else(|| {
                RdbcError::DriverError(format!("DuckDB DECIMAL value is too large: {value}"))
            })?;
            0
        } else {
            u8::try_from(exponent).map_err(|_| {
                RdbcError::DriverError(format!("DuckDB DECIMAL scale is too large: {exponent}"))
            })?
        };
        let digits = scaled
            .unsigned_abs()
            .checked_ilog10()
            .map_or(1, |value| value.saturating_add(1));
        let width = u8::try_from(digits).unwrap_or(u8::MAX).max(scale).max(1);
        DuckDbDecimal::new(width, scale, scaled)
            .map_err(|error| RdbcError::DriverError(error.to_string()))
    }

    fn bind_parameters(statement: &mut Statement<'_>, params: &[Value]) -> Result<(), RdbcError> {
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

    fn checked_i64<T>(value: T, type_name: &str) -> Result<i64, RdbcError>
    where
        i64: TryFrom<T>,
        <i64 as TryFrom<T>>::Error: std::fmt::Display,
    {
        i64::try_from(value).map_err(|error| {
            RdbcError::DriverError(format!(
                "DuckDB {type_name} value cannot fit JDBC signed BIGINT: {error}"
            ))
        })
    }

    fn row_value(value: DuckDbValueRef<'_>) -> Result<Value, RdbcError> {
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
                .map_err(|error| RdbcError::DriverError(error.to_string())),
            DuckDbValueRef::Text(value) => std::str::from_utf8(value)
                .map(|value| Value::String(value.to_owned()))
                .map_err(|error| RdbcError::DriverError(error.to_string())),
            DuckDbValueRef::Blob(value) | DuckDbValueRef::Geometry(value) => {
                Ok(Value::Bytes(value.to_vec()))
            }
            value @ DuckDbValueRef::Date32(_) => NaiveDate::column_result(value)
                .map(Value::Date)
                .map_err(|error| RdbcError::DriverError(error.to_string())),
            value @ DuckDbValueRef::Time64(..) => NaiveTime::column_result(value)
                .map(Value::Time)
                .map_err(|error| RdbcError::DriverError(error.to_string())),
            DuckDbValueRef::Enum(value, index) => DuckDbValueRef::Enum(value, index)
                .as_str()
                .map(|value| Value::String(value.to_owned()))
                .map_err(|error| RdbcError::DriverError(error.to_string())),
            DuckDbValueRef::Interval { .. }
            | DuckDbValueRef::List(..)
            | DuckDbValueRef::Struct(..)
            | DuckDbValueRef::Array(..)
            | DuckDbValueRef::Map(..)
            | DuckDbValueRef::Union(..) => Err(RdbcError::UnsupportedOperation {
                operation: "duckdb_complex_result_value",
            }),
            _ => Err(RdbcError::UnsupportedOperation {
                operation: "duckdb_unknown_result_value",
            }),
        }
    }

    fn query(
        connection: &Connection,
        sql: &str,
        params: &[Value],
    ) -> Result<(Vec<Row>, Vec<String>), RdbcError> {
        let mut statement = connection.prepare_cached(sql).map_err(Self::driver_error)?;
        Self::bind_parameters(&mut statement, params)?;
        statement.raw_execute().map_err(Self::driver_error)?;
        Self::collect_executed_rows(&statement)
    }

    fn collect_executed_rows(
        statement: &Statement<'_>,
    ) -> Result<(Vec<Row>, Vec<String>), RdbcError> {
        let column_count = statement.column_count();
        let labels = (0..column_count)
            .map(|index| {
                statement
                    .column_name(index)
                    .map_or_else(|_| format!("column_{}", index + 1), ToOwned::to_owned)
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
}

#[async_trait::async_trait]
impl AdapterExecutionCore for DuckDbConnectionAdapter {
    async fn standard_execute_update_core(
        &mut self,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<i64, RdbcError> {
        let result = self.execute_update_core(sql, values).await?;
        i64::try_from(result.rows_affected).map_err(|_| {
            RdbcError::InvalidArgument("rows affected exceeds canonical i64".to_owned())
        })
    }

    async fn standard_execute_query_core(
        &mut self,
        sql: &str,
        values: Vec<Value>,
    ) -> Result<StandardQueryResult, RdbcError> {
        let sql = sql.to_owned();
        self.run_blocking(move |connection| {
            Self::query(connection, &sql, &values)
                .map(|(rows, labels)| StandardQueryResult::with_labels(rows, labels))
        })
        .await
    }

    async fn standard_close_connection_core(&mut self) -> Result<(), RdbcError> {
        self.close_connection_core().await
    }

    async fn standard_parameter_count_core(
        &mut self,
        sql: &str,
    ) -> Result<Option<usize>, RdbcError> {
        let sql = sql.to_owned();
        self.run_blocking(move |connection| {
            connection
                .prepare_cached(&sql)
                .map(|statement| Some(statement.parameter_count()))
                .map_err(Self::driver_error)
        })
        .await
    }
}

druid_rdbc::impl_standard_wrapper!(DuckDbConnectionAdapter => Connection);

#[async_trait::async_trait]
impl druid_rdbc::connection::Connection for DuckDbConnectionAdapter {
    async fn exec(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<druid_rdbc::statement::ExecResult, RdbcError> {
        let result = self
            .standard_executor_core()?
            .execute_update_result(sql, params)
            .await?;
        Ok(druid_rdbc::statement::ExecResult {
            rows_affected: result.rows_affected,
            last_insert_id: result.last_insert_id,
            row_count: result.row_count,
        })
    }

    async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, RdbcError> {
        self.standard_executor_core()?
            .execute_query(sql, params)
            .await
            .map(|result| result.rows)
    }

    async fn is_valid(&mut self, timeout_seconds: u32) -> Result<bool, RdbcError> {
        let executor = self.standard_executor_core()?;
        let query = executor.execute_query("SELECT 1", Vec::new());
        if timeout_seconds == 0 {
            query.await?;
        } else {
            tokio::time::timeout(
                std::time::Duration::from_secs(u64::from(timeout_seconds)),
                query,
            )
            .await
            .map_err(|_| {
                RdbcError::DriverError("DuckDB connection validation timed out".to_owned())
            })??;
        }
        Ok(true)
    }

    async fn create_statement(
        &mut self,
        options: druid_rdbc::statement::StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::Statement>, RdbcError> {
        Ok(Box::new(StandardAdapterStatement::new(
            self.standard_executor_core()?,
            options,
        )))
    }

    async fn prepare_statement(
        &mut self,
        sql: &str,
        options: druid_rdbc::statement::StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::PreparedStatement>, RdbcError> {
        let executor = self.standard_executor_core()?;
        let parameter_count = executor.parameter_count(sql).await?;
        Ok(Box::new(StandardAdapterPreparedStatement::new(
            sql.to_owned(),
            executor,
            parameter_count,
            options,
        )))
    }

    async fn database_meta_data(
        &mut self,
    ) -> Result<Box<dyn druid_rdbc::metadata::DatabaseMetaData>, RdbcError> {
        Ok(Box::new(StandardAdapterDatabaseMetaData::new(
            self.standard_executor_core()?,
            Some("DuckDB".to_owned()),
            "duckdb_database_metadata_product_name",
        )))
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        if let Some(executor) = &self.standard_executor {
            executor.close().await
        } else {
            self.close_connection_core().await
        }
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.standard_executor.as_ref().map_or_else(
            || self.connection.is_none(),
            |executor| executor.is_closed(),
        ))
    }
}
