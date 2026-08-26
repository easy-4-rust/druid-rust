//! Canonical SQLx deadpool external-pool contracts.

use druid::core::Pool;
use druid_rdbc::connection::Connection;
use druid_rdbc::statement::{GeneratedKeys, StatementOptions};
use druid_wrapper::sqlx::deadpool::SqlxDeadpoolPool;
use std::time::Duration;

#[tokio::test]
async fn deadpool_executes_recycles_and_resets_one_canonical_lease() {
    let pool = SqlxDeadpoolPool::connect(
        "deadpool-canonical",
        "sqlite::memory:",
        1,
        Duration::from_secs(1),
        None,
    )
    .unwrap();
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement
        .execute_update("CREATE TABLE item(id INTEGER)", GeneratedKeys::None)
        .await
        .unwrap();
    statement.close().await.unwrap();
    connection.set_auto_commit(false).await.unwrap();
    connection.close().await.unwrap();
    assert_eq!(pool.state().recycle_count, 1);
    let reused = pool.get().await.unwrap();
    assert!(reused.auto_commit().unwrap());
}

#[tokio::test]
async fn deadpool_timeout_close_and_invalid_capacity_are_explicit() {
    assert!(SqlxDeadpoolPool::connect(
        "invalid",
        "sqlite::memory:",
        0,
        Duration::from_secs(1),
        None,
    )
    .is_err());
    let pool = SqlxDeadpoolPool::connect(
        "timeout",
        "sqlite::memory:",
        1,
        Duration::from_secs(1),
        None,
    )
    .unwrap();
    let _held = pool.get().await.unwrap();
    assert_eq!(
        pool.get_timeout(Duration::from_millis(1))
            .await
            .unwrap_err(),
        druid_rdbc::error::RdbcError::AcquireTimeout
    );
    pool.close();
    assert!(pool.state().closed);
}

#[tokio::test]
async fn deadpool_dirty_transaction_drop_is_discarded() {
    let pool = SqlxDeadpoolPool::connect(
        "deadpool-dirty",
        "sqlite::memory:",
        1,
        Duration::from_secs(1),
        None,
    )
    .unwrap();
    let mut connection = pool.get().await.unwrap();
    connection.set_auto_commit(false).await.unwrap();
    drop(connection);
    assert_eq!(pool.state().discard_count, 1);
}
