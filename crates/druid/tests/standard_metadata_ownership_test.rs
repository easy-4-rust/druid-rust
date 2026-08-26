mod standard_test_support;

use druid::core::{DruidConnectionHolder, DruidPooledConnection};
use druid_rdbc::connection::Connection;
use druid_rdbc::metadata::TablesQuery;
use druid_rdbc::result_set::ColumnIndex;
use druid_rdbc::statement::{GeneratedKeys, ParameterIndex, Statement, StatementOptions};
use druid_rdbc::types::Value;
use standard_test_support::{TrackingConnection, TrackingState};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[tokio::test]
async fn live_metadata_owns_logical_actor_and_snapshots_are_borrow_free() {
    let state = TrackingState::configured();
    let holder = DruidConnectionHolder::with_standard_connection(
        Box::new(TrackingConnection::new(state.clone())),
        61,
        Duration::ZERO,
        0,
    )
    .await
    .expect("standard holder");
    let returned = Arc::new(Mutex::new(None));
    let returned_holder = Arc::clone(&returned);
    let mut connection = DruidPooledConnection::with_standard_holder(
        holder,
        "task5-metadata".to_string(),
        None,
        false,
        false,
        Box::new(move |holder, _disposition| {
            *returned_holder
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(holder);
            false
        }),
    );

    let mut metadata = Connection::database_meta_data(&mut connection)
        .await
        .expect("owned live metadata");
    assert_eq!(
        metadata
            .database_product_name()
            .await
            .expect("live product name"),
        "Task5DB"
    );
    let mut tables = metadata
        .tables(TablesQuery::default())
        .await
        .expect("owned metadata result set");
    assert!(tables.next().await.expect("metadata result next"));
    assert_eq!(
        tables
            .value(ColumnIndex::new(1).expect("1-based column"))
            .expect("table name"),
        Value::String("task5_table".to_string())
    );
    let tables_snapshot = tables.meta_data().expect("metadata table snapshot");

    let mut prepared =
        Connection::prepare_statement(&mut connection, "SELECT ?", StatementOptions::default())
            .await
            .expect("prepared statement");
    let parameter_snapshot = prepared
        .parameter_meta_data()
        .expect("immutable parameter metadata");
    let mut result = prepared
        .execute_prepared_query()
        .await
        .expect("prepared result");
    assert!(result.next().await.expect("prepared result next"));
    let result_snapshot = result.meta_data().expect("immutable result metadata");
    let mut stale_statement =
        Connection::create_statement(&mut connection, StatementOptions::default())
            .await
            .expect("owned statement actor");

    prepared.close().await.expect("prepared close");
    Connection::close(&mut connection)
        .await
        .expect("logical connection close");
    let holder = returned
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
        .expect("first metadata lease returned holder");
    let mut second = DruidPooledConnection::with_standard_holder(
        holder,
        "task5-metadata-second".to_owned(),
        None,
        false,
        false,
        Box::new(|_holder, _disposition| false),
    );
    let mut second_statement =
        Connection::create_statement(&mut second, StatementOptions::default())
            .await
            .expect("second lease proves holder reborrow");
    assert!(metadata.database_product_name().await.is_err());
    assert!(tables.next().await.is_err());
    assert!(tables
        .value(ColumnIndex::new(1).expect("1-based column"))
        .is_err());
    assert!(tables.meta_data().is_err());
    assert!(Statement::is_closed(stale_statement.as_ref()).expect("logical close state"));
    assert!(stale_statement
        .execute_update("UPDATE stale", GeneratedKeys::None)
        .await
        .is_err());
    stale_statement
        .close()
        .await
        .expect("stale statement cleanup");
    assert!(result
        .value(ColumnIndex::new(1).expect("1-based column"))
        .is_err());
    assert!(result.meta_data().is_err());
    assert!(result.next().await.is_err());
    result.close().await.expect("stale result cleanup");
    drop(metadata);
    second_statement
        .close()
        .await
        .expect("second statement close");
    Connection::close(&mut second)
        .await
        .expect("second lease close");

    fn assert_snapshot<T: Send + Sync + 'static + ?Sized>(_snapshot: Arc<T>) {}
    assert_snapshot(result_snapshot.clone());
    assert_snapshot(parameter_snapshot.clone());
    assert_snapshot(tables_snapshot.clone());
    assert_eq!(tables_snapshot.column_count(), 1);
    assert_eq!(result_snapshot.column_count(), 1);
    assert_eq!(parameter_snapshot.parameter_count(), 1);
    assert_eq!(
        parameter_snapshot
            .parameter(ParameterIndex::new(1).expect("1-based parameter"))
            .expect("parameter descriptor")
            .type_name(),
        "BIGINT"
    );
}
