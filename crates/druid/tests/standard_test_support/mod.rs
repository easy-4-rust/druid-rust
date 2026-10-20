#![allow(dead_code)]

use druid_rdbc::connection::Connection;
use druid_rdbc::error::{BatchUpdateCounts, RdbcError, SqlException, SqlWarning};
use druid_rdbc::extensions::ConnectionCapabilities;
use druid_rdbc::metadata::{
    ColumnDescriptor, ColumnNullability, DatabaseMetaData, ParameterDescriptor, ParameterMetaData,
    ParameterMode, ParameterNullability, ResultSetMetaData, ResultSetMetaDataSnapshot, TablesQuery,
};
use druid_rdbc::result_set::{ColumnIndex, ResultSet};
use druid_rdbc::statement::{
    CallableOutParameter, CallableParameter, CallableStatement, ExecutionResult, GeneratedKeys,
    ParameterIndex, PreparedParameter, PreparedStatement, Statement, StatementOptions,
};
use druid_rdbc::types::Value;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
pub struct TrackingState {
    pub events: Mutex<Vec<String>>,
    pub mock_physical_closed: AtomicBool,
    pub raw_close_count: AtomicUsize,
    pub raw_close_attempt_count: AtomicUsize,
    pub statement_close_count: AtomicUsize,
    pub statement_close_attempt_count: AtomicUsize,
    pub statement_batch_add_attempt_count: AtomicUsize,
    pub statement_batch_clear_count: AtomicUsize,
    pub statement_batch_execute_count: AtomicUsize,
    pub statement_batch_executions: Mutex<Vec<Vec<String>>>,
    pub prepared_close_count: AtomicUsize,
    pub prepared_close_attempt_count: AtomicUsize,
    pub prepared_clear_attempt_count: AtomicUsize,
    pub prepared_is_closed_attempt_count: AtomicUsize,
    pub prepared_execution_parameters: Mutex<Vec<Vec<PreparedParameter>>>,
    pub prepared_input_bindings: Mutex<
        Vec<(
            ParameterIndex,
            druid_rdbc::statement::PreparedInputParameter,
        )>,
    >,
    pub statement_query_timeouts: Mutex<Vec<i32>>,
    pub cursor_names: Mutex<Vec<Option<String>>>,
    pub result_set_close_count: AtomicUsize,
    pub result_set_close_attempt_count: AtomicUsize,
    pub result_set_next_count: AtomicUsize,
    pub result_set_value_count: AtomicUsize,
    pub result_set_meta_data_count: AtomicUsize,
    pub result_set_was_null_count: AtomicUsize,
    pub metadata_query_count: AtomicUsize,
    pub factory_create_count: AtomicUsize,
    pub is_valid_attempt_count: AtomicUsize,
    pub auto_commit: AtomicBool,
    pub read_only: AtomicBool,
    pub catalog: Mutex<Option<String>>,
    pub schema: Mutex<Option<String>>,
    pub warnings: Mutex<Option<SqlWarning>>,
    pub fail_operation: Mutex<Option<&'static str>>,
    pub query_started: Mutex<Option<Arc<tokio::sync::Notify>>>,
    pub query_release: Mutex<Option<Arc<tokio::sync::Notify>>>,
}

impl TrackingState {
    pub fn configured() -> Arc<Self> {
        Arc::new(Self {
            auto_commit: AtomicBool::new(true),
            catalog: Mutex::new(Some("catalog_default".to_string())),
            schema: Mutex::new(Some("schema_default".to_string())),
            warnings: Mutex::new(Some(SqlWarning::new(
                "borrower warning",
                Some("01000".to_string()),
                7,
            ))),
            ..Self::default()
        })
    }

    pub fn record(&self, event: impl Into<String>) {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(event.into());
    }

    pub fn event_snapshot(&self) -> Vec<String> {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub fn fail_next(&self, operation: &'static str) {
        *self
            .fail_operation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(operation);
    }

    pub fn close_mock_physical(&self) {
        self.mock_physical_closed.store(true, Ordering::Release);
        self.record("mock.physical.close");
    }

    fn check_failure(&self, operation: &'static str) -> Result<(), RdbcError> {
        let mut failure = self
            .fail_operation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if failure
            .as_ref()
            .is_some_and(|candidate| *candidate == operation)
        {
            *failure = None;
            return Err(RdbcError::Other(format!("forced {operation} failure")));
        }
        Ok(())
    }

    fn check_sql_failure(&self, operation: &'static str) -> Result<(), RdbcError> {
        if self.check_failure(operation).is_err() {
            return Err(RdbcError::SqlException(Box::new(
                SqlException::new(
                    9002,
                    Some("08006".to_owned()),
                    Some(format!("forced {operation} SQL failure")),
                )
                .with_class_name("task5.FatalCreationException"),
            )));
        }
        Ok(())
    }

    fn check_plain_sql_failure(&self, operation: &'static str) -> Result<(), RdbcError> {
        if self.check_failure(operation).is_err() {
            return Err(RdbcError::SqlException(Box::new(SqlException::new(
                0,
                None,
                Some(format!("forced {operation} SQL failure")),
            ))));
        }
        Ok(())
    }

    pub fn install_query_barrier(
        &self,
        started: Arc<tokio::sync::Notify>,
        release: Arc<tokio::sync::Notify>,
    ) {
        *self
            .query_started
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(started);
        *self
            .query_release
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(release);
    }
}

pub struct TrackingFactory {
    state: Arc<TrackingState>,
}

pub struct UnsupportedStateFactory {
    state: Arc<TrackingState>,
    setter_attempts: Arc<AtomicUsize>,
}

impl UnsupportedStateFactory {
    pub fn new(state: Arc<TrackingState>, setter_attempts: Arc<AtomicUsize>) -> Self {
        Self {
            state,
            setter_attempts,
        }
    }
}

#[async_trait::async_trait]
impl druid_rdbc::driver::ConnectionFactory for UnsupportedStateFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        Ok(Box::new(UnsupportedStateConnection {
            inner: TrackingConnection::new(Arc::clone(&self.state)),
            setter_attempts: Arc::clone(&self.setter_attempts),
        }))
    }

    fn name(&self) -> &str {
        "task5-unsupported-state"
    }
}

struct UnsupportedStateConnection {
    inner: TrackingConnection,
    setter_attempts: Arc<AtomicUsize>,
}

druid_rdbc::impl_standard_wrapper!(UnsupportedStateConnection => Connection);

#[async_trait::async_trait]
impl Connection for UnsupportedStateConnection {
    async fn create_statement(
        &mut self,
        options: StatementOptions,
    ) -> Result<Box<dyn Statement>, RdbcError> {
        Connection::create_statement(&mut self.inner, options).await
    }

    async fn set_auto_commit(&mut self, _enabled: bool) -> Result<(), RdbcError> {
        self.setter_attempts.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    fn auto_commit(&self) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_auto_commit",
        })
    }

    fn read_only(&self) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_read_only",
        })
    }

    async fn set_read_only(&mut self, _read_only: bool) -> Result<(), RdbcError> {
        self.setter_attempts.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    fn catalog(&self) -> Result<Option<String>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_catalog",
        })
    }

    async fn set_catalog(&mut self, _catalog: Option<&str>) -> Result<(), RdbcError> {
        self.setter_attempts.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    fn schema(&self) -> Result<Option<String>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_schema",
        })
    }

    async fn set_schema(&mut self, _schema: Option<&str>) -> Result<(), RdbcError> {
        self.setter_attempts.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    async fn warnings(&mut self) -> Result<Option<SqlWarning>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_warnings",
        })
    }

    async fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        self.setter_attempts.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    async fn is_valid(&mut self, _timeout_seconds: u32) -> Result<bool, RdbcError> {
        Ok(true)
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        Connection::close(&mut self.inner).await
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Connection::is_closed(&self.inner)
    }
}

impl TrackingFactory {
    pub fn new(state: Arc<TrackingState>) -> Self {
        Self { state }
    }
}

#[async_trait::async_trait]
impl druid_rdbc::driver::ConnectionFactory for TrackingFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        self.state
            .factory_create_count
            .fetch_add(1, Ordering::AcqRel);
        self.state.record("factory.create");
        Ok(Box::new(TrackingConnection::new(Arc::clone(&self.state))))
    }

    fn name(&self) -> &str {
        "task5-tracking-standard"
    }
}

pub struct TrackingConnection {
    state: Arc<TrackingState>,
    closed: bool,
}

impl TrackingConnection {
    pub fn new(state: Arc<TrackingState>) -> Self {
        Self {
            state,
            closed: false,
        }
    }

    fn ensure_open(&self) -> Result<(), RdbcError> {
        if self.closed {
            Err(RdbcError::ConnectionDiscarded)
        } else {
            Ok(())
        }
    }
}

druid_rdbc::impl_standard_wrapper!(TrackingConnection => Connection);

#[async_trait::async_trait]
impl Connection for TrackingConnection {
    async fn create_statement(
        &mut self,
        _options: StatementOptions,
    ) -> Result<Box<dyn Statement>, RdbcError> {
        self.ensure_open()?;
        self.state.check_sql_failure("create_statement")?;
        self.state.record("connection.create_statement");
        Ok(Box::new(TrackingStatement::new(Arc::clone(&self.state))))
    }

    async fn prepare_statement(
        &mut self,
        sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn PreparedStatement>, RdbcError> {
        self.ensure_open()?;
        self.state.check_sql_failure("prepare_statement")?;
        self.state.record(format!("connection.prepare:{sql}"));
        Ok(Box::new(TrackingPreparedStatement::new(
            Arc::clone(&self.state),
            sql,
        )))
    }

    async fn prepare_call(
        &mut self,
        sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn CallableStatement>, RdbcError> {
        self.ensure_open()?;
        self.state.check_sql_failure("prepare_call")?;
        self.state.record(format!("connection.prepare_call:{sql}"));
        Ok(Box::new(TrackingCallableStatement::new(
            Arc::clone(&self.state),
            sql,
        )))
    }

    async fn database_meta_data(&mut self) -> Result<Box<dyn DatabaseMetaData>, RdbcError> {
        self.ensure_open()?;
        self.state.check_sql_failure("database_meta_data")?;
        self.state.record("connection.metadata");
        Ok(Box::new(TrackingDatabaseMetaData::new(Arc::clone(
            &self.state,
        ))))
    }

    async fn commit(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.state.record("connection.commit");
        Ok(())
    }

    async fn rollback(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.state.check_failure("rollback")?;
        self.state.record("connection.rollback");
        Ok(())
    }

    async fn set_auto_commit(&mut self, enabled: bool) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.state.check_failure("set_auto_commit")?;
        self.state.auto_commit.store(enabled, Ordering::Release);
        self.state
            .record(format!("connection.set_auto_commit:{enabled}"));
        Ok(())
    }

    fn auto_commit(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        self.state.check_failure("auto_commit_getter")?;
        Ok(self.state.auto_commit.load(Ordering::Acquire))
    }

    fn read_only(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
        self.state.check_failure("read_only_getter")?;
        Ok(self.state.read_only.load(Ordering::Acquire))
    }

    async fn set_read_only(&mut self, read_only: bool) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.state.check_failure("set_read_only")?;
        self.state.read_only.store(read_only, Ordering::Release);
        self.state
            .record(format!("connection.set_read_only:{read_only}"));
        Ok(())
    }

    fn catalog(&self) -> Result<Option<String>, RdbcError> {
        self.ensure_open()?;
        self.state.check_failure("catalog_getter")?;
        Ok(self
            .state
            .catalog
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone())
    }

    async fn set_catalog(&mut self, catalog: Option<&str>) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.state.check_failure("set_catalog")?;
        *self
            .state
            .catalog
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = catalog.map(str::to_owned);
        self.state.record(format!(
            "connection.set_catalog:{}",
            catalog.unwrap_or("<null>")
        ));
        Ok(())
    }

    fn schema(&self) -> Result<Option<String>, RdbcError> {
        self.ensure_open()?;
        self.state.check_failure("schema_getter")?;
        Ok(self
            .state
            .schema
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone())
    }

    async fn set_schema(&mut self, schema: Option<&str>) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.state.check_failure("set_schema")?;
        *self
            .state
            .schema
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = schema.map(str::to_owned);
        self.state.record(format!(
            "connection.set_schema:{}",
            schema.unwrap_or("<null>")
        ));
        Ok(())
    }

    async fn warnings(&mut self) -> Result<Option<SqlWarning>, RdbcError> {
        self.ensure_open()?;
        self.state.check_failure("warnings_getter")?;
        Ok(self
            .state
            .warnings
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone())
    }

    async fn clear_warnings(&mut self) -> Result<(), RdbcError> {
        self.ensure_open()?;
        self.state.check_failure("clear_warnings")?;
        *self
            .state
            .warnings
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        self.state.record("connection.clear_warnings");
        Ok(())
    }

    fn capabilities(&self) -> ConnectionCapabilities {
        ConnectionCapabilities {
            transactions: true,
            savepoints: false,
            catalogs: true,
            schemas: true,
            callable_statements: true,
        }
    }

    async fn is_valid(&mut self, _timeout_seconds: u32) -> Result<bool, RdbcError> {
        self.state
            .is_valid_attempt_count
            .fetch_add(1, Ordering::AcqRel);
        self.state.check_failure("is_valid")?;
        self.state.record("connection.is_valid");
        Ok(!self.closed)
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        if !self.closed {
            self.state
                .raw_close_attempt_count
                .fetch_add(1, Ordering::AcqRel);
            self.state.check_failure("close")?;
            self.closed = true;
            self.state.raw_close_count.fetch_add(1, Ordering::AcqRel);
            self.state.record("connection.close");
        }
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}

pub struct TrackingStatement {
    state: Arc<TrackingState>,
    closed: bool,
    query_timeout: i32,
    batch: Vec<String>,
}

impl TrackingStatement {
    fn new(state: Arc<TrackingState>) -> Self {
        Self {
            state,
            closed: false,
            query_timeout: 0,
            batch: Vec::new(),
        }
    }

    fn generic_results(
        &self,
        value: i64,
        multi_result: bool,
    ) -> Vec<druid_rdbc::statement::ExecutionResult> {
        let count = if multi_result { 3 } else { 1 };
        (0..count)
            .map(|offset| {
                druid_rdbc::statement::ExecutionResult::ResultSet(Box::new(TrackingResultSet::rows(
                    Arc::clone(&self.state),
                    vec![vec![Value::Int(value + i64::from(offset))]],
                ))
                    as Box<dyn ResultSet>)
            })
            .collect()
    }
}

druid_rdbc::impl_standard_wrapper!(TrackingStatement => Statement);

#[async_trait::async_trait]
impl Statement for TrackingStatement {
    fn result_set_type(&self) -> Result<druid_rdbc::statement::ResultSetType, RdbcError> {
        self.state.record("result_set_type");
        self.state
            .check_plain_sql_failure("statement_result_set_type")?;
        Ok(druid_rdbc::statement::ResultSetType::ScrollSensitive)
    }
    fn result_set_concurrency(
        &self,
    ) -> Result<druid_rdbc::statement::ResultSetConcurrency, RdbcError> {
        self.state.record("result_set_concurrency");
        self.state
            .check_plain_sql_failure("statement_result_set_concurrency")?;
        Ok(druid_rdbc::statement::ResultSetConcurrency::Updatable)
    }
    fn result_set_holdability(
        &self,
    ) -> Result<druid_rdbc::statement::ResultSetHoldability, RdbcError> {
        self.state.record("result_set_holdability");
        self.state
            .check_plain_sql_failure("statement_result_set_holdability")?;
        Ok(druid_rdbc::statement::ResultSetHoldability::HoldOverCommit)
    }
    fn set_cursor_name_nullable(&mut self, name: Option<&str>) -> Result<(), RdbcError> {
        self.state
            .check_failure("statement_set_cursor_name_nullable")?;
        self.state
            .cursor_names
            .lock()
            .unwrap()
            .push(name.map(str::to_owned));
        Ok(())
    }
    fn max_field_size(&self) -> Result<i32, RdbcError> {
        self.state
            .check_plain_sql_failure("statement_max_field_size")?;
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_max_field_size",
        })
    }

    fn max_rows(&self) -> Result<i64, RdbcError> {
        self.state.check_plain_sql_failure("statement_max_rows")?;
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_max_rows",
        })
    }

    fn query_timeout(&self) -> Result<i32, RdbcError> {
        self.state
            .check_plain_sql_failure("statement_query_timeout")?;
        Ok(self.query_timeout)
    }

    fn fetch_direction(&self) -> Result<druid_rdbc::statement::FetchDirection, RdbcError> {
        self.state
            .check_plain_sql_failure("statement_fetch_direction")?;
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_fetch_direction",
        })
    }

    fn fetch_size(&self) -> Result<i32, RdbcError> {
        self.state.check_plain_sql_failure("statement_fetch_size")?;
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_fetch_size",
        })
    }

    fn set_query_timeout(&mut self, seconds: i32) -> Result<(), RdbcError> {
        self.query_timeout = seconds;
        self.state
            .statement_query_timeouts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(seconds);
        Ok(())
    }

    fn add_batch(&mut self, sql: String) -> Result<(), RdbcError> {
        self.state
            .statement_batch_add_attempt_count
            .fetch_add(1, Ordering::AcqRel);
        self.state.check_sql_failure("statement_add_batch")?;
        self.state.record(format!("statement.batch.add:{sql}"));
        self.batch.push(sql);
        Ok(())
    }

    fn clear_batch(&mut self) -> Result<(), RdbcError> {
        self.state
            .statement_batch_clear_count
            .fetch_add(1, Ordering::AcqRel);
        self.state.check_sql_failure("statement_clear_batch")?;
        self.state.record("statement.batch.clear");
        self.batch.clear();
        Ok(())
    }

    async fn execute_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        self.state
            .statement_batch_execute_count
            .fetch_add(1, Ordering::AcqRel);
        self.state.record("statement.batch.execute");
        self.state
            .statement_batch_executions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(self.batch.clone());
        if self
            .state
            .check_failure("statement_execute_batch_partial")
            .is_err()
        {
            return Err(RdbcError::BatchUpdateException {
                update_counts: BatchUpdateCounts::from_large(vec![1]),
                cause: Box::new(RdbcError::SqlException(Box::new(
                    SqlException::new(
                        9010,
                        Some("08006".to_owned()),
                        Some("forced partial standard batch failure".to_owned()),
                    )
                    .with_class_name("task5.FatalBatchException"),
                ))),
            });
        }
        let counts = vec![1; self.batch.len()];
        self.batch.clear();
        Ok(counts)
    }

    async fn execute_query(&mut self, sql: &str) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.state.record(format!("statement.query:{sql}"));
        let started = self
            .state
            .query_started
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        let release = self
            .state
            .query_release
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(started) = started {
            started.notify_waiters();
        }
        if let Some(release) = release {
            release.notified().await;
        }
        if sql.contains("tenant_projection") {
            return Ok(Box::new(TrackingResultSet::rows_with_columns(
                Arc::clone(&self.state),
                vec![vec![Value::Int(11), Value::Int(7), Value::Int(33)]],
                ["visible", "tenant_id", "after_hidden"],
            )));
        }
        Ok(Box::new(TrackingResultSet::rows(
            Arc::clone(&self.state),
            vec![vec![Value::Int(11)]],
        )))
    }

    async fn execute_update(
        &mut self,
        sql: &str,
        _generated_keys: GeneratedKeys,
    ) -> Result<i64, RdbcError> {
        self.state.record(format!("statement.update:{sql}"));
        if self
            .state
            .check_failure("statement_execute_update")
            .is_err()
        {
            return Err(RdbcError::SqlException(Box::new(
                SqlException::new(
                    9001,
                    Some("08006".to_owned()),
                    Some("forced standard statement failure".to_owned()),
                )
                .with_class_name("task5.FatalConnectionException"),
            )));
        }
        Ok(1)
    }

    async fn execute(
        &mut self,
        sql: &str,
        _generated_keys: GeneratedKeys,
    ) -> Result<Vec<druid_rdbc::statement::ExecutionResult>, RdbcError> {
        self.state.record(format!("statement.execute:{sql}"));
        Ok(self.generic_results(44, sql.contains("multi_result")))
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.state
            .statement_close_attempt_count
            .fetch_add(1, Ordering::AcqRel);
        tokio::task::yield_now().await;
        if !self.closed {
            self.closed = true;
            self.state
                .statement_close_count
                .fetch_add(1, Ordering::AcqRel);
            self.state.record("statement.close");
        }
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}

pub struct TrackingPreparedStatement {
    state: Arc<TrackingState>,
    sql: String,
    parameters: Vec<PreparedParameter>,
    batches: Vec<Vec<PreparedParameter>>,
    closed: bool,
    query_timeout: i32,
}

impl TrackingPreparedStatement {
    fn new(state: Arc<TrackingState>, sql: &str) -> Self {
        Self {
            state,
            sql: sql.to_owned(),
            parameters: Vec::new(),
            batches: Vec::new(),
            closed: false,
            query_timeout: 0,
        }
    }

    fn generic_results(&self) -> Vec<druid_rdbc::statement::ExecutionResult> {
        // 保留 BASE PreparedConnection 的混合结果合同，不以三个查询游标代替。
        if self.sql == "MULTI" {
            return vec![
                ExecutionResult::ResultSet(Box::new(TrackingResultSet::rows(
                    Arc::clone(&self.state),
                    vec![vec![Value::Int(10)]],
                ))),
                ExecutionResult::UpdateCount(2),
                ExecutionResult::ResultSet(Box::new(TrackingResultSet::rows(
                    Arc::clone(&self.state),
                    vec![vec![Value::Int(20)]],
                ))),
            ];
        }
        let count = if self.sql.contains("multi_result") {
            3
        } else {
            1
        };
        (0..count)
            .map(|offset| {
                druid_rdbc::statement::ExecutionResult::ResultSet(Box::new(TrackingResultSet::rows(
                    Arc::clone(&self.state),
                    vec![vec![Value::Int(55 + i64::from(offset))]],
                ))
                    as Box<dyn ResultSet>)
            })
            .collect()
    }
}

druid_rdbc::impl_standard_wrapper!(TrackingPreparedStatement => PreparedStatement);

#[async_trait::async_trait]
impl Statement for TrackingPreparedStatement {
    fn max_field_size(&self) -> Result<i32, RdbcError> {
        self.state
            .check_plain_sql_failure("statement_max_field_size")?;
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_max_field_size",
        })
    }
    fn max_rows(&self) -> Result<i64, RdbcError> {
        self.state.check_plain_sql_failure("statement_max_rows")?;
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_max_rows",
        })
    }
    fn fetch_direction(&self) -> Result<druid_rdbc::statement::FetchDirection, RdbcError> {
        self.state
            .check_plain_sql_failure("statement_fetch_direction")?;
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_fetch_direction",
        })
    }
    fn fetch_size(&self) -> Result<i32, RdbcError> {
        self.state.check_plain_sql_failure("statement_fetch_size")?;
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_fetch_size",
        })
    }
    fn result_set_type(&self) -> Result<druid_rdbc::statement::ResultSetType, RdbcError> {
        self.state.record("result_set_type");
        self.state
            .check_plain_sql_failure("statement_result_set_type")?;
        Ok(druid_rdbc::statement::ResultSetType::ScrollSensitive)
    }
    fn result_set_concurrency(
        &self,
    ) -> Result<druid_rdbc::statement::ResultSetConcurrency, RdbcError> {
        self.state.record("result_set_concurrency");
        self.state
            .check_plain_sql_failure("statement_result_set_concurrency")?;
        Ok(druid_rdbc::statement::ResultSetConcurrency::Updatable)
    }
    fn result_set_holdability(
        &self,
    ) -> Result<druid_rdbc::statement::ResultSetHoldability, RdbcError> {
        self.state.record("result_set_holdability");
        self.state
            .check_plain_sql_failure("statement_result_set_holdability")?;
        Ok(druid_rdbc::statement::ResultSetHoldability::HoldOverCommit)
    }
    fn set_cursor_name_nullable(&mut self, name: Option<&str>) -> Result<(), RdbcError> {
        self.state
            .check_failure("statement_set_cursor_name_nullable")?;
        self.state
            .cursor_names
            .lock()
            .unwrap()
            .push(name.map(str::to_owned));
        Ok(())
    }
    async fn generated_keys(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.state.record("prepared.generated_keys");
        Ok(Box::new(TrackingResultSet::rows(
            Arc::clone(&self.state),
            vec![vec![Value::Int(99)]],
        )))
    }

    fn query_timeout(&self) -> Result<i32, RdbcError> {
        self.state
            .check_plain_sql_failure("statement_query_timeout")?;
        Ok(self.query_timeout)
    }

    fn set_query_timeout(&mut self, seconds: i32) -> Result<(), RdbcError> {
        self.query_timeout = seconds;
        self.state
            .statement_query_timeouts
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(seconds);
        Ok(())
    }

    fn add_batch(&mut self, sql: String) -> Result<(), RdbcError> {
        self.state.record(format!("prepared.batch.add:{sql}"));
        self.batches.push(self.parameters.clone());
        Ok(())
    }

    fn clear_batch(&mut self) -> Result<(), RdbcError> {
        self.state.record("prepared.batch.clear");
        self.batches.clear();
        Ok(())
    }

    async fn execute_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        self.state.record("prepared.batch.execute");
        let count = self.batches.len();
        self.batches.clear();
        Ok(vec![1; count])
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        if !self.closed {
            self.state
                .prepared_close_attempt_count
                .fetch_add(1, Ordering::AcqRel);
            self.state.check_failure("prepared_close")?;
            self.closed = true;
            self.state
                .prepared_close_count
                .fetch_add(1, Ordering::AcqRel);
            self.state.record("prepared.close");
        }
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        self.state
            .prepared_is_closed_attempt_count
            .fetch_add(1, Ordering::AcqRel);
        self.state.check_failure("prepared_is_closed")?;
        Ok(self.closed)
    }
}

#[async_trait::async_trait]
impl PreparedStatement for TrackingPreparedStatement {
    fn set_input_parameter(
        &mut self,
        index: ParameterIndex,
        input: druid_rdbc::statement::PreparedInputParameter,
    ) -> Result<(), RdbcError> {
        // 记录型 driver 与 BASE 参数槽 fixture 一样保留富描述符，不读取资源。
        match input.scalar_value() {
            Ok(value) => self.set_parameter(PreparedParameter { index, value })?,
            Err(RdbcError::UnsupportedOperation { .. }) => {
                self.state.check_failure("prepared_set_parameter")?;
            }
            Err(error) => return Err(error),
        }
        let mut bindings = self.state.prepared_input_bindings.lock().unwrap();
        if let Some(existing) = bindings.iter_mut().find(|(bound, _)| *bound == index) {
            *existing = (index, input);
        } else {
            bindings.push((index, input));
            bindings.sort_by_key(|(bound, _)| *bound);
        }
        Ok(())
    }

    fn set_parameter(&mut self, parameter: PreparedParameter) -> Result<(), RdbcError> {
        self.state.check_failure("prepared_set_parameter")?;
        self.state.record(format!(
            "prepared.parameter:{}:{:?}",
            parameter.index.get(),
            parameter.value
        ));
        if let Some(existing) = self
            .parameters
            .iter_mut()
            .find(|existing| existing.index == parameter.index)
        {
            *existing = parameter;
        } else {
            self.parameters.push(parameter);
            self.parameters.sort_by_key(|parameter| parameter.index);
        }
        Ok(())
    }

    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
        self.state
            .prepared_clear_attempt_count
            .fetch_add(1, Ordering::AcqRel);
        if self.state.mock_physical_closed.load(Ordering::Acquire) {
            return Err(RdbcError::SqlException(Box::new(
                // 对应 Java MockConnectionClosedException()：保留 NULL 消息与 SQLState。
                SqlException::new(0, None, None)
                    .with_class_name("com.alibaba.druid.mock.MockConnectionClosedException"),
            )));
        }
        self.state.check_failure("prepared_clear_parameters")?;
        self.parameters.clear();
        self.state.prepared_input_bindings.lock().unwrap().clear();
        self.state.record("prepared.clear_parameters");
        Ok(())
    }

    async fn execute_prepared_query(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.state.record(format!("prepared.query:{}", self.sql));
        self.state
            .prepared_execution_parameters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(self.parameters.clone());
        Ok(Box::new(TrackingResultSet::rows(
            Arc::clone(&self.state),
            vec![vec![Value::Int(22)]],
        )))
    }

    async fn execute_prepared_update(&mut self) -> Result<i64, RdbcError> {
        self.state.record(format!("prepared.update:{}", self.sql));
        self.state
            .prepared_execution_parameters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(self.parameters.clone());
        Ok(1)
    }

    async fn execute_prepared(
        &mut self,
    ) -> Result<Vec<druid_rdbc::statement::ExecutionResult>, RdbcError> {
        self.state
            .record(format!("prepared.execute:generic: {}", self.sql));
        self.state
            .prepared_execution_parameters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(self.parameters.clone());
        self.state.check_sql_failure("prepared_execute_generic")?;
        Ok(self.generic_results())
    }

    fn parameter_meta_data(&self) -> Result<Arc<dyn ParameterMetaData>, RdbcError> {
        Ok(Arc::new(TrackingParameterMetaData))
    }
}

pub struct TrackingCallableStatement {
    prepared: TrackingPreparedStatement,
    out_value: Value,
}

impl TrackingCallableStatement {
    fn new(state: Arc<TrackingState>, sql: &str) -> Self {
        Self {
            prepared: TrackingPreparedStatement::new(state, sql),
            out_value: Value::Int(33),
        }
    }
}

druid_rdbc::impl_standard_wrapper!(TrackingCallableStatement => CallableStatement);

#[async_trait::async_trait]
impl Statement for TrackingCallableStatement {
    fn max_field_size(&self) -> Result<i32, RdbcError> {
        Statement::max_field_size(&self.prepared)
    }
    fn max_rows(&self) -> Result<i64, RdbcError> {
        Statement::max_rows(&self.prepared)
    }
    fn fetch_direction(&self) -> Result<druid_rdbc::statement::FetchDirection, RdbcError> {
        Statement::fetch_direction(&self.prepared)
    }
    fn fetch_size(&self) -> Result<i32, RdbcError> {
        Statement::fetch_size(&self.prepared)
    }
    fn result_set_type(&self) -> Result<druid_rdbc::statement::ResultSetType, RdbcError> {
        Statement::result_set_type(&self.prepared)
    }
    fn result_set_concurrency(
        &self,
    ) -> Result<druid_rdbc::statement::ResultSetConcurrency, RdbcError> {
        Statement::result_set_concurrency(&self.prepared)
    }
    fn result_set_holdability(
        &self,
    ) -> Result<druid_rdbc::statement::ResultSetHoldability, RdbcError> {
        Statement::result_set_holdability(&self.prepared)
    }
    fn set_cursor_name_nullable(&mut self, name: Option<&str>) -> Result<(), RdbcError> {
        self.prepared.set_cursor_name_nullable(name)
    }
    async fn generated_keys(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        Statement::generated_keys(&mut self.prepared).await
    }

    fn query_timeout(&self) -> Result<i32, RdbcError> {
        Statement::query_timeout(&self.prepared)
    }

    fn set_query_timeout(&mut self, seconds: i32) -> Result<(), RdbcError> {
        Statement::set_query_timeout(&mut self.prepared, seconds)
    }

    fn add_batch(&mut self, sql: String) -> Result<(), RdbcError> {
        Statement::add_batch(&mut self.prepared, sql)
    }

    fn clear_batch(&mut self) -> Result<(), RdbcError> {
        Statement::clear_batch(&mut self.prepared)
    }

    async fn execute_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        Statement::execute_batch(&mut self.prepared).await
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        Statement::close(&mut self.prepared).await
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Statement::is_closed(&self.prepared)
    }
}

#[async_trait::async_trait]
impl PreparedStatement for TrackingCallableStatement {
    fn set_parameter(&mut self, parameter: PreparedParameter) -> Result<(), RdbcError> {
        PreparedStatement::set_parameter(&mut self.prepared, parameter)
    }

    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
        PreparedStatement::clear_parameters(&mut self.prepared)
    }

    async fn execute_prepared_query(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        PreparedStatement::execute_prepared_query(&mut self.prepared).await
    }

    async fn execute_prepared_update(&mut self) -> Result<i64, RdbcError> {
        PreparedStatement::execute_prepared_update(&mut self.prepared).await
    }

    async fn execute_prepared(
        &mut self,
    ) -> Result<Vec<druid_rdbc::statement::ExecutionResult>, RdbcError> {
        PreparedStatement::execute_prepared(&mut self.prepared).await
    }
}

#[async_trait::async_trait]
impl CallableStatement for TrackingCallableStatement {
    fn register_out_parameter(
        &mut self,
        parameter: CallableParameter,
        out_parameter: CallableOutParameter,
    ) -> Result<(), RdbcError> {
        self.prepared.state.record(format!(
            "callable.out:{parameter:?}:{}:{}",
            out_parameter.sql_type(),
            out_parameter.type_name().unwrap_or_default()
        ));
        Ok(())
    }

    fn out_value(&self, _index: ParameterIndex) -> Result<Value, RdbcError> {
        Ok(self.out_value.clone())
    }
}

pub struct TrackingResultSet {
    state: Arc<TrackingState>,
    rows: VecDeque<Vec<Value>>,
    current: Option<Vec<Value>>,
    closed: bool,
    was_null: AtomicBool,
    metadata: Arc<ResultSetMetaDataSnapshot>,
}

impl TrackingResultSet {
    fn rows(state: Arc<TrackingState>, rows: Vec<Vec<Value>>) -> Self {
        Self::rows_with_columns(state, rows, ["value"])
    }

    fn rows_with_columns<const N: usize>(
        state: Arc<TrackingState>,
        rows: Vec<Vec<Value>>,
        columns: [&str; N],
    ) -> Self {
        Self {
            state,
            rows: rows.into(),
            current: None,
            closed: false,
            was_null: AtomicBool::new(false),
            metadata: Arc::new(ResultSetMetaDataSnapshot::new(
                columns
                    .into_iter()
                    .map(|label| {
                        ColumnDescriptor::new(label, "BIGINT", ColumnNullability::Nullable)
                    })
                    .collect(),
            )),
        }
    }
}

druid_rdbc::impl_standard_wrapper!(TrackingResultSet => ResultSet);

#[async_trait::async_trait]
impl ResultSet for TrackingResultSet {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        self.state
            .result_set_next_count
            .fetch_add(1, Ordering::AcqRel);
        self.state.record("result_set.next");
        self.current = self.rows.pop_front();
        self.was_null.store(false, Ordering::Release);
        Ok(self.current.is_some())
    }

    fn value(&self, column: ColumnIndex) -> Result<Value, RdbcError> {
        self.state
            .result_set_value_count
            .fetch_add(1, Ordering::AcqRel);
        self.state.check_sql_failure("result_set_value")?;
        let value = self
            .current
            .as_ref()
            .and_then(|row| row.get(column.get().saturating_sub(1)))
            .cloned()
            .ok_or_else(|| RdbcError::InvalidArgument("no current row".to_string()))?;
        self.was_null
            .store(matches!(value, Value::Null), Ordering::Release);
        Ok(value)
    }

    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        self.state
            .result_set_meta_data_count
            .fetch_add(1, Ordering::AcqRel);
        self.state.check_sql_failure("result_set_meta_data")?;
        Ok(self.metadata.clone())
    }

    fn was_null(&self) -> Result<bool, RdbcError> {
        self.state
            .result_set_was_null_count
            .fetch_add(1, Ordering::AcqRel);
        self.state.check_sql_failure("result_set_was_null")?;
        Ok(self.was_null.load(Ordering::Acquire))
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        if !self.closed {
            self.state
                .result_set_close_attempt_count
                .fetch_add(1, Ordering::AcqRel);
            self.state.check_failure("result_set_close")?;
            self.closed = true;
            self.state
                .result_set_close_count
                .fetch_add(1, Ordering::AcqRel);
            self.state.record("result_set.close");
        }
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}

pub struct TrackingDatabaseMetaData {
    state: Arc<TrackingState>,
}

impl TrackingDatabaseMetaData {
    fn new(state: Arc<TrackingState>) -> Self {
        Self { state }
    }
}

druid_rdbc::impl_standard_wrapper!(TrackingDatabaseMetaData => DatabaseMetaData);

#[async_trait::async_trait]
impl DatabaseMetaData for TrackingDatabaseMetaData {
    async fn database_product_name(&mut self) -> Result<String, RdbcError> {
        Ok("Task5DB".to_string())
    }

    async fn tables(&mut self, _query: TablesQuery) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.state
            .metadata_query_count
            .fetch_add(1, Ordering::AcqRel);
        self.state.record("metadata.tables");
        let started = self
            .state
            .query_started
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        let release = self
            .state
            .query_release
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(started) = started {
            started.notify_waiters();
        }
        if let Some(release) = release {
            release.notified().await;
        }
        Ok(Box::new(TrackingResultSet::rows(
            Arc::clone(&self.state),
            vec![vec![Value::String("task5_table".to_string())]],
        )))
    }
}

pub struct TrackingParameterMetaData;

druid_rdbc::impl_standard_wrapper!(TrackingParameterMetaData => ParameterMetaData);

impl ParameterMetaData for TrackingParameterMetaData {
    fn parameter_count(&self) -> usize {
        1
    }

    fn parameter(&self, _index: ParameterIndex) -> Result<ParameterDescriptor, RdbcError> {
        Ok(ParameterDescriptor::new(
            "BIGINT",
            ParameterMode::In,
            ParameterNullability::Nullable,
        ))
    }
}
