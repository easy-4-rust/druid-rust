use super::{
    libsql_prepared_statement::LibSqlStatementExecutionError, LibSqlDatabaseMetaData,
    LibSqlPreparedStatement,
};
use druid_core::core::{
    DruidError, ExecResult, PhysicalConnection, PhysicalConnectionCapabilities,
    PhysicalDatabaseMetaData, PhysicalPreparedStatement, PhysicalResultSet, PreparedStatementKey,
    Row, RowSetResultSet, Savepoint, SqlException, SqlWarning, StatementExecuteResult,
    StatementGeneratedKeys, Value,
};
use std::sync::Arc;

struct LibSqlResult {
    rows: Vec<Row>,
    labels: Vec<String>,
    rows_affected: u64,
    last_insert_id: Option<i64>,
}

/// 单个 Druid holder 独占的 Turso/libSQL 物理连接。
///
/// 对象直接持有一个 libSQL `Connection`，不创建或持有第三方连接池。
pub struct LibSqlConnectionAdapter {
    connection: Option<::libsql::Connection>,
    url: String,
    auto_commit: bool,
    discarded: bool,
    savepoint_sequence: u64,
}

impl LibSqlConnectionAdapter {
    /// 连接远程 Turso/libSQL 数据库。
    pub async fn connect(url: &str, token: String) -> Result<Self, DruidError> {
        if !url.starts_with("libsql://") && !url.starts_with("https://") {
            return Err(DruidError::InvalidArgument(
                "Turso/libSQL URL must start with libsql:// or https://".to_owned(),
            ));
        }
        let database = ::libsql::Builder::new_remote(url.to_owned(), token)
            .build()
            .await
            .map_err(Self::driver_error)?;
        let connection = database.connect().map_err(Self::driver_error)?;
        Ok(Self {
            connection: Some(connection),
            url: url.to_owned(),
            auto_commit: true,
            discarded: false,
            savepoint_sequence: 0,
        })
    }

    fn connection(&self) -> Result<&::libsql::Connection, DruidError> {
        if self.discarded {
            return Err(DruidError::ConnectionDiscarded);
        }
        self.connection
            .as_ref()
            .ok_or(DruidError::ConnectionDiscarded)
    }

    fn driver_error(error: ::libsql::Error) -> DruidError {
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
            DruidError::SqlException(Box::new(exception.recoverable()))
        } else {
            DruidError::SqlException(Box::new(exception))
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

    async fn request(&mut self, sql: &str, params: Vec<Value>) -> Result<LibSqlResult, DruidError> {
        let connection = self.connection()?.clone();
        let before_changes = connection.total_changes();
        let mut rows = match connection
            .query(
                sql,
                params.into_iter().map(Self::parameter).collect::<Vec<_>>(),
            )
            .await
        {
            Ok(rows) => rows,
            Err(error) => return Err(self.record_driver_error(error)),
        };
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
            let row = match rows.next().await {
                Ok(Some(row)) => row,
                Ok(None) => break,
                Err(error) => return Err(self.record_driver_error(error)),
            };
            let values = (0..column_count)
                .map(|index| {
                    row.get_value(i32::try_from(index).unwrap_or(i32::MAX))
                        .map(Self::value)
                        .map_err(Self::driver_error)
                })
                .collect::<Result<Vec<_>, _>>()?;
            materialized.push(Row::new(values));
        }
        Ok(LibSqlResult {
            rows: materialized,
            labels,
            rows_affected: connection.total_changes().saturating_sub(before_changes),
            last_insert_id: Some(connection.last_insert_rowid()),
        })
    }

    fn record_driver_error(&mut self, error: ::libsql::Error) -> DruidError {
        let error = Self::driver_error(error);
        if matches!(&error, DruidError::SqlException(exception)
            if exception.is_recoverable()
                || exception.sql_state().is_some_and(|state| state.starts_with("08")))
        {
            self.discarded = true;
            self.connection.take();
        }
        error
    }

    fn prepared(
        statement: &dyn PhysicalPreparedStatement,
    ) -> Result<&LibSqlPreparedStatement, DruidError> {
        statement
            .as_any()
            .downcast_ref::<LibSqlPreparedStatement>()
            .ok_or_else(|| {
                DruidError::DriverError("prepared statement belongs to another driver".to_owned())
            })
    }

    fn controlled<T>(
        &mut self,
        result: Result<T, LibSqlStatementExecutionError>,
    ) -> Result<T, DruidError> {
        match result {
            Ok(value) => Ok(value),
            Err(LibSqlStatementExecutionError::Driver(error)) => Err(error),
            Err(LibSqlStatementExecutionError::TimedOut) => {
                self.discarded = true;
                self.connection.take();
                Err(DruidError::SqlException(Box::new(
                    SqlException::new(
                        0,
                        Some("HYT00".to_owned()),
                        Some("libSQL query timed out".to_owned()),
                    )
                    .with_class_name("java.sql.SQLTimeoutException")
                    .recoverable(),
                )))
            }
            Err(LibSqlStatementExecutionError::Cancelled) => {
                self.discarded = true;
                self.connection.take();
                Err(DruidError::SqlException(Box::new(
                    SqlException::new(
                        0,
                        Some("HY008".to_owned()),
                        Some("libSQL query was cancelled".to_owned()),
                    )
                    .with_class_name("java.sql.SQLException")
                    .recoverable(),
                )))
            }
        }
    }

    fn savepoint_name(savepoint: &Savepoint) -> Result<String, DruidError> {
        let name = savepoint
            .name
            .clone()
            .unwrap_or_else(|| format!("druid_sp_{}", savepoint.id));
        if name.is_empty()
            || !name
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '_')
        {
            return Err(DruidError::InvalidArgument(
                "invalid libSQL savepoint name".to_owned(),
            ));
        }
        Ok(name)
    }
}

#[async_trait::async_trait]
impl PhysicalConnection for LibSqlConnectionAdapter {
    async fn exec(&mut self, sql: &str, params: Vec<Value>) -> Result<ExecResult, DruidError> {
        let result = self.request(sql, params).await?;
        Ok(ExecResult {
            rows_affected: result.rows_affected,
            last_insert_id: result.last_insert_id,
            row_count: None,
        })
    }

    async fn execute(
        &mut self,
        sql: &str,
        params: Vec<Value>,
        _keys: StatementGeneratedKeys,
    ) -> Result<Vec<StatementExecuteResult>, DruidError> {
        let result = self.request(sql, params).await?;
        if result.labels.is_empty() {
            Ok(vec![StatementExecuteResult::Update(ExecResult {
                rows_affected: result.rows_affected,
                last_insert_id: result.last_insert_id,
                row_count: None,
            })])
        } else {
            Ok(vec![StatementExecuteResult::ResultSet(result.rows)])
        }
    }

    async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, DruidError> {
        self.request(sql, params).await.map(|result| result.rows)
    }

    async fn fetch_result_set(
        &mut self,
        sql: &str,
        params: Vec<Value>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let result = self.request(sql, params).await?;
        Ok(Arc::new(RowSetResultSet::with_column_labels(
            result.rows,
            result.labels,
        )))
    }

    async fn prepare_physical_statement(
        &mut self,
        key: &PreparedStatementKey,
    ) -> Result<Arc<dyn PhysicalPreparedStatement>, DruidError> {
        self.connection()?
            .prepare(key.sql())
            .await
            .map_err(Self::driver_error)?;
        Ok(Arc::new(LibSqlPreparedStatement::new(key.sql())))
    }

    async fn exec_prepared(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
    ) -> Result<ExecResult, DruidError> {
        let statement = Self::prepared(statement)?;
        let result = statement
            .execute_with_controls(self.request(statement.sql(), params))
            .await;
        let result = self.controlled(result)?;
        Ok(ExecResult {
            rows_affected: result.rows_affected,
            last_insert_id: result.last_insert_id,
            row_count: None,
        })
    }

    async fn fetch_prepared(
        &mut self,
        statement: &dyn PhysicalPreparedStatement,
        params: Vec<Value>,
    ) -> Result<Vec<Row>, DruidError> {
        let statement = Self::prepared(statement)?;
        let result = statement
            .execute_with_controls(self.request(statement.sql(), params))
            .await;
        self.controlled(result).map(|result| result.rows)
    }

    async fn begin(&mut self) -> Result<(), DruidError> {
        if !self.auto_commit {
            return Err(DruidError::DriverError(
                "libSQL transaction is already active".to_owned(),
            ));
        }
        self.exec("BEGIN", Vec::new()).await?;
        self.auto_commit = false;
        Ok(())
    }
    async fn commit(&mut self) -> Result<(), DruidError> {
        self.exec("COMMIT", Vec::new()).await?;
        self.auto_commit = true;
        Ok(())
    }
    async fn rollback(&mut self) -> Result<(), DruidError> {
        self.exec("ROLLBACK", Vec::new()).await?;
        self.auto_commit = true;
        Ok(())
    }

    async fn rollback_to(&mut self, savepoint: &Savepoint) -> Result<(), DruidError> {
        self.exec(
            &format!("ROLLBACK TO SAVEPOINT {}", Self::savepoint_name(savepoint)?),
            Vec::new(),
        )
        .await
        .map(|_| ())
    }

    async fn set_savepoint(&mut self) -> Result<Savepoint, DruidError> {
        self.savepoint_sequence = self.savepoint_sequence.saturating_add(1);
        let savepoint = Savepoint {
            id: self.savepoint_sequence,
            name: None,
        };
        self.exec(
            &format!("SAVEPOINT {}", Self::savepoint_name(&savepoint)?),
            Vec::new(),
        )
        .await?;
        Ok(savepoint)
    }

    async fn set_savepoint_named(&mut self, name: &str) -> Result<Savepoint, DruidError> {
        self.savepoint_sequence = self.savepoint_sequence.saturating_add(1);
        let savepoint = Savepoint {
            id: self.savepoint_sequence,
            name: Some(name.to_owned()),
        };
        self.exec(
            &format!("SAVEPOINT {}", Self::savepoint_name(&savepoint)?),
            Vec::new(),
        )
        .await?;
        Ok(savepoint)
    }

    async fn release_savepoint(&mut self, savepoint: &Savepoint) -> Result<(), DruidError> {
        self.exec(
            &format!("RELEASE SAVEPOINT {}", Self::savepoint_name(savepoint)?),
            Vec::new(),
        )
        .await
        .map(|_| ())
    }

    async fn ping(&mut self) -> Result<(), DruidError> {
        let rows = self.fetch("SELECT 1", Vec::new()).await?;
        if rows.is_empty() {
            Err(DruidError::ValidationFailed(
                "libSQL validation returned no rows".to_owned(),
            ))
        } else {
            Ok(())
        }
    }

    async fn close(&mut self) -> Result<(), DruidError> {
        self.connection.take();
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
    fn database_meta_data(&mut self) -> Result<Box<dyn PhysicalDatabaseMetaData + '_>, DruidError> {
        self.connection()?;
        Ok(Box::new(LibSqlDatabaseMetaData::new(&self.url)))
    }
    fn auto_commit(&self) -> bool {
        self.auto_commit
    }
    async fn set_auto_commit(&mut self, auto_commit: bool) -> Result<(), DruidError> {
        if auto_commit == self.auto_commit {
            return Ok(());
        }
        if auto_commit {
            self.commit().await
        } else {
            self.begin().await
        }
    }
    async fn warnings(&mut self) -> Result<Option<SqlWarning>, DruidError> {
        self.connection()?;
        Ok(None)
    }
    async fn clear_warnings(&mut self) -> Result<(), DruidError> {
        self.connection().map(|_| ())
    }
    fn mark_discarded(&mut self) {
        self.discarded = true;
        self.connection.take();
    }
    fn is_discarded(&self) -> bool {
        self.discarded
    }
    fn driver_name(&self) -> &str {
        "libsql-rs"
    }
}
