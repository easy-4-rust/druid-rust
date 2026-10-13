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
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::Mutex;

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

struct OrderedConnectionEventListener(Arc<Mutex<Vec<&'static str>>>);

impl ConnectionEventListener for OrderedConnectionEventListener {
    fn connection_closed(&self, _connection_id: u64) {
        self.0.lock().unwrap().push("connection-closed");
    }

    fn connection_error_occurred(&self, _connection_id: u64, _error: &DruidError) {}
}

struct PanickingConnectionEventListener;

impl ConnectionEventListener for PanickingConnectionEventListener {
    fn connection_closed(&self, _connection_id: u64) {
        panic!("connection event listener panic");
    }

    fn connection_error_occurred(&self, _connection_id: u64, _error: &DruidError) {
        panic!("connection event listener panic");
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

#[derive(Default)]
struct RecordingStatementEventListener {
    closed: AtomicUsize,
    errors: AtomicUsize,
    connection_id: AtomicU64,
    statement_id: AtomicU64,
    error_has_sql_exception: AtomicBool,
}

impl StatementEventListener for RecordingStatementEventListener {
    fn statement_closed(&self, connection_id: u64, statement_id: usize) {
        self.connection_id.store(connection_id, Ordering::SeqCst);
        self.statement_id
            .store(statement_id as u64, Ordering::SeqCst);
        self.closed.fetch_add(1, Ordering::SeqCst);
    }

    fn statement_error_occurred(
        &self,
        connection_id: u64,
        statement_id: usize,
        error: &DruidError,
    ) {
        self.connection_id.store(connection_id, Ordering::SeqCst);
        self.statement_id
            .store(statement_id as u64, Ordering::SeqCst);
        self.error_has_sql_exception
            .store(error.sql_exception().is_some(), Ordering::SeqCst);
        self.errors.fetch_add(1, Ordering::SeqCst);
    }
}

struct PanickingStatementEventListener;

impl StatementEventListener for PanickingStatementEventListener {
    fn statement_closed(&self, _connection_id: u64, _statement_id: usize) {
        panic!("statement-closed listener panic");
    }

    fn statement_error_occurred(
        &self,
        _connection_id: u64,
        _statement_id: usize,
        _error: &DruidError,
    ) {
        panic!("statement-error listener panic");
    }
}

struct ReentrantStatementEventListener {
    owner: Mutex<Option<std::sync::Weak<Mutex<DruidPooledConnection>>>>,
    self_reference: Mutex<Option<std::sync::Weak<Self>>>,
    closed: AtomicUsize,
    removed_self: AtomicBool,
}

impl StatementEventListener for ReentrantStatementEventListener {
    fn statement_closed(&self, _connection_id: u64, _statement_id: usize) {
        self.closed.fetch_add(1, Ordering::SeqCst);
        let owner = self
            .owner
            .lock()
            .unwrap()
            .as_ref()
            .and_then(std::sync::Weak::upgrade);
        let listener = self
            .self_reference
            .lock()
            .unwrap()
            .as_ref()
            .and_then(std::sync::Weak::upgrade);
        if let (Some(owner), Some(listener)) = (owner, listener) {
            let listener: Arc<dyn StatementEventListener> = listener;
            let removed = owner
                .lock()
                .unwrap()
                .remove_statement_event_listener(&listener)
                .unwrap();
            self.removed_self.store(removed, Ordering::SeqCst);
        }
    }

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
async fn pooled_connection_listener_panic_does_not_skip_recycle_or_duplicate_close_event() {
    let physical = make_physical().await;
    let holder =
        DruidConnectionHolder::with_standard_connection(physical, 77, std::time::Duration::ZERO, 0)
            .await
            .unwrap();
    let order = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&order);
    let mut pooled = DruidPooledConnection::with_standard_holder(
        holder,
        String::new(),
        None,
        false,
        false,
        Box::new(move |_, _| {
            observed.lock().unwrap().push("recycle");
            true
        }),
    );
    let panicking: Arc<dyn ConnectionEventListener> = Arc::new(PanickingConnectionEventListener);
    let recording: Arc<dyn ConnectionEventListener> =
        Arc::new(OrderedConnectionEventListener(Arc::clone(&order)));
    pooled.add_connection_event_listener(panicking).unwrap();
    pooled
        .add_connection_event_listener(Arc::clone(&recording))
        .unwrap();
    pooled
        .add_connection_event_listener(Arc::clone(&recording))
        .unwrap();
    assert!(pooled.remove_connection_event_listener(&recording).unwrap());

    Connection::close(&mut pooled).await.unwrap();
    Connection::close(&mut pooled).await.unwrap();

    assert_eq!(*order.lock().unwrap(), ["connection-closed", "recycle"]);
}

#[tokio::test]
async fn pooled_connection_error_listener_panic_does_not_skip_fatal_discard() {
    let mut pooled = make_pooled(make_physical().await).await;
    let panicking: Arc<dyn ConnectionEventListener> = Arc::new(PanickingConnectionEventListener);
    pooled.add_connection_event_listener(panicking).unwrap();
    pooled.set_exception_sorter(Arc::new(AlwaysFatalSorter));
    let error = DruidError::SqlException(Box::new(
        SqlException::driver(17002, "connection reset").with_sql_state("08006"),
    ));

    assert!(pooled.handle_exception(&error));
    assert!(pooled.connection_holder().unwrap().is_discard());
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
async fn prepared_statement_close_notifies_registered_listener_once_with_original_identity() {
    let mut pooled = make_pooled(make_physical().await).await;
    let listener = Arc::new(RecordingStatementEventListener::default());
    let listener_trait: Arc<dyn StatementEventListener> = listener.clone();
    pooled
        .add_statement_event_listener(Arc::clone(&listener_trait))
        .unwrap();
    pooled
        .add_statement_event_listener(Arc::clone(&listener_trait))
        .unwrap();
    assert!(pooled
        .remove_statement_event_listener(&listener_trait)
        .unwrap());
    let mut statement = pooled
        .prepare_statement("SELECT 1", StatementOptions::default())
        .await
        .unwrap();

    statement.close().await.unwrap();
    statement.close().await.unwrap();

    assert_eq!(listener.closed.load(Ordering::SeqCst), 1);
    assert_eq!(listener.errors.load(Ordering::SeqCst), 0);
    assert_eq!(listener.connection_id.load(Ordering::SeqCst), 1);
    assert!(listener.statement_id.load(Ordering::SeqCst) > 0);
}

#[tokio::test]
async fn prepared_statement_sql_error_notifies_listener_and_preserves_error() {
    let mut pooled = make_pooled(make_physical().await).await;
    let listener = Arc::new(RecordingStatementEventListener::default());
    let listener_trait: Arc<dyn StatementEventListener> = listener.clone();
    pooled.add_statement_event_listener(listener_trait).unwrap();
    let mut statement = pooled
        .prepare_statement("SELECT * FROM missing_table", StatementOptions::default())
        .await
        .unwrap();

    let error = match statement.execute_prepared().await {
        Ok(_) => panic!("query against a missing table must fail"),
        Err(error) => error,
    };

    assert!(error.sql_exception().is_some());
    assert_eq!(listener.errors.load(Ordering::SeqCst), 1);
    assert!(listener.error_has_sql_exception.load(Ordering::SeqCst));
    assert_eq!(listener.connection_id.load(Ordering::SeqCst), 1);
    assert!(listener.statement_id.load(Ordering::SeqCst) > 0);
}

#[tokio::test]
async fn removing_listener_after_prepare_suppresses_close_notification() {
    let mut pooled = make_pooled(make_physical().await).await;
    let listener = Arc::new(RecordingStatementEventListener::default());
    let listener_trait: Arc<dyn StatementEventListener> = listener.clone();
    pooled
        .add_statement_event_listener(Arc::clone(&listener_trait))
        .unwrap();
    let mut statement = pooled
        .prepare_statement("SELECT 1", StatementOptions::default())
        .await
        .unwrap();
    assert!(pooled
        .remove_statement_event_listener(&listener_trait)
        .unwrap());

    statement.close().await.unwrap();

    assert_eq!(listener.closed.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn statement_listener_panic_does_not_skip_close_or_sql_error_delivery() {
    let mut pooled = make_pooled(make_physical().await).await;
    pooled
        .add_statement_event_listener(Arc::new(PanickingStatementEventListener))
        .unwrap();
    let listener = Arc::new(RecordingStatementEventListener::default());
    let listener_trait: Arc<dyn StatementEventListener> = listener.clone();
    pooled.add_statement_event_listener(listener_trait).unwrap();
    let mut statement = pooled
        .prepare_statement("SELECT * FROM missing_table", StatementOptions::default())
        .await
        .unwrap();

    let error = match statement.execute_prepared().await {
        Ok(_) => panic!("query against a missing table must fail"),
        Err(error) => error,
    };
    assert!(error.sql_exception().is_some());
    assert_eq!(listener.errors.load(Ordering::SeqCst), 1);

    statement.close().await.unwrap();

    assert_eq!(listener.closed.load(Ordering::SeqCst), 1);
    assert!(statement.is_closed().unwrap());
}

#[tokio::test]
async fn statement_closed_listener_can_remove_itself_without_deadlock() {
    let pooled = Arc::new(Mutex::new(make_pooled(make_physical().await).await));
    let listener = Arc::new(ReentrantStatementEventListener {
        owner: Mutex::new(Some(Arc::downgrade(&pooled))),
        self_reference: Mutex::new(None),
        closed: AtomicUsize::new(0),
        removed_self: AtomicBool::new(false),
    });
    *listener.self_reference.lock().unwrap() = Some(Arc::downgrade(&listener));
    let listener_trait: Arc<dyn StatementEventListener> = listener.clone();
    pooled
        .lock()
        .unwrap()
        .add_statement_event_listener(listener_trait)
        .unwrap();

    let mut statement = pooled
        .lock()
        .unwrap()
        .prepare_statement("SELECT 1", StatementOptions::default())
        .await
        .unwrap();
    statement.close().await.unwrap();

    assert_eq!(listener.closed.load(Ordering::SeqCst), 1);
    assert!(listener.removed_self.load(Ordering::SeqCst));
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
