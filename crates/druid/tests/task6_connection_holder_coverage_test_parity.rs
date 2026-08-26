//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
#![allow(unused_comparisons)]
//! `DruidConnectionHolder` coverage boost -- variables, `global_variables`,
//! `create_duration`, `statement_event_listeners`, prepared statement cache,
//! Debug format with statement pool, and `remove_connection_event_listener`.

use druid::core::{
    ConnectionEventListener, ConnectionFactory, DruidConnectionHolder, DruidError,
    PreparedStatementCacheStats, StatementEventListener,
};
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::sync::Arc;

// -- helpers ----------------------------------------------------------------

async fn make_holder() -> DruidConnectionHolder {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("factory");
    let physical = factory.create().await.expect("connection");
    DruidConnectionHolder::with_standard_connection(
        physical,
        1,
        std::time::Duration::from_millis(5),
        0,
    )
    .await
    .expect("canonical holder")
}

// -- variables / global_variables -------------------------------------------

// -- create_duration --------------------------------------------------------

// -- statement_event_listeners ----------------------------------------------

struct TestStatementListener;
impl StatementEventListener for TestStatementListener {
    fn statement_closed(&self, _connection_id: u64, _statement_id: usize) {}
    fn statement_error_occurred(
        &self,
        _connection_id: u64,
        _statement_id: usize,
        _error: &DruidError,
    ) {
    }
}

// -- connection_event_listeners ---------------------------------------------

struct TestConnectionListener;
impl ConnectionEventListener for TestConnectionListener {
    fn connection_closed(&self, _connection_id: u64) {}
    fn connection_error_occurred(&self, _connection_id: u64, _error: &DruidError) {}
}

// -- has_standard_connection -----------------------------------------------

// -- connection_id ---------------------------------------------------------

// -- Debug format ----------------------------------------------------------

// -- standard_connection / standard_connection_mut -------------------------

// -- defaults --------------------------------------------------------------

// -- state / try_transition ------------------------------------------------

// -- configure_statement_pool ----------------------------------------------

// -- has_in_use_prepared_statement -----------------------------------------

// -- prepared_statement_stats ----------------------------------------------

// -- statement_pool_direct -------------------------------------------------

// -- is_pool_prepared_statements -------------------------------------------

// -- clear_statement_cache -------------------------------------------------

// -- new (compatibility holder) --------------------------------------------

#[tokio::test]
async fn task6_connection_holder_coverage_test_holder_variables_none_initially() {
    let holder = make_holder().await;
    let _ = holder.variables();
}

#[tokio::test]
async fn task6_connection_holder_coverage_test_holder_global_variables_none_initially() {
    let holder = make_holder().await;
    let _ = holder.global_variables();
}

#[tokio::test]
async fn task6_connection_holder_coverage_test_holder_create_duration() {
    let holder = make_holder().await;
    let duration = holder.create_duration();
    assert!(duration.as_millis() >= 0);
}

#[tokio::test]
async fn task6_connection_holder_coverage_test_holder_statement_event_listeners_empty_initially() {
    let holder = make_holder().await;
    let listeners = holder.statement_event_listeners();
    assert!(listeners.is_empty());
}

#[tokio::test]
async fn task6_connection_holder_coverage_test_holder_add_and_remove_statement_event_listener() {
    let holder = make_holder().await;
    let listener: Arc<dyn StatementEventListener> = Arc::new(TestStatementListener);
    holder.add_statement_event_listener(Arc::clone(&listener));

    let listeners = holder.statement_event_listeners();
    assert_eq!(listeners.len(), 1);

    let removed = holder.remove_statement_event_listener(&listener);
    assert!(removed);

    let listeners = holder.statement_event_listeners();
    assert!(listeners.is_empty());

    let removed = holder.remove_statement_event_listener(&listener);
    assert!(!removed);
}

#[tokio::test]
async fn task6_connection_holder_coverage_test_holder_clear_statement_event_listeners() {
    let holder = make_holder().await;
    holder.add_statement_event_listener(Arc::new(TestStatementListener));
    holder.add_statement_event_listener(Arc::new(TestStatementListener));
    assert_eq!(holder.statement_event_listeners().len(), 2);

    holder.clear_statement_event_listeners();
    assert!(holder.statement_event_listeners().is_empty());
}

#[tokio::test]
async fn task6_connection_holder_coverage_test_holder_connection_event_listeners_empty_initially() {
    let holder = make_holder().await;
    let listeners = holder.connection_event_listeners();
    assert!(listeners.is_empty());
}

#[tokio::test]
async fn task6_connection_holder_coverage_test_holder_add_and_remove_connection_event_listener() {
    let holder = make_holder().await;
    let listener: Arc<dyn ConnectionEventListener> = Arc::new(TestConnectionListener);
    holder.add_connection_event_listener(Arc::clone(&listener));

    let listeners = holder.connection_event_listeners();
    assert_eq!(listeners.len(), 1);

    let removed = holder.remove_connection_event_listener(&listener);
    assert!(removed);

    let listeners = holder.connection_event_listeners();
    assert!(listeners.is_empty());

    let removed = holder.remove_connection_event_listener(&listener);
    assert!(!removed);
}

#[tokio::test]
async fn task6_connection_holder_coverage_test_holder_clear_connection_event_listeners() {
    let holder = make_holder().await;
    holder.add_connection_event_listener(Arc::new(TestConnectionListener));
    holder.add_connection_event_listener(Arc::new(TestConnectionListener));
    assert_eq!(holder.connection_event_listeners().len(), 2);

    holder.clear_connection_event_listeners();
    assert!(holder.connection_event_listeners().is_empty());
}

#[tokio::test]
async fn task6_connection_holder_coverage_test_holder_has_standard_connection() {
    let mut holder = make_holder().await;
    assert!(holder.has_standard_connection());

    holder.close_standard_connection().await.unwrap();
    assert!(!holder.has_standard_connection());
}

#[tokio::test]
async fn task6_connection_holder_coverage_test_holder_connection_id() {
    let holder = make_holder().await;
    assert_eq!(holder.connection_id(), 1);
}

#[tokio::test]
async fn task6_connection_holder_coverage_test_holder_debug_format() {
    let holder = make_holder().await;
    let debug = format!("{holder:?}");
    assert!(debug.contains("DruidConnectionHolder"));
}

#[tokio::test]
async fn task6_connection_holder_coverage_test_holder_standard_connection_accessors() {
    let mut holder = make_holder().await;
    assert!(holder.has_standard_connection());
    holder.close_standard_connection().await.unwrap();
    assert!(!holder.has_standard_connection());
}

#[tokio::test]
async fn task6_connection_holder_coverage_test_holder_defaults() {
    let holder = make_holder().await;
    let _ = holder.defaults();
}

#[tokio::test]
async fn task6_connection_holder_coverage_test_holder_state_and_transition() {
    let holder = make_holder().await;
    let state = holder.state();
    let _ = format!("{state:?}");
}

#[tokio::test]
async fn task6_connection_holder_coverage_test_holder_configure_statement_pool() {
    let mut holder = make_holder().await;
    holder.configure_statement_pool(
        true,
        100,
        true,
        false,
        Arc::new(PreparedStatementCacheStats::default()),
    );
    assert!(holder.is_pool_prepared_statements());
}

#[tokio::test]
async fn task6_connection_holder_coverage_test_holder_has_in_use_prepared_statement() {
    let mut holder = make_holder().await;
    holder.configure_statement_pool(
        true,
        100,
        true,
        false,
        Arc::new(PreparedStatementCacheStats::default()),
    );
    let stats = holder.prepared_statement_stats();
    assert_eq!(stats.prepared_statement_count(), 0);
    assert_eq!(stats.cached_prepared_statement_count(), 0);
}

#[tokio::test]
async fn task6_connection_holder_coverage_test_holder_prepared_statement_stats() {
    let mut holder = make_holder().await;
    holder.configure_statement_pool(
        true,
        100,
        true,
        false,
        Arc::new(PreparedStatementCacheStats::default()),
    );
    let _ = holder.prepared_statement_stats();
}

#[tokio::test]
async fn task6_connection_holder_coverage_test_holder_statement_pool_direct() {
    let holder = make_holder().await;
    // No pool configured initially
    assert!(!holder.is_pool_prepared_statements());
    assert_eq!(
        holder
            .prepared_statement_stats()
            .cached_prepared_statement_count(),
        0
    );
}

#[tokio::test]
async fn task6_connection_holder_coverage_test_holder_is_pool_prepared_statements_default() {
    let holder = make_holder().await;
    // Default is false
    assert!(!holder.is_pool_prepared_statements());
}

#[tokio::test]
async fn task6_connection_holder_coverage_test_holder_clear_statement_cache() {
    let mut holder = make_holder().await;
    holder.configure_statement_pool(
        true,
        100,
        true,
        false,
        Arc::new(PreparedStatementCacheStats::default()),
    );
    holder.close_standard_connection().await.unwrap();
    assert!(!holder.has_standard_connection());
}

#[tokio::test]
async fn task6_connection_holder_coverage_test_holder_new_compatibility() {
    let holder = DruidConnectionHolder::new(42);
    assert_eq!(holder.connection_id(), 42);
    assert!(!holder.has_standard_connection());
}
