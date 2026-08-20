//! Differential tests for `PhysicalDatabaseMetaData` trait — Java `DatabaseMetaData` semantics.
//!
//! Exercises both the Toasty adapter implementations and the trait default
//! `UnsupportedOperation` paths through a real pooled SQLite connection.

extern crate druid_core as druid;
use druid::core::{DruidError, DruidPooledConnection, PhysicalConnectionFactory};
use druid_wrapper::toasty::ToastyConnectionFactory;

async fn make_pooled() -> DruidPooledConnection {
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("factory");
    let physical = factory.create().await.expect("connection");
    DruidPooledConnection::new(physical, 1, Box::new(|_, _| {}))
}

// ── Implemented methods (ToastyDatabaseMetaData) ───────────────────

#[tokio::test]
async fn get_url_returns_sqlite_memory() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let url = meta.get_url().await.unwrap();
    assert!(url.is_some());
    assert!(url.unwrap().contains("sqlite"));
}

#[tokio::test]
async fn get_user_name_returns_none() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let user = meta.get_user_name().await.unwrap();
    assert!(user.is_none());
}

#[tokio::test]
async fn is_read_only_false_for_default() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    assert!(!meta.is_read_only().await.unwrap());
}

#[tokio::test]
async fn get_database_product_name_returns_sqlite() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let name = meta.get_database_product_name().await.unwrap();
    assert_eq!(name, Some("SQLite".to_string()));
}

#[tokio::test]
async fn get_driver_name_returns_toasty() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let name = meta.get_driver_name().await.unwrap();
    assert!(name.is_some());
}

#[tokio::test]
async fn get_driver_version_returns_none() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let ver = meta.get_driver_version().await.unwrap();
    assert!(ver.is_none());
}

#[tokio::test]
async fn get_identifier_quote_string_returns_double_quote() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let quote = meta.get_identifier_quote_string().await.unwrap();
    assert_eq!(quote, Some("\"".to_string()));
}

#[tokio::test]
async fn get_search_string_escape_returns_backslash() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let esc = meta.get_search_string_escape().await.unwrap();
    assert_eq!(esc, Some("\\".to_string()));
}

#[tokio::test]
async fn get_catalog_separator_returns_dot() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let sep = meta.get_catalog_separator().await.unwrap();
    assert_eq!(sep, Some(".".to_string()));
}

#[tokio::test]
async fn get_max_connections_returns_zero() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    assert_eq!(meta.get_max_connections().await.unwrap(), 0);
}

#[tokio::test]
async fn supports_transactions_true() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    assert!(meta.supports_transactions().await.unwrap());
}

#[tokio::test]
async fn supports_transaction_isolation_level_serializable() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    assert!(meta.supports_transaction_isolation_level(8).await.unwrap());
}

#[tokio::test]
async fn supports_transaction_isolation_level_read_committed_false_for_sqlite() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    assert!(!meta.supports_transaction_isolation_level(2).await.unwrap());
}

#[tokio::test]
async fn supports_batch_updates_true() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    assert!(meta.supports_batch_updates().await.unwrap());
}

#[tokio::test]
async fn supports_savepoints_true() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    assert!(meta.supports_savepoints().await.unwrap());
}

#[tokio::test]
async fn supports_named_parameters_false() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    assert!(!meta.supports_named_parameters().await.unwrap());
}

// ── Default trait methods (UnsupportedOperation) ───────────────────

#[tokio::test]
async fn all_procedures_are_callable_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.all_procedures_are_callable().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn all_tables_are_selectable_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.all_tables_are_selectable().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn nulls_are_sorted_high_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.nulls_are_sorted_high().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn get_database_product_version_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.get_database_product_version().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn get_driver_major_version_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.get_driver_major_version().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn get_sql_keywords_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.get_sql_keywords().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn get_numeric_functions_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.get_numeric_functions().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn supports_column_aliasing_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.supports_column_aliasing().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn supports_convert_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.supports_convert().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn supports_convert_between_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.supports_convert_between(4, 12).await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn supports_expressions_in_order_by_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.supports_expressions_in_order_by().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn supports_group_by_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.supports_group_by().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn supports_like_escape_clause_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.supports_like_escape_clause().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn supports_outer_joins_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.supports_outer_joins().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn get_schema_term_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.get_schema_term().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn get_procedure_term_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.get_procedure_term().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn get_catalog_term_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.get_catalog_term().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn supports_schemas_in_data_manipulation_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta
        .supports_schemas_in_data_manipulation()
        .await
        .unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn supports_catalogs_in_data_manipulation_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta
        .supports_catalogs_in_data_manipulation()
        .await
        .unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn supports_select_for_update_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.supports_select_for_update().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn supports_subqueries_in_comparisons_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.supports_subqueries_in_comparisons().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn supports_union_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.supports_union().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn get_max_columns_in_select_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.get_max_columns_in_select().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn get_max_columns_in_table_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.get_max_columns_in_table().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn get_max_statement_length_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.get_max_statement_length().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn get_default_transaction_isolation_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.get_default_transaction_isolation().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn get_procedures_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.get_procedures(None, None, None).await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn get_tables_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.get_tables(None, None, None, None).await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn get_schemas_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.get_schemas().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn get_catalogs_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.get_catalogs().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn get_table_types_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.get_table_types().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn get_columns_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.get_columns(None, None, None, None).await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn get_primary_keys_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.get_primary_keys(None, None, None).await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn get_type_info_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.get_type_info().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn supports_result_set_type_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.supports_result_set_type(1003).await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn get_result_set_holdability_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.get_result_set_holdability().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn get_database_major_version_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.get_database_major_version().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn get_rdbc_major_version_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.get_rdbc_major_version().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn get_sql_state_type_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.get_sql_state_type().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn supports_statement_pooling_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.supports_statement_pooling().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn get_row_id_lifetime_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.get_row_id_lifetime().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn supports_stored_functions_using_call_syntax_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta
        .supports_stored_functions_using_call_syntax()
        .await
        .unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn auto_commit_failure_closes_all_result_sets_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta
        .auto_commit_failure_closes_all_result_sets()
        .await
        .unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn get_client_info_properties_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.get_client_info_properties().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn generated_key_always_returned_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.generated_key_always_returned().await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

// ── Additional parameterized default methods ───────────────────────

#[tokio::test]
async fn supports_result_set_concurrency_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta
        .supports_result_set_concurrency(1003, 1007)
        .await
        .unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn get_index_info_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta
        .get_index_info(None, None, None, true, false)
        .await
        .unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn get_cross_reference_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta
        .get_cross_reference(None, None, None, None, None, None)
        .await
        .unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn get_imported_keys_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.get_imported_keys(None, None, None).await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn get_exported_keys_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.get_exported_keys(None, None, None).await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}

#[tokio::test]
async fn get_functions_unsupported() {
    let mut p = make_pooled().await;
    let mut meta = p.get_meta_data().unwrap();
    let err = meta.get_functions(None, None, None).await.unwrap_err();
    assert!(matches!(err, DruidError::UnsupportedOperation { .. }));
}
