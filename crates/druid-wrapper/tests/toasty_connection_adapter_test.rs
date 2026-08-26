//! Canonical Toasty runtime-trace contract.

extern crate druid_wrapper as wrapper;

use druid_rdbc::types::Value;
use wrapper::toasty::ToastyConnectionFactory;

#[tokio::test]
async fn toasty_standard_factory_emits_typed_runtime_trace() {
    use druid_rdbc::connection::Connection;

    let mut factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite factory");
    let (trace_probe, trace_expectation) = factory.enable_task4_runtime_trace();
    let mut connection = druid_rdbc::driver::ConnectionFactory::create(&factory)
        .await
        .expect("standard connection");
    let mut statement = connection
        .create_statement(druid_rdbc::statement::StatementOptions::default())
        .await
        .expect("owned standard statement");
    statement
        .execute_update(
            "create table standard_item(id integer primary key, name text)",
            druid_rdbc::statement::GeneratedKeys::None,
        )
        .await
        .expect("standard DDL");
    let mut prepared = connection
        .prepare_statement(
            "select ?",
            druid_rdbc::statement::StatementOptions::default(),
        )
        .await
        .expect("owned standard prepared statement");
    prepared
        .set_parameter(druid_rdbc::statement::PreparedParameter {
            index: druid_rdbc::statement::ParameterIndex::new(1).unwrap(),
            value: Value::String("toasty-standard".to_owned()),
        })
        .expect("standard setter");
    let mut rows = prepared.execute_prepared_query().await.unwrap();
    assert!(rows.next().await.unwrap());
    let snapshot = rows.meta_data().unwrap();
    assert_eq!(
        snapshot
            .column(druid_rdbc::result_set::ColumnIndex::new(1).unwrap())
            .unwrap()
            .label(),
        "column_1"
    );
    assert_eq!(
        rows.value(druid_rdbc::result_set::ColumnIndex::new(1).unwrap())
            .unwrap(),
        Value::String("toasty-standard".to_owned())
    );
    let mut metadata = connection.database_meta_data().await.unwrap();
    assert_eq!(metadata.database_product_name().await.unwrap(), "SQLite");
    Connection::close(connection.as_mut()).await.unwrap();
    assert!(prepared.clear_parameters().is_err());
    assert!(metadata.database_product_name().await.is_err());
    Connection::close(connection.as_mut()).await.unwrap();
    assert_eq!(snapshot.column_count(), 1);
    wrapper::task4_runtime_trace::verify_and_emit(trace_probe, trace_expectation).unwrap();
}
