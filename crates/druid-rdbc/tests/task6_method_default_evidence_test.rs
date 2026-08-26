//! Executable method evidence for BASE methods without a direct BASE behavior test.

use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{
    DatabaseMetaData, DatabaseMetaDataInfoExt, DatabaseMetaDataLimitsExt, DatabaseMetaDataQueryExt,
    DatabaseMetaDataSqlCapabilitiesExt,
};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetUpdate};
use druid_rdbc::statement::{
    CallableInputParameter, CallableParameter, CallableStatement, CurrentResultDisposition,
    ParameterIndex, PreparedParameter, PreparedStatement, Statement,
};
use druid_rdbc::types::{RdbcTargetType, Value};

struct DefaultMetadata;
druid_rdbc::impl_standard_wrapper!(DefaultMetadata => DatabaseMetaData);
#[async_trait::async_trait]
impl DatabaseMetaData for DefaultMetadata {}

struct DefaultStatement;
druid_rdbc::impl_standard_wrapper!(DefaultStatement => Statement);
#[async_trait::async_trait]
impl Statement for DefaultStatement {}

struct DefaultPrepared;
druid_rdbc::impl_standard_wrapper!(DefaultPrepared => PreparedStatement);
#[async_trait::async_trait]
impl Statement for DefaultPrepared {}
#[async_trait::async_trait]
impl PreparedStatement for DefaultPrepared {}

struct DefaultCallable;
druid_rdbc::impl_standard_wrapper!(DefaultCallable => CallableStatement);
#[async_trait::async_trait]
impl Statement for DefaultCallable {}
#[async_trait::async_trait]
impl PreparedStatement for DefaultCallable {}
#[async_trait::async_trait]
impl CallableStatement for DefaultCallable {}

struct DefaultResultSet;
druid_rdbc::impl_standard_wrapper!(DefaultResultSet => ResultSet);
#[async_trait::async_trait]
impl ResultSet for DefaultResultSet {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        Ok(false)
    }
}

fn prepared_parameter() -> PreparedParameter {
    PreparedParameter {
        index: ParameterIndex::new(1).unwrap(),
        value: Value::Int(1),
    }
}

fn assert_fail_closed<T>(result: Result<T, RdbcError>) {
    assert!(result.is_err(), "canonical default must fail closed");
}

fn assert_metadata_error<T>(result: Result<T, RdbcError>, operation: &'static str) {
    assert!(matches!(
        result,
        Err(RdbcError::UnsupportedOperation { operation: actual }) if actual == operation
    ));
}

fn task6_method_default_evidence_case_ids() -> &'static [&'static str] {
    &[
        "physical_callable_statement__array_out_parameter",
        "physical_callable_statement__blob_out_parameter",
        "physical_callable_statement__boolean_out_parameter",
        "physical_callable_statement__byte_out_parameter",
        "physical_callable_statement__bytes_out_parameter",
        "physical_callable_statement__character_stream_out_parameter",
        "physical_callable_statement__clob_out_parameter",
        "physical_callable_statement__double_out_parameter",
        "physical_callable_statement__float_out_parameter",
        "physical_callable_statement__int_out_parameter",
        "physical_callable_statement__integer_out_parameter",
        "physical_callable_statement__long_out_parameter",
        "physical_callable_statement__n_character_stream_out_parameter",
        "physical_callable_statement__n_clob_out_parameter",
        "physical_callable_statement__n_string_out_parameter",
        "physical_callable_statement__out_parameter",
        "physical_callable_statement__out_parameter_as",
        "physical_callable_statement__out_parameter_with_type_map",
        "physical_callable_statement__ref_out_parameter",
        "physical_callable_statement__row_id_out_parameter",
        "physical_callable_statement__set_named_parameter",
        "physical_callable_statement__short_out_parameter",
        "physical_callable_statement__sql_xml_out_parameter",
        "physical_callable_statement__string_out_parameter",
        "physical_callable_statement__url_out_parameter",
        "physical_connection__exec_prepared_parameters",
        "physical_connection__fetch_prepared_parameters",
        "physical_database_meta_data__data_definition_causes_transaction_commit",
        "physical_database_meta_data__data_definition_ignored_in_transactions",
        "physical_database_meta_data__deletes_are_detected",
        "physical_database_meta_data__does_max_row_size_include_blobs",
        "physical_database_meta_data__get_attributes",
        "physical_database_meta_data__get_best_row_identifier",
        "physical_database_meta_data__get_column_privileges",
        "physical_database_meta_data__get_database_minor_version",
        "physical_database_meta_data__get_driver_minor_version",
        "physical_database_meta_data__get_extra_name_characters",
        "physical_database_meta_data__get_function_columns",
        "physical_database_meta_data__get_max_binary_literal_length",
        "physical_database_meta_data__get_max_catalog_name_length",
        "physical_database_meta_data__get_max_char_literal_length",
        "physical_database_meta_data__get_max_column_name_length",
        "physical_database_meta_data__get_max_columns_in_group_by",
        "physical_database_meta_data__get_max_columns_in_index",
        "physical_database_meta_data__get_max_columns_in_order_by",
        "physical_database_meta_data__get_max_cursor_name_length",
        "physical_database_meta_data__get_max_index_length",
        "physical_database_meta_data__get_max_procedure_name_length",
        "physical_database_meta_data__get_max_row_size",
        "physical_database_meta_data__get_max_schema_name_length",
        "physical_database_meta_data__get_max_statements",
        "physical_database_meta_data__get_max_table_name_length",
        "physical_database_meta_data__get_max_tables_in_select",
        "physical_database_meta_data__get_max_user_name_length",
        "physical_database_meta_data__get_procedure_columns",
        "physical_database_meta_data__get_pseudo_columns",
        "physical_database_meta_data__get_rdbc_minor_version",
        "physical_database_meta_data__get_schemas_with_pattern",
        "physical_database_meta_data__get_string_functions",
        "physical_database_meta_data__get_super_tables",
        "physical_database_meta_data__get_super_types",
        "physical_database_meta_data__get_system_functions",
        "physical_database_meta_data__get_table_privileges",
        "physical_database_meta_data__get_time_date_functions",
        "physical_database_meta_data__get_ud_ts",
        "physical_database_meta_data__get_version_columns",
        "physical_database_meta_data__inserts_are_detected",
        "physical_database_meta_data__is_catalog_at_start",
        "physical_database_meta_data__locators_update_copy",
        "physical_database_meta_data__null_plus_non_null_is_null",
        "physical_database_meta_data__nulls_are_sorted_at_end",
        "physical_database_meta_data__nulls_are_sorted_at_start",
        "physical_database_meta_data__nulls_are_sorted_low",
        "physical_database_meta_data__others_deletes_are_visible",
        "physical_database_meta_data__others_inserts_are_visible",
        "physical_database_meta_data__others_updates_are_visible",
        "physical_database_meta_data__own_deletes_are_visible",
        "physical_database_meta_data__own_inserts_are_visible",
        "physical_database_meta_data__own_updates_are_visible",
        "physical_database_meta_data__stores_lower_case_identifiers",
        "physical_database_meta_data__stores_lower_case_quoted_identifiers",
        "physical_database_meta_data__stores_mixed_case_identifiers",
        "physical_database_meta_data__stores_mixed_case_quoted_identifiers",
        "physical_database_meta_data__stores_upper_case_identifiers",
        "physical_database_meta_data__stores_upper_case_quoted_identifiers",
        "physical_database_meta_data__supports_alter_table_with_add_column",
        "physical_database_meta_data__supports_alter_table_with_drop_column",
        "physical_database_meta_data__supports_ansi92_entry_level_sql",
        "physical_database_meta_data__supports_ansi92_full_sql",
        "physical_database_meta_data__supports_ansi92_intermediate_sql",
        "physical_database_meta_data__supports_catalogs_in_index_definitions",
        "physical_database_meta_data__supports_catalogs_in_privilege_definitions",
        "physical_database_meta_data__supports_catalogs_in_procedure_calls",
        "physical_database_meta_data__supports_catalogs_in_table_definitions",
        "physical_database_meta_data__supports_core_sql_grammar",
        "physical_database_meta_data__supports_correlated_subqueries",
        "physical_database_meta_data__supports_data_definition_and_data_manipulation_transactions",
        "physical_database_meta_data__supports_data_manipulation_transactions_only",
        "physical_database_meta_data__supports_different_table_correlation_names",
        "physical_database_meta_data__supports_extended_sql_grammar",
        "physical_database_meta_data__supports_full_outer_joins",
        "physical_database_meta_data__supports_get_generated_keys",
        "physical_database_meta_data__supports_group_by_beyond_select",
        "physical_database_meta_data__supports_group_by_unrelated",
        "physical_database_meta_data__supports_integrity_enhancement_facility",
        "physical_database_meta_data__supports_limited_outer_joins",
        "physical_database_meta_data__supports_minimum_sql_grammar",
        "physical_database_meta_data__supports_mixed_case_identifiers",
        "physical_database_meta_data__supports_mixed_case_quoted_identifiers",
        "physical_database_meta_data__supports_multiple_open_results",
        "physical_database_meta_data__supports_multiple_result_sets",
        "physical_database_meta_data__supports_multiple_transactions",
        "physical_database_meta_data__supports_non_nullable_columns",
        "physical_database_meta_data__supports_open_cursors_across_commit",
        "physical_database_meta_data__supports_open_cursors_across_rollback",
        "physical_database_meta_data__supports_open_statements_across_commit",
        "physical_database_meta_data__supports_open_statements_across_rollback",
        "physical_database_meta_data__supports_order_by_unrelated",
        "physical_database_meta_data__supports_positioned_delete",
        "physical_database_meta_data__supports_positioned_update",
        "physical_database_meta_data__supports_result_set_holdability",
        "physical_database_meta_data__supports_schemas_in_index_definitions",
        "physical_database_meta_data__supports_schemas_in_privilege_definitions",
        "physical_database_meta_data__supports_schemas_in_procedure_calls",
        "physical_database_meta_data__supports_schemas_in_table_definitions",
        "physical_database_meta_data__supports_stored_procedures",
        "physical_database_meta_data__supports_subqueries_in_exists",
        "physical_database_meta_data__supports_subqueries_in_ins",
        "physical_database_meta_data__supports_subqueries_in_quantifieds",
        "physical_database_meta_data__supports_table_correlation_names",
        "physical_database_meta_data__supports_union_all",
        "physical_database_meta_data__updates_are_detected",
        "physical_database_meta_data__uses_local_file_per_table",
        "physical_database_meta_data__uses_local_files",
        "physical_prepared_statement__add_parameter_batch",
        "physical_prepared_statement__get_generated_keys",
        "physical_prepared_statement__get_more_results",
        "physical_prepared_statement__get_result_set",
        "physical_prepared_statement__get_update_count",
        "physical_prepared_statement__set_parameter",
        "physical_prepared_statement__sql",
        "physical_prepared_statement__statement_options",
        "physical_result_set__update_value",
        "physical_result_set__update_value_by_label",
        "physical_result_set__value_by_label",
        "physical_statement__batch",
        "physical_statement__get_generated_keys",
        "physical_statement__get_more_results",
        "physical_statement__get_result_set",
        "physical_statement__get_update_count",
        "physical_statement__options",
    ]
}

async fn exercise(case_id: &str) {
    let mut metadata = DefaultMetadata;
    let mut statement = DefaultStatement;
    let mut prepared = DefaultPrepared;
    let mut callable = DefaultCallable;
    let mut result_set = DefaultResultSet;
    let parameter = CallableParameter::by_index(1).unwrap();
    let column = ColumnIndex::new(1).unwrap();
    match case_id {
        "physical_callable_statement__array_out_parameter" => assert_fail_closed(callable.array_out_parameter(&parameter)),
        "physical_callable_statement__blob_out_parameter" => assert_fail_closed(callable.blob_out_parameter(&parameter)),
        "physical_callable_statement__boolean_out_parameter" => assert_fail_closed(callable.boolean_out_parameter(&parameter)),
        "physical_callable_statement__byte_out_parameter" => assert_fail_closed(callable.byte_out_parameter(&parameter)),
        "physical_callable_statement__bytes_out_parameter" => assert_fail_closed(callable.bytes_out_parameter(&parameter)),
        "physical_callable_statement__character_stream_out_parameter" => assert_fail_closed(callable.character_stream_out_parameter(&parameter)),
        "physical_callable_statement__clob_out_parameter" => assert_fail_closed(callable.clob_out_parameter(&parameter)),
        "physical_callable_statement__double_out_parameter" => assert_fail_closed(callable.double_out_parameter(&parameter)),
        "physical_callable_statement__float_out_parameter" => assert_fail_closed(callable.float_out_parameter(&parameter)),
        "physical_callable_statement__int_out_parameter" => assert_fail_closed(callable.int_out_parameter(&parameter)),
        "physical_callable_statement__integer_out_parameter" => assert_fail_closed(callable.integer_out_parameter(&parameter)),
        "physical_callable_statement__long_out_parameter" => assert_fail_closed(callable.long_out_parameter(&parameter)),
        "physical_callable_statement__n_character_stream_out_parameter" => assert_fail_closed(callable.n_character_stream_out_parameter(&parameter)),
        "physical_callable_statement__n_clob_out_parameter" => assert_fail_closed(callable.n_clob_out_parameter(&parameter)),
        "physical_callable_statement__n_string_out_parameter" => assert_fail_closed(callable.n_string_out_parameter(&parameter)),
        "physical_callable_statement__out_parameter" => assert_fail_closed(callable.out_parameter(&parameter)),
        "physical_callable_statement__out_parameter_as" => assert_fail_closed(callable.out_parameter_as(&parameter, &RdbcTargetType::Integer)),
        "physical_callable_statement__out_parameter_with_type_map" => assert_fail_closed(callable.out_parameter_with_type_map(&parameter, None)),
        "physical_callable_statement__ref_out_parameter" => assert_fail_closed(callable.ref_out_parameter(&parameter)),
        "physical_callable_statement__row_id_out_parameter" => assert_fail_closed(callable.row_id_out_parameter(&parameter)),
        "physical_callable_statement__set_named_parameter" => assert_fail_closed(callable.set_named_parameter("value", CallableInputParameter::Int(1))),
        "physical_callable_statement__short_out_parameter" => assert_fail_closed(callable.short_out_parameter(&parameter)),
        "physical_callable_statement__sql_xml_out_parameter" => assert_fail_closed(callable.sql_xml_out_parameter(&parameter)),
        "physical_callable_statement__string_out_parameter" => assert_fail_closed(callable.string_out_parameter(&parameter)),
        "physical_callable_statement__url_out_parameter" => assert_fail_closed(callable.url_out_parameter(&parameter)),
        "physical_connection__exec_prepared_parameters" => assert_fail_closed(prepared.execute_prepared_update().await),
        "physical_connection__fetch_prepared_parameters" => assert_fail_closed(prepared.execute_prepared_query().await),
        "physical_database_meta_data__data_definition_causes_transaction_commit" => assert_metadata_error(metadata.data_definition_causes_transaction_commit().await, "data_definition_causes_transaction_commit"),
        "physical_database_meta_data__data_definition_ignored_in_transactions" => assert_metadata_error(metadata.data_definition_ignored_in_transactions().await, "data_definition_ignored_in_transactions"),
        "physical_database_meta_data__deletes_are_detected" => assert_metadata_error(metadata.deletes_are_detected(1003).await, "deletes_are_detected"),
        "physical_database_meta_data__does_max_row_size_include_blobs" => assert_metadata_error(metadata.does_max_row_size_include_blobs().await, "does_max_row_size_include_blobs"),
        "physical_database_meta_data__get_attributes" => assert_metadata_error(metadata.get_attributes(None, None, None, None).await, "get_attributes"),
        "physical_database_meta_data__get_best_row_identifier" => assert_metadata_error(metadata.get_best_row_identifier(None, None, None, 0, false).await, "get_best_row_identifier"),
        "physical_database_meta_data__get_column_privileges" => assert_metadata_error(metadata.get_column_privileges(None, None, None, None).await, "get_column_privileges"),
        "physical_database_meta_data__get_database_minor_version" => assert_metadata_error(metadata.get_database_minor_version().await, "get_database_minor_version"),
        "physical_database_meta_data__get_driver_minor_version" => assert_metadata_error(metadata.get_driver_minor_version().await, "get_driver_minor_version"),
        "physical_database_meta_data__get_extra_name_characters" => assert_metadata_error(metadata.get_extra_name_characters().await, "get_extra_name_characters"),
        "physical_database_meta_data__get_function_columns" => assert_metadata_error(metadata.get_function_columns(None, None, None, None).await, "get_function_columns"),
        "physical_database_meta_data__get_max_binary_literal_length" => assert_metadata_error(metadata.get_max_binary_literal_length().await, "get_max_binary_literal_length"),
        "physical_database_meta_data__get_max_catalog_name_length" => assert_metadata_error(metadata.get_max_catalog_name_length().await, "get_max_catalog_name_length"),
        "physical_database_meta_data__get_max_char_literal_length" => assert_metadata_error(metadata.get_max_char_literal_length().await, "get_max_char_literal_length"),
        "physical_database_meta_data__get_max_column_name_length" => assert_metadata_error(metadata.get_max_column_name_length().await, "get_max_column_name_length"),
        "physical_database_meta_data__get_max_columns_in_group_by" => assert_metadata_error(metadata.get_max_columns_in_group_by().await, "get_max_columns_in_group_by"),
        "physical_database_meta_data__get_max_columns_in_index" => assert_metadata_error(metadata.get_max_columns_in_index().await, "get_max_columns_in_index"),
        "physical_database_meta_data__get_max_columns_in_order_by" => assert_metadata_error(metadata.get_max_columns_in_order_by().await, "get_max_columns_in_order_by"),
        "physical_database_meta_data__get_max_cursor_name_length" => assert_metadata_error(metadata.get_max_cursor_name_length().await, "get_max_cursor_name_length"),
        "physical_database_meta_data__get_max_index_length" => assert_metadata_error(metadata.get_max_index_length().await, "get_max_index_length"),
        "physical_database_meta_data__get_max_procedure_name_length" => assert_metadata_error(metadata.get_max_procedure_name_length().await, "get_max_procedure_name_length"),
        "physical_database_meta_data__get_max_row_size" => assert_metadata_error(metadata.get_max_row_size().await, "get_max_row_size"),
        "physical_database_meta_data__get_max_schema_name_length" => assert_metadata_error(metadata.get_max_schema_name_length().await, "get_max_schema_name_length"),
        "physical_database_meta_data__get_max_statements" => assert_metadata_error(metadata.get_max_statements().await, "get_max_statements"),
        "physical_database_meta_data__get_max_table_name_length" => assert_metadata_error(metadata.get_max_table_name_length().await, "get_max_table_name_length"),
        "physical_database_meta_data__get_max_tables_in_select" => assert_metadata_error(metadata.get_max_tables_in_select().await, "get_max_tables_in_select"),
        "physical_database_meta_data__get_max_user_name_length" => assert_metadata_error(metadata.get_max_user_name_length().await, "get_max_user_name_length"),
        "physical_database_meta_data__get_procedure_columns" => assert_metadata_error(metadata.get_procedure_columns(None, None, None, None).await, "get_procedure_columns"),
        "physical_database_meta_data__get_pseudo_columns" => assert_metadata_error(metadata.get_pseudo_columns(None, None, None, None).await, "get_pseudo_columns"),
        "physical_database_meta_data__get_rdbc_minor_version" => assert_metadata_error(metadata.get_rdbc_minor_version().await, "get_rdbc_minor_version"),
        "physical_database_meta_data__get_schemas_with_pattern" => assert_metadata_error(metadata.get_schemas_with_pattern(None, None).await, "get_schemas_with_pattern"),
        "physical_database_meta_data__get_string_functions" => assert_metadata_error(metadata.get_string_functions().await, "get_string_functions"),
        "physical_database_meta_data__get_super_tables" => assert_metadata_error(metadata.get_super_tables(None, None, None).await, "get_super_tables"),
        "physical_database_meta_data__get_super_types" => assert_metadata_error(metadata.get_super_types(None, None, None).await, "get_super_types"),
        "physical_database_meta_data__get_system_functions" => assert_metadata_error(metadata.get_system_functions().await, "get_system_functions"),
        "physical_database_meta_data__get_table_privileges" => assert_metadata_error(metadata.get_table_privileges(None, None, None).await, "get_table_privileges"),
        "physical_database_meta_data__get_time_date_functions" => assert_metadata_error(metadata.get_time_date_functions().await, "get_time_date_functions"),
        "physical_database_meta_data__get_ud_ts" => assert_metadata_error(metadata.get_ud_ts(None, None, None, None).await, "get_ud_ts"),
        "physical_database_meta_data__get_version_columns" => assert_metadata_error(metadata.get_version_columns(None, None, None).await, "get_version_columns"),
        "physical_database_meta_data__inserts_are_detected" => assert_metadata_error(metadata.inserts_are_detected(1003).await, "inserts_are_detected"),
        "physical_database_meta_data__is_catalog_at_start" => assert_metadata_error(metadata.is_catalog_at_start().await, "is_catalog_at_start"),
        "physical_database_meta_data__locators_update_copy" => assert_metadata_error(metadata.locators_update_copy().await, "locators_update_copy"),
        "physical_database_meta_data__null_plus_non_null_is_null" => assert_metadata_error(metadata.null_plus_non_null_is_null().await, "null_plus_non_null_is_null"),
        "physical_database_meta_data__nulls_are_sorted_at_end" => assert_metadata_error(metadata.nulls_are_sorted_at_end().await, "nulls_are_sorted_at_end"),
        "physical_database_meta_data__nulls_are_sorted_at_start" => assert_metadata_error(metadata.nulls_are_sorted_at_start().await, "nulls_are_sorted_at_start"),
        "physical_database_meta_data__nulls_are_sorted_low" => assert_metadata_error(metadata.nulls_are_sorted_low().await, "nulls_are_sorted_low"),
        "physical_database_meta_data__others_deletes_are_visible" => assert_metadata_error(metadata.others_deletes_are_visible(1003).await, "others_deletes_are_visible"),
        "physical_database_meta_data__others_inserts_are_visible" => assert_metadata_error(metadata.others_inserts_are_visible(1003).await, "others_inserts_are_visible"),
        "physical_database_meta_data__others_updates_are_visible" => assert_metadata_error(metadata.others_updates_are_visible(1003).await, "others_updates_are_visible"),
        "physical_database_meta_data__own_deletes_are_visible" => assert_metadata_error(metadata.own_deletes_are_visible(1003).await, "own_deletes_are_visible"),
        "physical_database_meta_data__own_inserts_are_visible" => assert_metadata_error(metadata.own_inserts_are_visible(1003).await, "own_inserts_are_visible"),
        "physical_database_meta_data__own_updates_are_visible" => assert_metadata_error(metadata.own_updates_are_visible(1003).await, "own_updates_are_visible"),
        "physical_database_meta_data__stores_lower_case_identifiers" => assert_metadata_error(metadata.stores_lower_case_identifiers().await, "stores_lower_case_identifiers"),
        "physical_database_meta_data__stores_lower_case_quoted_identifiers" => assert_metadata_error(metadata.stores_lower_case_quoted_identifiers().await, "stores_lower_case_quoted_identifiers"),
        "physical_database_meta_data__stores_mixed_case_identifiers" => assert_metadata_error(metadata.stores_mixed_case_identifiers().await, "stores_mixed_case_identifiers"),
        "physical_database_meta_data__stores_mixed_case_quoted_identifiers" => assert_metadata_error(metadata.stores_mixed_case_quoted_identifiers().await, "stores_mixed_case_quoted_identifiers"),
        "physical_database_meta_data__stores_upper_case_identifiers" => assert_metadata_error(metadata.stores_upper_case_identifiers().await, "stores_upper_case_identifiers"),
        "physical_database_meta_data__stores_upper_case_quoted_identifiers" => assert_metadata_error(metadata.stores_upper_case_quoted_identifiers().await, "stores_upper_case_quoted_identifiers"),
        "physical_database_meta_data__supports_alter_table_with_add_column" => assert_metadata_error(metadata.supports_alter_table_with_add_column().await, "supports_alter_table_with_add_column"),
        "physical_database_meta_data__supports_alter_table_with_drop_column" => assert_metadata_error(metadata.supports_alter_table_with_drop_column().await, "supports_alter_table_with_drop_column"),
        "physical_database_meta_data__supports_ansi92_entry_level_sql" => assert_metadata_error(metadata.supports_ansi92_entry_level_sql().await, "supports_ansi92_entry_level_sql"),
        "physical_database_meta_data__supports_ansi92_full_sql" => assert_metadata_error(metadata.supports_ansi92_full_sql().await, "supports_ansi92_full_sql"),
        "physical_database_meta_data__supports_ansi92_intermediate_sql" => assert_metadata_error(metadata.supports_ansi92_intermediate_sql().await, "supports_ansi92_intermediate_sql"),
        "physical_database_meta_data__supports_catalogs_in_index_definitions" => assert_metadata_error(metadata.supports_catalogs_in_index_definitions().await, "supports_catalogs_in_index_definitions"),
        "physical_database_meta_data__supports_catalogs_in_privilege_definitions" => assert_metadata_error(metadata.supports_catalogs_in_privilege_definitions().await, "supports_catalogs_in_privilege_definitions"),
        "physical_database_meta_data__supports_catalogs_in_procedure_calls" => assert_metadata_error(metadata.supports_catalogs_in_procedure_calls().await, "supports_catalogs_in_procedure_calls"),
        "physical_database_meta_data__supports_catalogs_in_table_definitions" => assert_metadata_error(metadata.supports_catalogs_in_table_definitions().await, "supports_catalogs_in_table_definitions"),
        "physical_database_meta_data__supports_core_sql_grammar" => assert_metadata_error(metadata.supports_core_sql_grammar().await, "supports_core_sql_grammar"),
        "physical_database_meta_data__supports_correlated_subqueries" => assert_metadata_error(metadata.supports_correlated_subqueries().await, "supports_correlated_subqueries"),
        "physical_database_meta_data__supports_data_definition_and_data_manipulation_transactions" => assert_metadata_error(metadata.supports_data_definition_and_data_manipulation_transactions().await, "supports_data_definition_and_data_manipulation_transactions"),
        "physical_database_meta_data__supports_data_manipulation_transactions_only" => assert_metadata_error(metadata.supports_data_manipulation_transactions_only().await, "supports_data_manipulation_transactions_only"),
        "physical_database_meta_data__supports_different_table_correlation_names" => assert_metadata_error(metadata.supports_different_table_correlation_names().await, "supports_different_table_correlation_names"),
        "physical_database_meta_data__supports_extended_sql_grammar" => assert_metadata_error(metadata.supports_extended_sql_grammar().await, "supports_extended_sql_grammar"),
        "physical_database_meta_data__supports_full_outer_joins" => assert_metadata_error(metadata.supports_full_outer_joins().await, "supports_full_outer_joins"),
        "physical_database_meta_data__supports_get_generated_keys" => assert_metadata_error(metadata.supports_get_generated_keys().await, "supports_get_generated_keys"),
        "physical_database_meta_data__supports_group_by_beyond_select" => assert_metadata_error(metadata.supports_group_by_beyond_select().await, "supports_group_by_beyond_select"),
        "physical_database_meta_data__supports_group_by_unrelated" => assert_metadata_error(metadata.supports_group_by_unrelated().await, "supports_group_by_unrelated"),
        "physical_database_meta_data__supports_integrity_enhancement_facility" => assert_metadata_error(metadata.supports_integrity_enhancement_facility().await, "supports_integrity_enhancement_facility"),
        "physical_database_meta_data__supports_limited_outer_joins" => assert_metadata_error(metadata.supports_limited_outer_joins().await, "supports_limited_outer_joins"),
        "physical_database_meta_data__supports_minimum_sql_grammar" => assert_metadata_error(metadata.supports_minimum_sql_grammar().await, "supports_minimum_sql_grammar"),
        "physical_database_meta_data__supports_mixed_case_identifiers" => assert_metadata_error(metadata.supports_mixed_case_identifiers().await, "supports_mixed_case_identifiers"),
        "physical_database_meta_data__supports_mixed_case_quoted_identifiers" => assert_metadata_error(metadata.supports_mixed_case_quoted_identifiers().await, "supports_mixed_case_quoted_identifiers"),
        "physical_database_meta_data__supports_multiple_open_results" => assert_metadata_error(metadata.supports_multiple_open_results().await, "supports_multiple_open_results"),
        "physical_database_meta_data__supports_multiple_result_sets" => assert_metadata_error(metadata.supports_multiple_result_sets().await, "supports_multiple_result_sets"),
        "physical_database_meta_data__supports_multiple_transactions" => assert_metadata_error(metadata.supports_multiple_transactions().await, "supports_multiple_transactions"),
        "physical_database_meta_data__supports_non_nullable_columns" => assert_metadata_error(metadata.supports_non_nullable_columns().await, "supports_non_nullable_columns"),
        "physical_database_meta_data__supports_open_cursors_across_commit" => assert_metadata_error(metadata.supports_open_cursors_across_commit().await, "supports_open_cursors_across_commit"),
        "physical_database_meta_data__supports_open_cursors_across_rollback" => assert_metadata_error(metadata.supports_open_cursors_across_rollback().await, "supports_open_cursors_across_rollback"),
        "physical_database_meta_data__supports_open_statements_across_commit" => assert_metadata_error(metadata.supports_open_statements_across_commit().await, "supports_open_statements_across_commit"),
        "physical_database_meta_data__supports_open_statements_across_rollback" => assert_metadata_error(metadata.supports_open_statements_across_rollback().await, "supports_open_statements_across_rollback"),
        "physical_database_meta_data__supports_order_by_unrelated" => assert_metadata_error(metadata.supports_order_by_unrelated().await, "supports_order_by_unrelated"),
        "physical_database_meta_data__supports_positioned_delete" => assert_metadata_error(metadata.supports_positioned_delete().await, "supports_positioned_delete"),
        "physical_database_meta_data__supports_positioned_update" => assert_metadata_error(metadata.supports_positioned_update().await, "supports_positioned_update"),
        "physical_database_meta_data__supports_result_set_holdability" => assert_metadata_error(metadata.supports_result_set_holdability(2).await, "supports_result_set_holdability"),
        "physical_database_meta_data__supports_schemas_in_index_definitions" => assert_metadata_error(metadata.supports_schemas_in_index_definitions().await, "supports_schemas_in_index_definitions"),
        "physical_database_meta_data__supports_schemas_in_privilege_definitions" => assert_metadata_error(metadata.supports_schemas_in_privilege_definitions().await, "supports_schemas_in_privilege_definitions"),
        "physical_database_meta_data__supports_schemas_in_procedure_calls" => assert_metadata_error(metadata.supports_schemas_in_procedure_calls().await, "supports_schemas_in_procedure_calls"),
        "physical_database_meta_data__supports_schemas_in_table_definitions" => assert_metadata_error(metadata.supports_schemas_in_table_definitions().await, "supports_schemas_in_table_definitions"),
        "physical_database_meta_data__supports_stored_procedures" => assert_metadata_error(metadata.supports_stored_procedures().await, "supports_stored_procedures"),
        "physical_database_meta_data__supports_subqueries_in_exists" => assert_metadata_error(metadata.supports_subqueries_in_exists().await, "supports_subqueries_in_exists"),
        "physical_database_meta_data__supports_subqueries_in_ins" => assert_metadata_error(metadata.supports_subqueries_in_ins().await, "supports_subqueries_in_ins"),
        "physical_database_meta_data__supports_subqueries_in_quantifieds" => assert_metadata_error(metadata.supports_subqueries_in_quantifieds().await, "supports_subqueries_in_quantifieds"),
        "physical_database_meta_data__supports_table_correlation_names" => assert_metadata_error(metadata.supports_table_correlation_names().await, "supports_table_correlation_names"),
        "physical_database_meta_data__supports_union_all" => assert_metadata_error(metadata.supports_union_all().await, "supports_union_all"),
        "physical_database_meta_data__updates_are_detected" => assert_metadata_error(metadata.updates_are_detected(1003).await, "updates_are_detected"),
        "physical_database_meta_data__uses_local_file_per_table" => assert_metadata_error(metadata.uses_local_file_per_table().await, "uses_local_file_per_table"),
        "physical_database_meta_data__uses_local_files" => assert_metadata_error(metadata.uses_local_files().await, "uses_local_files"),
        "physical_prepared_statement__add_parameter_batch" => assert_fail_closed(prepared.add_parameter_batch(vec![prepared_parameter()])),
        "physical_prepared_statement__get_generated_keys" | "physical_statement__get_generated_keys" => assert_fail_closed(statement.get_generated_keys().await),
        "physical_prepared_statement__get_more_results" | "physical_statement__get_more_results" => assert_fail_closed(statement.get_more_results(CurrentResultDisposition::Default).await),
        "physical_prepared_statement__get_result_set" | "physical_statement__get_result_set" => assert_fail_closed(statement.get_result_set().await),
        "physical_prepared_statement__get_update_count" | "physical_statement__get_update_count" => assert_fail_closed(statement.get_update_count()),
        "physical_prepared_statement__set_parameter" => assert_fail_closed(prepared.set_parameter(prepared_parameter())),
        "physical_prepared_statement__sql" => assert_fail_closed(prepared.sql()),
        "physical_prepared_statement__statement_options" | "physical_statement__options" => assert_fail_closed(statement.options()),
        "physical_result_set__update_value" => assert_fail_closed(result_set.update_value(column, ResultSetUpdate::Null)),
        "physical_result_set__update_value_by_label" => assert_fail_closed(result_set.update_value_by_label("value", ResultSetUpdate::Null)),
        "physical_result_set__value_by_label" => assert_fail_closed(result_set.value_by_label("value")),
        "physical_statement__batch" => assert_fail_closed(statement.batch()),
        unknown => panic!("unbound Task 6 method evidence case: {unknown}"),
    }
}

#[tokio::test]
async fn task6_method_default_evidence_matrix() {
    let cases = task6_method_default_evidence_case_ids();
    let mut executed = Vec::with_capacity(cases.len());
    for case_id in cases {
        exercise(case_id).await;
        executed.push(*case_id);
    }
    assert_eq!(executed, cases);
    println!(
        "TASK6_CASE_DIGEST=task6_method_default_evidence_matrix:{}",
        executed.join("|")
    );
}
