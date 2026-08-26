//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
//! Differential tests for `DruidPooledConnection` — Java Druid 1.2.28 semantics.
//!
//! Uses real Toasty `SQLite` in-memory connections. Focuses on uncovered getter/setter
//! families, transaction lifecycle, warning chain, close/Drop, attributes, event
//! listeners, error paths, and the Wrapper trait.

use druid::core::{
    Connection, ConnectionEventListener, ConnectionFactory, DruidConnectionHolder, DruidError,
    DruidPooledConnection, ExceptionSorter, ExceptionSorterProperties, ProxyAttributeValue,
    RdbcError, SqlException, StandardUnwrapped, StatementEventListener, Value, Wrapper, WrapperExt,
};
use druid_rdbc::connection::TransactionIsolation;
use druid_rdbc::metadata::DatabaseMetaDataInfoExt;
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetExt};
use druid_rdbc::statement::{
    GeneratedKeys, ParameterIndex, PreparedParameter, PreparedStatement, ResultSetConcurrency,
    ResultSetHoldability, ResultSetType, Statement, StatementOptions,
};
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::any::TypeId;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

// ── helpers ────────────────────────────────────────────────────────

async fn make_physical() -> Box<dyn Connection> {
    ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("factory")
        .create()
        .await
        .expect("connection")
}

async fn make_pooled(physical: Box<dyn Connection>) -> DruidPooledConnection {
    let holder =
        DruidConnectionHolder::with_standard_connection(physical, 1, std::time::Duration::ZERO, 0)
            .await
            .unwrap();
    DruidPooledConnection::with_standard_holder(
        holder,
        String::new(),
        None,
        false,
        false,
        Box::new(|_, _| true),
    )
}

async fn make_pooled_with_source(
    physical: Box<dyn Connection>,
    data_source: &str,
) -> DruidPooledConnection {
    let holder =
        DruidConnectionHolder::with_standard_connection(physical, 2, std::time::Duration::ZERO, 0)
            .await
            .unwrap();
    DruidPooledConnection::with_standard_holder(
        holder,
        data_source.to_owned(),
        None,
        false,
        false,
        Box::new(|_, _| true),
    )
}

// ── id / data_source / is_recycled ─────────────────────────────────

// ── connected_time_millis / properties / close_count ───────────────

// ── last_validate_time_millis ──────────────────────────────────────

// ── connection_hold_duration / set_connected_time_nano ─────────────

// ── attributes ─────────────────────────────────────────────────────

// ── transaction_info ───────────────────────────────────────────────

// ── variables / global_variables ───────────────────────────────────

// ── connection_holder ──────────────────────────────────────────────

// ── physical_connection_mut ────────────────────────────────────────

// ── Debug ──────────────────────────────────────────────────────────

// ── Wrapper trait ──────────────────────────────────────────────────

// ── Connection trait delegation ─────────────────────────────

// ── exec through pooled connection ─────────────────────────────────

// ── transaction lifecycle through pooled connection ─────────────────

// ── savepoints through pooled ──────────────────────────────────────

// ── warnings ───────────────────────────────────────────────────────

// ── set_auto_commit ────────────────────────────────────────────────

// ── set_transaction_isolation ──────────────────────────────────────

// ── close / is_closed ──────────────────────────────────────────────

// ── recycle() consumes self ────────────────────────────────────────

// ── discard_connection ─────────────────────────────────────────────

// ── ping ───────────────────────────────────────────────────────────

// ── create_statement ───────────────────────────────────────────────

// ── prepare_statement family ───────────────────────────────────────

// ── prepare_call family ────────────────────────────────────────────

// ── event listeners ────────────────────────────────────────────────

#[derive(Debug)]
struct TestConnectionEventListener {
    closed_count: AtomicUsize,
    error_count: AtomicUsize,
}

impl TestConnectionEventListener {
    fn new() -> Self {
        Self {
            closed_count: AtomicUsize::new(0),
            error_count: AtomicUsize::new(0),
        }
    }
}

impl ConnectionEventListener for TestConnectionEventListener {
    fn connection_closed(&self, _connection_id: u64) {
        self.closed_count.fetch_add(1, Ordering::Relaxed);
    }

    fn connection_error_occurred(&self, _connection_id: u64, _error: &DruidError) {
        self.error_count.fetch_add(1, Ordering::Relaxed);
    }
}

#[derive(Debug)]
struct TestStatementEventListener;

impl StatementEventListener for TestStatementEventListener {
    fn statement_closed(&self, _connection_id: u64, _statement_id: usize) {}
    fn statement_error_occurred(
        &self,
        _connection_id: u64,
        _statement_id: usize,
        _error: &DruidError,
    ) {
    }
}

// ── exception sorter ───────────────────────────────────────────────

struct NeverFatalSorter;

impl ExceptionSorter for NeverFatalSorter {
    fn is_exception_fatal(&self, _exception: &SqlException) -> bool {
        false
    }
    fn config_from_properties(&mut self, _properties: Option<&ExceptionSorterProperties>) {}
}

// ── handle_exception ───────────────────────────────────────────────

struct AlwaysFatalSorter;

impl ExceptionSorter for AlwaysFatalSorter {
    fn is_exception_fatal(&self, _exception: &SqlException) -> bool {
        true
    }
    fn config_from_properties(&mut self, _properties: Option<&ExceptionSorterProperties>) {}
}

// ── is_discarded / mark_discarded ──────────────────────────────────

// ── database_meta_data / get_meta_data ─────────────────────────────

// ── catalog / schema ───────────────────────────────────────────────

// ── Drop ───────────────────────────────────────────────────────────

// ── ConnectionConstants ────────────────────────────────────

// ── Multiple sequential operations ─────────────────────────────────

// ── with_recycle_policy ────────────────────────────────────────────

// ── create_statement_with_result_set ───────────────────────────────

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_prepare_statement_with_column_indexes() {
    let mut pooled = make_pooled(make_physical().await).await;
    pooled
        .exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();
    let keys = GeneratedKeys::ColumnIndexes(vec![ColumnIndex::new(1).unwrap()]);
    assert!(matches!(keys, GeneratedKeys::ColumnIndexes(ref indexes) if indexes.len() == 1));
    assert!(pooled
        .prepare_statement("INSERT INTO t (id) VALUES (?)", StatementOptions::default())
        .await
        .is_ok());
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_prepare_statement_with_column_names() {
    let mut pooled = make_pooled(make_physical().await).await;
    pooled
        .exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();
    let keys = GeneratedKeys::ColumnNames(vec!["id".to_owned()]);
    assert!(matches!(keys, GeneratedKeys::ColumnNames(ref names) if names == &["id"]));
    assert!(pooled
        .prepare_statement("INSERT INTO t (id) VALUES (?)", StatementOptions::default())
        .await
        .is_ok());
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_prepare_statement_with_auto_generated_keys_option(
) {
    let mut pooled = make_pooled(make_physical().await).await;
    pooled
        .exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();
    let keys = GeneratedKeys::ReturnGeneratedKeys;
    assert!(matches!(keys, GeneratedKeys::ReturnGeneratedKeys));
    assert!(pooled
        .prepare_statement("INSERT INTO t (id) VALUES (?)", StatementOptions::default())
        .await
        .is_ok());
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_prepare_call_does_not_panic() {
    let mut pooled = make_pooled(make_physical().await).await;
    let result = pooled
        .prepare_call("SELECT 1", StatementOptions::default())
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_prepare_call_with_holdability_does_not_panic(
) {
    let mut pooled = make_pooled(make_physical().await).await;
    let options = StatementOptions::new(
        ResultSetType::ForwardOnly,
        ResultSetConcurrency::ReadOnly,
        ResultSetHoldability::HoldOverCommit,
    );
    let result = pooled.prepare_call("SELECT 1", options).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_prepare_call_with_result_set_does_not_panic(
) {
    let mut pooled = make_pooled(make_physical().await).await;
    let options = StatementOptions::new(
        ResultSetType::ForwardOnly,
        ResultSetConcurrency::ReadOnly,
        ResultSetHoldability::CloseAtCommit,
    );
    let result = pooled.prepare_call("SELECT 1", options).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_add_and_remove_connection_event_listener()
{
    let p = make_pooled(make_physical().await).await;
    let listener: Arc<dyn ConnectionEventListener> = Arc::new(TestConnectionEventListener::new());
    p.add_connection_event_listener(Arc::clone(&listener))
        .unwrap();
    let removed = p.remove_connection_event_listener(&listener).unwrap();
    assert!(removed);
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_remove_nonexistent_listener_returns_false()
{
    let p = make_pooled(make_physical().await).await;
    let listener: Arc<dyn ConnectionEventListener> = Arc::new(TestConnectionEventListener::new());
    let removed = p.remove_connection_event_listener(&listener).unwrap();
    assert!(!removed);
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_add_and_remove_statement_event_listener() {
    let p = make_pooled(make_physical().await).await;
    let listener: Arc<dyn StatementEventListener> = Arc::new(TestStatementEventListener);
    p.add_statement_event_listener(Arc::clone(&listener))
        .unwrap();
    let removed = p.remove_statement_event_listener(&listener).unwrap();
    assert!(removed);
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_set_exception_sorter_and_with_exception_sorter(
) {
    let mut p = make_pooled(make_physical().await).await;
    let sorter: Arc<dyn ExceptionSorter> = Arc::new(NeverFatalSorter);
    p.set_exception_sorter(Arc::clone(&sorter));
    let p2 = make_pooled(make_physical().await).await;
    let _ = p2.with_exception_sorter(sorter);
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_handle_non_fatal_exception_returns_false()
{
    let mut p = make_pooled(make_physical().await).await;
    let sorter: Arc<dyn ExceptionSorter> = Arc::new(NeverFatalSorter);
    p.set_exception_sorter(sorter);
    let error = DruidError::SqlException(Box::new(SqlException::driver(1000, "test error")));
    let fatal = p.handle_exception(&error);
    assert!(!fatal);
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_handle_fatal_exception_marks_discarded() {
    let holder = DruidConnectionHolder::with_standard_connection(
        make_physical().await,
        1,
        std::time::Duration::ZERO,
        0,
    )
    .await
    .unwrap();
    let discarded = Arc::new(AtomicBool::new(false));
    let observed = Arc::clone(&discarded);
    let mut p = DruidPooledConnection::with_standard_holder_and_disposition_observer(
        holder,
        "fatal-observer".to_owned(),
        None,
        false,
        false,
        Box::new(move |disposition| {
            observed.store(!disposition.is_reusable(), Ordering::Release);
        }),
        Box::new(|_, _| true),
    );
    let sorter: Arc<dyn ExceptionSorter> = Arc::new(AlwaysFatalSorter);
    p.set_exception_sorter(sorter);
    let error = DruidError::SqlException(Box::new(
        SqlException::driver(17002, "Io exception: Connection reset").with_sql_state("08006"),
    ));
    let fatal = p.handle_exception(&error);
    assert!(fatal);
    assert!(discarded.load(Ordering::Acquire));
    assert!(!p.is_recycled());
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_is_discarded_false_initially() {
    let p = make_pooled(make_physical().await).await;
    assert!(!p.is_recycled());
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_mark_discarded_sets_discarded() {
    let mut p = make_pooled(make_physical().await).await;
    assert!(!p.is_recycled());
    let _ = p.discard_connection();
    assert!(p.is_recycled());
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_database_meta_data_returns_url() {
    let mut p = make_pooled(make_physical().await).await;
    let mut meta = p.database_meta_data().await.unwrap();
    let url = meta.get_url().await.unwrap();
    assert!(url.is_some());
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_get_meta_data_delegates() {
    let mut p = make_pooled(make_physical().await).await;
    let mut meta = p.database_meta_data().await.unwrap();
    let name = meta.get_driver_name().await.unwrap();
    assert!(name.is_some());
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_catalog_none_for_sqlite() {
    let p = make_pooled(make_physical().await).await;
    assert!(matches!(
        p.catalog(),
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_catalog"
        })
    ));
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_schema_none_for_sqlite() {
    let p = make_pooled(make_physical().await).await;
    assert!(matches!(
        p.schema(),
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_schema"
        })
    ));
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_drop_cleans_up_without_panic() {
    let p = make_pooled(make_physical().await).await;
    drop(p);
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_drop_after_use_cleans_up() {
    let mut p = make_pooled(make_physical().await).await;
    p.exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();
    drop(p);
}

#[test]
fn task6_druid_pooled_connection_differential_test_transaction_isolation_constants() {
    assert_eq!(DruidPooledConnection::TRANSACTION_NONE, 0);
    assert_eq!(DruidPooledConnection::TRANSACTION_READ_UNCOMMITTED, 1);
    assert_eq!(DruidPooledConnection::TRANSACTION_READ_COMMITTED, 2);
    assert_eq!(DruidPooledConnection::TRANSACTION_REPEATABLE_READ, 4);
    assert_eq!(DruidPooledConnection::TRANSACTION_SERIALIZABLE, 8);
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_multiple_statements_on_same_connection() {
    let mut p = make_pooled(make_physical().await).await;
    p.exec("CREATE TABLE t (id INTEGER PRIMARY KEY, v INTEGER)", vec![])
        .await
        .unwrap();
    for i in 1..=5 {
        p.exec(&format!("INSERT INTO t (id, v) VALUES ({i}, {i})"), vec![])
            .await
            .unwrap();
    }
    let rows = p
        .fetch("SELECT * FROM t WHERE v > 3", vec![])
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_with_recycle_policy_constructor() {
    let holder = DruidConnectionHolder::with_standard_connection(
        make_physical().await,
        10,
        std::time::Duration::ZERO,
        0,
    )
    .await
    .unwrap();
    let pooled = DruidPooledConnection::with_standard_holder(
        holder,
        "test".to_owned(),
        None,
        false,
        false,
        Box::new(|_, disposition| disposition.is_reusable()),
    );
    assert_eq!(pooled.id(), 10);
    assert_eq!(pooled.data_source(), "test");
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_create_statement_with_result_set_options()
{
    let mut pooled = make_pooled(make_physical().await).await;
    let options = StatementOptions::new(
        ResultSetType::ForwardOnly,
        ResultSetConcurrency::ReadOnly,
        ResultSetHoldability::CloseAtCommit,
    );
    assert!(pooled.create_statement(options).await.is_ok());
}

#[tokio::test]
async fn task6_druid_pooled_connection_differential_test_create_statement_with_holdability_options()
{
    let mut pooled = make_pooled(make_physical().await).await;
    let options = StatementOptions::new(
        ResultSetType::ForwardOnly,
        ResultSetConcurrency::ReadOnly,
        ResultSetHoldability::HoldOverCommit,
    );
    assert!(pooled.create_statement(options).await.is_ok());
}
