//! `druid-wrapper` 的真实 SQLite 适配边界契约。

use druid::core::{DruidError, PhysicalConnection, PhysicalConnectionFactory, Pool, Value};
use druid_wrapper::sqlx::SqlxConnectionFactory;
use druid_wrapper::sqlx_bb8::SqlxBb8Pool;
use druid_wrapper::sqlx_deadpool::SqlxDeadpoolPool;
use std::time::Duration;

#[tokio::test]
async fn direct_sqlx_wrapper_uses_real_sqlite_and_reports_callable_capability() {
    let factory = SqlxConnectionFactory::new("sqlite::memory:");
    let mut connection = factory.create().await.expect("必须创建真实 SQLite 连接");
    connection
        .exec(
            "CREATE TABLE item(id INTEGER PRIMARY KEY, value TEXT NOT NULL)",
            Vec::new(),
        )
        .await
        .expect("DDL 必须执行");
    connection
        .exec(
            "INSERT INTO item(id, value) VALUES (?, ?)",
            vec![Value::Int(1), Value::String("direct".to_string())],
        )
        .await
        .expect("参数绑定必须执行");
    let rows = connection
        .fetch("SELECT value FROM item WHERE id = ?", vec![Value::Int(1)])
        .await
        .expect("查询必须执行");
    assert_eq!(rows[0].values, vec![Value::String("direct".to_string())]);

    let callable = connection
        .prepare_physical_call(
            &druid::core::PreparedStatementKey::new(
                Some("{call sqlite_has_no_procedure()}".to_string()),
                None,
                druid::core::PreparedStatementMethodType::Precall1,
            )
            .unwrap(),
        )
        .await;
    assert!(matches!(
        callable,
        Err(DruidError::UnsupportedOperation {
            operation: "prepare_physical_call"
        })
    ));
    connection.close().await.expect("SQLx 物理连接必须显式关闭");
    assert!(connection.is_closed());
}

#[tokio::test]
async fn external_pool_wrappers_return_real_sqlite_leases_to_their_owner() {
    let bb8 = SqlxBb8Pool::connect(
        "wrapper-bb8",
        "sqlite::memory:",
        1,
        Duration::from_secs(1),
        None,
    )
    .await
    .expect("bb8 SQLite bridge 必须初始化");
    let mut bb8_connection = bb8.get().await.expect("bb8 必须返回连接");
    bb8_connection
        .exec("CREATE TABLE bb8_item(id INTEGER PRIMARY KEY)", Vec::new())
        .await
        .expect("bb8 lease 必须真实执行 SQLite");
    bb8_connection.close().await.expect("bb8 lease 必须归还");
    assert_eq!(bb8.state().recycle_count, 1);

    let deadpool = SqlxDeadpoolPool::connect(
        "wrapper-deadpool",
        "sqlite::memory:",
        1,
        Duration::from_secs(1),
        None,
    )
    .expect("deadpool SQLite bridge 必须初始化");
    let mut deadpool_connection = deadpool.get().await.expect("deadpool 必须返回连接");
    deadpool_connection
        .exec(
            "CREATE TABLE deadpool_item(id INTEGER PRIMARY KEY)",
            Vec::new(),
        )
        .await
        .expect("deadpool lease 必须真实执行 SQLite");
    deadpool_connection
        .close()
        .await
        .expect("deadpool lease 必须归还");
    assert_eq!(deadpool.state().recycle_count, 1);
}
