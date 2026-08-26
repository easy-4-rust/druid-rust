mod standard_test_support;

use druid::core::{
    DruidConnectionHolder, DruidPooledCallableStatement, DruidPooledConnection,
    DruidPooledPreparedStatement, DruidPooledResultSet, DruidPooledStatement,
    PreparedStatementCacheStats, RdbcError,
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
use standard_test_support::{TrackingFactory, TrackingState, UnsupportedStateFactory};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Barrier;

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
    let mut holder =
        DruidConnectionHolder::with_standard_connection(raw, 41, Duration::from_millis(3), 0)
            .await
            .expect("holder must capture standard defaults");
    holder.configure_statement_pool(
        true,
        10,
        false,
        false,
        Arc::new(PreparedStatementCacheStats::default()),
    );
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
    assert_eq!(state.raw_close_count.load(Ordering::Acquire), 0);

    let mut second = pool.get().await.expect("second standard native pool get");
    let mut cached = Connection::prepare_statement(
        &mut second,
        "SELECT cached_for_shutdown",
        StatementOptions::default(),
    )
    .await
    .expect("second lease reuses cached prepared statement");
    cached.close().await.expect("second prepared close");
    Connection::close(&mut second)
        .await
        .expect("second logical close returns same holder");
    assert_eq!(state.factory_create_count.load(Ordering::Acquire), 1);
    assert_eq!(state.raw_close_count.load(Ordering::Acquire), 0);

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
    .await
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

#[tokio::test]
async fn unsupported_standard_state_is_unmanaged_without_calling_its_setters() {
    let state = TrackingState::configured();
    let setter_attempts = Arc::new(AtomicUsize::new(0));
    let factory: Arc<dyn ConnectionFactory> = Arc::new(UnsupportedStateFactory::new(
        state,
        Arc::clone(&setter_attempts),
    ));
    let config = PoolInnerConfig {
        max_open: 1,
        max_idle: 1,
        acquire_timeout: Duration::from_millis(100),
        ..PoolInnerConfig::default()
    };
    let pool = DruidPool::new_standard(
        "task5-unmanaged-state".to_owned(),
        "unsupported-state".to_owned(),
        factory,
        config,
        None,
    );

    pool.init().await.expect("unsupported state pool init");
    let mut logical = pool
        .get()
        .await
        .expect("unsupported optional state must not reject admission");
    Connection::close(&mut logical)
        .await
        .expect("unmanaged state recycle");
    pool.close().await;
    assert_eq!(setter_attempts.load(Ordering::Acquire), 0);
}

#[tokio::test]
async fn standard_state_getter_driver_error_fails_admission_instead_of_becoming_unsupported() {
    let state = TrackingState::configured();
    state.fail_next("auto_commit_getter");
    let result = DruidConnectionHolder::with_standard_connection(
        Box::new(standard_test_support::TrackingConnection::new(state)),
        48,
        Duration::ZERO,
        0,
    )
    .await;
    assert!(matches!(
        result,
        Err(druid::core::DruidError::Rdbc(RdbcError::Other(message)))
            if message == "forced auto_commit_getter failure"
    ));
}

#[tokio::test]
async fn standard_test_on_return_validates_and_discards_once() {
    let state = TrackingState::configured();
    let factory: Arc<dyn ConnectionFactory> = Arc::new(TrackingFactory::new(Arc::clone(&state)));
    let config = PoolInnerConfig {
        max_open: 1,
        max_idle: 1,
        acquire_timeout: Duration::from_millis(200),
        test_on_return: true,
        ..PoolInnerConfig::default()
    };
    let pool = DruidPool::new_standard(
        "task5-standard-return-validation".to_owned(),
        "task5-tracking-standard".to_owned(),
        factory,
        config,
        None,
    );

    pool.init().await.expect("standard validation pool init");
    let mut first = pool.get().await.expect("first standard borrow");
    state.fail_next("is_valid");
    Connection::close(&mut first)
        .await
        .expect("validation failure folds into discard");
    let mut second = pool.get().await.expect("replacement standard borrow");
    assert_eq!(state.factory_create_count.load(Ordering::Acquire), 2);
    assert_eq!(state.is_valid_attempt_count.load(Ordering::Acquire), 1);
    Connection::close(&mut second)
        .await
        .expect("replacement close");
    pool.close().await;
    assert_eq!(state.raw_close_count.load(Ordering::Acquire), 2);
}

#[tokio::test]
async fn standard_idle_expiry_destroys_once_and_creates_a_replacement() {
    let state = TrackingState::configured();
    let factory: Arc<dyn ConnectionFactory> = Arc::new(TrackingFactory::new(Arc::clone(&state)));
    let config = PoolInnerConfig {
        max_open: 1,
        max_idle: 1,
        min_idle: 0,
        idle_timeout: Duration::ZERO,
        acquire_timeout: Duration::from_millis(200),
        ..PoolInnerConfig::default()
    };
    let pool = DruidPool::new_standard(
        "task5-standard-idle-expiry".to_owned(),
        "task5-tracking-standard".to_owned(),
        factory,
        config,
        None,
    );

    pool.init().await.expect("idle expiry pool init");
    let mut first = pool.get().await.expect("first borrow");
    Connection::close(&mut first).await.expect("first return");
    let mut second = pool.get().await.expect("replacement borrow");
    assert_eq!(state.factory_create_count.load(Ordering::Acquire), 2);
    Connection::close(&mut second).await.expect("second return");
    pool.close().await;
    assert_eq!(state.raw_close_attempt_count.load(Ordering::Acquire), 2);
}

#[tokio::test]
async fn standard_max_idle_remains_a_compatibility_field_not_a_return_cap() {
    let state = TrackingState::configured();
    let factory: Arc<dyn ConnectionFactory> = Arc::new(TrackingFactory::new(Arc::clone(&state)));
    let config = PoolInnerConfig {
        max_open: 2,
        max_idle: 0,
        acquire_timeout: Duration::from_millis(200),
        ..PoolInnerConfig::default()
    };
    let pool = DruidPool::new_standard(
        "task5-standard-max-idle".to_owned(),
        "task5-tracking-standard".to_owned(),
        factory,
        config,
        None,
    );

    pool.init().await.expect("maxIdle pool init");
    let mut first = pool.get().await.expect("first active");
    let mut second = pool.get().await.expect("second active");
    Connection::close(&mut first).await.expect("first idle");
    Connection::close(&mut second).await.expect("second idle");
    let mut reused_first = pool.get().await.expect("first reused idle");
    let mut reused_second = pool.get().await.expect("second reused idle");
    assert_eq!(state.factory_create_count.load(Ordering::Acquire), 2);
    Connection::close(&mut reused_first)
        .await
        .expect("first reused return");
    Connection::close(&mut reused_second)
        .await
        .expect("second reused return");
    pool.close().await;
    assert_eq!(state.raw_close_attempt_count.load(Ordering::Acquire), 2);
}

#[tokio::test]
async fn disabled_standard_prepared_cache_closes_each_raw_statement() {
    let (prepare_count, close_count) = standard_cache_counts(false, 10).await;
    assert_eq!(prepare_count, 2);
    assert_eq!(close_count, 2);
}

#[tokio::test]
async fn zero_capacity_disables_standard_prepared_cache_even_when_flag_is_true() {
    let (prepare_count, close_count) = standard_cache_counts(true, 0).await;
    assert_eq!(prepare_count, 2);
    assert_eq!(close_count, 2);
}

async fn standard_cache_counts(enabled: bool, capacity: usize) -> (usize, usize) {
    let state = TrackingState::configured();
    let factory: Arc<dyn ConnectionFactory> = Arc::new(TrackingFactory::new(Arc::clone(&state)));
    let config = PoolInnerConfig {
        max_open: 1,
        max_idle: 1,
        acquire_timeout: Duration::from_millis(200),
        pool_prepared_statements: enabled,
        max_pool_prepared_statements_per_connection: capacity,
        ..PoolInnerConfig::default()
    };
    let pool = DruidPool::new_standard(
        format!("task5-standard-cache-{enabled}-{capacity}"),
        "task5-tracking-standard".to_owned(),
        factory,
        config,
        None,
    );

    pool.init().await.expect("cache matrix pool init");
    for _ in 0..2 {
        let mut logical = pool.get().await.expect("cache matrix borrow");
        let mut prepared = Connection::prepare_statement(
            &mut logical,
            "SELECT cache_matrix",
            StatementOptions::default(),
        )
        .await
        .expect("cache matrix prepare");
        prepared.close().await.expect("cache matrix prepared close");
        Connection::close(&mut logical)
            .await
            .expect("cache matrix logical close");
    }
    let prepare_count = state
        .event_snapshot()
        .iter()
        .filter(|event| event.as_str() == "connection.prepare:SELECT cache_matrix")
        .count();
    let close_count = state.prepared_close_count.load(Ordering::Acquire);
    pool.close().await;
    (prepare_count, close_count)
}

#[tokio::test]
async fn prepared_clear_failure_retains_raw_handle_for_retry() {
    prepared_cleanup_failure_retains_raw("prepared_clear_parameters").await;
}

#[tokio::test]
async fn prepared_is_closed_failure_retains_raw_handle_for_retry() {
    prepared_cleanup_failure_retains_raw("prepared_is_closed").await;
}

async fn prepared_cleanup_failure_retains_raw(operation: &'static str) {
    let state = TrackingState::configured();
    let holder = DruidConnectionHolder::with_standard_connection(
        Box::new(standard_test_support::TrackingConnection::new(Arc::clone(
            &state,
        ))),
        43,
        Duration::ZERO,
        0,
    )
    .await
    .expect("standard holder");
    let mut logical = DruidPooledConnection::with_standard_holder(
        holder,
        "task5-prepared-cleanup".to_owned(),
        None,
        false,
        false,
        Box::new(|_holder, _disposition| false),
    );
    let mut prepared = Connection::prepare_statement(
        &mut logical,
        "SELECT cleanup_retry",
        StatementOptions::default(),
    )
    .await
    .expect("prepared statement");

    state.fail_next(operation);
    assert!(prepared.close().await.is_err());
    prepared
        .close()
        .await
        .expect("second close still owns and terminates raw statement");
    assert_eq!(
        state.prepared_close_attempt_count.load(Ordering::Acquire),
        1
    );
    assert_eq!(state.prepared_close_count.load(Ordering::Acquire), 1);
}

#[tokio::test]
async fn prepared_raw_close_failure_retains_handle_for_exact_retry() {
    let state = TrackingState::configured();
    let holder = DruidConnectionHolder::with_standard_connection(
        Box::new(standard_test_support::TrackingConnection::new(Arc::clone(
            &state,
        ))),
        44,
        Duration::ZERO,
        0,
    )
    .await
    .expect("standard holder");
    let mut logical = DruidPooledConnection::with_standard_holder(
        holder,
        "task5-prepared-close-retry".to_owned(),
        None,
        false,
        false,
        Box::new(|_holder, _disposition| false),
    );
    let mut prepared = Connection::prepare_statement(
        &mut logical,
        "SELECT close_retry",
        StatementOptions::default(),
    )
    .await
    .expect("prepared statement");

    state.fail_next("prepared_close");
    assert!(prepared.close().await.is_err());
    prepared
        .close()
        .await
        .expect("second close retries the same raw handle");
    assert_eq!(
        state.prepared_close_attempt_count.load(Ordering::Acquire),
        2
    );
    assert_eq!(state.prepared_close_count.load(Ordering::Acquire), 1);
}

#[tokio::test]
async fn prepared_cache_eviction_close_failure_is_retained_for_recycle_retry() {
    let state = TrackingState::configured();
    let factory: Arc<dyn ConnectionFactory> = Arc::new(TrackingFactory::new(Arc::clone(&state)));
    let pool = DruidPool::new_standard(
        "task5-prepared-eviction-retry".to_owned(),
        "task5-tracking-standard".to_owned(),
        factory,
        PoolInnerConfig {
            max_open: 1,
            max_idle: 1,
            acquire_timeout: Duration::from_millis(200),
            pool_prepared_statements: true,
            max_pool_prepared_statements_per_connection: 1,
            ..PoolInnerConfig::default()
        },
        None,
    );
    pool.init().await.expect("eviction retry pool init");
    let mut logical = pool.get().await.expect("logical connection");
    let mut first = Connection::prepare_statement(
        &mut logical,
        "SELECT eviction_first",
        StatementOptions::default(),
    )
    .await
    .expect("first prepared");
    first.close().await.expect("cache first");
    let mut second = Connection::prepare_statement(
        &mut logical,
        "SELECT eviction_second",
        StatementOptions::default(),
    )
    .await
    .expect("second prepared");
    state.fail_next("prepared_close");
    assert!(
        second.close().await.is_err(),
        "eldest close failure is visible"
    );

    Connection::close(&mut logical)
        .await
        .expect("recycle retries retained eldest and closes cache");
    pool.close().await;
    assert_eq!(
        state.prepared_close_attempt_count.load(Ordering::Acquire),
        3
    );
    assert_eq!(state.prepared_close_count.load(Ordering::Acquire), 2);
}

#[tokio::test]
async fn logical_close_cascades_all_open_standard_handles_exactly_once() {
    let state = TrackingState::configured();
    let holder = DruidConnectionHolder::with_standard_connection(
        Box::new(standard_test_support::TrackingConnection::new(Arc::clone(
            &state,
        ))),
        45,
        Duration::ZERO,
        0,
    )
    .await
    .expect("standard holder");
    let mut logical = DruidPooledConnection::with_standard_holder(
        holder,
        "task5-standard-cascade".to_owned(),
        None,
        false,
        false,
        Box::new(|_holder, _disposition| false),
    );
    let mut statement = Connection::create_statement(&mut logical, StatementOptions::default())
        .await
        .expect("open statement");
    let mut result = statement
        .execute_query("SELECT open_result")
        .await
        .expect("open result set");
    assert!(result.next().await.expect("result row"));
    let prepared = Connection::prepare_statement(
        &mut logical,
        "SELECT open_prepared",
        StatementOptions::default(),
    )
    .await
    .expect("open prepared");
    let callable = Connection::prepare_call(
        &mut logical,
        "CALL open_callable",
        StatementOptions::default(),
    )
    .await
    .expect("open callable");

    Connection::close(&mut logical)
        .await
        .expect("logical close cascades resources");
    assert_eq!(state.statement_close_count.load(Ordering::Acquire), 1);
    assert_eq!(state.prepared_close_count.load(Ordering::Acquire), 2);
    assert_eq!(state.result_set_close_count.load(Ordering::Acquire), 1);
    assert!(statement.is_closed().expect("statement closed"));
    assert!(prepared.is_closed().expect("prepared closed"));
    assert!(callable.is_closed().expect("callable closed"));
    assert!(result.is_closed().expect("result closed"));
}

#[tokio::test]
async fn dropped_standard_wrappers_remain_in_holder_close_cascade() {
    let state = TrackingState::configured();
    let holder = DruidConnectionHolder::with_standard_connection(
        Box::new(standard_test_support::TrackingConnection::new(Arc::clone(
            &state,
        ))),
        46,
        Duration::ZERO,
        0,
    )
    .await
    .expect("standard holder");
    let mut logical = DruidPooledConnection::with_standard_holder(
        holder,
        "task5-standard-drop-cascade".to_owned(),
        None,
        false,
        false,
        Box::new(|_holder, _disposition| false),
    );
    let mut statement = Connection::create_statement(&mut logical, StatementOptions::default())
        .await
        .expect("statement");
    let mut result = statement
        .execute_query("SELECT dropped_result")
        .await
        .expect("result");
    assert!(result.next().await.expect("row"));
    let prepared = Connection::prepare_statement(
        &mut logical,
        "SELECT dropped_prepared",
        StatementOptions::default(),
    )
    .await
    .expect("prepared");
    let callable = Connection::prepare_call(
        &mut logical,
        "CALL dropped_callable",
        StatementOptions::default(),
    )
    .await
    .expect("callable");

    drop(result);
    drop(statement);
    drop(prepared);
    drop(callable);
    Connection::close(&mut logical)
        .await
        .expect("holder closes dropped wrappers");
    assert_eq!(
        state.statement_close_attempt_count.load(Ordering::Acquire),
        1
    );
    assert_eq!(state.statement_close_count.load(Ordering::Acquire), 1);
    assert_eq!(
        state.prepared_close_attempt_count.load(Ordering::Acquire),
        2
    );
    assert_eq!(state.prepared_close_count.load(Ordering::Acquire), 2);
    assert_eq!(state.result_set_close_count.load(Ordering::Acquire), 1);
}

#[tokio::test]
async fn concurrent_statement_and_connection_close_call_raw_once() {
    let state = TrackingState::configured();
    let holder = DruidConnectionHolder::with_standard_connection(
        Box::new(standard_test_support::TrackingConnection::new(Arc::clone(
            &state,
        ))),
        47,
        Duration::ZERO,
        0,
    )
    .await
    .expect("standard holder");
    let mut logical = DruidPooledConnection::with_standard_holder(
        holder,
        "task5-standard-concurrent-close".to_owned(),
        None,
        false,
        false,
        Box::new(|_holder, _disposition| false),
    );
    let mut statement = Connection::create_statement(&mut logical, StatementOptions::default())
        .await
        .expect("statement");
    let barrier = Arc::new(Barrier::new(2));
    let statement_barrier = Arc::clone(&barrier);
    let statement_task = tokio::spawn(async move {
        statement_barrier.wait().await;
        statement.close().await
    });
    let connection_task = tokio::spawn(async move {
        barrier.wait().await;
        Connection::close(&mut logical).await
    });
    tokio::time::timeout(Duration::from_secs(2), async {
        statement_task
            .await
            .expect("statement task")
            .expect("statement close");
        connection_task
            .await
            .expect("connection task")
            .expect("connection close");
    })
    .await
    .expect("concurrent close must not hang");
    assert_eq!(
        state.statement_close_attempt_count.load(Ordering::Acquire),
        1
    );
    assert_eq!(state.statement_close_count.load(Ordering::Acquire), 1);
}
