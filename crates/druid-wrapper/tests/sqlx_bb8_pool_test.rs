//! Canonical SQLx bb8 external-pool contracts.

use druid::core::Pool;
use druid_rdbc::connection::Connection;
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::statement::{GeneratedKeys, StatementOptions};
use druid_rdbc::types::Value;
use druid_wrapper::sqlx::bb8::SqlxBb8Pool;
use std::time::Duration;

#[tokio::test]
async fn bb8_pool_executes_and_recycles_one_canonical_lease() {
    let pool = SqlxBb8Pool::connect(
        "bb8-canonical",
        "sqlite::memory:",
        1,
        Duration::from_secs(1),
        None,
    )
    .await
    .unwrap();
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement
        .execute_update(
            "CREATE TABLE item(id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
            GeneratedKeys::None,
        )
        .await
        .unwrap();
    statement
        .execute_update(
            "INSERT INTO item(id, name) VALUES (1, 'druid')",
            GeneratedKeys::None,
        )
        .await
        .unwrap();
    statement.close().await.unwrap();
    connection.close().await.unwrap();
    assert_eq!(pool.state().recycle_count, 1);

    let mut reused = pool.get().await.unwrap();
    let mut query = reused
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    let mut rows = query
        .execute_query("SELECT name FROM item WHERE id = 1")
        .await
        .unwrap();
    assert!(rows.next().await.unwrap());
    assert_eq!(
        rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::String("druid".into())
    );
}

#[tokio::test]
async fn bb8_pool_timeout_and_invalid_capacity_are_explicit() {
    assert!(SqlxBb8Pool::connect(
        "invalid",
        "sqlite::memory:",
        0,
        Duration::from_secs(1),
        None,
    )
    .await
    .is_err());
    let pool = SqlxBb8Pool::connect(
        "timeout",
        "sqlite::memory:",
        1,
        Duration::from_secs(1),
        None,
    )
    .await
    .unwrap();
    let _held = pool.get().await.unwrap();
    assert_eq!(
        pool.get_timeout(Duration::from_millis(1))
            .await
            .unwrap_err(),
        druid_rdbc::error::RdbcError::AcquireTimeout
    );
}

#[tokio::test]
async fn bb8_transactional_close_resets_and_dirty_drop_discards() {
    let pool = SqlxBb8Pool::connect(
        "bb8-reset",
        "sqlite::memory:",
        1,
        Duration::from_secs(1),
        None,
    )
    .await
    .unwrap();
    let mut connection = pool.get().await.unwrap();
    connection.set_auto_commit(false).await.unwrap();
    connection.close().await.unwrap();
    let mut reused = pool.get().await.unwrap();
    assert!(reused.auto_commit().unwrap());
    reused.set_auto_commit(false).await.unwrap();
    drop(reused);
    assert_eq!(pool.state().discard_count, 1);
}
