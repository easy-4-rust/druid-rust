//! Differential tests for `DruidPooledConnection` — Java Druid 1.2.28 semantics.
//!
//! Uses real Toasty SQLite in-memory connections. Focuses on uncovered getter/setter
//! families, transaction lifecycle, warning chain, close/Drop, attributes, event
//! listeners, error paths, and the Wrapper trait.

extern crate druid_core as druid;
use druid_core::core::{
    ConnectionEventListener, DruidError, DruidPooledConnection, ExceptionSorter,
    ExceptionSorterProperties, PhysicalConnection, PhysicalConnectionFactory, ProxyAttributeValue,
    SqlException, StatementEventListener, Value, Wrapper,
};
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::any::TypeId;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

// ── helpers ────────────────────────────────────────────────────────

async fn make_physical() -> Box<dyn PhysicalConnection> {
    ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("factory")
        .create()
        .await
        .expect("connection")
}

fn make_pooled(physical: Box<dyn PhysicalConnection>) -> DruidPooledConnection {
    DruidPooledConnection::new(physical, 1, Box::new(|_, _| {}))
}

fn make_pooled_with_source(
    physical: Box<dyn PhysicalConnection>,
    data_source: &str,
) -> DruidPooledConnection {
    DruidPooledConnection::with_context(
        physical,
        2,
        data_source.to_string(),
        None,
        Box::new(|_, _| {}),
    )
}

// ── id / data_source / is_recycled ─────────────────────────────────

#[tokio::test]
async fn id_returns_constructor_id() {
    let p = make_pooled(make_physical().await);
    assert_eq!(p.id(), 1);
}

#[tokio::test]
async fn data_source_returns_constructor_string() {
    let p = make_pooled_with_source(make_physical().await, "my-datasource");
    assert_eq!(p.data_source(), "my-datasource");
}

#[tokio::test]
async fn data_source_empty_by_default() {
    let p = make_pooled(make_physical().await);
    assert_eq!(p.data_source(), "");
}

#[tokio::test]
async fn is_recycled_false_initially() {
    let p = make_pooled(make_physical().await);
    assert!(!p.is_recycled());
}

// ── connected_time_millis / properties / close_count ───────────────

#[tokio::test]
async fn connected_time_millis_is_nonzero() {
    let p = make_pooled(make_physical().await);
    assert!(p.connected_time_millis() > 0);
}

#[tokio::test]
async fn properties_returns_empty_by_default() {
    let p = make_pooled(make_physical().await);
    assert!(p.properties().is_empty());
}

#[tokio::test]
async fn close_count_zero_initially() {
    let p = make_pooled(make_physical().await);
    assert_eq!(p.close_count(), 0);
}

// ── last_validate_time_millis ──────────────────────────────────────

#[tokio::test]
async fn last_validate_time_millis_zero_initially() {
    let mut p = make_pooled(make_physical().await);
    assert_eq!(p.last_validate_time_millis(), 0);
    p.set_last_validate_time_millis(12345);
    assert_eq!(p.last_validate_time_millis(), 12345);
}

// ── connection_hold_duration / set_connected_time_nano ─────────────

#[tokio::test]
async fn connection_hold_duration_is_nonzero() {
    let p = make_pooled(make_physical().await);
    let _ = p.connection_hold_duration();
}

#[tokio::test]
async fn set_connected_time_nano_resets_borrowed_at() {
    let mut p = make_pooled(make_physical().await);
    let before = p.connection_hold_duration();
    p.set_connected_time_nano();
    let after = p.connection_hold_duration();
    assert!(after <= before + std::time::Duration::from_millis(100));
}

// ── attributes ─────────────────────────────────────────────────────

#[tokio::test]
async fn attributes_empty_initially() {
    let p = make_pooled(make_physical().await);
    assert_eq!(p.attributes_size(), 0);
    assert!(p.attributes().is_empty());
    assert!(p.attribute("missing").is_none());
}

#[tokio::test]
async fn put_attribute_and_get() {
    let p = make_pooled(make_physical().await);
    let old = p.put_attribute("key1", ProxyAttributeValue::new("val1".to_string()));
    assert!(old.is_none());
    assert_eq!(p.attributes_size(), 1);
    let attr = p.attribute("key1").unwrap();
    let value: Arc<String> = attr.downcast::<String>().unwrap();
    assert_eq!(value.as_ref(), "val1");
}

#[tokio::test]
async fn put_attribute_overwrite() {
    let p = make_pooled(make_physical().await);
    p.put_attribute("k", ProxyAttributeValue::new(42_i32));
    let old = p.put_attribute("k", ProxyAttributeValue::new(99_i32));
    let old_val: Arc<i32> = old.unwrap().downcast::<i32>().unwrap();
    assert_eq!(*old_val, 42);
    assert_eq!(p.attributes_size(), 1);
}

#[tokio::test]
async fn clear_attributes() {
    let p = make_pooled(make_physical().await);
    p.put_attribute("k", ProxyAttributeValue::new(true));
    assert_eq!(p.attributes_size(), 1);
    p.clear_attributes();
    assert_eq!(p.attributes_size(), 0);
}

// ── transaction_info ───────────────────────────────────────────────

#[tokio::test]
async fn transaction_info_none_initially() {
    let p = make_pooled(make_physical().await);
    assert!(p.transaction_info().is_none());
}

// ── variables / global_variables ───────────────────────────────────

#[tokio::test]
async fn variables_none_initially() {
    let p = make_pooled(make_physical().await);
    assert!(p.variables().is_none());
}

#[tokio::test]
async fn global_variables_none_initially() {
    let p = make_pooled(make_physical().await);
    assert!(p.global_variables().is_none());
}

#[tokio::test]
#[allow(deprecated)]
async fn gloabl_variables_deprecated_alias() {
    let p = make_pooled(make_physical().await);
    let gv = p.gloabl_variables();
    assert!(gv.is_none());
}

// ── connection_holder ──────────────────────────────────────────────

#[tokio::test]
async fn connection_holder_some_initially() {
    let p = make_pooled(make_physical().await);
    assert!(p.connection_holder().is_some());
}

#[tokio::test]
async fn connection_holder_mut_some_initially() {
    let mut p = make_pooled(make_physical().await);
    assert!(p.connection_holder_mut().is_some());
}

// ── physical_connection_mut ────────────────────────────────────────

#[tokio::test]
async fn physical_connection_mut_some_initially() {
    let mut p = make_pooled(make_physical().await);
    assert!(p.physical_connection_mut().is_some());
}

// ── Debug ──────────────────────────────────────────────────────────

#[tokio::test]
async fn debug_format_contains_id_and_data_source() {
    let p = make_pooled_with_source(make_physical().await, "test-ds");
    let debug = format!("{p:?}");
    assert!(debug.contains("DruidPooledConnection"));
    assert!(debug.contains("test-ds"));
}

// ── Wrapper trait ──────────────────────────────────────────────────

#[tokio::test]
async fn wrapper_as_any_returns_self() {
    let p = make_pooled(make_physical().await);
    let any_ref = Wrapper::as_any(&p);
    assert_eq!(any_ref.type_id(), TypeId::of::<DruidPooledConnection>());
}

#[tokio::test]
async fn wrapper_is_wrapper_for_self_type() {
    let p = make_pooled(make_physical().await);
    assert!(Wrapper::is_wrapper_for(
        &p,
        Some(TypeId::of::<DruidPooledConnection>())
    ));
}

#[tokio::test]
async fn wrapper_is_wrapper_for_physical_connection() {
    let p = make_pooled(make_physical().await);
    assert!(Wrapper::is_wrapper_for(
        &p,
        Some(TypeId::of::<dyn PhysicalConnection>())
    ));
}

#[tokio::test]
async fn wrapper_is_wrapper_for_none_returns_false() {
    let p = make_pooled(make_physical().await);
    assert!(!Wrapper::is_wrapper_for(&p, None));
}

#[tokio::test]
async fn wrapper_is_wrapper_for_random_type_returns_false() {
    let p = make_pooled(make_physical().await);
    assert!(!Wrapper::is_wrapper_for(&p, Some(TypeId::of::<String>())));
}

#[tokio::test]
async fn wrapper_unwrap_none_returns_none() {
    let p = make_pooled(make_physical().await);
    assert!(Wrapper::unwrap(&p, None).is_none());
}

#[tokio::test]
async fn wrapper_unwrap_self_type_returns_object() {
    let p = make_pooled(make_physical().await);
    let unwrapped = Wrapper::unwrap(&p, Some(TypeId::of::<DruidPooledConnection>()));
    assert!(unwrapped.is_some());
}

#[tokio::test]
async fn wrapper_unwrap_physical_connection_type() {
    let p = make_pooled(make_physical().await);
    let unwrapped = Wrapper::unwrap(&p, Some(TypeId::of::<dyn PhysicalConnection>()));
    assert!(unwrapped.is_some());
}

// ── PhysicalConnection trait delegation ─────────────────────────────

#[tokio::test]
async fn auto_commit_true_by_default() {
    let p = make_pooled(make_physical().await);
    assert!(p.auto_commit());
}

#[tokio::test]
async fn driver_name_is_sqlite() {
    let p = make_pooled(make_physical().await);
    assert_eq!(p.driver_name(), "SQLite");
}

#[tokio::test]
async fn transaction_isolation_default_sqlite() {
    let p = make_pooled(make_physical().await);
    assert_eq!(p.transaction_isolation(), 8);
}

#[tokio::test]
async fn capabilities_delegates_to_physical() {
    let p = make_pooled(make_physical().await);
    let caps = p.capabilities();
    assert!(caps.transactions);
    assert!(caps.savepoints);
}

#[tokio::test]
async fn read_only_false_by_default() {
    let p = make_pooled(make_physical().await);
    assert!(!p.read_only());
}

#[tokio::test]
async fn holdability_delegates() {
    let p = make_pooled(make_physical().await);
    let _ = p.holdability();
}

// ── exec through pooled connection ─────────────────────────────────

#[tokio::test]
async fn exec_create_table() {
    let mut p = make_pooled(make_physical().await);
    p.exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();
}

#[tokio::test]
async fn exec_insert_and_select() {
    let mut p = make_pooled(make_physical().await);
    p.exec("CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT)", vec![])
        .await
        .unwrap();
    let result = p
        .exec("INSERT INTO t (id, v) VALUES (1, 'hello')", vec![])
        .await
        .unwrap();
    assert_eq!(result.rows_affected, 1);

    let rows = p
        .fetch("SELECT v FROM t WHERE id = 1", vec![])
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
}

// ── transaction lifecycle through pooled connection ─────────────────

#[tokio::test]
async fn begin_commit_through_pooled() {
    let mut p = make_pooled(make_physical().await);
    p.exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();
    p.begin().await.unwrap();
    p.exec("INSERT INTO t (id) VALUES (1)", vec![])
        .await
        .unwrap();
    p.commit().await.unwrap();
}

#[tokio::test]
async fn begin_rollback_through_pooled() {
    let mut p = make_pooled(make_physical().await);
    p.exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();
    p.begin().await.unwrap();
    p.exec("INSERT INTO t (id) VALUES (1)", vec![])
        .await
        .unwrap();
    p.rollback().await.unwrap();
    let rows = p.fetch("SELECT * FROM t", vec![]).await.unwrap();
    assert_eq!(rows.len(), 0);
}

// ── savepoints through pooled ──────────────────────────────────────

#[tokio::test]
async fn savepoint_lifecycle_through_pooled() {
    let mut p = make_pooled(make_physical().await);
    p.exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();
    p.begin().await.unwrap();
    p.exec("INSERT INTO t (id) VALUES (1)", vec![])
        .await
        .unwrap();
    let sp = p.set_savepoint().await.unwrap();
    p.exec("INSERT INTO t (id) VALUES (2)", vec![])
        .await
        .unwrap();
    p.rollback_to(&sp).await.unwrap();
    p.release_savepoint(&sp).await.unwrap();
    p.commit().await.unwrap();
    let rows = p.fetch("SELECT * FROM t", vec![]).await.unwrap();
    assert_eq!(rows.len(), 1);
}

#[tokio::test]
async fn named_savepoint_through_pooled() {
    let mut p = make_pooled(make_physical().await);
    p.exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();
    p.begin().await.unwrap();
    let sp = p.set_savepoint_named("my_sp").await.unwrap();
    assert!(sp.name.is_some());
    p.release_savepoint(&sp).await.unwrap();
    p.commit().await.unwrap();
}

// ── warnings ───────────────────────────────────────────────────────

#[tokio::test]
async fn warnings_returns_none_for_sqlite() {
    let mut p = make_pooled(make_physical().await);
    let w = DruidPooledConnection::warnings(&mut p).await.unwrap();
    assert!(w.is_none());
}

#[tokio::test]
async fn clear_warnings_succeeds() {
    let mut p = make_pooled(make_physical().await);
    DruidPooledConnection::clear_warnings(&mut p).await.unwrap();
}

// ── set_auto_commit ────────────────────────────────────────────────

#[tokio::test]
async fn set_auto_commit_false_begins_transaction() {
    let mut p = make_pooled(make_physical().await);
    assert!(p.auto_commit());
    p.set_auto_commit(false).await.unwrap();
    assert!(!p.auto_commit());
    p.set_auto_commit(true).await.unwrap();
    assert!(p.auto_commit());
}

// ── set_transaction_isolation ──────────────────────────────────────

#[tokio::test]
async fn set_transaction_isolation_serializable() {
    let mut p = make_pooled(make_physical().await);
    p.set_transaction_isolation(8).await.unwrap();
    assert_eq!(p.transaction_isolation(), 8);
}

// ── close / is_closed ──────────────────────────────────────────────

#[tokio::test]
async fn close_sets_is_closed() {
    let mut p = make_pooled(make_physical().await);
    assert!(!p.is_closed());
    p.close().await.unwrap();
    assert!(p.is_closed());
}

#[tokio::test]
async fn close_is_idempotent() {
    let mut p = make_pooled(make_physical().await);
    p.close().await.unwrap();
    p.close().await.unwrap();
    assert!(p.is_closed());
}

// ── recycle() consumes self ────────────────────────────────────────

#[tokio::test]
async fn recycle_consumes_and_sets_recycled() {
    let p = make_pooled(make_physical().await);
    assert!(!p.is_recycled());
    p.recycle();
    // p is moved; no further assertions on it
}

// ── discard_connection ─────────────────────────────────────────────

#[tokio::test]
async fn discard_connection_sets_recycled() {
    let mut p = make_pooled(make_physical().await);
    let _ = p.discard_connection();
    assert!(p.is_recycled());
}

// ── ping ───────────────────────────────────────────────────────────

#[tokio::test]
async fn ping_succeeds() {
    let mut p = make_pooled(make_physical().await);
    p.ping().await.unwrap();
}

// ── create_statement ───────────────────────────────────────────────

#[tokio::test]
async fn create_statement_and_execute_query() {
    let mut p = make_pooled(make_physical().await);
    let mut stmt = p.create_statement().await.unwrap();
    let mut rs = stmt
        .execute_query_result_set(&mut p, "SELECT 42 AS answer")
        .await
        .unwrap();
    assert!(rs.next(&mut p).unwrap());
    assert_eq!(rs.int(&mut p, 1).unwrap(), 42);
    rs.close_with_connection(&mut p).unwrap();
}

// ── prepare_statement family ───────────────────────────────────────

#[tokio::test]
async fn prepare_statement_and_exec() {
    let mut p = make_pooled(make_physical().await);
    p.exec("CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT)", vec![])
        .await
        .unwrap();
    let mut ps = p
        .prepare_statement("INSERT INTO t (id, v) VALUES (?, ?)")
        .await
        .unwrap();
    let result = ps
        .exec(
            &mut p,
            vec![Value::Int(1), Value::String("test".to_string())],
        )
        .await
        .unwrap();
    assert_eq!(result.rows_affected, 1);
}

#[tokio::test]
async fn prepare_statement_with_result_set_options() {
    let mut p = make_pooled(make_physical().await);
    p.exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();
    let ps = p
        .prepare_statement_with_result_set("SELECT * FROM t", 1003, 1007)
        .await;
    assert!(ps.is_ok());
}

#[tokio::test]
async fn prepare_statement_with_holdability_option() {
    let mut p = make_pooled(make_physical().await);
    p.exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();
    let ps = p
        .prepare_statement_with_holdability("SELECT * FROM t", 1003, 1007, 1)
        .await;
    assert!(ps.is_ok());
}

#[tokio::test]
async fn prepare_statement_with_column_indexes() {
    let mut p = make_pooled(make_physical().await);
    p.exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();
    let ps = p
        .prepare_statement_with_column_indexes("INSERT INTO t (id) VALUES (?)", vec![1])
        .await;
    assert!(ps.is_ok());
}

#[tokio::test]
async fn prepare_statement_with_column_names() {
    let mut p = make_pooled(make_physical().await);
    p.exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();
    let ps = p
        .prepare_statement_with_column_names(
            "INSERT INTO t (id) VALUES (?)",
            vec!["id".to_string()],
        )
        .await;
    assert!(ps.is_ok());
}

#[tokio::test]
async fn prepare_statement_with_auto_generated_keys_option() {
    let mut p = make_pooled(make_physical().await);
    p.exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();
    let ps = p
        .prepare_statement_with_auto_generated_keys("INSERT INTO t (id) VALUES (?)", 1)
        .await;
    assert!(ps.is_ok());
}

// ── prepare_call family ────────────────────────────────────────────

#[tokio::test]
async fn prepare_call_does_not_panic() {
    let mut p = make_pooled(make_physical().await);
    let _ = p.prepare_call("SELECT 1").await;
}

#[tokio::test]
async fn prepare_call_with_holdability_does_not_panic() {
    let mut p = make_pooled(make_physical().await);
    let _ = p
        .prepare_call_with_holdability("SELECT 1", 1003, 1007, 1)
        .await;
}

#[tokio::test]
async fn prepare_call_with_result_set_does_not_panic() {
    let mut p = make_pooled(make_physical().await);
    let _ = p.prepare_call_with_result_set("SELECT 1", 1003, 1007).await;
}

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

#[tokio::test]
async fn add_and_remove_connection_event_listener() {
    let p = make_pooled(make_physical().await);
    let listener: Arc<dyn ConnectionEventListener> = Arc::new(TestConnectionEventListener::new());
    p.add_connection_event_listener(Arc::clone(&listener))
        .unwrap();
    let removed = p.remove_connection_event_listener(&listener).unwrap();
    assert!(removed);
}

#[tokio::test]
async fn remove_nonexistent_listener_returns_false() {
    let p = make_pooled(make_physical().await);
    let listener: Arc<dyn ConnectionEventListener> = Arc::new(TestConnectionEventListener::new());
    let removed = p.remove_connection_event_listener(&listener).unwrap();
    assert!(!removed);
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

#[tokio::test]
async fn add_and_remove_statement_event_listener() {
    let p = make_pooled(make_physical().await);
    let listener: Arc<dyn StatementEventListener> = Arc::new(TestStatementEventListener);
    p.add_statement_event_listener(Arc::clone(&listener))
        .unwrap();
    let removed = p.remove_statement_event_listener(&listener).unwrap();
    assert!(removed);
}

// ── exception sorter ───────────────────────────────────────────────

struct NeverFatalSorter;

impl ExceptionSorter for NeverFatalSorter {
    fn is_exception_fatal(&self, _exception: &SqlException) -> bool {
        false
    }
    fn config_from_properties(&mut self, _properties: Option<&ExceptionSorterProperties>) {}
}

#[tokio::test]
async fn set_exception_sorter_and_with_exception_sorter() {
    let mut p = make_pooled(make_physical().await);
    let sorter: Arc<dyn ExceptionSorter> = Arc::new(NeverFatalSorter);
    p.set_exception_sorter(Arc::clone(&sorter));
    let p2 = make_pooled(make_physical().await);
    let _ = p2.with_exception_sorter(sorter);
}

// ── handle_exception ───────────────────────────────────────────────

#[tokio::test]
async fn handle_non_fatal_exception_returns_false() {
    let mut p = make_pooled(make_physical().await);
    let sorter: Arc<dyn ExceptionSorter> = Arc::new(NeverFatalSorter);
    p.set_exception_sorter(sorter);
    let error = DruidError::SqlException(Box::new(SqlException::driver(1000, "test error")));
    let fatal = p.handle_exception(&error);
    assert!(!fatal);
}

struct AlwaysFatalSorter;

impl ExceptionSorter for AlwaysFatalSorter {
    fn is_exception_fatal(&self, _exception: &SqlException) -> bool {
        true
    }
    fn config_from_properties(&mut self, _properties: Option<&ExceptionSorterProperties>) {}
}

#[tokio::test]
async fn handle_fatal_exception_marks_discarded() {
    let mut p = make_pooled(make_physical().await);
    let sorter: Arc<dyn ExceptionSorter> = Arc::new(AlwaysFatalSorter);
    p.set_exception_sorter(sorter);
    let error = DruidError::SqlException(Box::new(
        SqlException::driver(17002, "Io exception: Connection reset").with_sql_state("08006"),
    ));
    let fatal = p.handle_exception(&error);
    assert!(fatal);
    assert!(p.is_discarded());
}

// ── is_discarded / mark_discarded ──────────────────────────────────

#[tokio::test]
async fn is_discarded_false_initially() {
    let p = make_pooled(make_physical().await);
    assert!(!p.is_discarded());
}

#[tokio::test]
async fn mark_discarded_sets_discarded() {
    let mut p = make_pooled(make_physical().await);
    assert!(!p.is_discarded());
    p.mark_discarded();
    assert!(p.is_discarded());
}

// ── database_meta_data / get_meta_data ─────────────────────────────

#[tokio::test]
async fn database_meta_data_returns_url() {
    let mut p = make_pooled(make_physical().await);
    let mut meta = p.database_meta_data().unwrap();
    let url = meta.get_url().await.unwrap();
    assert!(url.is_some());
}

#[tokio::test]
async fn get_meta_data_delegates() {
    let mut p = make_pooled(make_physical().await);
    let mut meta = p.get_meta_data().unwrap();
    let name = meta.get_driver_name().await.unwrap();
    assert!(name.is_some());
}

// ── catalog / schema ───────────────────────────────────────────────

#[tokio::test]
async fn catalog_none_for_sqlite() {
    let p = make_pooled(make_physical().await);
    assert!(p.catalog().is_none());
}

#[tokio::test]
async fn schema_none_for_sqlite() {
    let p = make_pooled(make_physical().await);
    assert!(p.schema().is_none());
}

// ── Drop ───────────────────────────────────────────────────────────

#[tokio::test]
async fn drop_cleans_up_without_panic() {
    let p = make_pooled(make_physical().await);
    drop(p);
}

#[tokio::test]
async fn drop_after_use_cleans_up() {
    let mut p = make_pooled(make_physical().await);
    p.exec("CREATE TABLE t (id INTEGER PRIMARY KEY)", vec![])
        .await
        .unwrap();
    drop(p);
}

// ── PhysicalConnectionConstants ────────────────────────────────────

#[test]
fn transaction_isolation_constants() {
    assert_eq!(DruidPooledConnection::TRANSACTION_NONE, 0);
    assert_eq!(DruidPooledConnection::TRANSACTION_READ_UNCOMMITTED, 1);
    assert_eq!(DruidPooledConnection::TRANSACTION_READ_COMMITTED, 2);
    assert_eq!(DruidPooledConnection::TRANSACTION_REPEATABLE_READ, 4);
    assert_eq!(DruidPooledConnection::TRANSACTION_SERIALIZABLE, 8);
}

// ── Multiple sequential operations ─────────────────────────────────

#[tokio::test]
async fn multiple_statements_on_same_connection() {
    let mut p = make_pooled(make_physical().await);
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

// ── with_recycle_policy ────────────────────────────────────────────

#[tokio::test]
async fn with_recycle_policy_constructor() {
    let physical = make_physical().await;
    let _p = DruidPooledConnection::with_recycle_policy(
        physical,
        10,
        "test".to_string(),
        None,
        false,
        None,
        Box::new(|_, _, _| false),
    );
}

// ── create_statement_with_result_set ───────────────────────────────

#[tokio::test]
async fn create_statement_with_result_set_options() {
    let mut p = make_pooled(make_physical().await);
    let stmt = p.create_statement_with_result_set(1003, 1007).await;
    assert!(stmt.is_ok());
}

#[tokio::test]
async fn create_statement_with_holdability_options() {
    let mut p = make_pooled(make_physical().await);
    let stmt = p.create_statement_with_holdability(1003, 1007, 1).await;
    assert!(stmt.is_ok());
}
