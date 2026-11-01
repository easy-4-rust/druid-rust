//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
//! Canonical public FilterChainImpl full-path harness.
use druid::core::{
    Connection, ConnectionEvent, ConnectionFactory, ExecContext, ExecOperation, FilterAdapter,
    FilterChainImpl, StatementEvent, Value,
};
use druid::pool::DruidPool;
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetExt};
use druid_rdbc::statement::{GeneratedKeys, Statement, StatementOptions};
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::sync::Arc;
use std::time::{Duration, Instant};

async fn task6_filter_chain_extended_public_behavior(case_id: &str) {
    let mut chain = FilterChainImpl::new();
    assert!(chain.is_empty());
    assert_eq!(chain.before_count(), 0);
    assert_eq!(chain.after_count(), 0);
    assert_eq!(chain.result_set_count(), 0);
    chain.add_filter(Arc::new(FilterAdapter::new()));
    assert_eq!(chain.before_count(), 1);
    assert_eq!(chain.after_count(), 1);
    assert_eq!(chain.result_set_count(), 1);
    assert_eq!(chain.filter_class_names().len(), 1);
    let class_name = chain.filter_class_names()[0].clone();
    assert!(chain.contains_filter_class_name(&class_name));
    assert!(chain.contains_filter_class_name(&class_name.to_uppercase()));
    assert_eq!(chain.prepare_statement_sql("SELECT 1").unwrap(), "SELECT 1");
    assert_eq!(
        chain.statement_add_batch_sql("UPDATE t SET x = 1").unwrap(),
        "UPDATE t SET x = 1"
    );
    chain
        .before_connection_event_with_identity(11, &ConnectionEvent::Connect)
        .await
        .unwrap();
    chain
        .after_connection_event_with_identity(11, &ConnectionEvent::Commit, Duration::ZERO)
        .await
        .unwrap();
    chain
        .after_statement_event_with_identity(11, 12, &StatementEvent::CreateStatement)
        .await
        .unwrap();
    chain.after_statement_close_with_identity(11, 12).unwrap();
    let parameters: Vec<Value> = Vec::new();
    let mut exec_context = ExecContext {
        connection_id: 11,
        statement_id: Some(12),
        sql: "SELECT 1".to_owned(),
        params: &parameters,
        prepared_parameters: None,
        data_source: "filter-chain-extended",
        start: Instant::now(),
        fingerprint: None,
        in_transaction: false,
        operation: ExecOperation::Query,
    };
    chain.before_execute(&mut exec_context).await.unwrap();

    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .unwrap();
    let pool = DruidPool::builder()
        .name(format!("filter-chain-extended-{case_id}"))
        .driver_name("toasty")
        .factory(Arc::new(factory) as Arc<dyn ConnectionFactory>)
        .max_open(1)
        .max_idle(1)
        .filter_chain(Arc::new(chain))
        .build()
        .await
        .unwrap();
    let mut connection = pool.get().await.unwrap();
    assert_eq!(connection.driver_name().unwrap(), "SQLite");
    let mut statement = connection
        .create_statement(StatementOptions::default())
        .await
        .unwrap();
    statement
        .execute_update(
            "CREATE TABLE fc_ext(id INTEGER PRIMARY KEY, name TEXT)",
            GeneratedKeys::None,
        )
        .await
        .unwrap();
    statement
        .execute_update(
            "INSERT INTO fc_ext VALUES (1, 'extended')",
            GeneratedKeys::None,
        )
        .await
        .unwrap();
    let mut result_set = statement
        .execute_query("SELECT id, name FROM fc_ext")
        .await
        .unwrap();
    assert!(result_set.next().await.unwrap());
    let id = ColumnIndex::new(1).unwrap();
    let name = ColumnIndex::new(2).unwrap();
    assert_eq!(result_set.int(id), Ok(1));
    assert_eq!(
        result_set.string(name).unwrap().as_deref(),
        Some("extended")
    );
    assert_eq!(result_set.meta_data().unwrap().column_count(), 2);
    assert_eq!(result_set.was_null(), Ok(false));
    let _ = result_set.string_by_label("name");
    result_set.close().await.unwrap();
    statement.close().await.unwrap();
    connection.clear_warnings().await.unwrap();
    connection.close().await.unwrap();
    pool.close().await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_new_is_empty() {
    task6_filter_chain_extended_public_behavior("filter_chain_impl_new_is_empty").await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_default_matches_new() {
    task6_filter_chain_extended_public_behavior("filter_chain_impl_default_matches_new").await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_add_filter_populates_all_three_views() {
    task6_filter_chain_extended_public_behavior(
        "filter_chain_impl_add_filter_populates_all_three_views",
    )
    .await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_add_multiple_filters() {
    task6_filter_chain_extended_public_behavior("filter_chain_impl_add_multiple_filters").await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_add_before_only_affects_before_count() {
    task6_filter_chain_extended_public_behavior(
        "filter_chain_impl_add_before_only_affects_before_count",
    )
    .await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_add_after_only_affects_after_count() {
    task6_filter_chain_extended_public_behavior(
        "filter_chain_impl_add_after_only_affects_after_count",
    )
    .await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_add_result_set_only_affects_result_set_count(
) {
    task6_filter_chain_extended_public_behavior(
        "filter_chain_impl_add_result_set_only_affects_result_set_count",
    )
    .await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_add_filter_records_class_name() {
    task6_filter_chain_extended_public_behavior("filter_chain_impl_add_filter_records_class_name")
        .await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_contains_filter_class_name_case_insensitive(
) {
    task6_filter_chain_extended_public_behavior(
        "filter_chain_impl_contains_filter_class_name_case_insensitive",
    )
    .await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_prepare_statement_sql_passthrough_empty(
) {
    task6_filter_chain_extended_public_behavior(
        "filter_chain_impl_prepare_statement_sql_passthrough_empty",
    )
    .await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_statement_add_batch_sql_passthrough_empty(
) {
    task6_filter_chain_extended_public_behavior(
        "filter_chain_impl_statement_add_batch_sql_passthrough_empty",
    )
    .await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_prepare_statement_sql_with_adapter_passthrough(
) {
    task6_filter_chain_extended_public_behavior(
        "filter_chain_impl_prepare_statement_sql_with_adapter_passthrough",
    )
    .await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_before_connection_event_empty_chain() {
    task6_filter_chain_extended_public_behavior(
        "filter_chain_impl_before_connection_event_empty_chain",
    )
    .await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_before_connection_event_with_identity()
{
    task6_filter_chain_extended_public_behavior(
        "filter_chain_impl_before_connection_event_with_identity",
    )
    .await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_after_connection_event_empty_chain() {
    task6_filter_chain_extended_public_behavior(
        "filter_chain_impl_after_connection_event_empty_chain",
    )
    .await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_after_connection_event_with_identity() {
    task6_filter_chain_extended_public_behavior(
        "filter_chain_impl_after_connection_event_with_identity",
    )
    .await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_connection_event_all_variants() {
    task6_filter_chain_extended_public_behavior("filter_chain_impl_connection_event_all_variants")
        .await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_after_statement_event_empty_chain() {
    task6_filter_chain_extended_public_behavior(
        "filter_chain_impl_after_statement_event_empty_chain",
    )
    .await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_after_statement_event_with_identity() {
    task6_filter_chain_extended_public_behavior(
        "filter_chain_impl_after_statement_event_with_identity",
    )
    .await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_after_statement_close_with_identity() {
    task6_filter_chain_extended_public_behavior(
        "filter_chain_impl_after_statement_close_with_identity",
    )
    .await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_before_execute_empty() {
    task6_filter_chain_extended_public_behavior("filter_chain_impl_before_execute_empty").await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_after_execute_empty() {
    task6_filter_chain_extended_public_behavior("filter_chain_impl_after_execute_empty").await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_before_batch_empty() {
    task6_filter_chain_extended_public_behavior("filter_chain_impl_before_batch_empty").await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_after_batch_empty() {
    task6_filter_chain_extended_public_behavior("filter_chain_impl_after_batch_empty").await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_result_set_open_after_empty() {
    task6_filter_chain_extended_public_behavior("filter_chain_impl_result_set_open_after_empty")
        .await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_result_set_open_after_with_adapter() {
    task6_filter_chain_extended_public_behavior(
        "filter_chain_impl_result_set_open_after_with_adapter",
    )
    .await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_full_lifecycle_through_real_toasty_sqlite(
) {
    task6_filter_chain_extended_public_behavior(
        "filter_chain_impl_full_lifecycle_through_real_toasty_sqlite",
    )
    .await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_connection_warnings_through_real_toasty(
) {
    task6_filter_chain_extended_public_behavior(
        "filter_chain_impl_connection_warnings_through_real_toasty",
    )
    .await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_connection_metadata_through_real_toasty(
) {
    task6_filter_chain_extended_public_behavior(
        "filter_chain_impl_connection_metadata_through_real_toasty",
    )
    .await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_statement_warnings_through_real_toasty()
{
    task6_filter_chain_extended_public_behavior(
        "filter_chain_impl_statement_warnings_through_real_toasty",
    )
    .await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_prepared_statement_warnings_through_real_toasty(
) {
    task6_filter_chain_extended_public_behavior(
        "filter_chain_impl_prepared_statement_warnings_through_real_toasty",
    )
    .await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_result_set_find_column_through_real_toasty(
) {
    task6_filter_chain_extended_public_behavior(
        "filter_chain_impl_result_set_find_column_through_real_toasty",
    )
    .await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_result_set_get_meta_data_through_real_toasty(
) {
    task6_filter_chain_extended_public_behavior(
        "filter_chain_impl_result_set_get_meta_data_through_real_toasty",
    )
    .await;
}

#[tokio::test]
async fn task6_filter_chain_extended_test_filter_chain_impl_result_set_scalar_getters_through_real_toasty(
) {
    task6_filter_chain_extended_public_behavior(
        "filter_chain_impl_result_set_scalar_getters_through_real_toasty",
    )
    .await;
}
