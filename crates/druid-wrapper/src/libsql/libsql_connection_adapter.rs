use crate::standard_adapter_database_meta_data::StandardAdapterDatabaseMetaData;
use crate::standard_adapter_executor::{
    owned_executor, AdapterExecutionCore, AdapterUpdateResult, StandardExecutor,
};
use crate::standard_adapter_prepared_statement::StandardAdapterPreparedStatement;
use crate::standard_adapter_statement::StandardAdapterStatement;
use crate::standard_query_result::StandardQueryResult;
use crate::task4_runtime_trace::RuntimeTraceEmitter;
use druid_rdbc::error::RdbcError;
use druid_rdbc::error::SqlException;
use druid_rdbc::types::{Row, Value};
use std::sync::Arc;

pub(super) struct LibSqlResult {
    pub(super) rows: Vec<Row>,
    pub(super) labels: Vec<String>,
    pub(super) rows_affected: u64,
    pub(super) last_insert_id: Option<i64>,
}

#[async_trait::async_trait]
pub(super) trait LibSqlTransport: Send + Sync {
    async fn request(&self, sql: &str, params: Vec<Value>) -> Result<LibSqlResult, RdbcError>;
}

struct NativeLibSqlTransport {
    connection: ::libsql::Connection,
}

#[async_trait::async_trait]
impl LibSqlTransport for NativeLibSqlTransport {
    async fn request(&self, sql: &str, params: Vec<Value>) -> Result<LibSqlResult, RdbcError> {
        let before_changes = self.connection.total_changes();
        let mut rows = self
            .connection
            .query(
                sql,
                params
                    .into_iter()
                    .map(LibSqlConnectionAdapter::parameter)
                    .collect::<Vec<_>>(),
            )
            .await
            .map_err(LibSqlConnectionAdapter::driver_error)?;
        let column_count = usize::try_from(rows.column_count()).unwrap_or_default();
        let labels = (0..column_count)
            .map(|index| {
                rows.column_name(i32::try_from(index).unwrap_or(i32::MAX))
                    .unwrap_or("")
                    .to_owned()
            })
            .collect::<Vec<_>>();
        let mut materialized = Vec::new();
        loop {
            let Some(row) = rows
                .next()
                .await
                .map_err(LibSqlConnectionAdapter::driver_error)?
            else {
                break;
            };
            let values = (0..column_count)
                .map(|index| {
                    row.get_value(i32::try_from(index).unwrap_or(i32::MAX))
                        .map(LibSqlConnectionAdapter::value)
                        .map_err(LibSqlConnectionAdapter::driver_error)
                })
                .collect::<Result<Vec<_>, _>>()?;
            materialized.push(Row::new(values));
        }
        Ok(LibSqlResult {
            rows: materialized,
            labels,
            rows_affected: self
                .connection
                .total_changes()
                .saturating_sub(before_changes),
            last_insert_id: Some(self.connection.last_insert_rowid()),
        })
    }
}

/// 单个 Druid holder 独占的 Turso/libSQL 物理连接。
///
/// 对象直接持有一个 libSQL `Connection`，不创建或持有第三方连接池。
pub struct LibSqlConnectionAdapter {
    transport: Option<Arc<dyn LibSqlTransport>>,
    url: String,
    auto_commit: bool,
    discarded: bool,
    savepoint_sequence: u64,
    standard_executor: Option<Arc<dyn StandardExecutor>>,
    runtime_trace: Option<RuntimeTraceEmitter>,
}

impl LibSqlConnectionAdapter {
    /// 连接远程 Turso/libSQL 数据库。
    pub async fn connect(url: &str, token: String) -> Result<Self, RdbcError> {
        if !url.starts_with("libsql://") && !url.starts_with("https://") {
            return Err(RdbcError::InvalidArgument(
                "Turso/libSQL URL must start with libsql:// or https://".to_owned(),
            ));
        }
        let database = ::libsql::Builder::new_remote(url.to_owned(), token)
            .build()
            .await
            .map_err(Self::driver_error)?;
        let connection = database.connect().map_err(Self::driver_error)?;
        Ok(Self {
            transport: Some(Arc::new(NativeLibSqlTransport { connection })),
            url: url.to_owned(),
            auto_commit: true,
            discarded: false,
            savepoint_sequence: 0,
            standard_executor: None,
            runtime_trace: None,
        })
    }

    pub(super) fn from_transport(
        url: impl Into<String>,
        transport: Arc<dyn LibSqlTransport>,
    ) -> Self {
        Self {
            transport: Some(transport),
            url: url.into(),
            auto_commit: true,
            discarded: false,
            savepoint_sequence: 0,
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
        self.transport()?;
        let runtime_trace = self.runtime_trace.take();
        let shell = Self {
            transport: None,
            url: self.url.clone(),
            auto_commit: self.auto_commit,
            discarded: self.discarded,
            savepoint_sequence: self.savepoint_sequence,
            standard_executor: None,
            runtime_trace: None,
        };
        let owned = std::mem::replace(self, shell);
        let executor = owned_executor(owned, runtime_trace);
        self.standard_executor = Some(Arc::clone(&executor));
        Ok(executor)
    }

    async fn execute_update_core(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<AdapterUpdateResult, RdbcError> {
        let result = self.request(sql, params).await?;
        Ok(AdapterUpdateResult {
            rows_affected: result.rows_affected,
            last_insert_id: result.last_insert_id,
            row_count: None,
        })
    }

    #[allow(
        clippy::unused_async,
        reason = "uniform async close core across adapter families"
    )]
    async fn close_connection_core(&mut self) -> Result<(), RdbcError> {
        self.transport.take();
        Ok(())
    }

    fn transport(&self) -> Result<&Arc<dyn LibSqlTransport>, RdbcError> {
        if self.discarded {
            return Err(RdbcError::ConnectionDiscarded);
        }
        self.transport
            .as_ref()
            .ok_or(RdbcError::ConnectionDiscarded)
    }

    fn driver_error(error: ::libsql::Error) -> RdbcError {
        let message = error.to_string();
        let lower = message.to_ascii_lowercase();
        let (state, fatal) = if lower.contains("unauthorized") || lower.contains("authentication") {
            ("28000", false)
        } else if lower.contains("timeout") || lower.contains("timed out") {
            ("HYT00", true)
        } else if lower.contains("connection")
            || lower.contains("network")
            || lower.contains("protocol")
        {
            ("08006", true)
        } else if lower.contains("constraint") {
            ("23000", false)
        } else if lower.contains("syntax") {
            ("42000", false)
        } else {
            ("HY000", false)
        };
        let exception = SqlException::new(0, Some(state.to_owned()), Some(message))
            .with_class_name("libsql::Error");
        if fatal {
            RdbcError::SqlException(Box::new(exception.recoverable()))
        } else {
            RdbcError::SqlException(Box::new(exception))
        }
    }

    fn parameter(value: Value) -> ::libsql::Value {
        match value {
            Value::Null => ::libsql::Value::Null,
            Value::Bool(value) => ::libsql::Value::Integer(i64::from(value)),
            Value::Int(value) => ::libsql::Value::Integer(value),
            Value::Float(value) => ::libsql::Value::Real(value),
            Value::Decimal(value) => ::libsql::Value::Text(value.to_string()),
            Value::Date(value) => ::libsql::Value::Text(value.to_string()),
            Value::Time(value) => ::libsql::Value::Text(value.to_string()),
            Value::Timestamp(value) => ::libsql::Value::Text(value.to_string()),
            Value::String(value) => ::libsql::Value::Text(value),
            Value::Bytes(value) => ::libsql::Value::Blob(value),
        }
    }

    fn value(value: ::libsql::Value) -> Value {
        match value {
            ::libsql::Value::Null => Value::Null,
            ::libsql::Value::Integer(value) => Value::Int(value),
            ::libsql::Value::Real(value) => Value::Float(value),
            ::libsql::Value::Text(value) => Value::String(value),
            ::libsql::Value::Blob(value) => Value::Bytes(value),
        }
    }

    async fn request(&mut self, sql: &str, params: Vec<Value>) -> Result<LibSqlResult, RdbcError> {
        let transport = Arc::clone(self.transport()?);
        match transport.request(sql, params).await {
            Ok(result) => Ok(result),
            Err(error) => Err(self.record_driver_error(error)),
        }
    }

    fn record_driver_error(&mut self, error: RdbcError) -> RdbcError {
        if matches!(&error, RdbcError::SqlException(exception)
            if exception.is_recoverable()
                || exception.sql_state().is_some_and(|state| state.starts_with("08")))
        {
            self.discarded = true;
            self.transport.take();
        }
        error
    }
}

#[async_trait::async_trait]
impl AdapterExecutionCore for LibSqlConnectionAdapter {
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
        self.request(sql, values)
            .await
            .map(|result| StandardQueryResult::with_labels(result.rows, result.labels))
    }

    async fn standard_close_connection_core(&mut self) -> Result<(), RdbcError> {
        self.close_connection_core().await
    }
}

druid_rdbc::impl_standard_wrapper!(LibSqlConnectionAdapter => Connection);

#[async_trait::async_trait]
impl druid_rdbc::connection::Connection for LibSqlConnectionAdapter {
    async fn create_statement(
        &mut self,
        _options: druid_rdbc::statement::StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::Statement>, RdbcError> {
        Ok(Box::new(StandardAdapterStatement::new(
            self.standard_executor_core()?,
        )))
    }

    async fn prepare_statement(
        &mut self,
        sql: &str,
        _options: druid_rdbc::statement::StatementOptions,
    ) -> Result<Box<dyn druid_rdbc::statement::PreparedStatement>, RdbcError> {
        let executor = self.standard_executor_core()?;
        let parameter_count = executor.parameter_count(sql).await?;
        Ok(Box::new(StandardAdapterPreparedStatement::new(
            sql.to_owned(),
            executor,
            parameter_count,
        )))
    }

    async fn database_meta_data(
        &mut self,
    ) -> Result<Box<dyn druid_rdbc::metadata::DatabaseMetaData>, RdbcError> {
        Ok(Box::new(StandardAdapterDatabaseMetaData::new(
            self.standard_executor_core()?,
            Some("Turso/libSQL".to_owned()),
            "libsql_database_metadata_product_name",
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
        Ok(self
            .standard_executor
            .as_ref()
            .map_or_else(|| self.transport.is_none(), |executor| executor.is_closed()))
    }

    async fn is_valid(&mut self, _timeout_seconds: u32) -> Result<bool, RdbcError> {
        self.request("SELECT 1", Vec::new()).await.map(|_| true)
    }
}
