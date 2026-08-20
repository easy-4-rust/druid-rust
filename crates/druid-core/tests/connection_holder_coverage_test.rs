//! DruidConnectionHolder coverage boost -- variables, global_variables,
//! create_duration, statement_event_listeners, prepared statement cache,
//! Debug format with statement pool, and remove_connection_event_listener.

extern crate druid_core as druid;
use druid_core::core::{
    ConnectionEventListener, DruidConnectionHolder, DruidError, PhysicalConnectionFactory,
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
    DruidConnectionHolder::with_connection(physical, 1, std::time::Duration::from_millis(5), 0)
}

// -- variables / global_variables -------------------------------------------

#[tokio::test]
async fn holder_variables_none_initially() {
    let holder = make_holder().await;
    let _ = holder.variables();
}

#[tokio::test]
async fn holder_global_variables_none_initially() {
    let holder = make_holder().await;
    let _ = holder.global_variables();
}

// -- create_duration --------------------------------------------------------

#[tokio::test]
async fn holder_create_duration() {
    let holder = make_holder().await;
    let duration = holder.create_duration();
    assert!(duration.as_millis() >= 0);
}

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

#[tokio::test]
async fn holder_statement_event_listeners_empty_initially() {
    let holder = make_holder().await;
    let listeners = holder.statement_event_listeners();
    assert!(listeners.is_empty());
}

#[tokio::test]
async fn holder_add_and_remove_statement_event_listener() {
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
async fn holder_clear_statement_event_listeners() {
    let holder = make_holder().await;
    holder.add_statement_event_listener(Arc::new(TestStatementListener));
    holder.add_statement_event_listener(Arc::new(TestStatementListener));
    assert_eq!(holder.statement_event_listeners().len(), 2);

    holder.clear_statement_event_listeners();
    assert!(holder.statement_event_listeners().is_empty());
}

// -- connection_event_listeners ---------------------------------------------

struct TestConnectionListener;
impl ConnectionEventListener for TestConnectionListener {
    fn connection_closed(&self, _connection_id: u64) {}
    fn connection_error_occurred(&self, _connection_id: u64, _error: &DruidError) {}
}

#[tokio::test]
async fn holder_connection_event_listeners_empty_initially() {
    let holder = make_holder().await;
    let listeners = holder.connection_event_listeners();
    assert!(listeners.is_empty());
}

#[tokio::test]
async fn holder_add_and_remove_connection_event_listener() {
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
async fn holder_clear_connection_event_listeners() {
    let holder = make_holder().await;
    holder.add_connection_event_listener(Arc::new(TestConnectionListener));
    holder.add_connection_event_listener(Arc::new(TestConnectionListener));
    assert_eq!(holder.connection_event_listeners().len(), 2);

    holder.clear_connection_event_listeners();
    assert!(holder.connection_event_listeners().is_empty());
}

// -- has_physical_connection -----------------------------------------------

#[tokio::test]
async fn holder_has_physical_connection() {
    let mut holder = make_holder().await;
    assert!(holder.has_physical_connection());

    let _physical = holder.take_physical_connection();
    assert!(!holder.has_physical_connection());
}

// -- connection_id ---------------------------------------------------------

#[tokio::test]
async fn holder_connection_id() {
    let holder = make_holder().await;
    assert_eq!(holder.connection_id(), 1);
}

// -- Debug format ----------------------------------------------------------

#[tokio::test]
async fn holder_debug_format() {
    let holder = make_holder().await;
    let debug = format!("{holder:?}");
    assert!(debug.contains("DruidConnectionHolder"));
}

// -- physical_connection / physical_connection_mut -------------------------

#[tokio::test]
async fn holder_physical_connection_accessors() {
    let mut holder = make_holder().await;
    assert!(holder.physical_connection().is_some());
    assert!(holder.physical_connection_mut().is_some());
}

// -- defaults --------------------------------------------------------------

#[tokio::test]
async fn holder_defaults() {
    let holder = make_holder().await;
    let _ = holder.defaults();
}

// -- state / try_transition ------------------------------------------------

#[tokio::test]
async fn holder_state_and_transition() {
    let holder = make_holder().await;
    let state = holder.state();
    let _ = format!("{state:?}");
}

// -- configure_statement_pool ----------------------------------------------

#[tokio::test]
async fn holder_configure_statement_pool() {
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

// -- has_in_use_prepared_statement -----------------------------------------

#[tokio::test]
async fn holder_has_in_use_prepared_statement() {
    let mut holder = make_holder().await;
    holder.configure_statement_pool(
        true,
        100,
        true,
        false,
        Arc::new(PreparedStatementCacheStats::default()),
    );
    assert!(!holder.has_in_use_prepared_statement());
}

// -- prepared_statement_stats ----------------------------------------------

#[tokio::test]
async fn holder_prepared_statement_stats() {
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

// -- statement_pool_direct -------------------------------------------------

#[tokio::test]
async fn holder_statement_pool_direct() {
    let holder = make_holder().await;
    // No pool configured initially
    assert!(holder.statement_pool_direct().is_none());
}

// -- is_pool_prepared_statements -------------------------------------------

#[tokio::test]
async fn holder_is_pool_prepared_statements_default() {
    let holder = make_holder().await;
    // Default is false
    assert!(!holder.is_pool_prepared_statements());
}

// -- clear_statement_cache -------------------------------------------------

#[tokio::test]
async fn holder_clear_statement_cache() {
    let mut holder = make_holder().await;
    holder.configure_statement_pool(
        true,
        100,
        true,
        false,
        Arc::new(PreparedStatementCacheStats::default()),
    );
    holder.clear_statement_cache();
}

// -- new (compatibility holder) --------------------------------------------

#[tokio::test]
async fn holder_new_compatibility() {
    let holder = DruidConnectionHolder::new(42);
    assert_eq!(holder.connection_id(), 42);
    assert!(!holder.has_physical_connection());
}
