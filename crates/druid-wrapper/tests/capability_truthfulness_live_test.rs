//! 验证 SQLx/Toasty 宣称的事务与保存点能力；远程产品由 CI 实库执行。
#![cfg(all(
    feature = "sqlx",
    any(feature = "sqlite", feature = "postgresql", feature = "mysql")
))]

use druid_rdbc::connection::Connection;
use druid_rdbc::driver::ConnectionFactory;
use druid_rdbc::error::RdbcError;
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::statement::StatementOptions;
use druid_rdbc::types::{Row, Value};
use druid_wrapper::sqlx::SqlxConnectionFactory;
use druid_wrapper::toasty::ToastyConnectionFactory;

#[cfg(feature = "sqlite")]
#[tokio::test]
async fn sqlite_capability_claims_match_committed_rows() {
    const URL: &str = "sqlite::memory:";
    let factories: Vec<Box<dyn ConnectionFactory>> = vec![
        Box::new(SqlxConnectionFactory::new(URL)),
        Box::new(
            ToastyConnectionFactory::new(URL)
                .await
                .expect("create SQLite Toasty factory"),
        ),
    ];
    for factory in factories {
        let driver = factory.name().to_owned();
        let connection = factory.create().await.expect("connect to SQLite");
        verify_capabilities(connection, "sqlite", &driver).await;
    }
}

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

    assert_feature_not_supported(
        connection
            .create_array_of(Some("INTEGER"), Some(vec![]))
            .await,
        backend,
        driver,
        "createArrayOf",
    );
    assert_feature_not_supported(
        connection.create_blob().await,
        backend,
        driver,
        "createBlob",
    );
    assert_feature_not_supported(
        connection.create_clob().await,
        backend,
        driver,
        "createClob",
    );
    assert_feature_not_supported(
        connection.create_n_clob().await,
        backend,
        driver,
        "createNClob",
    );
    assert_feature_not_supported(
        connection.create_sql_xml().await,
        backend,
        driver,
        "createSQLXML",
    );
    assert_feature_not_supported(
        connection.create_struct(Some("probe"), Some(vec![])).await,
        backend,
        driver,
        "createStruct",
    );

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

    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .expect("create standard statement");
    let mut result_set = statement
        .execute_query("SELECT value FROM druid_capability_probe")
        .await
        .expect("execute standard query");
    let column = ColumnIndex::new(1).expect("one-based column index");
    assert!(result_set.next().await.expect("read committed row"));
    assert_eq!(
        result_set.value(column).expect("read current value"),
        Value::Int(1)
    );
    assert!(!result_set.next().await.expect("observe EOF"));
    result_set.close().await.expect("close result set");
    assert!(result_set.is_closed().expect("closed state"));
    statement.close().await.expect("close statement");
    connection.close().await.expect("close live connection");
}

fn assert_feature_not_supported<T>(
    result: Result<T, RdbcError>,
    backend: &str,
    driver: &str,
    operation: &str,
) {
    match result {
        Err(RdbcError::SqlException(error)) => assert_eq!(
            error.sql_state(),
            Some("0A000"),
            "{backend}/{driver} {operation} must reject unsupported resources"
        ),
        Err(error) => panic!("{backend}/{driver} {operation} returned {error:?}"),
        Ok(_) => panic!("{backend}/{driver} {operation} unexpectedly succeeded"),
    }
}
