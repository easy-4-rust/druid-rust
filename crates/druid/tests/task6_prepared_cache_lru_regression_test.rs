//! Task 6 prepared-cache LRU regression coverage.

#[path = "standard_test_support/mod.rs"]
mod standard_test_support;

use druid::core::{ConnectionFactory, RdbcError};
use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::statement::StatementOptions;
use standard_test_support::{TrackingFactory, TrackingState};
use std::sync::atomic::Ordering;
use std::sync::Arc;

#[derive(Clone, Copy)]
enum CacheAffectingState {
    Catalog,
    Schema,
}

async fn assert_state_change_invalidates_cached_statement(change: CacheAffectingState) {
    let tracking = TrackingState::configured();
    let pool =
        DruidPool::builder()
            .name("task6-prepared-state-invalidation")
            .driver_name("tracking")
            .factory(
                Arc::new(TrackingFactory::new(Arc::clone(&tracking))) as Arc<dyn ConnectionFactory>
            )
            .max_open(1)
            .max_idle(1)
            .pool_prepared_statements(true)
            .max_pool_prepared_statements_per_connection(3)
            .build()
            .await
            .expect("pool");
    let mut connection = pool.get().await.expect("connection");
    let mut first = connection
        .prepare_statement("SELECT 1", StatementOptions::default())
        .await
        .expect("first prepare");
    first.close().await.expect("first close");
    assert_eq!(pool.state().cached_prepared_statement_count, 1);

    match change {
        CacheAffectingState::Catalog => connection
            .set_catalog(Some("catalog_tenant"))
            .await
            .expect("catalog"),
        CacheAffectingState::Schema => connection
            .set_schema(Some("schema_tenant"))
            .await
            .expect("schema"),
    }
    let state = pool.state();
    assert_eq!(state.cached_prepared_statement_count, 0);
    assert_eq!(state.cached_prepared_statement_delete_count, 1);
    assert_eq!(tracking.prepared_close_count.load(Ordering::Acquire), 1);
    match change {
        CacheAffectingState::Catalog => {
            assert_eq!(
                connection.catalog().expect("catalog getter").as_deref(),
                Some("catalog_tenant")
            );
        }
        CacheAffectingState::Schema => {
            assert_eq!(
                connection.schema().expect("schema getter").as_deref(),
                Some("schema_tenant")
            );
        }
    }

    let mut second = connection
        .prepare_statement("SELECT 1", StatementOptions::default())
        .await
        .expect("second prepare");
    let prepare_count = tracking
        .event_snapshot()
        .iter()
        .filter(|event| event.as_str() == "connection.prepare:SELECT 1")
        .count();
    assert_eq!(
        prepare_count, 2,
        "state change must prevent stale cache reuse"
    );
    second.close().await.expect("second close");
    connection.close().await.expect("connection close");
    pool.close().await;
    assert_eq!(tracking.prepared_close_count.load(Ordering::Acquire), 2);
}

#[tokio::test]
async fn active_prepared_statement_is_not_counted_and_lru_evictions_close_once() {
    let tracking = TrackingState::configured();
    let pool =
        DruidPool::builder()
            .name("task6-prepared-lru-regression")
            .driver_name("tracking")
            .factory(
                Arc::new(TrackingFactory::new(Arc::clone(&tracking))) as Arc<dyn ConnectionFactory>
            )
            .max_open(1)
            .max_idle(1)
            .pool_prepared_statements(true)
            .max_pool_prepared_statements_per_connection(3)
            .build()
            .await
            .expect("pool");

    let mut connection = pool.get().await.expect("connection");
    let mut seed = connection
        .prepare_statement("SELECT 0", StatementOptions::default())
        .await
        .expect("seed");
    seed.close().await.expect("seed close");
    let mut active = connection
        .prepare_statement("SELECT 0", StatementOptions::default())
        .await
        .expect("cache hit");

    assert_eq!(pool.state().cached_prepared_statement_count, 0);
    for sql in ["SELECT 1", "SELECT 2", "SELECT 3", "SELECT 4"] {
        let mut statement = connection
            .prepare_statement(sql, StatementOptions::default())
            .await
            .expect("prepared statement");
        statement.close().await.expect("logical close");
    }

    let state = pool.state();
    assert_eq!(state.cached_prepared_statement_count, 3);
    assert_eq!(state.cached_prepared_statement_delete_count, 1);
    assert_eq!(tracking.prepared_close_count.load(Ordering::Acquire), 1);

    active.close().await.expect("active return");
    let state = pool.state();
    assert_eq!(state.cached_prepared_statement_count, 3);
    assert_eq!(state.cached_prepared_statement_delete_count, 2);
    assert_eq!(tracking.prepared_close_count.load(Ordering::Acquire), 2);

    connection.close().await.expect("connection close");
    pool.close().await;
    assert_eq!(tracking.prepared_close_count.load(Ordering::Acquire), 5);
}

#[tokio::test]
async fn schema_change_invalidates_cached_statement_before_visibility() {
    assert_state_change_invalidates_cached_statement(CacheAffectingState::Schema).await;
}

#[tokio::test]
async fn catalog_change_invalidates_cached_statement_before_visibility() {
    assert_state_change_invalidates_cached_statement(CacheAffectingState::Catalog).await;
}

#[tokio::test]
async fn failed_prepared_execution_never_returns_handle_to_cache() {
    let tracking = TrackingState::configured();
    let pool =
        DruidPool::builder()
            .name("task6-prepared-error-invalidation")
            .driver_name("tracking")
            .factory(
                Arc::new(TrackingFactory::new(Arc::clone(&tracking))) as Arc<dyn ConnectionFactory>
            )
            .max_open(1)
            .max_idle(1)
            .pool_prepared_statements(true)
            .max_pool_prepared_statements_per_connection(3)
            .build()
            .await
            .expect("pool");
    let mut connection = pool.get().await.expect("connection");
    let mut statement = connection
        .prepare_statement("SELECT failure", StatementOptions::default())
        .await
        .expect("prepare");
    tracking.fail_next("prepared_execute_generic");
    assert!(matches!(
        statement.execute_prepared().await,
        Err(RdbcError::SqlException(_))
    ));
    statement.close().await.expect("logical close");

    let state = pool.state();
    assert_eq!(state.cached_prepared_statement_count, 0);
    assert_eq!(state.cached_prepared_statement_delete_count, 1);
    assert_eq!(state.closed_prepared_statement_count, 1);
    assert_eq!(tracking.prepared_close_count.load(Ordering::Acquire), 1);

    connection.close().await.expect("connection close");
    pool.close().await;
    assert_eq!(tracking.prepared_close_count.load(Ordering::Acquire), 1);
}
