// @generated receiver-exact group: DatabaseMetaDataLimitsExt
#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_data_definition_causes_transaction_commit()
 {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::data_definition_causes_transaction_commit";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataLimitsExt::data_definition_causes_transaction_commit(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::data_definition_causes_transaction_commit",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_data_definition_ignored_in_transactions()
{
    let expected =
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::data_definition_ignored_in_transactions";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataLimitsExt::data_definition_ignored_in_transactions(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::data_definition_ignored_in_transactions",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_does_max_row_size_include_blobs() {
    let expected =
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::does_max_row_size_include_blobs";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataLimitsExt::does_max_row_size_include_blobs(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::does_max_row_size_include_blobs",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_catalogs() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_catalogs";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataLimitsExt::get_catalogs(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_catalogs",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_columns() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_columns";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataLimitsExt::get_columns(
        &mut target,
        Some("task6"),
        Some("task6"),
        Some("task6"),
        Some("task6"),
    )
    .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_columns",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_default_transaction_isolation() {
    let expected =
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_default_transaction_isolation";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataLimitsExt::get_default_transaction_isolation(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_default_transaction_isolation",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_max_binary_literal_length() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_binary_literal_length";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataLimitsExt::get_max_binary_literal_length(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_binary_literal_length",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_max_catalog_name_length() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_catalog_name_length";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataLimitsExt::get_max_catalog_name_length(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_catalog_name_length",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_max_char_literal_length() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_char_literal_length";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataLimitsExt::get_max_char_literal_length(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_char_literal_length",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_max_column_name_length() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_column_name_length";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataLimitsExt::get_max_column_name_length(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_column_name_length",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_max_columns_in_group_by() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_columns_in_group_by";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataLimitsExt::get_max_columns_in_group_by(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_columns_in_group_by",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_max_columns_in_index() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_columns_in_index";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataLimitsExt::get_max_columns_in_index(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_columns_in_index",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_max_columns_in_order_by() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_columns_in_order_by";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataLimitsExt::get_max_columns_in_order_by(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_columns_in_order_by",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_max_columns_in_select() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_columns_in_select";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataLimitsExt::get_max_columns_in_select(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_columns_in_select",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_max_columns_in_table() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_columns_in_table";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataLimitsExt::get_max_columns_in_table(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_columns_in_table",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_max_connections() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_connections";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataLimitsExt::get_max_connections(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_connections",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_max_cursor_name_length() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_cursor_name_length";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataLimitsExt::get_max_cursor_name_length(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_cursor_name_length",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_max_index_length() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_index_length";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataLimitsExt::get_max_index_length(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_index_length",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_max_procedure_name_length() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_procedure_name_length";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataLimitsExt::get_max_procedure_name_length(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_procedure_name_length",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_max_row_size() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_row_size";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataLimitsExt::get_max_row_size(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_row_size",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_max_schema_name_length() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_schema_name_length";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataLimitsExt::get_max_schema_name_length(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_schema_name_length",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_max_statement_length() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_statement_length";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataLimitsExt::get_max_statement_length(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_statement_length",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_max_statements() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_statements";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataLimitsExt::get_max_statements(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_statements",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_max_table_name_length() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_table_name_length";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataLimitsExt::get_max_table_name_length(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_table_name_length",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_max_tables_in_select() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_tables_in_select";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataLimitsExt::get_max_tables_in_select(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_tables_in_select",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_max_user_name_length() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_user_name_length";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataLimitsExt::get_max_user_name_length(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_max_user_name_length",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_procedure_columns() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_procedure_columns";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataLimitsExt::get_procedure_columns(
        &mut target,
        Some("task6"),
        Some("task6"),
        Some("task6"),
        Some("task6"),
    )
    .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_procedure_columns",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_procedures() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_procedures";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataLimitsExt::get_procedures(
        &mut target,
        Some("task6"),
        Some("task6"),
        Some("task6"),
    )
    .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_procedures",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_schemas() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_schemas";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataLimitsExt::get_schemas(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_schemas",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_table_types() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_table_types";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataLimitsExt::get_table_types(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_table_types",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_tables() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_tables";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataLimitsExt::get_tables(
        &mut target,
        Some("task6"),
        Some("task6"),
        Some("task6"),
        None,
    )
    .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::get_tables",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_data_definition_and_data_manipulation_transactions()
 {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::supports_data_definition_and_data_manipulation_transactions";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataLimitsExt::supports_data_definition_and_data_manipulation_transactions(
            &mut target,
        )
        .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::supports_data_definition_and_data_manipulation_transactions",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_data_manipulation_transactions_only()
 {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::supports_data_manipulation_transactions_only";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataLimitsExt::supports_data_manipulation_transactions_only(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::supports_data_manipulation_transactions_only",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_transaction_isolation_level() {
    let expected =
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::supports_transaction_isolation_level";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataLimitsExt::supports_transaction_isolation_level(&mut target, 1).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::supports_transaction_isolation_level",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_transactions() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::supports_transactions";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataLimitsExt::supports_transactions(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataLimitsExt::supports_transactions",
    );
}
