// @generated receiver-exact group: DatabaseMetaDataQueryExt
#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_auto_commit_failure_closes_all_result_sets()
 {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::auto_commit_failure_closes_all_result_sets";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataQueryExt::auto_commit_failure_closes_all_result_sets(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::auto_commit_failure_closes_all_result_sets",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_deletes_are_detected() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::deletes_are_detected";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::deletes_are_detected(&mut target, 1).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::deletes_are_detected",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_generated_key_always_returned() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::generated_key_always_returned";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::generated_key_always_returned(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::generated_key_always_returned",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_attributes() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_attributes";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::get_attributes(
        &mut target,
        Some("task6"),
        Some("task6"),
        Some("task6"),
        Some("task6"),
    )
    .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_attributes",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_best_row_identifier() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_best_row_identifier";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::get_best_row_identifier(
        &mut target,
        Some("task6"),
        Some("task6"),
        Some("task6"),
        1,
        true,
    )
    .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_best_row_identifier",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_client_info_properties() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_client_info_properties";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::get_client_info_properties(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_client_info_properties",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_column_privileges() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_column_privileges";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::get_column_privileges(
        &mut target,
        Some("task6"),
        Some("task6"),
        Some("task6"),
        Some("task6"),
    )
    .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_column_privileges",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_cross_reference() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_cross_reference";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::get_cross_reference(
        &mut target,
        Some("task6"),
        Some("task6"),
        Some("task6"),
        Some("task6"),
        Some("task6"),
        Some("task6"),
    )
    .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_cross_reference",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_database_major_version() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_database_major_version";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::get_database_major_version(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_database_major_version",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_database_minor_version() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_database_minor_version";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::get_database_minor_version(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_database_minor_version",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_exported_keys() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_exported_keys";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::get_exported_keys(
        &mut target,
        Some("task6"),
        Some("task6"),
        Some("task6"),
    )
    .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_exported_keys",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_function_columns() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_function_columns";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::get_function_columns(
        &mut target,
        Some("task6"),
        Some("task6"),
        Some("task6"),
        Some("task6"),
    )
    .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_function_columns",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_functions() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_functions";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::get_functions(
        &mut target,
        Some("task6"),
        Some("task6"),
        Some("task6"),
    )
    .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_functions",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_imported_keys() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_imported_keys";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::get_imported_keys(
        &mut target,
        Some("task6"),
        Some("task6"),
        Some("task6"),
    )
    .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_imported_keys",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_index_info() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_index_info";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::get_index_info(
        &mut target,
        Some("task6"),
        Some("task6"),
        Some("task6"),
        true,
        true,
    )
    .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_index_info",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_primary_keys() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_primary_keys";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::get_primary_keys(
        &mut target,
        Some("task6"),
        Some("task6"),
        Some("task6"),
    )
    .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_primary_keys",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_pseudo_columns() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_pseudo_columns";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::get_pseudo_columns(
        &mut target,
        Some("task6"),
        Some("task6"),
        Some("task6"),
        Some("task6"),
    )
    .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_pseudo_columns",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_rdbc_major_version() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_rdbc_major_version";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::get_rdbc_major_version(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_rdbc_major_version",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_rdbc_minor_version() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_rdbc_minor_version";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::get_rdbc_minor_version(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_rdbc_minor_version",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_result_set_holdability() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_result_set_holdability";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::get_result_set_holdability(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_result_set_holdability",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_row_id_lifetime() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_row_id_lifetime";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::get_row_id_lifetime(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_row_id_lifetime",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_schemas_with_pattern() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_schemas_with_pattern";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::get_schemas_with_pattern(
        &mut target,
        Some("task6"),
        Some("task6"),
    )
    .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_schemas_with_pattern",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_sql_state_type() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_sql_state_type";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::get_sql_state_type(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_sql_state_type",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_super_tables() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_super_tables";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::get_super_tables(
        &mut target,
        Some("task6"),
        Some("task6"),
        Some("task6"),
    )
    .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_super_tables",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_super_types() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_super_types";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::get_super_types(
        &mut target,
        Some("task6"),
        Some("task6"),
        Some("task6"),
    )
    .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_super_types",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_table_privileges() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_table_privileges";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::get_table_privileges(
        &mut target,
        Some("task6"),
        Some("task6"),
        Some("task6"),
    )
    .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_table_privileges",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_type_info() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_type_info";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::get_type_info(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_type_info",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_ud_ts() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_ud_ts";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::get_ud_ts(
        &mut target,
        Some("task6"),
        Some("task6"),
        Some("task6"),
        None,
    )
    .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_ud_ts",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_version_columns() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_version_columns";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::get_version_columns(
        &mut target,
        Some("task6"),
        Some("task6"),
        Some("task6"),
    )
    .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::get_version_columns",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_inserts_are_detected() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::inserts_are_detected";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::inserts_are_detected(&mut target, 1).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::inserts_are_detected",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_locators_update_copy() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::locators_update_copy";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::locators_update_copy(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::locators_update_copy",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_others_deletes_are_visible() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::others_deletes_are_visible";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::others_deletes_are_visible(&mut target, 1).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::others_deletes_are_visible",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_others_inserts_are_visible() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::others_inserts_are_visible";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::others_inserts_are_visible(&mut target, 1).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::others_inserts_are_visible",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_others_updates_are_visible() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::others_updates_are_visible";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::others_updates_are_visible(&mut target, 1).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::others_updates_are_visible",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_own_deletes_are_visible() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::own_deletes_are_visible";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::own_deletes_are_visible(&mut target, 1).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::own_deletes_are_visible",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_own_inserts_are_visible() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::own_inserts_are_visible";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::own_inserts_are_visible(&mut target, 1).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::own_inserts_are_visible",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_own_updates_are_visible() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::own_updates_are_visible";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::own_updates_are_visible(&mut target, 1).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::own_updates_are_visible",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_batch_updates() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::supports_batch_updates";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::supports_batch_updates(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::supports_batch_updates",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_get_generated_keys() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::supports_get_generated_keys";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::supports_get_generated_keys(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::supports_get_generated_keys",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_multiple_open_results() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::supports_multiple_open_results";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::supports_multiple_open_results(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::supports_multiple_open_results",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_named_parameters() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::supports_named_parameters";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::supports_named_parameters(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::supports_named_parameters",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_result_set_concurrency() {
    let expected =
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::supports_result_set_concurrency";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::supports_result_set_concurrency(&mut target, 1, 1).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::supports_result_set_concurrency",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_result_set_holdability() {
    let expected =
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::supports_result_set_holdability";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::supports_result_set_holdability(&mut target, 1).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::supports_result_set_holdability",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_result_set_type() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::supports_result_set_type";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::supports_result_set_type(&mut target, 1).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::supports_result_set_type",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_savepoints() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::supports_savepoints";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::supports_savepoints(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::supports_savepoints",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_statement_pooling() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::supports_statement_pooling";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::supports_statement_pooling(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::supports_statement_pooling",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_stored_functions_using_call_syntax()
 {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::supports_stored_functions_using_call_syntax";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataQueryExt::supports_stored_functions_using_call_syntax(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::supports_stored_functions_using_call_syntax",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_updates_are_detected() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataQueryExt::updates_are_detected";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataQueryExt::updates_are_detected(&mut target, 1).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataQueryExt::updates_are_detected",
    );
}
