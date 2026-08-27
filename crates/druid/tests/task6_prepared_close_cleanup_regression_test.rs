//! Task 6 canonical prepared-close cleanup ordering regressions.

use druid::core::{ExceptionSorter, ExceptionSorterProperties, RdbcError, SqlException};
use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::statement::{
    ParameterIndex, PreparedParameter, PreparedStatement, Statement, StatementOptions,
};
use druid_rdbc::types::Value;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CleanupStage {
    AddParameterBatch,
    PreparedBatch,
    StatementBatch,
    Parameters,
    SetParameter,
}

#[derive(Default)]
struct CleanupTrace {
    events: Mutex<Vec<&'static str>>,
    prepared_close_count: AtomicUsize,
}

impl CleanupTrace {
    fn record(&self, event: &'static str) {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(event);
    }

    fn snapshot(&self) -> Vec<&'static str> {
        self.events
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

struct CleanupPrepared {
    failure: CleanupStage,
    trace: Arc<CleanupTrace>,
    closed: bool,
}

druid_rdbc::impl_standard_wrapper!(CleanupPrepared => PreparedStatement);

#[async_trait::async_trait]
impl Statement for CleanupPrepared {
    fn clear_batch(&mut self) -> Result<(), RdbcError> {
        self.trace.record("statement_batch");
        if self.failure == CleanupStage::StatementBatch {
            Err(cleanup_error("statement_batch"))
        } else {
            Ok(())
        }
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.trace.record("close");
        if !self.closed {
            self.closed = true;
            self.trace
                .prepared_close_count
                .fetch_add(1, Ordering::AcqRel);
        }
        Ok(())
    }

    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(self.closed)
    }
}

#[async_trait::async_trait]
impl PreparedStatement for CleanupPrepared {
    fn sql(&self) -> Result<&str, RdbcError> {
        Ok("SELECT 1")
    }

    fn clear_prepared_batch(&mut self) -> Result<(), RdbcError> {
        self.trace.record("prepared_batch");
        if self.failure == CleanupStage::PreparedBatch {
            Err(cleanup_error("prepared_batch"))
        } else {
            Ok(())
        }
    }

    fn clear_parameters(&mut self) -> Result<(), RdbcError> {
        self.trace.record("parameters");
        if self.failure == CleanupStage::Parameters {
            Err(cleanup_error("parameters"))
        } else {
            Ok(())
        }
    }

    fn set_parameter(&mut self, _parameter: PreparedParameter) -> Result<(), RdbcError> {
        if self.failure == CleanupStage::SetParameter {
            Err(cleanup_error("set_parameter"))
        } else {
            Ok(())
        }
    }

    fn add_parameter_batch(
        &mut self,
        _parameters: Vec<PreparedParameter>,
    ) -> Result<(), RdbcError> {
        if self.failure == CleanupStage::AddParameterBatch {
            Err(cleanup_error("add_parameter_batch"))
        } else {
            Ok(())
        }
    }
}

struct CleanupConnection {
    failure: CleanupStage,
    trace: Arc<CleanupTrace>,
    closed: bool,
}

druid_rdbc::impl_standard_wrapper!(CleanupConnection => Connection);

#[async_trait::async_trait]
impl Connection for CleanupConnection {
    async fn prepare_statement(
        &mut self,
        _sql: &str,
        _options: StatementOptions,
    ) -> Result<Box<dyn PreparedStatement>, RdbcError> {
        Ok(Box::new(CleanupPrepared {
            failure: self.failure,
            trace: Arc::clone(&self.trace),
            closed: false,
        }))
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
}

struct CleanupFactory {
    failure: CleanupStage,
    trace: Arc<CleanupTrace>,
}

#[async_trait::async_trait]
impl ConnectionFactory for CleanupFactory {
    async fn create(&self) -> Result<Box<dyn Connection>, RdbcError> {
        Ok(Box::new(CleanupConnection {
            failure: self.failure,
            trace: Arc::clone(&self.trace),
            closed: false,
        }))
    }
}

struct FatalCleanupSorter;

impl ExceptionSorter for FatalCleanupSorter {
    fn is_exception_fatal(&self, _exception: &SqlException) -> bool {
        true
    }

    fn config_from_properties(&mut self, _properties: Option<&ExceptionSorterProperties>) {}
}

fn cleanup_error(stage: &'static str) -> RdbcError {
    RdbcError::SqlException(Box::new(
        SqlException::driver(17_002, format!("forced {stage} cleanup failure"))
            .with_sql_state("08006")
            .with_class_name("task6.PreparedCleanupFailure"),
    ))
}

async fn assert_cleanup_failure(failure: CleanupStage, expected_events: &[&'static str]) {
    let trace = Arc::new(CleanupTrace::default());
    let pool = DruidPool::builder()
        .name("task6-prepared-cleanup-regression")
        .driver_name("cleanup")
        .factory(Arc::new(CleanupFactory {
            failure,
            trace: Arc::clone(&trace),
        }) as Arc<dyn ConnectionFactory>)
        .exception_sorter(Arc::new(FatalCleanupSorter))
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(1)
        .build()
        .await
        .expect("pool");
    let mut connection = pool.get().await.expect("connection");
    let mut statement = connection
        .prepare_statement("SELECT 1", StatementOptions::default())
        .await
        .expect("statement");

    assert!(matches!(
        statement.close().await,
        Err(RdbcError::SqlException(_))
    ));
    assert_eq!(trace.snapshot(), expected_events);
    assert_eq!(trace.prepared_close_count.load(Ordering::Acquire), 1);
    assert!(Connection::is_closed(&connection).expect("logical state"));
    assert!(!connection.is_recycled());

    Connection::close(&mut connection)
        .await
        .expect("logical connection close");
    assert!(connection.is_recycled());
    assert_eq!(pool.state().discard_count, 1);
    assert_eq!(trace.prepared_close_count.load(Ordering::Acquire), 1);
    pool.close().await;
}

async fn assert_direct_mutator_failure(failure: CleanupStage) {
    let trace = Arc::new(CleanupTrace::default());
    let pool = DruidPool::builder()
        .name("task6-prepared-mutator-regression")
        .driver_name("cleanup")
        .factory(Arc::new(CleanupFactory {
            failure,
            trace: Arc::clone(&trace),
        }) as Arc<dyn ConnectionFactory>)
        .exception_sorter(Arc::new(FatalCleanupSorter))
        .max_open(1)
        .max_idle(1)
        .pool_prepared_statements(true)
        .max_pool_prepared_statements_per_connection(1)
        .build()
        .await
        .expect("pool");
    let mut connection = pool.get().await.expect("connection");
    let mut statement = connection
        .prepare_statement("SELECT 1", StatementOptions::default())
        .await
        .expect("statement");
    let parameter = PreparedParameter {
        index: ParameterIndex::new(1).expect("index"),
        value: Value::Int(1),
    };
    let result = match failure {
        CleanupStage::SetParameter => statement.set_parameter(parameter),
        CleanupStage::AddParameterBatch => statement.add_parameter_batch(vec![parameter]),
        CleanupStage::PreparedBatch => statement.clear_prepared_batch(),
        CleanupStage::Parameters => statement.clear_parameters(),
        CleanupStage::StatementBatch => unreachable!("covered by close cleanup order"),
    };

    assert!(matches!(result, Err(RdbcError::SqlException(_))));
    assert!(Connection::is_closed(&connection).expect("logical state"));
    assert!(!connection.is_recycled());
    assert_eq!(trace.prepared_close_count.load(Ordering::Acquire), 0);

    Connection::close(&mut connection)
        .await
        .expect("logical connection close");
    assert!(connection.is_recycled());
    assert_eq!(pool.state().discard_count, 1);
    pool.close().await;
}

#[tokio::test]
async fn prepared_batch_cleanup_failure_stops_later_stages_and_discards() {
    assert_cleanup_failure(CleanupStage::PreparedBatch, &["prepared_batch", "close"]).await;
}

#[tokio::test]
async fn statement_batch_cleanup_failure_runs_after_prepared_batch_and_discards() {
    assert_cleanup_failure(
        CleanupStage::StatementBatch,
        &["prepared_batch", "statement_batch", "close"],
    )
    .await;
}

#[tokio::test]
async fn parameter_cleanup_failure_runs_last_and_discards() {
    assert_cleanup_failure(
        CleanupStage::Parameters,
        &["prepared_batch", "statement_batch", "parameters", "close"],
    )
    .await;
}

#[tokio::test]
async fn set_parameter_failure_uses_owner_disposition_once() {
    assert_direct_mutator_failure(CleanupStage::SetParameter).await;
}

#[tokio::test]
async fn add_parameter_batch_failure_uses_owner_disposition_once() {
    assert_direct_mutator_failure(CleanupStage::AddParameterBatch).await;
}

#[tokio::test]
async fn clear_parameter_batch_failure_uses_owner_disposition_once() {
    assert_direct_mutator_failure(CleanupStage::PreparedBatch).await;
}

#[tokio::test]
async fn clear_parameters_failure_uses_owner_disposition_once() {
    assert_direct_mutator_failure(CleanupStage::Parameters).await;
}
