#![cfg(feature = "duckdb-native")]

//! Canonical DuckDB runtime-trace contract.

use druid_rdbc::types::Value;
use druid_wrapper::duckdb::DuckDbConnectionFactory;

#[tokio::test]
async fn duckdb_standard_factory_emits_typed_runtime_trace() {
    use druid_rdbc::connection::Connection;

    let mut factory = DuckDbConnectionFactory::new("duckdb::memory:");
    let (trace_probe, trace_expectation) = factory.enable_task4_runtime_trace();
    let mut connection = druid_rdbc::driver::ConnectionFactory::create(&factory)
        .await
        .expect("standard DuckDB connection");
    let mut statement = connection
        .create_statement(druid_rdbc::statement::StatementOptions::default())
        .await
        .expect("owned DuckDB statement");
    statement
        .execute_update(
            "create table standard_duck(id bigint, name varchar)",
            druid_rdbc::statement::GeneratedKeys::None,
        )
        .await
        .unwrap();
    let mut prepared = connection
        .prepare_statement(
            "select ? as name",
            druid_rdbc::statement::StatementOptions::default(),
        )
        .await
        .unwrap();
    prepared
        .set_parameter(druid_rdbc::statement::PreparedParameter {
            index: druid_rdbc::statement::ParameterIndex::new(1).unwrap(),
            value: Value::String("duck-standard".to_owned()),
        })
        .unwrap();
    let mut rows = prepared.execute_prepared_query().await.unwrap();
    assert!(rows.next().await.unwrap());
    assert_eq!(
        rows.value(druid_rdbc::result_set::ColumnIndex::new(1).unwrap())
            .unwrap(),
        Value::String("duck-standard".to_owned())
    );
    let snapshot = rows.meta_data().unwrap();
    assert_eq!(
        snapshot
            .column(druid_rdbc::result_set::ColumnIndex::new(1).unwrap())
            .unwrap()
            .label(),
        "name"
    );
    let mut metadata = connection.database_meta_data().await.unwrap();
    assert_eq!(metadata.database_product_name().await.unwrap(), "DuckDB");
    Connection::close(connection.as_mut()).await.unwrap();
    assert!(prepared.clear_parameters().is_err());
    assert!(metadata.database_product_name().await.is_err());
    Connection::close(connection.as_mut()).await.unwrap();
    assert_eq!(snapshot.column_count(), 1);
    druid_wrapper::task4_runtime_trace::verify_and_emit(trace_probe, trace_expectation).unwrap();
}

#[tokio::test]
async fn duckdb_capabilities_match_standard_transaction_behavior() {
    use druid_rdbc::connection::Connection;
    use druid_rdbc::error::RdbcError;
    use druid_rdbc::statement::{GeneratedKeys, StatementOptions};

    let factory = DuckDbConnectionFactory::new("duckdb::memory:");
    let mut connection = druid_rdbc::driver::ConnectionFactory::create(&factory)
        .await
        .expect("standard DuckDB connection");
    let capabilities = connection.capabilities();
    assert!(!capabilities.transactions);
    assert!(!capabilities.savepoints);

    assert!(matches!(
        connection.set_auto_commit(false).await,
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_set_auto_commit"
        })
    ));
    assert!(matches!(
        connection.commit().await,
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_commit"
        })
    ));
    assert!(matches!(
        connection.rollback().await,
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_rollback"
        })
    ));
    assert!(matches!(
        connection.set_savepoint().await,
        Err(RdbcError::UnsupportedOperation {
            operation: "connection_set_savepoint"
        })
    ));

    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .expect("connection remains usable after unsupported controls");
    assert_eq!(
        statement
            .execute_update(
                "create table capability_probe(id integer)",
                GeneratedKeys::None
            )
            .await
            .expect("create table"),
        0
    );
    Connection::close(connection.as_mut())
        .await
        .expect("close DuckDB connection");
}
