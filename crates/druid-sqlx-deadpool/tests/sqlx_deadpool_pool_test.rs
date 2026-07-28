//! SQLx + deadpool 外部池桥接契约测试。

use druid_core::{DruidError, PhysicalConnection, Pool, Value};
use druid_sqlx_deadpool::SqlxDeadpoolPool;
use std::time::Duration;

#[tokio::test]
async fn deadpool_returns_canonical_druid_connection_and_recycles_once() {
    let pool = SqlxDeadpoolPool::connect(
        "deadpool-sqlite",
        "sqlite::memory:",
        1,
        Duration::from_secs(1),
        None,
    )
    .expect("deadpool must build");

    let mut connection = pool.get().await.expect("connection must be acquired");
    assert_eq!(connection.data_source(), "deadpool-sqlite");
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
    connection.close().await.expect("close must recycle");
    connection
        .close()
        .await
        .expect("duplicate close must be idempotent");
    assert_eq!(pool.state().recycle_count, 1);

    // 第二次获取必须等待 deadpool 完成回收，因此无需 sleep 即可验证租约已归还。
    let mut reused = pool.get().await.expect("external lease must be reusable");
    let rows = reused
        .fetch("SELECT name FROM item WHERE id = 1", Vec::new())
        .await
        .expect("row must be fetched");
    assert_eq!(rows[0].values, vec![Value::String("druid".to_string())]);
}

#[tokio::test]
async fn deadpool_maps_timeout_and_closed_state() {
    let pool = SqlxDeadpoolPool::connect(
        "deadpool-state",
        "sqlite::memory:",
        1,
        Duration::from_secs(1),
        None,
    )
    .expect("deadpool must build");
    let _held = pool.get().await.expect("first connection must be acquired");

    let error = pool
        .get_timeout(Duration::from_millis(1))
        .await
        .expect_err("second connection must time out");
    assert_eq!(error, DruidError::AcquireTimeout);
    assert_eq!(pool.state().connect_error_count, 1);

    pool.close();
    assert!(pool.state().closed);
    let error = pool
        .get()
        .await
        .expect_err("closed pool must reject acquire");
    assert_eq!(error, DruidError::PoolClosed);
}

#[test]
fn deadpool_rejects_invalid_capacity() {
    let result = SqlxDeadpoolPool::connect(
        "invalid",
        "sqlite::memory:",
        0,
        Duration::from_secs(1),
        None,
    );
    assert!(result.is_err());
}
