//! 物理数据库元数据 SPI。
//!
//! 对应 Java 平台接口：`java.sql.DatabaseMetaData`。Druid canonical
//! `DatabaseMetaDataProxyImpl` 借用本 trait，不要求 Rust 驱动实现 JDBC
//! 类型；每个 Adapter 只需逐项提供真实能力，未支持方法返回明确错误。

use super::{DatabaseMetaDataRowIdLifetime, DruidError, PhysicalResultSet};
use std::sync::Arc;

/// 未池化驱动连接提供的数据库元数据合同。
#[async_trait::async_trait]
pub trait PhysicalDatabaseMetaData: Send {
    /// 委托 Java `DatabaseMetaData#allProceduresAreCallable` 的可观察结果。
    async fn all_procedures_are_callable(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_all_procedures_are_callable",
        })
    }

    /// 委托 Java `DatabaseMetaData#allTablesAreSelectable` 的可观察结果。
    async fn all_tables_are_selectable(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_all_tables_are_selectable",
        })
    }

    /// 委托 Java `DatabaseMetaData#getURL` 的可观察结果。
    async fn get_url(&mut self) -> Result<Option<String>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_url",
        })
    }

    /// 委托 Java `DatabaseMetaData#getUserName` 的可观察结果。
    async fn get_user_name(&mut self) -> Result<Option<String>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_user_name",
        })
    }

    /// 委托 Java `DatabaseMetaData#isReadOnly` 的可观察结果。
    async fn is_read_only(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_is_read_only",
        })
    }

    /// 委托 Java `DatabaseMetaData#nullsAreSortedHigh` 的可观察结果。
    async fn nulls_are_sorted_high(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_nulls_are_sorted_high",
        })
    }

    /// 委托 Java `DatabaseMetaData#nullsAreSortedLow` 的可观察结果。
    async fn nulls_are_sorted_low(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_nulls_are_sorted_low",
        })
    }

    /// 委托 Java `DatabaseMetaData#nullsAreSortedAtStart` 的可观察结果。
    async fn nulls_are_sorted_at_start(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_nulls_are_sorted_at_start",
        })
    }

    /// 委托 Java `DatabaseMetaData#nullsAreSortedAtEnd` 的可观察结果。
    async fn nulls_are_sorted_at_end(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_nulls_are_sorted_at_end",
        })
    }

    /// 委托 Java `DatabaseMetaData#getDatabaseProductName` 的可观察结果。
    async fn get_database_product_name(&mut self) -> Result<Option<String>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_database_product_name",
        })
    }

    /// 委托 Java `DatabaseMetaData#getDatabaseProductVersion` 的可观察结果。
    async fn get_database_product_version(&mut self) -> Result<Option<String>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_database_product_version",
        })
    }

    /// 委托 Java `DatabaseMetaData#getDriverName` 的可观察结果。
    async fn get_driver_name(&mut self) -> Result<Option<String>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_driver_name",
        })
    }

    /// 委托 Java `DatabaseMetaData#getDriverVersion` 的可观察结果。
    async fn get_driver_version(&mut self) -> Result<Option<String>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_driver_version",
        })
    }

    /// 委托 Java `DatabaseMetaData#getDriverMajorVersion` 的可观察结果。
    async fn get_driver_major_version(&mut self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_driver_major_version",
        })
    }

    /// 委托 Java `DatabaseMetaData#getDriverMinorVersion` 的可观察结果。
    async fn get_driver_minor_version(&mut self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_driver_minor_version",
        })
    }

    /// 委托 Java `DatabaseMetaData#usesLocalFiles` 的可观察结果。
    async fn uses_local_files(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_uses_local_files",
        })
    }

    /// 委托 Java `DatabaseMetaData#usesLocalFilePerTable` 的可观察结果。
    async fn uses_local_file_per_table(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_uses_local_file_per_table",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsMixedCaseIdentifiers` 的可观察结果。
    async fn supports_mixed_case_identifiers(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_mixed_case_identifiers",
        })
    }

    /// 委托 Java `DatabaseMetaData#storesUpperCaseIdentifiers` 的可观察结果。
    async fn stores_upper_case_identifiers(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_stores_upper_case_identifiers",
        })
    }

    /// 委托 Java `DatabaseMetaData#storesLowerCaseIdentifiers` 的可观察结果。
    async fn stores_lower_case_identifiers(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_stores_lower_case_identifiers",
        })
    }

    /// 委托 Java `DatabaseMetaData#storesMixedCaseIdentifiers` 的可观察结果。
    async fn stores_mixed_case_identifiers(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_stores_mixed_case_identifiers",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsMixedCaseQuotedIdentifiers` 的可观察结果。
    async fn supports_mixed_case_quoted_identifiers(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_mixed_case_quoted_identifiers",
        })
    }

    /// 委托 Java `DatabaseMetaData#storesUpperCaseQuotedIdentifiers` 的可观察结果。
    async fn stores_upper_case_quoted_identifiers(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_stores_upper_case_quoted_identifiers",
        })
    }

    /// 委托 Java `DatabaseMetaData#storesLowerCaseQuotedIdentifiers` 的可观察结果。
    async fn stores_lower_case_quoted_identifiers(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_stores_lower_case_quoted_identifiers",
        })
    }

    /// 委托 Java `DatabaseMetaData#storesMixedCaseQuotedIdentifiers` 的可观察结果。
    async fn stores_mixed_case_quoted_identifiers(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_stores_mixed_case_quoted_identifiers",
        })
    }

    /// 委托 Java `DatabaseMetaData#getIdentifierQuoteString` 的可观察结果。
    async fn get_identifier_quote_string(&mut self) -> Result<Option<String>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_identifier_quote_string",
        })
    }

    /// 委托 Java `DatabaseMetaData#getSQLKeywords` 的可观察结果。
    async fn get_sql_keywords(&mut self) -> Result<Option<String>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_sql_keywords",
        })
    }

    /// 委托 Java `DatabaseMetaData#getNumericFunctions` 的可观察结果。
    async fn get_numeric_functions(&mut self) -> Result<Option<String>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_numeric_functions",
        })
    }

    /// 委托 Java `DatabaseMetaData#getStringFunctions` 的可观察结果。
    async fn get_string_functions(&mut self) -> Result<Option<String>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_string_functions",
        })
    }

    /// 委托 Java `DatabaseMetaData#getSystemFunctions` 的可观察结果。
    async fn get_system_functions(&mut self) -> Result<Option<String>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_system_functions",
        })
    }

    /// 委托 Java `DatabaseMetaData#getTimeDateFunctions` 的可观察结果。
    async fn get_time_date_functions(&mut self) -> Result<Option<String>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_time_date_functions",
        })
    }

    /// 委托 Java `DatabaseMetaData#getSearchStringEscape` 的可观察结果。
    async fn get_search_string_escape(&mut self) -> Result<Option<String>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_search_string_escape",
        })
    }

    /// 委托 Java `DatabaseMetaData#getExtraNameCharacters` 的可观察结果。
    async fn get_extra_name_characters(&mut self) -> Result<Option<String>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_extra_name_characters",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsAlterTableWithAddColumn` 的可观察结果。
    async fn supports_alter_table_with_add_column(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_alter_table_with_add_column",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsAlterTableWithDropColumn` 的可观察结果。
    async fn supports_alter_table_with_drop_column(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_alter_table_with_drop_column",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsColumnAliasing` 的可观察结果。
    async fn supports_column_aliasing(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_column_aliasing",
        })
    }

    /// 委托 Java `DatabaseMetaData#nullPlusNonNullIsNull` 的可观察结果。
    async fn null_plus_non_null_is_null(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_null_plus_non_null_is_null",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsConvert` 的可观察结果。
    async fn supports_convert(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_convert",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsConvert` 的可观察结果。
    async fn supports_convert_between(
        &mut self,
        from_type: i32,
        to_type: i32,
    ) -> Result<bool, DruidError> {
        let _ = (from_type, to_type);
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_convert_between",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsTableCorrelationNames` 的可观察结果。
    async fn supports_table_correlation_names(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_table_correlation_names",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsDifferentTableCorrelationNames` 的可观察结果。
    async fn supports_different_table_correlation_names(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_different_table_correlation_names",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsExpressionsInOrderBy` 的可观察结果。
    async fn supports_expressions_in_order_by(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_expressions_in_order_by",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsOrderByUnrelated` 的可观察结果。
    async fn supports_order_by_unrelated(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_order_by_unrelated",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsGroupBy` 的可观察结果。
    async fn supports_group_by(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_group_by",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsGroupByUnrelated` 的可观察结果。
    async fn supports_group_by_unrelated(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_group_by_unrelated",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsGroupByBeyondSelect` 的可观察结果。
    async fn supports_group_by_beyond_select(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_group_by_beyond_select",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsLikeEscapeClause` 的可观察结果。
    async fn supports_like_escape_clause(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_like_escape_clause",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsMultipleResultSets` 的可观察结果。
    async fn supports_multiple_result_sets(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_multiple_result_sets",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsMultipleTransactions` 的可观察结果。
    async fn supports_multiple_transactions(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_multiple_transactions",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsNonNullableColumns` 的可观察结果。
    async fn supports_non_nullable_columns(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_non_nullable_columns",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsMinimumSQLGrammar` 的可观察结果。
    async fn supports_minimum_sql_grammar(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_minimum_sql_grammar",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsCoreSQLGrammar` 的可观察结果。
    async fn supports_core_sql_grammar(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_core_sql_grammar",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsExtendedSQLGrammar` 的可观察结果。
    async fn supports_extended_sql_grammar(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_extended_sql_grammar",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsANSI92EntryLevelSQL` 的可观察结果。
    async fn supports_ansi92_entry_level_sql(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_ansi92_entry_level_sql",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsANSI92IntermediateSQL` 的可观察结果。
    async fn supports_ansi92_intermediate_sql(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_ansi92_intermediate_sql",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsANSI92FullSQL` 的可观察结果。
    async fn supports_ansi92_full_sql(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_ansi92_full_sql",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsIntegrityEnhancementFacility` 的可观察结果。
    async fn supports_integrity_enhancement_facility(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_integrity_enhancement_facility",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsOuterJoins` 的可观察结果。
    async fn supports_outer_joins(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_outer_joins",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsFullOuterJoins` 的可观察结果。
    async fn supports_full_outer_joins(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_full_outer_joins",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsLimitedOuterJoins` 的可观察结果。
    async fn supports_limited_outer_joins(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_limited_outer_joins",
        })
    }

    /// 委托 Java `DatabaseMetaData#getSchemaTerm` 的可观察结果。
    async fn get_schema_term(&mut self) -> Result<Option<String>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_schema_term",
        })
    }

    /// 委托 Java `DatabaseMetaData#getProcedureTerm` 的可观察结果。
    async fn get_procedure_term(&mut self) -> Result<Option<String>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_procedure_term",
        })
    }

    /// 委托 Java `DatabaseMetaData#getCatalogTerm` 的可观察结果。
    async fn get_catalog_term(&mut self) -> Result<Option<String>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_catalog_term",
        })
    }

    /// 委托 Java `DatabaseMetaData#isCatalogAtStart` 的可观察结果。
    async fn is_catalog_at_start(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_is_catalog_at_start",
        })
    }

    /// 委托 Java `DatabaseMetaData#getCatalogSeparator` 的可观察结果。
    async fn get_catalog_separator(&mut self) -> Result<Option<String>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_catalog_separator",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsSchemasInDataManipulation` 的可观察结果。
    async fn supports_schemas_in_data_manipulation(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_schemas_in_data_manipulation",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsSchemasInProcedureCalls` 的可观察结果。
    async fn supports_schemas_in_procedure_calls(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_schemas_in_procedure_calls",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsSchemasInTableDefinitions` 的可观察结果。
    async fn supports_schemas_in_table_definitions(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_schemas_in_table_definitions",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsSchemasInIndexDefinitions` 的可观察结果。
    async fn supports_schemas_in_index_definitions(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_schemas_in_index_definitions",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsSchemasInPrivilegeDefinitions` 的可观察结果。
    async fn supports_schemas_in_privilege_definitions(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_schemas_in_privilege_definitions",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsCatalogsInDataManipulation` 的可观察结果。
    async fn supports_catalogs_in_data_manipulation(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_catalogs_in_data_manipulation",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsCatalogsInProcedureCalls` 的可观察结果。
    async fn supports_catalogs_in_procedure_calls(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_catalogs_in_procedure_calls",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsCatalogsInTableDefinitions` 的可观察结果。
    async fn supports_catalogs_in_table_definitions(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_catalogs_in_table_definitions",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsCatalogsInIndexDefinitions` 的可观察结果。
    async fn supports_catalogs_in_index_definitions(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_catalogs_in_index_definitions",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsCatalogsInPrivilegeDefinitions` 的可观察结果。
    async fn supports_catalogs_in_privilege_definitions(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_catalogs_in_privilege_definitions",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsPositionedDelete` 的可观察结果。
    async fn supports_positioned_delete(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_positioned_delete",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsPositionedUpdate` 的可观察结果。
    async fn supports_positioned_update(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_positioned_update",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsSelectForUpdate` 的可观察结果。
    async fn supports_select_for_update(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_select_for_update",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsStoredProcedures` 的可观察结果。
    async fn supports_stored_procedures(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_stored_procedures",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsSubqueriesInComparisons` 的可观察结果。
    async fn supports_subqueries_in_comparisons(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_subqueries_in_comparisons",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsSubqueriesInExists` 的可观察结果。
    async fn supports_subqueries_in_exists(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_subqueries_in_exists",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsSubqueriesInIns` 的可观察结果。
    async fn supports_subqueries_in_ins(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_subqueries_in_ins",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsSubqueriesInQuantifieds` 的可观察结果。
    async fn supports_subqueries_in_quantifieds(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_subqueries_in_quantifieds",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsCorrelatedSubqueries` 的可观察结果。
    async fn supports_correlated_subqueries(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_correlated_subqueries",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsUnion` 的可观察结果。
    async fn supports_union(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_union",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsUnionAll` 的可观察结果。
    async fn supports_union_all(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_union_all",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsOpenCursorsAcrossCommit` 的可观察结果。
    async fn supports_open_cursors_across_commit(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_open_cursors_across_commit",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsOpenCursorsAcrossRollback` 的可观察结果。
    async fn supports_open_cursors_across_rollback(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_open_cursors_across_rollback",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsOpenStatementsAcrossCommit` 的可观察结果。
    async fn supports_open_statements_across_commit(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_open_statements_across_commit",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsOpenStatementsAcrossRollback` 的可观察结果。
    async fn supports_open_statements_across_rollback(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_open_statements_across_rollback",
        })
    }

    /// 委托 Java `DatabaseMetaData#getMaxBinaryLiteralLength` 的可观察结果。
    async fn get_max_binary_literal_length(&mut self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_max_binary_literal_length",
        })
    }

    /// 委托 Java `DatabaseMetaData#getMaxCharLiteralLength` 的可观察结果。
    async fn get_max_char_literal_length(&mut self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_max_char_literal_length",
        })
    }

    /// 委托 Java `DatabaseMetaData#getMaxColumnNameLength` 的可观察结果。
    async fn get_max_column_name_length(&mut self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_max_column_name_length",
        })
    }

    /// 委托 Java `DatabaseMetaData#getMaxColumnsInGroupBy` 的可观察结果。
    async fn get_max_columns_in_group_by(&mut self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_max_columns_in_group_by",
        })
    }

    /// 委托 Java `DatabaseMetaData#getMaxColumnsInIndex` 的可观察结果。
    async fn get_max_columns_in_index(&mut self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_max_columns_in_index",
        })
    }

    /// 委托 Java `DatabaseMetaData#getMaxColumnsInOrderBy` 的可观察结果。
    async fn get_max_columns_in_order_by(&mut self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_max_columns_in_order_by",
        })
    }

    /// 委托 Java `DatabaseMetaData#getMaxColumnsInSelect` 的可观察结果。
    async fn get_max_columns_in_select(&mut self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_max_columns_in_select",
        })
    }

    /// 委托 Java `DatabaseMetaData#getMaxColumnsInTable` 的可观察结果。
    async fn get_max_columns_in_table(&mut self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_max_columns_in_table",
        })
    }

    /// 委托 Java `DatabaseMetaData#getMaxConnections` 的可观察结果。
    async fn get_max_connections(&mut self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_max_connections",
        })
    }

    /// 委托 Java `DatabaseMetaData#getMaxCursorNameLength` 的可观察结果。
    async fn get_max_cursor_name_length(&mut self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_max_cursor_name_length",
        })
    }

    /// 委托 Java `DatabaseMetaData#getMaxIndexLength` 的可观察结果。
    async fn get_max_index_length(&mut self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_max_index_length",
        })
    }

    /// 委托 Java `DatabaseMetaData#getMaxSchemaNameLength` 的可观察结果。
    async fn get_max_schema_name_length(&mut self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_max_schema_name_length",
        })
    }

    /// 委托 Java `DatabaseMetaData#getMaxProcedureNameLength` 的可观察结果。
    async fn get_max_procedure_name_length(&mut self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_max_procedure_name_length",
        })
    }

    /// 委托 Java `DatabaseMetaData#getMaxCatalogNameLength` 的可观察结果。
    async fn get_max_catalog_name_length(&mut self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_max_catalog_name_length",
        })
    }

    /// 委托 Java `DatabaseMetaData#getMaxRowSize` 的可观察结果。
    async fn get_max_row_size(&mut self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_max_row_size",
        })
    }

    /// 委托 Java `DatabaseMetaData#doesMaxRowSizeIncludeBlobs` 的可观察结果。
    async fn does_max_row_size_include_blobs(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_does_max_row_size_include_blobs",
        })
    }

    /// 委托 Java `DatabaseMetaData#getMaxStatementLength` 的可观察结果。
    async fn get_max_statement_length(&mut self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_max_statement_length",
        })
    }

    /// 委托 Java `DatabaseMetaData#getMaxStatements` 的可观察结果。
    async fn get_max_statements(&mut self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_max_statements",
        })
    }

    /// 委托 Java `DatabaseMetaData#getMaxTableNameLength` 的可观察结果。
    async fn get_max_table_name_length(&mut self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_max_table_name_length",
        })
    }

    /// 委托 Java `DatabaseMetaData#getMaxTablesInSelect` 的可观察结果。
    async fn get_max_tables_in_select(&mut self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_max_tables_in_select",
        })
    }

    /// 委托 Java `DatabaseMetaData#getMaxUserNameLength` 的可观察结果。
    async fn get_max_user_name_length(&mut self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_max_user_name_length",
        })
    }

    /// 委托 Java `DatabaseMetaData#getDefaultTransactionIsolation` 的可观察结果。
    async fn get_default_transaction_isolation(&mut self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_default_transaction_isolation",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsTransactions` 的可观察结果。
    async fn supports_transactions(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_transactions",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsTransactionIsolationLevel` 的可观察结果。
    async fn supports_transaction_isolation_level(
        &mut self,
        level: i32,
    ) -> Result<bool, DruidError> {
        let _ = level;
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_transaction_isolation_level",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsDataDefinitionAndDataManipulationTransactions` 的可观察结果。
    async fn supports_data_definition_and_data_manipulation_transactions(
        &mut self,
    ) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation:
                "database_metadata_supports_data_definition_and_data_manipulation_transactions",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsDataManipulationTransactionsOnly` 的可观察结果。
    async fn supports_data_manipulation_transactions_only(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_data_manipulation_transactions_only",
        })
    }

    /// 委托 Java `DatabaseMetaData#dataDefinitionCausesTransactionCommit` 的可观察结果。
    async fn data_definition_causes_transaction_commit(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_data_definition_causes_transaction_commit",
        })
    }

    /// 委托 Java `DatabaseMetaData#dataDefinitionIgnoredInTransactions` 的可观察结果。
    async fn data_definition_ignored_in_transactions(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_data_definition_ignored_in_transactions",
        })
    }

    /// 委托 Java `DatabaseMetaData#getProcedures` 的可观察结果。
    async fn get_procedures(
        &mut self,
        catalog: Option<&str>,
        schema_pattern: Option<&str>,
        procedure_name_pattern: Option<&str>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let _ = (catalog, schema_pattern, procedure_name_pattern);
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_procedures",
        })
    }

    /// 委托 Java `DatabaseMetaData#getProcedureColumns` 的可观察结果。
    async fn get_procedure_columns(
        &mut self,
        catalog: Option<&str>,
        schema_pattern: Option<&str>,
        procedure_name_pattern: Option<&str>,
        column_name_pattern: Option<&str>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let _ = (
            catalog,
            schema_pattern,
            procedure_name_pattern,
            column_name_pattern,
        );
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_procedure_columns",
        })
    }

    /// 委托 Java `DatabaseMetaData#getTables` 的可观察结果。
    async fn get_tables(
        &mut self,
        catalog: Option<&str>,
        schema_pattern: Option<&str>,
        table_name_pattern: Option<&str>,
        types: Option<&[String]>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let _ = (catalog, schema_pattern, table_name_pattern, types);
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_tables",
        })
    }

    /// 委托 Java `DatabaseMetaData#getSchemas` 的可观察结果。
    async fn get_schemas(&mut self) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_schemas",
        })
    }

    /// 委托 Java `DatabaseMetaData#getCatalogs` 的可观察结果。
    async fn get_catalogs(&mut self) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_catalogs",
        })
    }

    /// 委托 Java `DatabaseMetaData#getTableTypes` 的可观察结果。
    async fn get_table_types(&mut self) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_table_types",
        })
    }

    /// 委托 Java `DatabaseMetaData#getColumns` 的可观察结果。
    async fn get_columns(
        &mut self,
        catalog: Option<&str>,
        schema_pattern: Option<&str>,
        table_name_pattern: Option<&str>,
        column_name_pattern: Option<&str>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let _ = (
            catalog,
            schema_pattern,
            table_name_pattern,
            column_name_pattern,
        );
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_columns",
        })
    }

    /// 委托 Java `DatabaseMetaData#getColumnPrivileges` 的可观察结果。
    async fn get_column_privileges(
        &mut self,
        catalog: Option<&str>,
        schema: Option<&str>,
        table: Option<&str>,
        column_name_pattern: Option<&str>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let _ = (catalog, schema, table, column_name_pattern);
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_column_privileges",
        })
    }

    /// 委托 Java `DatabaseMetaData#getTablePrivileges` 的可观察结果。
    async fn get_table_privileges(
        &mut self,
        catalog: Option<&str>,
        schema_pattern: Option<&str>,
        table_name_pattern: Option<&str>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let _ = (catalog, schema_pattern, table_name_pattern);
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_table_privileges",
        })
    }

    /// 委托 Java `DatabaseMetaData#getBestRowIdentifier` 的可观察结果。
    async fn get_best_row_identifier(
        &mut self,
        catalog: Option<&str>,
        schema: Option<&str>,
        table: Option<&str>,
        scope: i32,
        nullable: bool,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let _ = (catalog, schema, table, scope, nullable);
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_best_row_identifier",
        })
    }

    /// 委托 Java `DatabaseMetaData#getVersionColumns` 的可观察结果。
    async fn get_version_columns(
        &mut self,
        catalog: Option<&str>,
        schema: Option<&str>,
        table: Option<&str>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let _ = (catalog, schema, table);
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_version_columns",
        })
    }

    /// 委托 Java `DatabaseMetaData#getPrimaryKeys` 的可观察结果。
    async fn get_primary_keys(
        &mut self,
        catalog: Option<&str>,
        schema: Option<&str>,
        table: Option<&str>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let _ = (catalog, schema, table);
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_primary_keys",
        })
    }

    /// 委托 Java `DatabaseMetaData#getImportedKeys` 的可观察结果。
    async fn get_imported_keys(
        &mut self,
        catalog: Option<&str>,
        schema: Option<&str>,
        table: Option<&str>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let _ = (catalog, schema, table);
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_imported_keys",
        })
    }

    /// 委托 Java `DatabaseMetaData#getExportedKeys` 的可观察结果。
    async fn get_exported_keys(
        &mut self,
        catalog: Option<&str>,
        schema: Option<&str>,
        table: Option<&str>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let _ = (catalog, schema, table);
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_exported_keys",
        })
    }

    /// 委托 Java `DatabaseMetaData#getCrossReference` 的可观察结果。
    async fn get_cross_reference(
        &mut self,
        parent_catalog: Option<&str>,
        parent_schema: Option<&str>,
        parent_table: Option<&str>,
        foreign_catalog: Option<&str>,
        foreign_schema: Option<&str>,
        foreign_table: Option<&str>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let _ = (
            parent_catalog,
            parent_schema,
            parent_table,
            foreign_catalog,
            foreign_schema,
            foreign_table,
        );
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_cross_reference",
        })
    }

    /// 委托 Java `DatabaseMetaData#getTypeInfo` 的可观察结果。
    async fn get_type_info(&mut self) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_type_info",
        })
    }

    /// 委托 Java `DatabaseMetaData#getIndexInfo` 的可观察结果。
    async fn get_index_info(
        &mut self,
        catalog: Option<&str>,
        schema: Option<&str>,
        table: Option<&str>,
        unique: bool,
        approximate: bool,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let _ = (catalog, schema, table, unique, approximate);
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_index_info",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsResultSetType` 的可观察结果。
    async fn supports_result_set_type(&mut self, result_set_type: i32) -> Result<bool, DruidError> {
        let _ = result_set_type;
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_result_set_type",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsResultSetConcurrency` 的可观察结果。
    async fn supports_result_set_concurrency(
        &mut self,
        result_set_type: i32,
        concurrency: i32,
    ) -> Result<bool, DruidError> {
        let _ = (result_set_type, concurrency);
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_result_set_concurrency",
        })
    }

    /// 委托 Java `DatabaseMetaData#ownUpdatesAreVisible` 的可观察结果。
    async fn own_updates_are_visible(&mut self, result_set_type: i32) -> Result<bool, DruidError> {
        let _ = result_set_type;
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_own_updates_are_visible",
        })
    }

    /// 委托 Java `DatabaseMetaData#ownDeletesAreVisible` 的可观察结果。
    async fn own_deletes_are_visible(&mut self, result_set_type: i32) -> Result<bool, DruidError> {
        let _ = result_set_type;
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_own_deletes_are_visible",
        })
    }

    /// 委托 Java `DatabaseMetaData#ownInsertsAreVisible` 的可观察结果。
    async fn own_inserts_are_visible(&mut self, result_set_type: i32) -> Result<bool, DruidError> {
        let _ = result_set_type;
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_own_inserts_are_visible",
        })
    }

    /// 委托 Java `DatabaseMetaData#othersUpdatesAreVisible` 的可观察结果。
    async fn others_updates_are_visible(
        &mut self,
        result_set_type: i32,
    ) -> Result<bool, DruidError> {
        let _ = result_set_type;
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_others_updates_are_visible",
        })
    }

    /// 委托 Java `DatabaseMetaData#othersDeletesAreVisible` 的可观察结果。
    async fn others_deletes_are_visible(
        &mut self,
        result_set_type: i32,
    ) -> Result<bool, DruidError> {
        let _ = result_set_type;
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_others_deletes_are_visible",
        })
    }

    /// 委托 Java `DatabaseMetaData#othersInsertsAreVisible` 的可观察结果。
    async fn others_inserts_are_visible(
        &mut self,
        result_set_type: i32,
    ) -> Result<bool, DruidError> {
        let _ = result_set_type;
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_others_inserts_are_visible",
        })
    }

    /// 委托 Java `DatabaseMetaData#updatesAreDetected` 的可观察结果。
    async fn updates_are_detected(&mut self, result_set_type: i32) -> Result<bool, DruidError> {
        let _ = result_set_type;
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_updates_are_detected",
        })
    }

    /// 委托 Java `DatabaseMetaData#deletesAreDetected` 的可观察结果。
    async fn deletes_are_detected(&mut self, result_set_type: i32) -> Result<bool, DruidError> {
        let _ = result_set_type;
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_deletes_are_detected",
        })
    }

    /// 委托 Java `DatabaseMetaData#insertsAreDetected` 的可观察结果。
    async fn inserts_are_detected(&mut self, result_set_type: i32) -> Result<bool, DruidError> {
        let _ = result_set_type;
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_inserts_are_detected",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsBatchUpdates` 的可观察结果。
    async fn supports_batch_updates(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_batch_updates",
        })
    }

    /// 委托 Java `DatabaseMetaData#getUDTs` 的可观察结果。
    async fn get_ud_ts(
        &mut self,
        catalog: Option<&str>,
        schema_pattern: Option<&str>,
        type_name_pattern: Option<&str>,
        types: Option<&[i32]>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let _ = (catalog, schema_pattern, type_name_pattern, types);
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_ud_ts",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsSavepoints` 的可观察结果。
    async fn supports_savepoints(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_savepoints",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsNamedParameters` 的可观察结果。
    async fn supports_named_parameters(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_named_parameters",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsMultipleOpenResults` 的可观察结果。
    async fn supports_multiple_open_results(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_multiple_open_results",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsGetGeneratedKeys` 的可观察结果。
    async fn supports_get_generated_keys(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_get_generated_keys",
        })
    }

    /// 委托 Java `DatabaseMetaData#getSuperTypes` 的可观察结果。
    async fn get_super_types(
        &mut self,
        catalog: Option<&str>,
        schema_pattern: Option<&str>,
        type_name_pattern: Option<&str>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let _ = (catalog, schema_pattern, type_name_pattern);
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_super_types",
        })
    }

    /// 委托 Java `DatabaseMetaData#getSuperTables` 的可观察结果。
    async fn get_super_tables(
        &mut self,
        catalog: Option<&str>,
        schema_pattern: Option<&str>,
        table_name_pattern: Option<&str>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let _ = (catalog, schema_pattern, table_name_pattern);
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_super_tables",
        })
    }

    /// 委托 Java `DatabaseMetaData#getAttributes` 的可观察结果。
    async fn get_attributes(
        &mut self,
        catalog: Option<&str>,
        schema_pattern: Option<&str>,
        type_name_pattern: Option<&str>,
        attribute_name_pattern: Option<&str>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let _ = (
            catalog,
            schema_pattern,
            type_name_pattern,
            attribute_name_pattern,
        );
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_attributes",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsResultSetHoldability` 的可观察结果。
    async fn supports_result_set_holdability(
        &mut self,
        holdability: i32,
    ) -> Result<bool, DruidError> {
        let _ = holdability;
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_result_set_holdability",
        })
    }

    /// 委托 Java `DatabaseMetaData#getResultSetHoldability` 的可观察结果。
    async fn get_result_set_holdability(&mut self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_result_set_holdability",
        })
    }

    /// 委托 Java `DatabaseMetaData#getDatabaseMajorVersion` 的可观察结果。
    async fn get_database_major_version(&mut self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_database_major_version",
        })
    }

    /// 委托 Java `DatabaseMetaData#getDatabaseMinorVersion` 的可观察结果。
    async fn get_database_minor_version(&mut self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_database_minor_version",
        })
    }

    /// 委托 Java `DatabaseMetaData#getJDBCMajorVersion` 的可观察结果。
    async fn get_jdbc_major_version(&mut self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_jdbc_major_version",
        })
    }

    /// 委托 Java `DatabaseMetaData#getJDBCMinorVersion` 的可观察结果。
    async fn get_jdbc_minor_version(&mut self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_jdbc_minor_version",
        })
    }

    /// 委托 Java `DatabaseMetaData#getSQLStateType` 的可观察结果。
    async fn get_sql_state_type(&mut self) -> Result<i32, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_sql_state_type",
        })
    }

    /// 委托 Java `DatabaseMetaData#locatorsUpdateCopy` 的可观察结果。
    async fn locators_update_copy(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_locators_update_copy",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsStatementPooling` 的可观察结果。
    async fn supports_statement_pooling(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_statement_pooling",
        })
    }

    /// 委托 Java `DatabaseMetaData#getRowIdLifetime` 的可观察结果。
    async fn get_row_id_lifetime(&mut self) -> Result<DatabaseMetaDataRowIdLifetime, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_row_id_lifetime",
        })
    }

    /// 委托 Java `DatabaseMetaData#getSchemas` 的可观察结果。
    async fn get_schemas_with_pattern(
        &mut self,
        catalog: Option<&str>,
        schema_pattern: Option<&str>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let _ = (catalog, schema_pattern);
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_schemas_with_pattern",
        })
    }

    /// 委托 Java `DatabaseMetaData#supportsStoredFunctionsUsingCallSyntax` 的可观察结果。
    async fn supports_stored_functions_using_call_syntax(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_supports_stored_functions_using_call_syntax",
        })
    }

    /// 委托 Java `DatabaseMetaData#autoCommitFailureClosesAllResultSets` 的可观察结果。
    async fn auto_commit_failure_closes_all_result_sets(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_auto_commit_failure_closes_all_result_sets",
        })
    }

    /// 委托 Java `DatabaseMetaData#getClientInfoProperties` 的可观察结果。
    async fn get_client_info_properties(
        &mut self,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_client_info_properties",
        })
    }

    /// 委托 Java `DatabaseMetaData#getFunctions` 的可观察结果。
    async fn get_functions(
        &mut self,
        catalog: Option<&str>,
        schema_pattern: Option<&str>,
        function_name_pattern: Option<&str>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let _ = (catalog, schema_pattern, function_name_pattern);
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_functions",
        })
    }

    /// 委托 Java `DatabaseMetaData#getFunctionColumns` 的可观察结果。
    async fn get_function_columns(
        &mut self,
        catalog: Option<&str>,
        schema_pattern: Option<&str>,
        function_name_pattern: Option<&str>,
        column_name_pattern: Option<&str>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let _ = (
            catalog,
            schema_pattern,
            function_name_pattern,
            column_name_pattern,
        );
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_function_columns",
        })
    }

    /// 委托 Java `DatabaseMetaData#getPseudoColumns` 的可观察结果。
    async fn get_pseudo_columns(
        &mut self,
        catalog: Option<&str>,
        schema_pattern: Option<&str>,
        table_name_pattern: Option<&str>,
        column_name_pattern: Option<&str>,
    ) -> Result<Arc<dyn PhysicalResultSet>, DruidError> {
        let _ = (
            catalog,
            schema_pattern,
            table_name_pattern,
            column_name_pattern,
        );
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_get_pseudo_columns",
        })
    }

    /// 委托 Java `DatabaseMetaData#generatedKeyAlwaysReturned` 的可观察结果。
    async fn generated_key_always_returned(&mut self) -> Result<bool, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "database_metadata_generated_key_always_returned",
        })
    }
}
