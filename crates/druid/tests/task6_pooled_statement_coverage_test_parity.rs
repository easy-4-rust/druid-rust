//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
//! Canonical pooled `Statement` public behavior coverage.

use druid::core::{
    Connection, ConnectionFactory, DruidConnectionHolder, DruidPooledConnection, FilterAdapter,
    FilterChainImpl, ProxyAttributeValue,
};
use druid_rdbc::result_set::{ColumnIndex, ResultSetExt};
use druid_rdbc::statement::{
    CurrentResultDisposition, ExecutionResult, FetchDirection, GeneratedKeys, ResultSetConcurrency,
    ResultSetHoldability, ResultSetType, Statement, StatementOptions,
};
use druid_rdbc::wrapper::{StandardUnwrapped, Wrapper};
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::any::TypeId;
use std::sync::Arc;
use std::time::Duration;

async fn make_connection() -> DruidPooledConnection {
    make_connection_with_filter(None, 1, "stmt-coverage").await
}

async fn make_connection_with_chain() -> DruidPooledConnection {
    let mut chain = FilterChainImpl::new();
    chain.add_filter(Arc::new(FilterAdapter::new()));
    make_connection_with_filter(Some(Arc::new(chain)), 2, "stmt-filter-coverage").await
}

async fn make_connection_with_filter(
    filter_chain: Option<Arc<druid::core::FilterChain>>,
    id: u64,
    data_source: &str,
) -> DruidPooledConnection {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("factory");
    let physical = factory.create().await.expect("connection");
    let holder = DruidConnectionHolder::with_standard_connection(physical, id, Duration::ZERO, 0)
        .await
        .expect("holder");
    DruidPooledConnection::with_standard_holder(
        holder,
        data_source.to_owned(),
        filter_chain,
        false,
        false,
        Box::new(|_, _| true),
    )
}

async fn create_statement(connection: &mut DruidPooledConnection) -> Box<dyn Statement> {
    Connection::create_statement(connection, StatementOptions::default())
        .await
        .expect("statement")
}

#[tokio::test]
async fn task6_pooled_statement_coverage_test_statement_batch_add_clear_execute() {
    let mut connection = make_connection().await;
    let mut statement = create_statement(&mut connection).await;
    statement
        .add_batch("CREATE TABLE ps_batch(id INTEGER)".to_owned())
        .unwrap();
    statement
        .add_batch("INSERT INTO ps_batch VALUES (1)".to_owned())
        .unwrap();
    assert_eq!(statement.execute_batch().await.unwrap(), vec![0, 1]);
    statement
        .add_batch("INSERT INTO ps_batch VALUES (2)".to_owned())
        .unwrap();
    statement.clear_batch().unwrap();
    assert_eq!(statement.batch().unwrap(), Vec::<String>::new());
    statement.close().await.unwrap();
}

#[tokio::test]
async fn task6_pooled_statement_coverage_test_statement_batch_through_filter_chain() {
    let mut connection = make_connection_with_chain().await;
    let mut statement = create_statement(&mut connection).await;
    statement
        .add_batch("CREATE TABLE ps_batch2(id INTEGER)".to_owned())
        .unwrap();
    statement
        .add_batch("INSERT INTO ps_batch2 VALUES (1)".to_owned())
        .unwrap();
    assert_eq!(statement.execute_batch().await.unwrap(), vec![0, 1]);
    statement.close().await.unwrap();
}

#[tokio::test]
async fn task6_pooled_statement_coverage_test_statement_cancel() {
    let mut connection = make_connection().await;
    let mut statement = create_statement(&mut connection).await;
    statement.cancel().unwrap();
    statement.close().await.unwrap();
    assert!(statement.is_closed().unwrap());
}

#[tokio::test]
async fn task6_pooled_statement_coverage_test_statement_close_idempotent() {
    let mut connection = make_connection().await;
    let mut statement = create_statement(&mut connection).await;
    statement.close().await.unwrap();
    statement.close().await.unwrap();
    assert!(statement.is_closed().unwrap());
}

#[tokio::test]
async fn task6_pooled_statement_coverage_test_statement_max_field_size_getter_setter() {
    let mut connection = make_connection().await;
    let mut statement = create_statement(&mut connection).await;
    assert_eq!(statement.max_field_size().unwrap(), 0);
    statement.set_max_field_size(1024).unwrap();
    assert_eq!(statement.max_field_size().unwrap(), 1024);
    statement.close().await.unwrap();
    assert!(statement.is_closed().unwrap());
}

#[tokio::test]
async fn task6_pooled_statement_coverage_test_statement_max_rows_getter_setter() {
    let mut connection = make_connection().await;
    let mut statement = create_statement(&mut connection).await;
    assert_eq!(statement.max_rows().unwrap(), 0);
    statement.set_max_rows(100).unwrap();
    assert_eq!(statement.max_rows().unwrap(), 100);
    statement.close().await.unwrap();
    assert!(statement.is_closed().unwrap());
}

#[tokio::test]
async fn task6_pooled_statement_coverage_test_statement_query_timeout_getter_setter() {
    let mut connection = make_connection().await;
    let mut statement = create_statement(&mut connection).await;
    assert_eq!(statement.query_timeout().unwrap(), 0);
    statement.set_query_timeout(30).unwrap();
    assert_eq!(statement.query_timeout().unwrap(), 30);
    statement.close().await.unwrap();
    assert!(statement.is_closed().unwrap());
}

#[tokio::test]
async fn task6_pooled_statement_coverage_test_statement_set_escape_processing() {
    let mut connection = make_connection().await;
    let mut statement = create_statement(&mut connection).await;
    statement.set_escape_processing(true).unwrap();
    statement.set_escape_processing(false).unwrap();
    statement.close().await.unwrap();
    assert!(statement.is_closed().unwrap());
}

#[tokio::test]
async fn task6_pooled_statement_coverage_test_statement_fetch_direction_getter_setter() {
    let mut connection = make_connection().await;
    let mut statement = create_statement(&mut connection).await;
    assert_eq!(
        statement.fetch_direction().unwrap(),
        FetchDirection::Unknown
    );
    statement
        .set_fetch_direction(FetchDirection::Forward)
        .unwrap();
    assert_eq!(
        statement.fetch_direction().unwrap(),
        FetchDirection::Forward
    );
    statement.close().await.unwrap();
    assert!(statement.is_closed().unwrap());
}

#[tokio::test]
async fn task6_pooled_statement_coverage_test_statement_fetch_size_getter_setter() {
    let mut connection = make_connection().await;
    let mut statement = create_statement(&mut connection).await;
    assert_eq!(statement.fetch_size().unwrap(), 0);
    statement.set_fetch_size(50).unwrap();
    assert_eq!(statement.fetch_size().unwrap(), 50);
    statement.close().await.unwrap();
    assert!(statement.is_closed().unwrap());
}

#[tokio::test]
async fn task6_pooled_statement_coverage_test_statement_set_cursor_name() {
    let mut connection = make_connection().await;
    let mut statement = create_statement(&mut connection).await;
    assert!(statement.set_cursor_name("my_cursor").is_ok());
    statement.close().await.unwrap();
    assert!(statement.is_closed().unwrap());
}

#[tokio::test]
async fn task6_pooled_statement_coverage_test_statement_poolable_getter_setter() {
    let mut connection = make_connection().await;
    let mut statement = create_statement(&mut connection).await;
    assert!(!statement.is_poolable().unwrap());
    statement.set_poolable(true).unwrap();
    assert!(statement.is_poolable().unwrap());
    statement.set_poolable(false).unwrap();
    assert!(!statement.is_poolable().unwrap());
    statement.close().await.unwrap();
    assert!(statement.is_closed().unwrap());
}

#[tokio::test]
async fn task6_pooled_statement_coverage_test_statement_close_on_completion() {
    let mut connection = make_connection().await;
    let mut statement = create_statement(&mut connection).await;
    statement.close_on_completion().unwrap();
    assert!(statement.is_close_on_completion().unwrap());
    statement.close().await.unwrap();
    assert!(statement.is_closed().unwrap());
}

#[tokio::test]
async fn task6_pooled_statement_coverage_test_statement_update_count_after_execute() {
    let mut connection = make_connection().await;
    let mut statement = create_statement(&mut connection).await;
    statement
        .execute_update("CREATE TABLE ps_uc(id INTEGER)", GeneratedKeys::None)
        .await
        .unwrap();
    assert_eq!(
        statement
            .execute_update("INSERT INTO ps_uc VALUES (1)", GeneratedKeys::None)
            .await
            .unwrap(),
        1
    );
    assert_eq!(statement.get_update_count().unwrap(), Some(1));
    statement.close().await.unwrap();
}

#[tokio::test]
async fn task6_pooled_statement_coverage_test_statement_result_set_properties() {
    let mut connection = make_connection().await;
    let mut statement = create_statement(&mut connection).await;
    let options = statement.options().unwrap();
    assert_eq!(options.result_set_type(), ResultSetType::ForwardOnly);
    assert_eq!(options.concurrency(), ResultSetConcurrency::ReadOnly);
    assert_eq!(options.holdability(), ResultSetHoldability::CloseAtCommit);
    statement.close().await.unwrap();
    assert!(statement.is_closed().unwrap());
}

#[tokio::test]
async fn task6_pooled_statement_coverage_test_statement_generated_keys() {
    let mut connection = make_connection().await;
    let mut statement = create_statement(&mut connection).await;
    statement
        .execute_update(
            "CREATE TABLE ps_gk(id INTEGER PRIMARY KEY, name TEXT)",
            GeneratedKeys::None,
        )
        .await
        .unwrap();
    statement
        .execute_update(
            "INSERT INTO ps_gk(name) VALUES ('test')",
            GeneratedKeys::ReturnGeneratedKeys,
        )
        .await
        .unwrap();
    let mut keys = statement.generated_keys().await.unwrap();
    assert!(!keys.is_closed().unwrap());
    assert!(keys.next().await.unwrap());
    assert!(keys.long(ColumnIndex::new(1).unwrap()).unwrap() > 0);
    keys.close().await.unwrap();
    statement.close().await.unwrap();
}

#[tokio::test]
async fn task6_pooled_statement_coverage_test_statement_more_results() {
    let mut connection = make_connection().await;
    let mut statement = create_statement(&mut connection).await;
    statement
        .execute_update("CREATE TABLE ps_mr(id INTEGER)", GeneratedKeys::None)
        .await
        .unwrap();
    assert!(!statement
        .get_more_results(CurrentResultDisposition::Default)
        .await
        .unwrap());
    assert!(!statement
        .get_more_results(CurrentResultDisposition::CloseCurrent)
        .await
        .unwrap());
    assert!(!statement
        .get_more_results(CurrentResultDisposition::KeepCurrent)
        .await
        .unwrap());
    assert!(!statement
        .get_more_results(CurrentResultDisposition::CloseAll)
        .await
        .unwrap());
    statement.close().await.unwrap();
    assert!(statement.is_closed().unwrap());
}

#[tokio::test]
async fn task6_pooled_statement_coverage_test_statement_execute_with_generated_keys() {
    let mut connection = make_connection().await;
    let mut statement = create_statement(&mut connection).await;
    statement
        .execute_update(
            "CREATE TABLE ps_egk(id INTEGER PRIMARY KEY, name TEXT)",
            GeneratedKeys::None,
        )
        .await
        .unwrap();
    let results = statement
        .execute(
            "INSERT INTO ps_egk(name) VALUES ('test')",
            GeneratedKeys::ReturnGeneratedKeys,
        )
        .await
        .unwrap();
    assert!(matches!(
        results.first(),
        Some(ExecutionResult::UpdateCount(1))
    ));
    statement.close().await.unwrap();
}

#[tokio::test]
async fn task6_pooled_statement_coverage_test_statement_execute_with_column_indexes() {
    let mut connection = make_connection().await;
    let mut statement = create_statement(&mut connection).await;
    statement
        .execute_update(
            "CREATE TABLE ps_eci(id INTEGER PRIMARY KEY, name TEXT)",
            GeneratedKeys::None,
        )
        .await
        .unwrap();
    let result = statement
        .execute(
            "INSERT INTO ps_eci(name) VALUES ('test')",
            GeneratedKeys::ColumnIndexes(vec![ColumnIndex::new(1).unwrap()]),
        )
        .await;
    assert!(result.is_err());
    statement.close().await.unwrap();
}

#[tokio::test]
async fn task6_pooled_statement_coverage_test_statement_execute_with_column_names() {
    let mut connection = make_connection().await;
    let mut statement = create_statement(&mut connection).await;
    statement
        .execute_update(
            "CREATE TABLE ps_ecn(id INTEGER PRIMARY KEY, name TEXT)",
            GeneratedKeys::None,
        )
        .await
        .unwrap();
    let result = statement
        .execute(
            "INSERT INTO ps_ecn(name) VALUES ('test')",
            GeneratedKeys::ColumnNames(vec!["id".to_owned()]),
        )
        .await;
    assert!(result.is_err());
    statement.close().await.unwrap();
}

#[tokio::test]
async fn task6_pooled_statement_coverage_test_statement_result_set_getter() {
    let mut connection = make_connection().await;
    let mut statement = create_statement(&mut connection).await;
    statement
        .execute_update("CREATE TABLE ps_rs(id INTEGER)", GeneratedKeys::None)
        .await
        .unwrap();
    statement
        .execute_update("INSERT INTO ps_rs VALUES (1)", GeneratedKeys::None)
        .await
        .unwrap();
    let mut results = statement
        .execute("SELECT id FROM ps_rs", GeneratedKeys::None)
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    let ExecutionResult::ResultSet(mut result_set) = results.remove(0) else {
        panic!("query result");
    };
    assert!(result_set.next().await.unwrap());
    assert_eq!(result_set.int(ColumnIndex::new(1).unwrap()).unwrap(), 1);
    result_set.close().await.unwrap();
    assert!(statement.generated_keys().await.is_ok());
    statement.close().await.unwrap();
}

#[tokio::test]
async fn task6_pooled_statement_coverage_test_statement_attributes() {
    let mut connection = make_connection().await;
    let mut statement = create_statement(&mut connection).await;
    assert_eq!(connection.attributes_size(), 0);
    assert!(connection.attributes().is_empty());
    assert!(connection.attribute("missing").is_none());
    connection.put_attribute("key1", ProxyAttributeValue::new(42_i32));
    assert_eq!(connection.attributes_size(), 1);
    let value: Arc<i32> = connection
        .attribute("key1")
        .unwrap()
        .downcast::<i32>()
        .unwrap();
    assert_eq!(*value, 42);
    connection.clear_attributes();
    assert_eq!(connection.attributes_size(), 0);
    statement.close().await.unwrap();
    assert!(statement.is_closed().unwrap());
}

#[tokio::test]
async fn task6_pooled_statement_coverage_test_statement_warnings_through_filter_chain() {
    let mut connection = make_connection_with_chain().await;
    let mut statement = create_statement(&mut connection).await;
    assert_eq!(statement.warnings().unwrap(), None);
    statement.clear_warnings().unwrap();
    statement.close().await.unwrap();
    assert!(statement.is_closed().unwrap());
}

#[tokio::test]
async fn task6_pooled_statement_coverage_test_statement_identity() {
    let mut connection = make_connection().await;
    let first = create_statement(&mut connection).await;
    let second = create_statement(&mut connection).await;
    assert!(std::ptr::eq(first.as_ref(), first.as_ref()));
    assert!(!std::ptr::eq(first.as_ref(), second.as_ref()));
    assert!(matches!(
        Wrapper::unwrap(&*first, Some(TypeId::of::<dyn Statement>())),
        Ok(StandardUnwrapped::Statement(_))
    ));
    assert!(matches!(
        Wrapper::unwrap(&*second, Some(TypeId::of::<dyn Statement>())),
        Ok(StandardUnwrapped::Statement(_))
    ));
}

#[tokio::test]
async fn task6_pooled_statement_coverage_test_statement_id_and_accessor() {
    let mut connection = make_connection().await;
    let statement = create_statement(&mut connection).await;
    assert_eq!(statement.options().unwrap(), StatementOptions::default());
    assert!(Wrapper::is_wrapper_for(&*statement, Some(TypeId::of::<dyn Statement>())).unwrap());
    assert!(matches!(
        Wrapper::unwrap(&*statement, Some(TypeId::of::<dyn Statement>())),
        Ok(StandardUnwrapped::Statement(_))
    ));
}

#[tokio::test]
async fn task6_pooled_statement_coverage_test_statement_debug_format() {
    let mut connection = make_connection().await;
    let statement = create_statement(&mut connection).await;
    let debug = format!(
        "{:?}",
        Wrapper::unwrap(&*statement, Some(TypeId::of::<dyn Statement>())).unwrap()
    );
    assert!(debug.contains("StandardUnwrapped"));
}
