#![cfg(feature = "jdbc-agent")]

use druid::core::{
    DruidError, PhysicalConnectionFactory, PreparedStatementKey, PreparedStatementMethodType, Value,
};
use druid_wrapper::jdbc_agent::{JdbcAgentConnectionFactory, JdbcAgentOptions};
use std::path::PathBuf;

/// Runs the second embedded-JDBC compatibility contract required by the database plan.
///
/// H2 exercises cancellation and process-failure paths in its dedicated test. HSQLDB proves that
/// the same Agent protocol is not accidentally coupled to H2-specific URL, metadata, transaction,
/// savepoint, prepared-statement, paging, or authentication behavior.
#[tokio::test]
async fn jdbc_agent_hsqldb_contract_when_configured() {
    let Some(agent_jar) = std::env::var_os("DRUID_JDBC_AGENT_JAR") else {
        return;
    };
    let Some(driver_jar) = std::env::var_os("DRUID_HSQLDB_DRIVER_JAR") else {
        return;
    };
    let options = JdbcAgentOptions::java(
        "java",
        [PathBuf::from(agent_jar), PathBuf::from(driver_jar)],
    )
    .expect("the Java classpath must be portable");
    let factory = JdbcAgentConnectionFactory::new(
        "jdbc:hsqldb:mem:druid_rust_agent_hsqldb",
        Some("VALUES (1)".to_owned()),
        options.clone(),
    )
    .user_name("SA")
    .password("");

    let mut connection = factory
        .create()
        .await
        .expect("HSQLDB must open through the JDBC Agent");
    let mut shared_session = factory
        .create()
        .await
        .expect("a second physical connection must use an isolated Agent session");

    connection
        .exec(
            "CREATE TABLE sample(id BIGINT PRIMARY KEY, name VARCHAR(32))",
            Vec::new(),
        )
        .await
        .expect("HSQLDB DDL must execute");
    let statement_key = PreparedStatementKey::new(
        Some("INSERT INTO sample(id, name) VALUES (?, ?)".to_owned()),
        connection.catalog().map(ToOwned::to_owned),
        PreparedStatementMethodType::M1,
    )
    .expect("a prepared-statement cache key must be valid");
    let statement = connection
        .prepare_physical_statement(&statement_key)
        .await
        .expect("the Agent must create a real HSQLDB PreparedStatement");
    connection
        .exec_prepared(
            statement.as_ref(),
            vec![Value::Int(1), Value::String("druid".to_owned())],
        )
        .await
        .expect("the remote PreparedStatement must bind and execute");
    let update_counts = connection
        .exec_prepared_batch(
            statement.as_ref(),
            vec![
                vec![Value::Int(521), Value::String("batch-521".to_owned())],
                vec![Value::Int(522), Value::String("batch-522".to_owned())],
            ],
        )
        .await
        .expect("the Agent must execute a native JDBC PreparedStatement batch");
    assert_eq!(update_counts, [1, 1]);
    connection
        .begin()
        .await
        .expect("batch failure testing needs a transaction");
    let batch_error = connection
        .exec_prepared_batch(
            statement.as_ref(),
            vec![
                vec![Value::Int(523), Value::String("batch-523".to_owned())],
                vec![Value::Int(1), Value::String("duplicate".to_owned())],
            ],
        )
        .await
        .expect_err("a duplicate key in a JDBC batch must fail");
    match batch_error {
        DruidError::BatchUpdateException {
            update_counts,
            cause,
        } => {
            assert!(
                !update_counts.is_empty(),
                "the Agent must preserve JDBC partial update counts"
            );
            assert!(matches!(*cause, DruidError::SqlException(_)));
        }
        error => panic!("expected BatchUpdateException, got {error:?}"),
    }
    connection
        .rollback()
        .await
        .expect("the failed batch transaction must roll back");
    connection
        .set_auto_commit(true)
        .await
        .expect("auto-commit must be restored after batch failure");
    statement
        .close()
        .expect("the remote PreparedStatement must close");

    let values = (2_i64..=520)
        .map(|id| format!("({id}, 'name-{id}')"))
        .collect::<Vec<_>>()
        .join(",");
    connection
        .exec(
            &format!("INSERT INTO sample(id, name) VALUES {values}"),
            Vec::new(),
        )
        .await
        .expect("HSQLDB must accept a standards-based multi-row insert");
    let rows = shared_session
        .fetch("SELECT id, name FROM sample ORDER BY id", Vec::new())
        .await
        .expect("the Agent must collect all pages from HSQLDB");
    assert_eq!(rows.len(), 522);
    assert_eq!(
        rows[0].values,
        [Value::Int(1), Value::String("druid".to_owned())]
    );

    connection
        .begin()
        .await
        .expect("HSQLDB must start a transaction");
    connection
        .exec("DELETE FROM sample", Vec::new())
        .await
        .expect("the transactional update must execute");
    connection
        .rollback()
        .await
        .expect("HSQLDB must roll back through the Agent");
    connection
        .set_auto_commit(true)
        .await
        .expect("auto-commit must be restorable");
    assert_eq!(
        connection
            .fetch("SELECT id FROM sample", Vec::new())
            .await
            .expect("rolled-back rows must remain visible")
            .len(),
        522
    );

    if connection.capabilities().savepoints {
        connection
            .begin()
            .await
            .expect("savepoint testing needs a transaction");
        let savepoint = connection
            .set_savepoint_named("druid_hsqldb_contract")
            .await
            .expect("HSQLDB must create a named savepoint");
        connection
            .exec("DELETE FROM sample WHERE id = 1", Vec::new())
            .await
            .expect("the update after a savepoint must execute");
        connection
            .rollback_to(&savepoint)
            .await
            .expect("the Agent must preserve the remote savepoint identity");
        // HSQLDB invalidates the rollback target after rollback. Use a fresh
        // savepoint to verify the independent JDBC releaseSavepoint contract.
        let releasable = connection
            .set_savepoint_named("druid_hsqldb_release_contract")
            .await
            .expect("HSQLDB must create a second named savepoint");
        connection
            .release_savepoint(&releasable)
            .await
            .expect("the remote savepoint must be releasable");
        connection
            .commit()
            .await
            .expect("the transaction must commit");
        connection
            .set_auto_commit(true)
            .await
            .expect("auto-commit must be restored after the savepoint contract");
    }

    let mut metadata = connection
        .database_meta_data()
        .expect("the Agent must expose HSQLDB metadata");
    assert_eq!(
        metadata
            .get_database_product_name()
            .await
            .expect("the product name must be readable")
            .as_deref(),
        Some("HSQL Database Engine")
    );
    drop(metadata);

    let bad_factory = JdbcAgentConnectionFactory::new(
        "jdbc:hsqldb:mem:druid_rust_agent_hsqldb",
        Some("VALUES (1)".to_owned()),
        options,
    )
    .user_name("SA")
    .password("wrong-password");
    assert!(
        bad_factory.create().await.is_err(),
        "invalid HSQLDB credentials must produce a structured SQL error"
    );

    connection
        .ping()
        .await
        .expect("the connection must validate");
    shared_session
        .close()
        .await
        .expect("the shared session must close");
    connection
        .close()
        .await
        .expect("the primary session must close");
}
