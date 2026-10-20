//! Task 6 Druid prepared/callable Statement-supertrait property matrix.

#[path = "standard_test_support/mod.rs"]
mod standard_test_support;

use druid::core::ConnectionFactory;
use druid::pool::DruidPool;
use druid_rdbc::connection::Connection;
use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::{
    FetchDirection, ResultSetConcurrency, ResultSetHoldability, ResultSetType, Statement,
    StatementOptions,
};
use standard_test_support::{TrackingFactory, TrackingState};
use std::sync::Arc;

fn assert_all_properties(statement: &mut dyn Statement, options: StatementOptions) {
    assert_eq!(statement.options().expect("options"), options);
    assert_eq!(statement.max_field_size().expect("max field size"), 0);
    statement.set_max_field_size(1_024).expect("set field size");
    assert_eq!(statement.max_field_size().expect("max field size"), 1_024);
    assert_eq!(statement.max_rows().expect("max rows"), 0);
    statement.set_max_rows(100).expect("set max rows");
    assert_eq!(statement.max_rows().expect("max rows"), 100);
    assert_eq!(statement.query_timeout().expect("query timeout"), 0);
    statement.set_query_timeout(3).expect("set timeout");
    assert_eq!(statement.query_timeout().expect("query timeout"), 3);
    assert_eq!(
        statement.fetch_direction().expect("fetch direction"),
        FetchDirection::Unknown
    );
    statement
        .set_fetch_direction(FetchDirection::Reverse)
        .expect("set fetch direction");
    assert_eq!(
        statement.fetch_direction().expect("fetch direction"),
        FetchDirection::Reverse
    );
    assert_eq!(statement.fetch_size().expect("fetch size"), 0);
    statement.set_fetch_size(25).expect("set fetch size");
    assert_eq!(statement.fetch_size().expect("fetch size"), 25);
    statement
        .set_escape_processing(false)
        .expect("escape processing");
    statement.set_cursor_name("cursor").expect("cursor name");
    assert!(matches!(
        statement.cancel(),
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_cancel"
        })
    ));
    assert!(!statement.is_poolable().expect("poolable"));
    statement.set_poolable(true).expect("set poolable");
    assert!(statement.is_poolable().expect("poolable"));
    statement
        .close_on_completion()
        .expect("close on completion");
    assert!(statement
        .is_close_on_completion()
        .expect("close-on-completion state"));
}

#[tokio::test]
async fn prepared_and_callable_share_complete_logical_statement_properties() {
    let tracking = TrackingState::configured();
    let pool = DruidPool::builder()
        .name("task6-statement-properties")
        .driver_name("tracking")
        .factory(Arc::new(TrackingFactory::new(tracking)) as Arc<dyn ConnectionFactory>)
        .max_open(1)
        .max_idle(1)
        .build()
        .await
        .expect("pool");
    let mut connection = pool.get().await.expect("connection");
    let options = StatementOptions::new(
        ResultSetType::ScrollInsensitive,
        ResultSetConcurrency::Updatable,
        ResultSetHoldability::HoldOverCommit,
    );

    let mut prepared = connection
        .prepare_statement("SELECT 1", options)
        .await
        .expect("prepared");
    assert_all_properties(&mut *prepared, options);
    prepared.close().await.expect("prepared close");
    assert!(prepared.is_closed().expect("prepared closed"));

    let mut callable = connection
        .prepare_call("CALL task6(?)", options)
        .await
        .expect("callable");
    assert_all_properties(&mut *callable, options);
    callable.close().await.expect("callable close");
    assert!(callable.is_closed().expect("callable closed"));

    connection.close().await.expect("connection close");
    pool.close().await;
}
