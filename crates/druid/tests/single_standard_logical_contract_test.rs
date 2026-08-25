mod standard_test_support;

use druid::core::{
    DruidConnectionHolder, DruidPooledCallableStatement, DruidPooledConnection,
    DruidPooledPreparedStatement, DruidPooledResultSet, DruidPooledStatement,
};
use druid::pool::config::PoolInnerConfig;
use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::metadata::DatabaseMetaData;
use druid_rdbc::result_set::{ColumnIndex, ResultSet};
use druid_rdbc::statement::{
    CallableStatement, GeneratedKeys, ParameterIndex, PreparedParameter, PreparedStatement,
    Statement, StatementOptions,
};
use druid_rdbc::types::Value;
use standard_test_support::{TrackingFactory, TrackingState};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

fn assert_connection<T: Connection>() {}
fn assert_statement<T: Statement>() {}
fn assert_prepared<T: PreparedStatement>() {}
fn assert_callable<T: CallableStatement>() {}
fn assert_result_set<T: ResultSet>() {}
fn assert_metadata<T: DatabaseMetaData>() {}

#[test]
fn all_druid_logical_proxies_implement_the_canonical_standard() {
    assert_connection::<DruidPooledConnection>();
    assert_statement::<DruidPooledStatement>();
    assert_prepared::<DruidPooledPreparedStatement>();
    assert_callable::<DruidPooledCallableStatement>();
    assert_result_set::<DruidPooledResultSet>();
    assert_metadata::<druid::core::DatabaseMetaDataProxyImpl>();
}

#[tokio::test]
async fn standard_factory_holder_and_logical_handles_execute_one_owned_chain() {
    let state = TrackingState::configured();
    let factory = TrackingFactory::new(state.clone());
    let raw = factory
        .create()
        .await
        .expect("standard factory must create");
    let holder =
        DruidConnectionHolder::with_standard_connection(raw, 41, Duration::from_millis(3), 0)
            .expect("holder must capture standard defaults");
    let mut connection = DruidPooledConnection::with_standard_holder(
        holder,
        "task5-native".to_string(),
        None,
        false,
        false,
        Box::new(|_holder, _disposition| false),
    );

    let mut statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .expect("logical statement must be created");
    assert_eq!(
        statement
            .execute_update("UPDATE item SET value = 1", GeneratedKeys::None)
            .await
            .expect("standard update must execute"),
        1
    );
    let mut result = statement
        .execute_query("SELECT value FROM item")
        .await
        .expect("standard query must execute");
    assert!(result.next().await.expect("result set must advance"));
    assert_eq!(
        result
            .value(ColumnIndex::new(1).expect("1-based index"))
            .expect("value must exist"),
        Value::Int(11)
    );
    result.close().await.expect("result set close must work");
    statement.close().await.expect("statement close must work");

    let mut prepared =
        Connection::prepare_statement(&mut connection, "SELECT ?", StatementOptions::default())
            .await
            .expect("logical prepared statement must be created");
    prepared
        .set_parameter(PreparedParameter {
            index: ParameterIndex::new(1).expect("1-based parameter"),
            value: Value::Int(22),
        })
        .expect("parameter must be delegated");
    let mut prepared_result = prepared
        .execute_prepared_query()
        .await
        .expect("prepared query must execute");
    assert!(prepared_result.next().await.expect("prepared row"));
    prepared_result
        .close()
        .await
        .expect("prepared result close");
    prepared.close().await.expect("prepared close/cache return");

    let mut cached =
        Connection::prepare_statement(&mut connection, "SELECT ?", StatementOptions::default())
            .await
            .expect("cached logical prepared statement must be created");
    cached.close().await.expect("cached statement close");

    assert_eq!(state.factory_create_count.load(Ordering::Acquire), 1);
    let events = state.event_snapshot();
    assert!(events
        .iter()
        .any(|event| event == "prepared.query:SELECT ?"));
    assert_eq!(
        events
            .iter()
            .filter(|event| event.as_str() == "connection.prepare:SELECT ?")
            .count(),
        1,
        "standard prepared cache must own and reuse one PreparedStatement"
    );
}

#[tokio::test]
async fn native_pool_factory_output_enters_a_standard_holder() {
    let state = TrackingState::configured();
    let factory: std::sync::Arc<dyn ConnectionFactory> =
        std::sync::Arc::new(TrackingFactory::new(state.clone()));
    let config = PoolInnerConfig {
        max_open: 1,
        max_idle: 1,
        acquire_timeout: Duration::from_secs(2),
        pool_prepared_statements: true,
        max_pool_prepared_statements_per_connection: 1,
        ..PoolInnerConfig::default()
    };
    let pool = DruidPool::new_standard(
        "task5-standard-pool".to_string(),
        "task5-tracking-standard".to_string(),
        factory,
        config,
        None,
    );
    pool.init().await.expect("standard native pool init");
    let mut logical = pool.get().await.expect("standard native pool get");
    let mut statement = Connection::create_statement(&mut logical, StatementOptions::default())
        .await
        .expect("pooled standard statement");
    assert_eq!(
        statement
            .execute_update("UPDATE item SET value = 2", GeneratedKeys::None)
            .await
            .expect("pooled update"),
        1
    );
    statement.close().await.expect("pooled statement close");
    let mut prepared = Connection::prepare_statement(
        &mut logical,
        "SELECT cached_for_shutdown",
        StatementOptions::default(),
    )
    .await
    .expect("pooled prepared statement");
    prepared
        .close()
        .await
        .expect("prepared close returns raw handle to cache");
    Connection::close(&mut logical)
        .await
        .expect("logical close returns native holder");
    pool.close().await;
    assert_eq!(state.raw_close_count.load(Ordering::Acquire), 1);
    assert_eq!(state.prepared_close_count.load(Ordering::Acquire), 1);
}

#[tokio::test]
async fn stale_prepared_close_cannot_enter_the_next_logical_lease_cache() {
    let state = TrackingState::configured();
    let holder = DruidConnectionHolder::with_standard_connection(
        Box::new(standard_test_support::TrackingConnection::new(
            state.clone(),
        )),
        42,
        Duration::ZERO,
        0,
    )
    .expect("standard holder");
    let returned = Arc::new(Mutex::new(None));
    let first_returned = returned.clone();
    let mut first = DruidPooledConnection::with_standard_holder(
        holder,
        "task5-first-lease".to_string(),
        None,
        false,
        false,
        Box::new(move |holder, _disposition| {
            *first_returned
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(holder);
            false
        }),
    );
    let mut stale = Connection::prepare_statement(
        &mut first,
        "SELECT stale_cache",
        StatementOptions::default(),
    )
    .await
    .expect("first prepared statement");
    Connection::close(&mut first)
        .await
        .expect("first lease close");
    stale
        .close()
        .await
        .expect("stale statement closes without caching");

    let holder = returned
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
        .expect("first lease returned holder");
    let mut second = DruidPooledConnection::with_standard_holder(
        holder,
        "task5-second-lease".to_string(),
        None,
        false,
        false,
        Box::new(|_holder, _disposition| false),
    );
    let mut fresh = Connection::prepare_statement(
        &mut second,
        "SELECT stale_cache",
        StatementOptions::default(),
    )
    .await
    .expect("second prepared statement");
    fresh.close().await.expect("fresh statement close");
    let prepare_count = state
        .event_snapshot()
        .into_iter()
        .filter(|event| event == "connection.prepare:SELECT stale_cache")
        .count();
    assert_eq!(prepare_count, 2);
}
