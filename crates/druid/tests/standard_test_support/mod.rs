#![allow(dead_code)]

use druid_rdbc::connection::Connection;
use druid_rdbc::error::{RdbcError, SqlWarning};
use druid_rdbc::extensions::ConnectionCapabilities;
use druid_rdbc::metadata::{
    ColumnDescriptor, ColumnNullability, DatabaseMetaData, ParameterDescriptor, ParameterMetaData,
    ParameterMode, ParameterNullability, ResultSetMetaData, ResultSetMetaDataSnapshot, TablesQuery,
};
use druid_rdbc::result_set::{ColumnIndex, ResultSet};
use druid_rdbc::statement::{
    CallableOutParameter, CallableStatement, GeneratedKeys, ParameterIndex, PreparedParameter,
    PreparedStatement, Statement, StatementOptions,
};
use druid_rdbc::types::Value;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
pub struct TrackingState {
    pub events: Mutex<Vec<String>>,
    pub raw_close_count: AtomicUsize,
    pub raw_close_attempt_count: AtomicUsize,
    pub statement_close_count: AtomicUsize,
    pub prepared_close_count: AtomicUsize,
    pub result_set_close_count: AtomicUsize,
    pub metadata_query_count: AtomicUsize,
    pub factory_create_count: AtomicUsize,
    pub auto_commit: AtomicBool,
    pub read_only: AtomicBool,
    pub catalog: Mutex<Option<String>>,
    pub schema: Mutex<Option<String>>,
    pub warnings: Mutex<Option<SqlWarning>>,
    pub fail_operation: Mutex<Option<&'static str>>,
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
}

pub struct TrackingFactory {
    state: Arc<TrackingState>,
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
        self.state.record("connection.create_statement");
        Ok(Box::new(TrackingStatement::new(Arc::clone(&self.state))))
    }

    async fn prepare_statement(
        &mut self,
        sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn PreparedStatement>, RdbcError> {
        self.ensure_open()?;
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
        self.state.record(format!("connection.prepare_call:{sql}"));
        Ok(Box::new(TrackingCallableStatement::new(
            Arc::clone(&self.state),
            sql,
        )))
    }

    async fn database_meta_data(&mut self) -> Result<Box<dyn DatabaseMetaData>, RdbcError> {
        self.ensure_open()?;
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
        Ok(self.state.auto_commit.load(Ordering::Acquire))
    }

    fn read_only(&self) -> Result<bool, RdbcError> {
        self.ensure_open()?;
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
}

impl TrackingStatement {
    fn new(state: Arc<TrackingState>) -> Self {
        Self {
            state,
            closed: false,
        }
    }
}

druid_rdbc::impl_standard_wrapper!(TrackingStatement => Statement);

#[async_trait::async_trait]
impl Statement for TrackingStatement {
    async fn execute_query(&mut self, sql: &str) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.state.record(format!("statement.query:{sql}"));
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
        Ok(1)
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
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
    closed: bool,
}

impl TrackingPreparedStatement {
    fn new(state: Arc<TrackingState>, sql: &str) -> Self {
        Self {
            state,
            sql: sql.to_owned(),
            parameters: Vec::new(),
            closed: false,
        }
    }
}

druid_rdbc::impl_standard_wrapper!(TrackingPreparedStatement => PreparedStatement);

#[async_trait::async_trait]
impl Statement for TrackingPreparedStatement {
    async fn close(&mut self) -> Result<(), RdbcError> {
        if !self.closed {
            self.closed = true;
            self.state
                .prepared_close_count
                .fetch_add(1, Ordering::AcqRel);
            self.state.record("prepared.close");
        }
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}

#[async_trait::async_trait]
impl PreparedStatement for TrackingPreparedStatement {
    fn set_parameter(&mut self, parameter: PreparedParameter) -> Result<(), RdbcError> {
        self.state
            .record(format!("prepared.parameter:{}", parameter.index.get()));
        self.parameters.push(parameter);
        Ok(())
    }

    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
        self.parameters.clear();
        Ok(())
    }

    async fn execute_prepared_query(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.state.record(format!("prepared.query:{}", self.sql));
        Ok(Box::new(TrackingResultSet::rows(
            Arc::clone(&self.state),
            vec![vec![Value::Int(22)]],
        )))
    }

    async fn execute_prepared_update(&mut self) -> Result<i64, RdbcError> {
        self.state.record(format!("prepared.update:{}", self.sql));
        Ok(1)
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

    async fn execute_prepared_query(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        PreparedStatement::execute_prepared_query(&mut self.prepared).await
    }

    async fn execute_prepared_update(&mut self) -> Result<i64, RdbcError> {
        PreparedStatement::execute_prepared_update(&mut self.prepared).await
    }
}

#[async_trait::async_trait]
impl CallableStatement for TrackingCallableStatement {
    fn register_out_parameter(&mut self, parameter: CallableOutParameter) -> Result<(), RdbcError> {
        self.prepared.state.record(format!(
            "callable.out:{}:{}",
            parameter.index.get(),
            parameter.type_name
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
    was_null: bool,
    metadata: Arc<ResultSetMetaDataSnapshot>,
}

impl TrackingResultSet {
    fn rows(state: Arc<TrackingState>, rows: Vec<Vec<Value>>) -> Self {
        Self {
            state,
            rows: rows.into(),
            current: None,
            closed: false,
            was_null: false,
            metadata: Arc::new(ResultSetMetaDataSnapshot::new(vec![ColumnDescriptor::new(
                "value",
                "BIGINT",
                ColumnNullability::Nullable,
            )])),
        }
    }
}

druid_rdbc::impl_standard_wrapper!(TrackingResultSet => ResultSet);

#[async_trait::async_trait]
impl ResultSet for TrackingResultSet {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        self.current = self.rows.pop_front();
        self.was_null = false;
        Ok(self.current.is_some())
    }

    fn value(&self, column: ColumnIndex) -> Result<Value, RdbcError> {
        self.current
            .as_ref()
            .and_then(|row| row.get(column.get().saturating_sub(1)))
            .cloned()
            .ok_or_else(|| RdbcError::InvalidArgument("no current row".to_string()))
    }

    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        Ok(self.metadata.clone())
    }

    fn was_null(&self) -> Result<bool, RdbcError> {
        Ok(self.was_null)
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        if !self.closed {
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
