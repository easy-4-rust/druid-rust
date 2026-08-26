//! Canonical PreparedStatement current-result disposition contracts.

use druid::core::{Connection, ConnectionFactory, DruidConnectionHolder, DruidPooledConnection};
use druid_rdbc::statement::{
    CurrentResultDisposition, ExecutionResult, PreparedStatement, StatementOptions,
};
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::time::Duration;

async fn prepared_query() -> (DruidPooledConnection, Box<dyn PreparedStatement>) {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("Toasty SQLite factory");
    let connection = factory.create().await.expect("Toasty SQLite connection");
    let holder = DruidConnectionHolder::with_standard_connection(connection, 61, Duration::ZERO, 0)
        .await
        .expect("canonical holder");
    let mut connection = DruidPooledConnection::with_standard_holder(
        holder,
        "prepared-more-results".to_owned(),
        None,
        false,
        false,
        Box::new(|_, _| false),
    );
    let prepared = connection
        .prepare_statement("SELECT 1", StatementOptions::default())
        .await
        .expect("prepared query");
    (connection, prepared)
}

fn take_result_set(results: Vec<ExecutionResult>) -> Box<dyn druid_rdbc::result_set::ResultSet> {
    let mut results = results.into_iter();
    match (results.next(), results.next()) {
        (Some(ExecutionResult::ResultSet(result_set)), None) => result_set,
        _ => panic!("prepared query must return exactly one ResultSet"),
    }
}

#[tokio::test]
async fn prepared_more_results_dispositions_close_or_retain_current_results() {
    let (mut connection, mut prepared) = prepared_query().await;

    let kept = take_result_set(prepared.execute_prepared().await.unwrap());
    assert!(!kept.is_closed().unwrap());
    assert!(!prepared
        .get_more_results(CurrentResultDisposition::KeepCurrent)
        .await
        .unwrap());
    assert!(!kept.is_closed().unwrap());

    let current = take_result_set(prepared.execute_prepared().await.unwrap());
    assert!(!prepared
        .get_more_results(CurrentResultDisposition::CloseCurrent)
        .await
        .unwrap());
    assert!(current.is_closed().unwrap());
    assert!(!kept.is_closed().unwrap());

    let final_result = take_result_set(prepared.execute_prepared().await.unwrap());
    assert!(!prepared
        .get_more_results(CurrentResultDisposition::CloseAll)
        .await
        .unwrap());
    assert!(kept.is_closed().unwrap());
    assert!(final_result.is_closed().unwrap());
    assert_eq!(prepared.get_update_count().unwrap(), None);

    prepared.close().await.unwrap();
    connection.close().await.unwrap();
}
