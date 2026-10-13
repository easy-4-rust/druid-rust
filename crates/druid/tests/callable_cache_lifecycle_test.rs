//! Callable 与 Prepared 共用连接缓存的可观察生命周期合同。

mod standard_test_support;

use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::{
    CallableInputParameter, CallableParameter, CallableStatement, ParameterIndex,
    PreparedStatement, ResultSetConcurrency, ResultSetHoldability, ResultSetType, Statement,
    StatementCancellation, StatementOptions,
};
use druid_rdbc::types::{RdbcObject, Value};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct DriverState {
    prepares: Vec<(String, bool, StatementOptions)>,
    closes: Vec<usize>,
    cancellations: Vec<Arc<AtomicUsize>>,
}

struct DriverStatement {
    state: Arc<Mutex<DriverState>>,
    id: usize,
    named: HashMap<String, CallableInputParameter>,
    timeout: i32,
    closed: bool,
    fatal: bool,
    cancellation_count: Arc<AtomicUsize>,
}

struct CountingCancellation(Arc<AtomicUsize>);

#[async_trait::async_trait]
impl StatementCancellation for CountingCancellation {
    async fn cancel(&self) -> Result<(), RdbcError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

impl DriverStatement {
    fn execution_error(&self) -> RdbcError {
        if self.fatal {
            RdbcError::SqlException(Box::new(druid::core::SqlException::driver(
                1040,
                "fatal callable",
            )))
        } else {
            RdbcError::UnsupportedOperation {
                operation: "test_callable_execution",
            }
        }
    }
}

druid_rdbc::impl_standard_wrapper!(DriverStatement => CallableStatement);

#[async_trait::async_trait]
impl Statement for DriverStatement {
    fn cancellation_control(&self) -> Result<Arc<dyn StatementCancellation>, RdbcError> {
        Ok(Arc::new(CountingCancellation(Arc::clone(
            &self.cancellation_count,
        ))))
    }

    async fn execute_query(
        &mut self,
        _sql: &str,
    ) -> Result<Box<dyn druid_rdbc::result_set::ResultSet>, RdbcError> {
        Err(self.execution_error())
    }
    async fn execute_update(
        &mut self,
        _sql: &str,
        _keys: druid_rdbc::statement::GeneratedKeys,
    ) -> Result<i64, RdbcError> {
        Err(self.execution_error())
    }
    async fn execute(
        &mut self,
        _sql: &str,
        _keys: druid_rdbc::statement::GeneratedKeys,
    ) -> Result<Vec<druid_rdbc::statement::ExecutionResult>, RdbcError> {
        Err(self.execution_error())
    }
    async fn execute_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        Err(self.execution_error())
    }
    fn query_timeout(&self) -> Result<i32, RdbcError> {
        Ok(self.timeout)
    }
    fn set_query_timeout(&mut self, seconds: i32) -> Result<(), RdbcError> {
        self.timeout = seconds;
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        if !self.closed {
            self.state.lock().unwrap().closes.push(self.id);
            self.closed = true;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl PreparedStatement for DriverStatement {
    async fn execute_prepared(
        &mut self,
    ) -> Result<Vec<druid_rdbc::statement::ExecutionResult>, RdbcError> {
        Err(self.execution_error())
    }
    async fn execute_prepared_query(
        &mut self,
    ) -> Result<Box<dyn druid_rdbc::result_set::ResultSet>, RdbcError> {
        Err(self.execution_error())
    }
    async fn execute_prepared_update(&mut self) -> Result<i64, RdbcError> {
        Err(self.execution_error())
    }
    async fn execute_prepared_batch(&mut self) -> Result<Vec<i64>, RdbcError> {
        Err(self.execution_error())
    }
    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
        self.named.clear();
        Ok(())
    }
    fn clear_prepared_batch(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
}

#[async_trait::async_trait]
impl CallableStatement for DriverStatement {
    fn set_named_parameter(
        &mut self,
        name: &str,
        value: CallableInputParameter,
    ) -> Result<(), RdbcError> {
        self.named.insert(name.to_owned(), value);
        Ok(())
    }
    fn out_parameter(&self, selector: &CallableParameter) -> Result<RdbcObject, RdbcError> {
        match selector {
            CallableParameter::Name(name) if name == "missing" => {
                Err(RdbcError::DriverError("missing OUT".to_owned()))
            }
            CallableParameter::Name(name) if name == "overflow" => {
                Ok(RdbcObject::from(Value::Int(300)))
            }
            CallableParameter::Name(name) if name == "parameter_count" => {
                Ok(RdbcObject::from(Value::Int(self.named.len() as i64)))
            }
            _ => Ok(RdbcObject::from(Value::Int(self.id as i64))),
        }
    }
}

struct DriverConnection {
    state: Arc<Mutex<DriverState>>,
    closed: bool,
}

impl DriverConnection {
    fn prepare(&self, sql: &str, callable: bool, options: StatementOptions) -> DriverStatement {
        let mut state = self.state.lock().unwrap();
        state.prepares.push((sql.to_owned(), callable, options));
        let cancellation_count = Arc::new(AtomicUsize::new(0));
        state.cancellations.push(Arc::clone(&cancellation_count));
        DriverStatement {
            state: Arc::clone(&self.state),
            id: state.prepares.len(),
            named: HashMap::new(),
            timeout: 0,
            closed: false,
            fatal: sql == "fatal",
            cancellation_count,
        }
    }
}

struct CountingMySqlSorter(Arc<AtomicUsize>);

impl druid::core::ExceptionSorter for CountingMySqlSorter {
    fn is_exception_fatal(&self, error: &druid::core::SqlException) -> bool {
        self.0.fetch_add(1, Ordering::Relaxed);
        druid::core::ExceptionSorter::is_exception_fatal(&druid::core::MySqlExceptionSorter, error)
    }
    fn config_from_properties(
        &mut self,
        _properties: Option<&druid::core::ExceptionSorterProperties>,
    ) {
    }
}

#[tokio::test]
async fn callable_execution_failure_classifies_fatal_error_once() {
    for operation in [
        "generic",
        "prepared-query",
        "prepared-update",
        "prepared-batch",
        "query",
        "update",
        "batch",
    ] {
        let driver = Arc::new(Mutex::new(DriverState::default()));
        let classifications = Arc::new(AtomicUsize::new(0));
        let pool = DruidPool::builder()
            .name("callable-fatal-once")
            .db_type_name("mysql")
            .factory(Arc::new(DriverFactory(Arc::clone(&driver))))
            .exception_sorter(Arc::new(CountingMySqlSorter(Arc::clone(&classifications))))
            .max_open(1)
            .max_idle(1)
            .pool_prepared_statements(true)
            .max_pool_prepared_statements_per_connection(2)
            .build()
            .await
            .unwrap();
        let mut connection = pool.get().await.unwrap();
        let mut statement = connection
            .prepare_call("fatal", StatementOptions::default())
            .await
            .unwrap();
        match operation {
            "generic" => assert!(matches!(
                statement.execute_prepared().await,
                Err(RdbcError::SqlException(_))
            )),
            "prepared-query" => assert!(matches!(
                statement.execute_prepared_query().await,
                Err(RdbcError::SqlException(_))
            )),
            "prepared-update" => assert!(matches!(
                statement.execute_prepared_update().await,
                Err(RdbcError::SqlException(_))
            )),
            "prepared-batch" => assert!(matches!(
                statement.execute_prepared_batch().await,
                Err(RdbcError::SqlException(_))
            )),
            "query" => assert!(matches!(
                statement.execute_query("fatal").await,
                Err(RdbcError::SqlException(_))
            )),
            "update" => assert!(matches!(
                statement
                    .execute_update("fatal", druid_rdbc::statement::GeneratedKeys::None)
                    .await,
                Err(RdbcError::SqlException(_))
            )),
            "batch" => assert!(matches!(
                statement.execute_batch().await,
                Err(RdbcError::SqlException(_))
            )),
            _ => unreachable!(),
        }
        assert_eq!(classifications.load(Ordering::Relaxed), 1, "{operation}");
        assert_eq!(pool.state().discard_count, 1);
        assert_eq!(pool.state().active_count, 0);
        statement.close().await.unwrap();
        connection.close().await.unwrap();
        pool.close().await;
        assert_eq!(driver.lock().unwrap().closes, vec![1]);
    }
}

druid_rdbc::impl_standard_wrapper!(DriverConnection => Connection);

#[async_trait::async_trait]
impl Connection for DriverConnection {
    async fn create_statement(
        &mut self,
        options: StatementOptions,
    ) -> Result<Box<dyn Statement>, RdbcError> {
        let mut validation = standard_test_support::TrackingConnection::new(
            standard_test_support::TrackingState::configured(),
        );
        validation.create_statement(options).await
    }
    async fn prepare_statement(
        &mut self,
        sql: &str,
        options: StatementOptions,
    ) -> Result<Box<dyn PreparedStatement>, RdbcError> {
        Ok(Box::new(self.prepare(sql, false, options)))
    }
    async fn prepare_call(
        &mut self,
        sql: &str,
        options: StatementOptions,
    ) -> Result<Box<dyn CallableStatement>, RdbcError> {
        Ok(Box::new(self.prepare(sql, true, options)))
    }
    async fn is_valid(&mut self, _seconds: u32) -> Result<bool, RdbcError> {
        Ok(!self.closed)
    }
    async fn ping(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
    fn driver_name(&self) -> Result<String, RdbcError> {
        Ok("callable-cache-test".to_owned())
    }
}

struct DriverFactory(Arc<Mutex<DriverState>>);

#[async_trait::async_trait]
impl ConnectionFactory for DriverFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        Ok(Box::new(DriverConnection {
            state: Arc::clone(&self.0),
            closed: false,
        }))
    }
}

async fn pool(capacity: usize) -> (DruidPool, Arc<Mutex<DriverState>>) {
    let state = Arc::new(Mutex::new(DriverState::default()));
    let pool = DruidPool::builder()
        .name("callable-cache")
        .db_type_name("mysql")
        .factory(Arc::new(DriverFactory(Arc::clone(&state))))
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(capacity > 0)
        .max_pool_prepared_statements_per_connection(capacity)
        .build()
        .await
        .unwrap();
    (pool, state)
}

fn id(statement: &dyn CallableStatement) -> i64 {
    statement
        .long_out_parameter(&CallableParameter::Index(ParameterIndex::new(1).unwrap()))
        .unwrap()
}

#[tokio::test]
async fn callable_parameter_getter_conversion_and_execution_errors_prevent_reuse() {
    for failure in [
        "empty-name",
        "missing",
        "wrong-string",
        "overflow",
        "execution",
        "prepared-query",
        "prepared-update",
        "prepared-batch",
        "query",
        "update",
        "batch",
        "keys",
        "current-result",
    ] {
        let (pool, driver) = pool(2).await;
        let mut connection = pool.get().await.unwrap();
        let mut first = connection
            .prepare_call("same", StatementOptions::default())
            .await
            .unwrap();
        match failure {
            "empty-name" => assert!(first
                .set_named_parameter("", CallableInputParameter::Int(7))
                .is_err()),
            "missing" => assert!(first
                .out_parameter(&CallableParameter::Name("missing".to_owned()))
                .is_err()),
            "wrong-string" => assert!(first
                .string_out_parameter(&CallableParameter::Index(ParameterIndex::new(1).unwrap()))
                .is_err()),
            "overflow" => assert!(first
                .byte_out_parameter(&CallableParameter::Name("overflow".to_owned()))
                .is_err()),
            "execution" => assert!(first.execute_prepared().await.is_err()),
            "prepared-query" => assert!(first.execute_prepared_query().await.is_err()),
            "prepared-update" => assert!(first.execute_prepared_update().await.is_err()),
            "prepared-batch" => assert!(first.execute_prepared_batch().await.is_err()),
            "query" => assert!(first.execute_query("same").await.is_err()),
            "update" => assert!(first
                .execute_update("same", druid_rdbc::statement::GeneratedKeys::None)
                .await
                .is_err()),
            "batch" => assert!(first.execute_batch().await.is_err()),
            "keys" => assert!(first.generated_keys().await.is_err()),
            "current-result" => assert!(first.get_result_set().await.is_err()),
            _ => unreachable!(),
        }
        first.close().await.unwrap();
        assert_eq!(driver.lock().unwrap().closes, vec![1], "{failure}");
        assert_eq!(
            pool.state().cached_prepared_statement_delete_count,
            1,
            "{failure}"
        );
        assert_eq!(
            pool.state().cached_prepared_statement_count,
            -1,
            "{failure}"
        );
        let mut second = connection
            .prepare_call("same", StatementOptions::default())
            .await
            .unwrap();
        assert_eq!(id(second.as_ref()), 2, "{failure}");
        second.close().await.unwrap();
        connection.close().await.unwrap();
        pool.close().await;
        assert_eq!(driver.lock().unwrap().closes, vec![1, 2]);
    }
}

#[tokio::test]
async fn callable_close_reuses_raw_handle_clears_parameters_and_restores_timeout() {
    let (pool, driver) = pool(2).await;
    let mut connection = pool.get().await.unwrap();
    let mut first = connection
        .prepare_call("same", StatementOptions::default())
        .await
        .unwrap();
    assert_eq!(id(first.as_ref()), 1);
    first
        .set_named_parameter("input", CallableInputParameter::Int(7))
        .unwrap();
    first.set_query_timeout(19).unwrap();
    first.close().await.unwrap();
    first.close().await.unwrap();
    assert_eq!(first.is_closed(), Ok(true));
    assert!(driver.lock().unwrap().closes.is_empty());
    let mut second = connection
        .prepare_call("same", StatementOptions::default())
        .await
        .unwrap();
    assert_eq!(id(second.as_ref()), 1);
    assert_eq!(second.query_timeout(), Ok(0));
    assert_eq!(
        second.long_out_parameter(&CallableParameter::Name("parameter_count".to_owned())),
        Ok(0)
    );
    drop(first);
    assert_eq!(id(second.as_ref()), 1);
    second.close().await.unwrap();
    let state = pool.state();
    assert_eq!(state.prepared_statement_count, 1);
    assert_eq!(state.cached_prepared_statement_count, 1);
    assert_eq!(state.cached_prepared_statement_hit_count, 1);
    assert_eq!(state.cached_prepared_statement_miss_count, 1);
    assert_eq!(state.cached_prepared_statement_access_count, 2);
    connection.close().await.unwrap();
    pool.close().await;
    assert_eq!(driver.lock().unwrap().closes, vec![1]);
}

#[tokio::test]
async fn callable_and_prepared_share_one_lru_budget_and_distinct_handle_kinds() {
    let (pool, driver) = pool(1).await;
    let mut connection = pool.get().await.unwrap();
    let mut callable = connection
        .prepare_call("same", StatementOptions::default())
        .await
        .unwrap();
    callable.close().await.unwrap();
    let mut prepared = connection
        .prepare_statement("same", StatementOptions::default())
        .await
        .unwrap();
    prepared.close().await.unwrap();
    assert_eq!(driver.lock().unwrap().closes, vec![1]);
    let mut callable = connection
        .prepare_call("same", StatementOptions::default())
        .await
        .unwrap();
    assert_eq!(id(callable.as_ref()), 3);
    callable.close().await.unwrap();
    assert_eq!(driver.lock().unwrap().closes, vec![1, 2]);
    assert_eq!(pool.state().cached_prepared_statement_count, 1);
    assert_eq!(pool.state().cached_prepared_statement_delete_count, 2);
    assert_eq!(pool.state().prepared_statement_count, 3);
    assert_eq!(pool.state().cached_prepared_statement_hit_count, 0);
    connection.close().await.unwrap();
    pool.close().await;
    assert_eq!(driver.lock().unwrap().closes, vec![1, 2, 3]);
}

#[tokio::test]
async fn callable_lease_return_recycles_raw_but_rejects_previous_logical_handle() {
    let (pool, driver) = pool(2).await;
    let mut connection = pool.get().await.unwrap();
    let old = connection
        .prepare_call("same", StatementOptions::default())
        .await
        .unwrap();
    let old_control = old
        .cancellation_control()
        .expect("callable proxy should expose the underlying control");
    connection.close().await.unwrap();
    assert_eq!(old.is_closed(), Ok(true));
    assert!(old
        .out_parameter(&CallableParameter::Index(ParameterIndex::new(1).unwrap()))
        .is_err());
    let mut next_connection = pool.get().await.unwrap();
    let mut next = next_connection
        .prepare_call("same", StatementOptions::default())
        .await
        .unwrap();
    assert_eq!(id(next.as_ref()), 1);
    let next_control = next
        .cancellation_control()
        .expect("new callable cache owner should expose its control");
    assert!(matches!(
        old_control.cancel().await,
        Err(RdbcError::ConnectionDiscarded)
    ));
    assert_eq!(
        driver.lock().unwrap().cancellations[0].load(Ordering::SeqCst),
        0
    );
    next_control
        .cancel()
        .await
        .expect("current callable cache owner should retain cancellation access");
    assert_eq!(
        driver.lock().unwrap().cancellations[0].load(Ordering::SeqCst),
        1
    );
    drop(old);
    assert_eq!(id(next.as_ref()), 1);
    next.close().await.unwrap();
    next_connection.close().await.unwrap();
    pool.close().await;
    assert_eq!(driver.lock().unwrap().prepares.len(), 1);
    assert_eq!(driver.lock().unwrap().closes, vec![1]);
}

#[tokio::test]
async fn callable_options_partition_cache_and_disabled_cache_closes_immediately() {
    let (pool, driver) = pool(3).await;
    let mut connection = pool.get().await.unwrap();
    let options = [
        StatementOptions::default(),
        StatementOptions::new(
            ResultSetType::ScrollInsensitive,
            ResultSetConcurrency::ReadOnly,
            ResultSetHoldability::CloseAtCommit,
        ),
        StatementOptions::new(
            ResultSetType::ForwardOnly,
            ResultSetConcurrency::Updatable,
            ResultSetHoldability::HoldOverCommit,
        ),
    ];
    for (offset, option) in options.into_iter().enumerate() {
        let mut statement = connection.prepare_call("same", option).await.unwrap();
        assert_eq!(id(statement.as_ref()), (offset + 1) as i64);
        assert_eq!(statement.options(), Ok(option));
        statement.close().await.unwrap();
    }
    for (offset, option) in options.into_iter().enumerate() {
        let mut statement = connection.prepare_call("same", option).await.unwrap();
        assert_eq!(id(statement.as_ref()), (offset + 1) as i64);
        statement.close().await.unwrap();
    }
    assert_eq!(pool.state().prepared_statement_count, 3);
    assert_eq!(pool.state().cached_prepared_statement_hit_count, 3);
    assert_eq!(driver.lock().unwrap().prepares.len(), 3);
    connection.close().await.unwrap();
    pool.close().await;

    let (pool, driver) = self::pool(0).await;
    let mut connection = pool.get().await.unwrap();
    for expected in [1, 2] {
        let mut statement = connection
            .prepare_call("same", StatementOptions::default())
            .await
            .unwrap();
        assert_eq!(id(statement.as_ref()), expected);
        statement.close().await.unwrap();
    }
    assert_eq!(driver.lock().unwrap().closes, vec![1, 2]);
    assert_eq!(pool.state().prepared_statement_count, 2);
    assert_eq!(pool.state().closed_prepared_statement_count, 2);
    assert_eq!(pool.state().cached_prepared_statement_count, 0);
    assert_eq!(pool.state().cached_prepared_statement_access_count, 0);
    connection.close().await.unwrap();
    pool.close().await;
}
