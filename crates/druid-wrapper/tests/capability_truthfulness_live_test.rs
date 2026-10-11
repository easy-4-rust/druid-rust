//! 在 CI 的 PostgreSQL/MySQL 服务上验证 SQLx/Toasty 宣称的事务与保存点能力。
#![cfg(feature = "sqlx")]

use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::types::{Row, Value};
use druid_wrapper::sqlx::SqlxConnectionFactory;
#[cfg(any(feature = "postgresql", feature = "mysql"))]
use druid_wrapper::toasty::ToastyConnectionFactory;

#[tokio::test]
#[ignore = "requires DRUID_DATABASE_URL and a live PostgreSQL or MySQL service"]
async fn live_backend_capability_claims_match_committed_rows() {
    let url = std::env::var("DRUID_DATABASE_URL")
        .expect("CI must provide DRUID_DATABASE_URL for the ignored live capability test");
    let backend = std::env::var("DRUID_DATABASE_PROFILE")
        .expect("CI must provide DRUID_DATABASE_PROFILE for the live capability test");
    assert!(matches!(backend.as_str(), "postgresql" | "mysql"));

    let mut factories: Vec<Box<dyn ConnectionFactory>> =
        vec![Box::new(SqlxConnectionFactory::new(url.clone()))];
    #[cfg(any(feature = "postgresql", feature = "mysql"))]
    factories.push(Box::new(
        ToastyConnectionFactory::new(url)
            .await
            .expect("create live Toasty factory"),
    ));
    assert_eq!(
        factories.len(),
        2,
        "live capability gate must execute SQLx and Toasty"
    );
    for factory in factories {
        let driver = factory.name().to_owned();
        let connection = factory.create().await.expect("connect to live database");
        verify_capabilities(connection, &backend, &driver).await;
    }
}

async fn verify_capabilities(mut connection: Box<dyn Connection>, backend: &str, driver: &str) {
    let capabilities = connection.capabilities();
    assert!(
        capabilities.transactions,
        "{backend}/{driver} must support transactions"
    );
    assert!(
        capabilities.savepoints,
        "{backend}/{driver} must support savepoints"
    );
    assert!(!capabilities.catalogs);
    assert!(!capabilities.schemas);
    assert!(!capabilities.callable_statements);

    connection
        .exec(
            "CREATE TEMPORARY TABLE druid_capability_probe (value INTEGER NOT NULL)",
            vec![],
        )
        .await
        .expect("create session-local probe table");
    connection
        .set_auto_commit(false)
        .await
        .expect("begin transaction");
    connection
        .exec("INSERT INTO druid_capability_probe VALUES (1)", vec![])
        .await
        .expect("insert before savepoint");
    let savepoint = connection
        .set_savepoint_named("druid_capability_sp")
        .await
        .expect("create savepoint");
    connection
        .exec("INSERT INTO druid_capability_probe VALUES (2)", vec![])
        .await
        .expect("insert after savepoint");
    connection
        .rollback_to(&savepoint)
        .await
        .expect("roll back to savepoint");
    connection
        .release_savepoint(&savepoint)
        .await
        .expect("release savepoint");
    connection.commit().await.expect("commit transaction");
    connection
        .set_auto_commit(true)
        .await
        .expect("restore auto commit");
    assert_eq!(
        connection
            .fetch("SELECT value FROM druid_capability_probe", vec![])
            .await
            .expect("read committed rows"),
        vec![Row::new(vec![Value::Int(1)])],
        "{backend}/{driver} must preserve only the row before the savepoint"
    );

    connection
        .set_auto_commit(false)
        .await
        .expect("begin rollback transaction");
    connection
        .exec("INSERT INTO druid_capability_probe VALUES (3)", vec![])
        .await
        .expect("insert rollback candidate");
    connection.rollback().await.expect("roll back transaction");
    connection
        .set_auto_commit(true)
        .await
        .expect("restore auto commit after rollback");
    assert_eq!(
        connection
            .fetch("SELECT value FROM druid_capability_probe", vec![])
            .await
            .expect("read rows after rollback"),
        vec![Row::new(vec![Value::Int(1)])],
        "{backend}/{driver} must roll back the whole second transaction"
    );
    connection.close().await.expect("close live connection");
}
