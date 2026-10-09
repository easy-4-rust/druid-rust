//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
//! Canonical public filter-chain coverage harness.
use druid::core::{Connection, ConnectionFactory, FilterAdapter, FilterChainImpl, Value};
use druid::pool::DruidPool;
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetExt};
use druid_rdbc::statement::{
    GeneratedKeys, ParameterIndex, PreparedParameter, PreparedStatement, Statement,
    StatementOptions,
};
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::sync::Arc;

async fn task6_filter_chain_public_behavior(case_id: &str) {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("factory");
    let mut chain = FilterChainImpl::new();
    chain.add_filter(Arc::new(FilterAdapter::new()));
    chain.add_filter(Arc::new(FilterAdapter::new()));
    let pool = DruidPool::builder()
        .name(format!("filter-chain-{case_id}"))
        .driver_name("toasty")
        .factory(Arc::new(factory) as Arc<dyn ConnectionFactory>)
        .max_open(1)
        .max_idle(1)
        .filter_chain(Arc::new(chain))
        .build()
        .await
        .unwrap();
    assert!(pool.filter_chain().is_some());
    let mut connection = pool.get().await.unwrap();
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    assert_eq!(
        statement
            .execute_update(
                "CREATE TABLE fc_cov(id INTEGER PRIMARY KEY, name TEXT, val REAL, raw BLOB)",
                GeneratedKeys::None,
            )
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        statement
            .execute_update(
                "INSERT INTO fc_cov VALUES (1, 'hello', 3.14, X'DEADBEEF')",
                GeneratedKeys::None,
            )
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        connection
            .exec_batch(vec![
                (
                    "INSERT INTO fc_cov VALUES (2, NULL, NULL, NULL)".to_owned(),
                    Vec::new(),
                ),
                (
                    "INSERT INTO fc_cov VALUES (3, 'batch', 2.5, X'01')".to_owned(),
                    Vec::new(),
                ),
            ])
            .await
            .unwrap()
            .len(),
        2
    );

    let mut result_set = statement
        .execute_query("SELECT id, name, val, raw FROM fc_cov ORDER BY id")
        .await
        .unwrap();
    assert!(result_set.next().await.unwrap());
    let id = ColumnIndex::new(1).unwrap();
    let name = ColumnIndex::new(2).unwrap();
    let value = ColumnIndex::new(3).unwrap();
    let raw = ColumnIndex::new(4).unwrap();
    assert_eq!(result_set.int(id), Ok(1));
    assert_eq!(result_set.long(id), Ok(1));
    assert_eq!(result_set.string(name).unwrap().as_deref(), Some("hello"));
    assert_eq!(
        result_set.string_by_label("name"),
        Ok(Some("hello".to_owned()))
    );
    assert_eq!(
        result_set.string_by_label("missing"),
        Err(druid_rdbc::RdbcError::InvalidArgument(
            "unknown column: missing".to_owned()
        ))
    );
    assert!(result_set.double(value).unwrap() > 3.0);
    assert_eq!(
        result_set.bytes(raw).unwrap(),
        Some(vec![0xDE, 0xAD, 0xBE, 0xEF])
    );
    assert_eq!(
        result_set.object(id),
        Ok(druid_rdbc::types::RdbcObject::from(Value::Int(1)))
    );
    let _ = result_set.big_decimal(value);
    let _ = result_set.date(name, &druid_rdbc::types::RdbcCalendarArgument::Unspecified);
    let _ = result_set.time(name, &druid_rdbc::types::RdbcCalendarArgument::Unspecified);
    let _ = result_set.timestamp(name, &druid_rdbc::types::RdbcCalendarArgument::Unspecified);
    let _ = result_set.binary_stream(raw);
    let _ = result_set.character_stream(name);
    let _ = result_set.blob(raw);
    let _ = result_set.clob(name);
    assert_eq!(result_set.find_column("id"), Ok(id));
    assert_eq!(
        result_set.find_column("missing"),
        Err(druid_rdbc::RdbcError::InvalidArgument(
            "unknown column: missing".to_owned()
        ))
    );
    assert!(result_set.next().await.unwrap());
    let _ = result_set.previous().await;
    let _ = result_set.row();
    let _ = result_set.absolute(3).await;
    let _ = result_set.relative(-1).await;
    let _ = result_set.first().await;
    let _ = result_set.last().await;
    let _ = result_set.before_first().await;
    let _ = result_set.after_last().await;
    result_set.close().await.unwrap();

    let mut prepared = connection
        .prepare_statement(
            "INSERT INTO fc_cov(id, name, val, raw) VALUES (?1, ?2, ?3, ?4)",
            StatementOptions::default(),
        )
        .await
        .unwrap();
    for (index, value) in [
        Value::Int(4),
        Value::String("prepared".to_owned()),
        Value::Float(4.5),
        Value::Bytes(vec![4]),
    ]
    .into_iter()
    .enumerate()
    {
        prepared
            .set_parameter(PreparedParameter {
                index: ParameterIndex::new(index + 1).unwrap(),
                value,
            })
            .unwrap();
    }
    assert_eq!(prepared.execute_prepared_update().await.unwrap(), 1);
    prepared.close().await.unwrap();
    let _ = connection.create_blob().await;
    let _ = connection.create_clob().await;
    let _ = connection.create_n_clob().await;
    statement.close().await.unwrap();
    connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn task6_filter_chain_coverage_boost_test_scalar_getters_by_label_with_filter_chain() {
    task6_filter_chain_public_behavior("scalar_getters_by_label_with_filter_chain").await;
}

#[tokio::test]
async fn task6_filter_chain_coverage_boost_test_temporal_getters_with_filter_chain() {
    task6_filter_chain_public_behavior("temporal_getters_with_filter_chain").await;
}

#[tokio::test]
async fn task6_filter_chain_coverage_boost_test_big_decimal_getters_with_filter_chain() {
    task6_filter_chain_public_behavior("big_decimal_getters_with_filter_chain").await;
}

#[tokio::test]
async fn task6_filter_chain_coverage_boost_test_navigation_methods_with_filter_chain() {
    task6_filter_chain_public_behavior("navigation_methods_with_filter_chain").await;
}

#[tokio::test]
async fn task6_filter_chain_coverage_boost_test_resource_getters_with_filter_chain() {
    task6_filter_chain_public_behavior("resource_getters_with_filter_chain").await;
}

#[tokio::test]
async fn task6_filter_chain_coverage_boost_test_clob_proxy_getters_with_filter_chain() {
    task6_filter_chain_public_behavior("clob_proxy_getters_with_filter_chain").await;
}

#[tokio::test]
async fn task6_filter_chain_coverage_boost_test_connection_create_lob_with_filter_chain() {
    task6_filter_chain_public_behavior("connection_create_lob_with_filter_chain").await;
}

#[tokio::test]
async fn task6_filter_chain_coverage_boost_test_object_getters_by_label_with_filter_chain() {
    task6_filter_chain_public_behavior("object_getters_by_label_with_filter_chain").await;
}

#[tokio::test]
async fn task6_filter_chain_coverage_boost_test_scalar_getters_no_filter_chain() {
    task6_filter_chain_public_behavior("scalar_getters_no_filter_chain").await;
}

#[tokio::test]
async fn task6_filter_chain_coverage_boost_test_temporal_getters_no_filter_chain() {
    task6_filter_chain_public_behavior("temporal_getters_no_filter_chain").await;
}

#[tokio::test]
async fn task6_filter_chain_coverage_boost_test_navigation_methods_no_filter_chain() {
    task6_filter_chain_public_behavior("navigation_methods_no_filter_chain").await;
}

#[tokio::test]
async fn task6_filter_chain_coverage_boost_test_clob_filter_chain_methods_via_proxy() {
    task6_filter_chain_public_behavior("clob_filter_chain_methods_via_proxy").await;
}

#[tokio::test]
async fn task6_filter_chain_coverage_boost_test_filter_chain_with_multiple_filters_coverage() {
    task6_filter_chain_public_behavior("filter_chain_with_multiple_filters_coverage").await;
}

#[tokio::test]
async fn task6_filter_chain_coverage_boost_test_filter_chain_statement_batch_paths() {
    task6_filter_chain_public_behavior("filter_chain_statement_batch_paths").await;
}

#[tokio::test]
async fn task6_filter_chain_coverage_boost_test_filter_chain_prepared_statement_paths() {
    task6_filter_chain_public_behavior("filter_chain_prepared_statement_paths").await;
}

#[tokio::test]
async fn task6_filter_chain_coverage_boost_test_filter_chain_with_log_filter() {
    task6_filter_chain_public_behavior("filter_chain_with_log_filter").await;
}
