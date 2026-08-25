//! SQLx live metadata owns its actor while metadata snapshots remain connection-free.

use druid_rdbc::connection::Connection;
use druid_rdbc::metadata::TablesQuery;
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::statement::StatementOptions;
use druid_rdbc::types::Value;
use druid_wrapper::sqlx::SqlxConnectionAdapter;

#[tokio::test]
async fn live_database_metadata_and_its_result_set_outlive_the_connection_call_borrow() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .expect("SQLite connection");
    let mut statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .expect("owned statement");
    statement
        .execute_update(
            "create table metadata_owned(id integer primary key)",
            druid_rdbc::statement::GeneratedKeys::None,
        )
        .await
        .expect("create table");
    drop(statement);

    let mut metadata = Connection::database_meta_data(&mut connection)
        .await
        .expect("owned live metadata");
    drop(connection);
    assert_eq!(
        metadata.database_product_name().await.expect("product"),
        "SQLite"
    );
    let mut tables = metadata
        .tables(TablesQuery {
            table_name_pattern: Some("metadata_owned".to_owned()),
            ..TablesQuery::default()
        })
        .await
        .expect("owned metadata result set");
    drop(metadata);
    assert!(tables.next().await.expect("advance metadata rows"));
    assert_eq!(
        tables
            .value(ColumnIndex::new(3).expect("TABLE_NAME index"))
            .expect("table name"),
        Value::String("metadata_owned".to_owned())
    );

    let snapshot = tables.meta_data().expect("immutable result metadata");
    drop(tables);
    assert!(snapshot.column_count() >= 4);
}

#[tokio::test]
async fn parameter_metadata_snapshot_is_arc_owned_and_connection_free() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .expect("SQLite connection");
    let prepared =
        Connection::prepare_statement(&mut connection, "select ?, ?", StatementOptions::default())
            .await
            .expect("owned prepared statement");
    let snapshot = prepared
        .parameter_meta_data()
        .expect("immutable parameter metadata");
    drop(prepared);
    drop(connection);
    assert_eq!(snapshot.parameter_count(), 2);
}
