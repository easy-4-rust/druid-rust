//! SQLx + bb8 外部池桥接契约测试。

use druid_core::{PhysicalConnection, Pool, Value};
use druid_sqlx_bb8::SqlxBb8Pool;
use std::time::Duration;

#[tokio::test]
async fn bb8_pool_returns_canonical_druid_connection_and_recycles_once() {
    let pool = SqlxBb8Pool::connect(
        "bb8-sqlite",
        "sqlite::memory:",
        1,
        Duration::from_secs(1),
        None,
    )
    .await
    .expect("bb8 pool must build");

    let mut connection = pool.get().await.expect("connection must be acquired");
    assert_eq!(connection.data_source(), "bb8-sqlite");
    assert_eq!(connection.driver_name(), "SQLite");
    connection
        .exec(
            "CREATE TABLE item (id INTEGER PRIMARY KEY, name TEXT NOT NULL)",
            Vec::new(),
        )
        .await
        .expect("table must be created");
    connection
        .exec(
            "INSERT INTO item (name) VALUES (?)",
            vec![Value::String("druid".to_string())],
        )
        .await
        .expect("row must be inserted");

    assert_eq!(pool.state().active_count, 1);
    connection.close().await.expect("close must recycle");
    connection
        .close()
        .await
        .expect("duplicate close must be idempotent");

    let state = pool.state();
    assert_eq!(state.active_count, 0);
    assert_eq!(state.idle_count, 1);
    assert_eq!(state.recycle_count, 1);

    let mut reused = pool
        .get()
        .await
        .expect("same external lease must be reusable");
    let rows = reused
        .fetch("SELECT name FROM item WHERE id = 1", Vec::new())
        .await
        .expect("row must be fetched");
    assert_eq!(rows[0].values, vec![Value::String("druid".to_string())]);
}

#[tokio::test]
async fn bb8_pool_maps_capacity_wait_to_acquire_timeout() {
    let pool = SqlxBb8Pool::connect(
        "bb8-timeout",
        "sqlite::memory:",
        1,
        Duration::from_secs(1),
        None,
    )
    .await
    .expect("bb8 pool must build");
    let _held = pool.get().await.expect("first connection must be acquired");

    let error = pool
        .get_timeout(Duration::from_millis(1))
        .await
        .expect_err("second connection must time out");
    assert_eq!(error, druid_core::DruidError::AcquireTimeout);
    assert_eq!(pool.state().connect_error_count, 1);
}

#[tokio::test]
async fn bb8_pool_rejects_invalid_capacity() {
    let result = SqlxBb8Pool::connect(
        "invalid",
        "sqlite::memory:",
        0,
        Duration::from_secs(1),
        None,
    )
    .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn bb8_explicit_close_rolls_back_and_reuses_physical_connection() {
    let pool = SqlxBb8Pool::connect(
        "bb8-explicit-recycle",
        "sqlite::memory:",
        1,
        Duration::from_secs(1),
        None,
    )
    .await
    .expect("bb8 pool must build");

    let mut connection = pool.get().await.expect("connection must be acquired");
    connection.begin().await.expect("transaction must begin");
    connection
        .close()
        .await
        .expect("explicit close must rollback and recycle");

    let state = pool.state();
    assert_eq!(state.recycle_count, 1);
    assert_eq!(state.discard_count, 0);
    assert_eq!(state.create_count, 1);

    let reused = pool
        .get()
        .await
        .expect("physical connection must be reused");
    assert!(reused.auto_commit());
    assert_eq!(pool.state().create_count, 1);
}

#[tokio::test]
async fn bb8_dirty_drop_marks_external_lease_broken_and_replaces_it() {
    let pool = SqlxBb8Pool::connect(
        "bb8-dirty-drop",
        "sqlite::memory:",
        1,
        Duration::from_secs(1),
        None,
    )
    .await
    .expect("bb8 pool must build");

    let mut connection = pool.get().await.expect("connection must be acquired");
    connection.begin().await.expect("transaction must begin");
    drop(connection);

    let state = pool.state();
    assert_eq!(state.recycle_count, 0);
    assert_eq!(state.discard_count, 1);

    let replacement = pool
        .get()
        .await
        .expect("bb8 must replace the broken physical connection");
    assert!(replacement.auto_commit());
    assert_eq!(pool.state().create_count, 2);
}
