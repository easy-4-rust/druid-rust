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

#[tokio::test]
async fn live_metadata_observes_connection_first_close() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut metadata = Connection::database_meta_data(&mut connection)
        .await
        .unwrap();
    Connection::close(&mut connection).await.unwrap();
    assert!(metadata.database_product_name().await.is_err());
    assert!(matches!(
        metadata.tables(TablesQuery::default()).await,
        Err(druid_rdbc::error::RdbcError::ConnectionDiscarded)
    ));
}

#[tokio::test]
async fn result_set_was_null_tracks_each_successful_getter_and_preserves_errors() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .unwrap();
    let mut rows = statement
        .execute_query("select null, 7 union all select 8, null")
        .await
        .unwrap();
    assert!(rows.next().await.unwrap());
    assert_eq!(
        rows.value(ColumnIndex::new(1).unwrap()).unwrap(),
        Value::Null
    );
    assert!(rows.was_null().unwrap());
    assert_eq!(
        rows.value(ColumnIndex::new(2).unwrap()).unwrap(),
        Value::Int(7)
    );
    assert!(!rows.was_null().unwrap());
    assert!(rows.next().await.unwrap());
    assert!(!rows.was_null().unwrap());
    assert_eq!(
        rows.value(ColumnIndex::new(2).unwrap()).unwrap(),
        Value::Null
    );
    assert!(rows.was_null().unwrap());
    rows.close().await.unwrap();
    assert!(rows.value(ColumnIndex::new(1).unwrap()).is_err());
    assert!(rows.was_null().unwrap());
}

#[tokio::test]
async fn sqlite_tables_query_preserves_none_empty_and_type_filters() {
    let mut connection = SqlxConnectionAdapter::connect("sqlite::memory:")
        .await
        .unwrap();
    let mut statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .unwrap();
    statement
        .execute_update(
            "create table table_item(id integer)",
            druid_rdbc::statement::GeneratedKeys::None,
        )
        .await
        .unwrap();
    statement
        .execute_update(
            "create view view_item as select id from table_item",
            druid_rdbc::statement::GeneratedKeys::None,
        )
        .await
        .unwrap();
    let mut metadata = Connection::database_meta_data(&mut connection)
        .await
        .unwrap();

    let mut empty = metadata
        .tables(TablesQuery {
            table_types: Some(Vec::new()),
            ..TablesQuery::default()
        })
        .await
        .unwrap();
    assert!(!empty.next().await.unwrap());
    for query in [
        TablesQuery {
            catalog: Some("not_sqlite".to_owned()),
            ..TablesQuery::default()
        },
        TablesQuery {
            schema_pattern: Some("not_main".to_owned()),
            ..TablesQuery::default()
        },
    ] {
        let mut rows = metadata.tables(query).await.unwrap();
        assert!(!rows.next().await.unwrap());
    }
    let mut tables = metadata
        .tables(TablesQuery {
            table_types: Some(vec!["TABLE".to_owned()]),
            ..TablesQuery::default()
        })
        .await
        .unwrap();
    let mut names = Vec::new();
    while tables.next().await.unwrap() {
        names.push(tables.value(ColumnIndex::new(3).unwrap()).unwrap());
    }
    assert!(names.contains(&Value::String("table_item".to_owned())));
    assert!(!names.contains(&Value::String("view_item".to_owned())));
}
