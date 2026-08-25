//! SQLx Statement/PreparedStatement must own an executor beyond the create-call borrow.

use druid_rdbc::connection::Connection;
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::statement::{GeneratedKeys, PreparedParameter, StatementOptions};
use druid_rdbc::types::Value;
use druid_wrapper::sqlx::{SqlxConnectionAdapter, SqlxConnectionFactory};

#[tokio::test]
async fn sqlx_standard_factory_emits_typed_runtime_trace() {
    let mut factory = SqlxConnectionFactory::new("sqlite::memory:");
    let (trace_probe, trace_expectation) = factory.enable_task4_runtime_trace();
    let mut connection = druid_rdbc::driver::ConnectionFactory::create(&factory)
        .await
        .unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    assert_eq!(
        statement
            .execute_update(
                "create table runtime_trace_item(id integer)",
                GeneratedKeys::None,
            )
            .await
            .unwrap(),
        0
    );
    let mut prepared = connection
        .prepare_statement("select ?1 as traced_value", StatementOptions::default())
        .await
        .unwrap();
    prepared
        .set_parameter(PreparedParameter {
            index: druid_rdbc::statement::ParameterIndex::new(1).unwrap(),
            value: Value::String("sqlx-traced".to_owned()),
        })
        .unwrap();
    let mut rows = prepared.execute_prepared_query().await.unwrap();
    assert!(rows.next().await.unwrap());
    assert_eq!(
        rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::String("sqlx-traced".to_owned())
    );
    let snapshot = rows.meta_data().unwrap();
    assert_eq!(
        snapshot
            .column(ColumnIndex::new(1).unwrap())
            .unwrap()
            .label(),
        "traced_value"
    );
    let mut metadata = connection.database_meta_data().await.unwrap();
    assert_eq!(metadata.database_product_name().await.unwrap(), "SQLite");
    Connection::close(connection.as_mut()).await.unwrap();
    assert!(prepared.clear_parameters().is_err());
    assert!(metadata.database_product_name().await.is_err());
    Connection::close(connection.as_mut()).await.unwrap();
    assert_eq!(snapshot.column_count(), 1);
    druid_wrapper::task4_runtime_trace::verify_and_emit(trace_probe, trace_expectation).unwrap();
}

#[tokio::test]
async fn statement_and_result_set_keep_the_sqlx_connection_lease_owned() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .expect("SQLite connection");
    let mut statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .expect("owned statement");
    drop(connection);

    statement
        .execute_update(
            "create table owned_item(id integer primary key, name text not null)",
            GeneratedKeys::None,
        )
        .await
        .expect("create table after create-call borrow ended");
    statement
        .execute_update(
            "insert into owned_item(id, name) values (1, 'alpha')",
            GeneratedKeys::None,
        )
        .await
        .expect("insert through statement-owned actor");
    let mut result_set = statement
        .execute_query("select name from owned_item where id = 1")
        .await
        .expect("owned result set");
    assert_eq!(
        result_set
            .meta_data()
            .unwrap()
            .column(ColumnIndex::new(1).unwrap())
            .unwrap()
            .label(),
        "name"
    );
    drop(statement);

    assert!(result_set.next().await.expect("advance owned result set"));
    assert_eq!(
        result_set
            .value(ColumnIndex::new(1).expect("1-based index"))
            .expect("current value"),
        Value::String("alpha".to_owned())
    );
    result_set.close().await.expect("close owned result set");
}

#[tokio::test]
async fn prepared_statement_materializes_standard_parameters_through_the_owned_actor() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .expect("SQLite connection");
    let mut prepared =
        Connection::prepare_statement(&mut connection, "select ?", StatementOptions::default())
            .await
            .expect("owned prepared statement");
    prepared
        .set_parameter(PreparedParameter {
            index: druid_rdbc::statement::ParameterIndex::new(1).expect("1-based index"),
            value: Value::String("owned".to_owned()),
        })
        .expect("set standard parameter");
    drop(connection);

    let mut result_set = prepared
        .execute_prepared_query()
        .await
        .expect("prepared query after connection borrow ended");
    drop(prepared);
    assert!(result_set.next().await.expect("advance result set"));
    assert_eq!(
        result_set
            .value(ColumnIndex::new(1).expect("1-based index"))
            .expect("prepared value"),
        Value::String("owned".to_owned())
    );
}

#[tokio::test]
async fn sqlx_standard_close_defaults_and_generated_keys_are_consistent() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let capabilities = Connection::capabilities(&connection);
    assert!(!capabilities.transactions);
    assert!(!capabilities.savepoints);
    assert!(Connection::commit(&mut connection).await.is_err());

    let mut statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .unwrap();
    statement.close().await.unwrap();
    statement.close().await.unwrap();
    assert!(statement.execute_query("select 1").await.is_err());

    let mut prepared =
        Connection::prepare_statement(&mut connection, "select ?1", StatementOptions::default())
            .await
            .unwrap();
    prepared.close().await.unwrap();
    assert!(prepared
        .set_parameter(PreparedParameter {
            index: druid_rdbc::statement::ParameterIndex::new(1).unwrap(),
            value: Value::Int(1),
        })
        .is_err());

    let mut statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .unwrap();
    let error = statement
        .execute_update(
            "create table generated_key_guard(id integer)",
            GeneratedKeys::ReturnGeneratedKeys,
        )
        .await
        .expect_err("unsupported generated keys must not be ignored");
    assert!(matches!(
        error,
        druid_rdbc::error::RdbcError::UnsupportedOperation { .. }
    ));
}

#[tokio::test]
async fn sqlx_parameter_metadata_uses_driver_placeholder_description() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut prepared = Connection::prepare_statement(
        &mut connection,
        "select '?' as literal, ?1 as bound_value -- ? in comment",
        StatementOptions::default(),
    )
    .await
    .unwrap();
    assert_eq!(prepared.parameter_meta_data().unwrap().parameter_count(), 1);
    prepared
        .set_parameter(PreparedParameter {
            index: druid_rdbc::statement::ParameterIndex::new(1).unwrap(),
            value: Value::String("driver-described".to_owned()),
        })
        .unwrap();
    let mut rows = prepared.execute_prepared_query().await.unwrap();
    assert!(rows.next().await.unwrap());
    assert_eq!(
        rows.value(ColumnIndex::new(2).unwrap()).unwrap(),
        Value::String("driver-described".to_owned())
    );
}

#[tokio::test]
async fn sqlx_connection_close_serializes_with_statement_execute_and_releases_in_both_orders() {
    use std::sync::Arc;
    use tokio::sync::{Barrier, Mutex};

    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .unwrap();
    let connection = Arc::new(Mutex::new(connection));
    let statement = Arc::new(Mutex::new(statement));
    let barrier = Arc::new(Barrier::new(2));
    let close_task = {
        let connection = Arc::clone(&connection);
        let barrier = Arc::clone(&barrier);
        tokio::spawn(async move {
            barrier.wait().await;
            Connection::close(&mut *connection.lock().await).await
        })
    };
    let execute_task = {
        let statement = Arc::clone(&statement);
        let barrier = Arc::clone(&barrier);
        tokio::spawn(async move {
            barrier.wait().await;
            statement
                .lock()
                .await
                .execute_update("create table close_race(id integer)", GeneratedKeys::None)
                .await
        })
    };
    assert!(close_task.await.unwrap().is_ok());
    let _ = execute_task.await.unwrap();
    assert!(statement
        .lock()
        .await
        .execute_query("select 1")
        .await
        .is_err());

    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .unwrap();
    statement.close().await.unwrap();
    assert!(Connection::close(&mut connection).await.is_ok());
}

#[tokio::test]
async fn sqlx_connection_first_close_rejects_prepared_mutation_and_preserves_snapshot() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut prepared =
        Connection::prepare_statement(&mut connection, "select ?1", StatementOptions::default())
            .await
            .unwrap();
    let snapshot = prepared.parameter_meta_data().unwrap();
    Connection::close(&mut connection).await.unwrap();

    let parameter = PreparedParameter {
        index: druid_rdbc::statement::ParameterIndex::new(1).unwrap(),
        value: Value::Int(7),
    };
    assert!(prepared.set_parameter(parameter).is_err());
    assert!(prepared.clear_parameters().is_err());
    assert!(prepared.execute_prepared_query().await.is_err());
    assert_eq!(snapshot.parameter_count(), 1);
}
