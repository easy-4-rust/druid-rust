//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
#![allow(clippy::approx_constant)]
//! Canonical pooled `PreparedStatement` coverage contract.

use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid::core::{
    Connection, ConnectionFactory, DruidConnectionHolder, DruidPooledConnection, FilterAdapter,
    FilterChainImpl, ProxyAttributeValue, Value,
};
use druid_rdbc::metadata::ParameterMode;
use druid_rdbc::result_set::{ColumnIndex, ResultSetExt};
use druid_rdbc::statement::{
    CurrentResultDisposition, ExecutionResult, FetchDirection, GeneratedKeys, ParameterIndex,
    PreparedInputParameter, PreparedParameter, PreparedStatement, ResultSetConcurrency,
    ResultSetHoldability, ResultSetType, Statement, StatementOptions,
};
use druid_rdbc::types::{RdbcCalendarArgument, RdbcCharacterLength, RdbcStreamLength};
use druid_rdbc::wrapper::{StandardUnwrapped, Wrapper};
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::any::TypeId;
use std::sync::Arc;
use std::time::Duration;

async fn make_connection() -> DruidPooledConnection {
    make_connection_with_filter(None, 1, "ps-coverage").await
}

async fn make_connection_with_chain() -> DruidPooledConnection {
    let mut chain = FilterChainImpl::new();
    chain.add_filter(Arc::new(FilterAdapter::new()));
    make_connection_with_filter(Some(Arc::new(chain)), 2, "ps-filter-coverage").await
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

async fn prepare_statement(
    connection: &mut DruidPooledConnection,
    sql: &str,
) -> Box<dyn PreparedStatement> {
    Connection::prepare_statement(connection, sql, StatementOptions::default())
        .await
        .expect("prepared statement")
}

async fn setup_table(connection: &mut DruidPooledConnection) {
    let mut statement = create_statement(connection).await;
    statement
        .execute_update(
            "CREATE TABLE IF NOT EXISTS ps_cov(id INTEGER PRIMARY KEY, name TEXT, val REAL)",
            GeneratedKeys::None,
        )
        .await
        .expect("create table");
    statement.close().await.expect("statement close");
}

fn index(value: usize) -> ParameterIndex {
    ParameterIndex::new(value).expect("1-based parameter index")
}

fn parameter(
    value: usize,
    input: PreparedInputParameter,
) -> (ParameterIndex, PreparedInputParameter) {
    (index(value), input)
}

#[tokio::test]
async fn task6_pooled_prepared_statement_coverage_test_ps_parameter_slot_count_and_rdbc_parameters()
{
    let mut connection = make_connection().await;
    setup_table(&mut connection).await;
    let mut prepared = prepare_statement(
        &mut connection,
        "INSERT INTO ps_cov(id, name) VALUES (?1, ?2)",
    )
    .await;
    let metadata = prepared.parameter_meta_data().unwrap();
    assert_eq!(metadata.parameter_count(), 2);
    assert_eq!(metadata.parameter(index(1)).unwrap().type_name(), "UNKNOWN");
    assert_eq!(metadata.parameter(index(2)).unwrap().type_name(), "UNKNOWN");
    assert!(matches!(
        metadata.parameter(index(1)).unwrap().mode(),
        ParameterMode::In
    ));
    assert!(metadata.parameter(index(3)).is_err());
    prepared
        .set_parameter(PreparedParameter {
            index: index(1),
            value: Value::Int(1),
        })
        .unwrap();
    prepared
        .set_parameter(PreparedParameter {
            index: index(2),
            value: Value::String("test".to_owned()),
        })
        .unwrap();
    assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
    assert_eq!(
        connection
            .fetch("SELECT id, name FROM ps_cov WHERE id = 1", Vec::new())
            .await
            .unwrap()[0]
            .values,
        vec![Value::Int(1), Value::String("test".to_owned())]
    );
    assert_eq!(metadata.parameter_count(), 2);
    assert_eq!(
        prepared.sql().unwrap(),
        "INSERT INTO ps_cov(id, name) VALUES (?1, ?2)"
    );
    assert!(ParameterIndex::new(0).is_err());
    assert_eq!(index(2).get(), 2);
    prepared.close().await.unwrap();
}

#[tokio::test]
async fn task6_pooled_prepared_statement_coverage_test_ps_property_getter_setter_families() {
    let mut connection = make_connection().await;
    setup_table(&mut connection).await;
    let mut prepared = prepare_statement(&mut connection, "SELECT 1").await;
    let options = prepared.options().unwrap();
    assert_eq!(options.result_set_type(), ResultSetType::ForwardOnly);
    assert_eq!(options.concurrency(), ResultSetConcurrency::ReadOnly);
    assert_eq!(options.holdability(), ResultSetHoldability::CloseAtCommit);
    assert_eq!(prepared.max_field_size().unwrap(), 0);
    prepared.set_max_field_size(2048).unwrap();
    assert_eq!(prepared.max_field_size().unwrap(), 2048);
    assert_eq!(prepared.max_rows().unwrap(), 0);
    prepared.set_max_rows(500).unwrap();
    assert_eq!(prepared.max_rows().unwrap(), 500);
    assert_eq!(prepared.query_timeout().unwrap(), 0);
    prepared.set_query_timeout(60).unwrap();
    assert_eq!(prepared.query_timeout().unwrap(), 60);
    prepared.set_escape_processing(true).unwrap();
    prepared.set_escape_processing(false).unwrap();
    prepared
        .set_fetch_direction(FetchDirection::Forward)
        .unwrap();
    assert_eq!(prepared.fetch_direction().unwrap(), FetchDirection::Forward);
    prepared.set_fetch_size(25).unwrap();
    assert_eq!(prepared.fetch_size().unwrap(), 25);
    let _ = prepared.set_cursor_name("ps_cursor");
    let _ = prepared.cancel();
    assert!(!prepared.is_poolable().unwrap());
    prepared.set_poolable(true).unwrap();
    assert!(prepared.is_poolable().unwrap());
    prepared.close_on_completion().unwrap();
    assert!(prepared.is_close_on_completion().unwrap());
    assert!(matches!(
        Wrapper::unwrap(&*prepared, Some(TypeId::of::<dyn Statement>())),
        Ok(StandardUnwrapped::Statement(_))
    ));
    prepared.close().await.unwrap();
}

#[tokio::test]
async fn task6_pooled_prepared_statement_coverage_test_ps_warnings_through_filter_chain() {
    let mut connection = make_connection_with_chain().await;
    setup_table(&mut connection).await;
    let mut prepared = prepare_statement(&mut connection, "SELECT 1").await;
    assert!(prepared.warnings().unwrap().is_none());
    prepared.clear_warnings().unwrap();
    assert!(prepared.warnings().unwrap().is_none());
    prepared.close().await.unwrap();
}

#[tokio::test]
async fn task6_pooled_prepared_statement_coverage_test_ps_close_idempotent() {
    let mut connection = make_connection().await;
    setup_table(&mut connection).await;
    let mut prepared = prepare_statement(&mut connection, "SELECT 1").await;
    prepared.close().await.unwrap();
    assert!(prepared.is_closed().unwrap());
    prepared.close().await.unwrap();
}

#[tokio::test]
async fn task6_pooled_prepared_statement_coverage_test_ps_close_without_connection() {
    let mut connection = make_connection().await;
    setup_table(&mut connection).await;
    let mut prepared = prepare_statement(&mut connection, "SELECT 1").await;
    prepared.close().await.unwrap();
    assert!(prepared.is_closed().unwrap());
}

#[tokio::test]
async fn task6_pooled_prepared_statement_coverage_test_ps_id_and_identity() {
    let mut connection = make_connection().await;
    let first = prepare_statement(&mut connection, "SELECT 1").await;
    let second = prepare_statement(&mut connection, "SELECT 2").await;
    assert_eq!(first.sql().unwrap(), "SELECT 1");
    assert_eq!(second.sql().unwrap(), "SELECT 2");
    assert_ne!(first.sql().unwrap(), second.sql().unwrap());
}

#[tokio::test]
async fn task6_pooled_prepared_statement_coverage_test_ps_wrapper_trait() {
    let mut connection = make_connection().await;
    let prepared = prepare_statement(&mut connection, "SELECT 1").await;
    assert!(
        Wrapper::is_wrapper_for(&*prepared, Some(TypeId::of::<dyn PreparedStatement>())).unwrap()
    );
    assert!(Wrapper::is_wrapper_for(&*prepared, Some(TypeId::of::<dyn Statement>())).unwrap());
    assert!(Wrapper::is_wrapper_for(&*prepared, None).is_err());
    assert!(!Wrapper::is_wrapper_for(&*prepared, Some(TypeId::of::<String>())).unwrap());
    assert!(matches!(
        Wrapper::unwrap(&*prepared, Some(TypeId::of::<dyn PreparedStatement>())),
        Ok(StandardUnwrapped::PreparedStatement(_))
    ));
    assert!(matches!(
        Wrapper::unwrap(&*prepared, Some(TypeId::of::<dyn Statement>())),
        Ok(StandardUnwrapped::Statement(_))
    ));
    assert!(Wrapper::unwrap(&*prepared, None).is_err());
}

#[tokio::test]
async fn task6_pooled_prepared_statement_coverage_test_ps_debug_format() {
    let mut connection = make_connection().await;
    let prepared = prepare_statement(&mut connection, "SELECT 1").await;
    let debug = format!(
        "{:?}",
        Wrapper::unwrap(&*prepared, Some(TypeId::of::<dyn PreparedStatement>())).unwrap()
    );
    assert!(debug.contains("StandardUnwrapped"));
}

#[tokio::test]
async fn task6_pooled_prepared_statement_coverage_test_ps_attributes_via_base() {
    let mut connection = make_connection().await;
    setup_table(&mut connection).await;
    let prepared = prepare_statement(&mut connection, "SELECT 1").await;
    let base = Wrapper::unwrap(&*prepared, Some(TypeId::of::<dyn Statement>())).unwrap();
    assert!(matches!(base, StandardUnwrapped::Statement(_)));
    assert_eq!(connection.attributes_size(), 0);
    connection.put_attribute("key", ProxyAttributeValue::new(42_i32));
    assert_eq!(connection.attributes_size(), 1);
    connection.clear_attributes();
    assert_eq!(connection.attributes_size(), 0);
}

#[tokio::test]
async fn task6_pooled_prepared_statement_coverage_test_ps_property_restore_on_close() {
    let mut connection = make_connection().await;
    let mut prepared = prepare_statement(&mut connection, "SELECT 1").await;
    prepared.set_max_field_size(9999).unwrap();
    prepared.set_max_rows(9999).unwrap();
    prepared.set_query_timeout(99).unwrap();
    prepared
        .set_fetch_direction(FetchDirection::Reverse)
        .unwrap();
    prepared.set_fetch_size(999).unwrap();
    prepared.close().await.unwrap();
    let mut second = prepare_statement(&mut connection, "SELECT 1").await;
    assert_eq!(second.max_field_size().unwrap(), 0);
    assert_eq!(second.max_rows().unwrap(), 0);
    assert_eq!(second.query_timeout().unwrap(), 0);
    assert_eq!(second.fetch_direction().unwrap(), FetchDirection::Unknown);
    assert_eq!(second.fetch_size().unwrap(), 0);
    second.close().await.unwrap();
}
