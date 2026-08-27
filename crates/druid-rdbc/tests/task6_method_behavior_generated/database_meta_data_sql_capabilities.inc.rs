// @generated receiver-exact group: DatabaseMetaDataSqlCapabilitiesExt
#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_catalog_separator() {
    let expected =
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::get_catalog_separator";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataSqlCapabilitiesExt::get_catalog_separator(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::get_catalog_separator",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_catalog_term() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::get_catalog_term";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataSqlCapabilitiesExt::get_catalog_term(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::get_catalog_term",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_procedure_term() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::get_procedure_term";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataSqlCapabilitiesExt::get_procedure_term(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::get_procedure_term",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_get_schema_term() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::get_schema_term";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataSqlCapabilitiesExt::get_schema_term(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::get_schema_term",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_is_catalog_at_start() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::is_catalog_at_start";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataSqlCapabilitiesExt::is_catalog_at_start(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::is_catalog_at_start",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_ansi92_entry_level_sql() {
    let expected =
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_ansi92_entry_level_sql";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataSqlCapabilitiesExt::supports_ansi92_entry_level_sql(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_ansi92_entry_level_sql",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_ansi92_full_sql() {
    let expected =
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_ansi92_full_sql";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataSqlCapabilitiesExt::supports_ansi92_full_sql(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_ansi92_full_sql",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_ansi92_intermediate_sql() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_ansi92_intermediate_sql";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataSqlCapabilitiesExt::supports_ansi92_intermediate_sql(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_ansi92_intermediate_sql",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_catalogs_in_data_manipulation()
{
    let expected = "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_catalogs_in_data_manipulation";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataSqlCapabilitiesExt::supports_catalogs_in_data_manipulation(&mut target)
            .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_catalogs_in_data_manipulation",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_catalogs_in_index_definitions()
{
    let expected = "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_catalogs_in_index_definitions";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataSqlCapabilitiesExt::supports_catalogs_in_index_definitions(&mut target)
            .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_catalogs_in_index_definitions",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_catalogs_in_privilege_definitions()
 {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_catalogs_in_privilege_definitions";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataSqlCapabilitiesExt::supports_catalogs_in_privilege_definitions(&mut target)
            .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_catalogs_in_privilege_definitions",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_catalogs_in_procedure_calls() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_catalogs_in_procedure_calls";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataSqlCapabilitiesExt::supports_catalogs_in_procedure_calls(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_catalogs_in_procedure_calls",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_catalogs_in_table_definitions()
{
    let expected = "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_catalogs_in_table_definitions";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataSqlCapabilitiesExt::supports_catalogs_in_table_definitions(&mut target)
            .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_catalogs_in_table_definitions",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_core_sql_grammar() {
    let expected =
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_core_sql_grammar";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataSqlCapabilitiesExt::supports_core_sql_grammar(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_core_sql_grammar",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_correlated_subqueries() {
    let expected =
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_correlated_subqueries";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataSqlCapabilitiesExt::supports_correlated_subqueries(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_correlated_subqueries",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_extended_sql_grammar() {
    let expected =
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_extended_sql_grammar";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataSqlCapabilitiesExt::supports_extended_sql_grammar(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_extended_sql_grammar",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_full_outer_joins() {
    let expected =
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_full_outer_joins";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataSqlCapabilitiesExt::supports_full_outer_joins(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_full_outer_joins",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_group_by_beyond_select() {
    let expected =
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_group_by_beyond_select";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataSqlCapabilitiesExt::supports_group_by_beyond_select(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_group_by_beyond_select",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_integrity_enhancement_facility()
{
    let expected = "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_integrity_enhancement_facility";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataSqlCapabilitiesExt::supports_integrity_enhancement_facility(&mut target)
            .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_integrity_enhancement_facility",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_like_escape_clause() {
    let expected =
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_like_escape_clause";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataSqlCapabilitiesExt::supports_like_escape_clause(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_like_escape_clause",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_limited_outer_joins() {
    let expected =
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_limited_outer_joins";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataSqlCapabilitiesExt::supports_limited_outer_joins(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_limited_outer_joins",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_minimum_sql_grammar() {
    let expected =
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_minimum_sql_grammar";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataSqlCapabilitiesExt::supports_minimum_sql_grammar(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_minimum_sql_grammar",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_multiple_result_sets() {
    let expected =
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_multiple_result_sets";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataSqlCapabilitiesExt::supports_multiple_result_sets(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_multiple_result_sets",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_multiple_transactions() {
    let expected =
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_multiple_transactions";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataSqlCapabilitiesExt::supports_multiple_transactions(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_multiple_transactions",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_non_nullable_columns() {
    let expected =
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_non_nullable_columns";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataSqlCapabilitiesExt::supports_non_nullable_columns(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_non_nullable_columns",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_open_cursors_across_commit() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_open_cursors_across_commit";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataSqlCapabilitiesExt::supports_open_cursors_across_commit(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_open_cursors_across_commit",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_open_cursors_across_rollback() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_open_cursors_across_rollback";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataSqlCapabilitiesExt::supports_open_cursors_across_rollback(&mut target)
            .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_open_cursors_across_rollback",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_open_statements_across_commit()
{
    let expected = "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_open_statements_across_commit";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataSqlCapabilitiesExt::supports_open_statements_across_commit(&mut target)
            .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_open_statements_across_commit",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_open_statements_across_rollback()
 {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_open_statements_across_rollback";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataSqlCapabilitiesExt::supports_open_statements_across_rollback(&mut target)
            .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_open_statements_across_rollback",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_outer_joins() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_outer_joins";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataSqlCapabilitiesExt::supports_outer_joins(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_outer_joins",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_positioned_delete() {
    let expected =
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_positioned_delete";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataSqlCapabilitiesExt::supports_positioned_delete(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_positioned_delete",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_positioned_update() {
    let expected =
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_positioned_update";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataSqlCapabilitiesExt::supports_positioned_update(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_positioned_update",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_schemas_in_data_manipulation() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_schemas_in_data_manipulation";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataSqlCapabilitiesExt::supports_schemas_in_data_manipulation(&mut target)
            .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_schemas_in_data_manipulation",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_schemas_in_index_definitions() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_schemas_in_index_definitions";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataSqlCapabilitiesExt::supports_schemas_in_index_definitions(&mut target)
            .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_schemas_in_index_definitions",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_schemas_in_privilege_definitions()
 {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_schemas_in_privilege_definitions";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataSqlCapabilitiesExt::supports_schemas_in_privilege_definitions(&mut target)
            .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_schemas_in_privilege_definitions",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_schemas_in_procedure_calls() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_schemas_in_procedure_calls";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataSqlCapabilitiesExt::supports_schemas_in_procedure_calls(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_schemas_in_procedure_calls",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_schemas_in_table_definitions() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_schemas_in_table_definitions";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataSqlCapabilitiesExt::supports_schemas_in_table_definitions(&mut target)
            .await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_schemas_in_table_definitions",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_select_for_update() {
    let expected =
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_select_for_update";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataSqlCapabilitiesExt::supports_select_for_update(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_select_for_update",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_stored_procedures() {
    let expected =
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_stored_procedures";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataSqlCapabilitiesExt::supports_stored_procedures(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_stored_procedures",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_subqueries_in_comparisons() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_subqueries_in_comparisons";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataSqlCapabilitiesExt::supports_subqueries_in_comparisons(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_subqueries_in_comparisons",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_subqueries_in_exists() {
    let expected =
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_subqueries_in_exists";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataSqlCapabilitiesExt::supports_subqueries_in_exists(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_subqueries_in_exists",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_subqueries_in_ins() {
    let expected =
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_subqueries_in_ins";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataSqlCapabilitiesExt::supports_subqueries_in_ins(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_subqueries_in_ins",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_subqueries_in_quantifieds() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_subqueries_in_quantifieds";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual =
        DatabaseMetaDataSqlCapabilitiesExt::supports_subqueries_in_quantifieds(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_subqueries_in_quantifieds",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_union() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_union";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataSqlCapabilitiesExt::supports_union(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_union",
    );
}

#[tokio::test]
async fn task6_method_behavior_physical_database_meta_data_supports_union_all() {
    let expected = "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_union_all";
    let mut target = StrictDatabaseMetaData::new(expected);
    let actual = DatabaseMetaDataSqlCapabilitiesExt::supports_union_all(&mut target).await;
    assert_trace(
        actual,
        "druid_rdbc::metadata::DatabaseMetaDataSqlCapabilitiesExt::supports_union_all",
    );
}
