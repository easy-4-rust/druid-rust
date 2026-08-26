use std::fmt;

/// DatabaseMetaData 方法标识；每个 variant 唯一绑定一个 BASE direct method。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DatabaseMetaDataOperation {
    /// 对应 JDBC-like 方法 `all_procedures_are_callable`。
    AllProceduresAreCallable,
    /// 对应 JDBC-like 方法 `all_tables_are_selectable`。
    AllTablesAreSelectable,
    /// 对应 JDBC-like 方法 `get_url`。
    GetUrl,
    /// 对应 JDBC-like 方法 `get_user_name`。
    GetUserName,
    /// 对应 JDBC-like 方法 `is_read_only`。
    IsReadOnly,
    /// 对应 JDBC-like 方法 `nulls_are_sorted_high`。
    NullsAreSortedHigh,
    /// 对应 JDBC-like 方法 `nulls_are_sorted_low`。
    NullsAreSortedLow,
    /// 对应 JDBC-like 方法 `nulls_are_sorted_at_start`。
    NullsAreSortedAtStart,
    /// 对应 JDBC-like 方法 `nulls_are_sorted_at_end`。
    NullsAreSortedAtEnd,
    /// 对应 JDBC-like 方法 `get_database_product_name`。
    GetDatabaseProductName,
    /// 对应 JDBC-like 方法 `get_database_product_version`。
    GetDatabaseProductVersion,
    /// 对应 JDBC-like 方法 `get_driver_name`。
    GetDriverName,
    /// 对应 JDBC-like 方法 `get_driver_version`。
    GetDriverVersion,
    /// 对应 JDBC-like 方法 `get_driver_major_version`。
    GetDriverMajorVersion,
    /// 对应 JDBC-like 方法 `get_driver_minor_version`。
    GetDriverMinorVersion,
    /// 对应 JDBC-like 方法 `uses_local_files`。
    UsesLocalFiles,
    /// 对应 JDBC-like 方法 `uses_local_file_per_table`。
    UsesLocalFilePerTable,
    /// 对应 JDBC-like 方法 `supports_mixed_case_identifiers`。
    SupportsMixedCaseIdentifiers,
    /// 对应 JDBC-like 方法 `stores_upper_case_identifiers`。
    StoresUpperCaseIdentifiers,
    /// 对应 JDBC-like 方法 `stores_lower_case_identifiers`。
    StoresLowerCaseIdentifiers,
    /// 对应 JDBC-like 方法 `stores_mixed_case_identifiers`。
    StoresMixedCaseIdentifiers,
    /// 对应 JDBC-like 方法 `supports_mixed_case_quoted_identifiers`。
    SupportsMixedCaseQuotedIdentifiers,
    /// 对应 JDBC-like 方法 `stores_upper_case_quoted_identifiers`。
    StoresUpperCaseQuotedIdentifiers,
    /// 对应 JDBC-like 方法 `stores_lower_case_quoted_identifiers`。
    StoresLowerCaseQuotedIdentifiers,
    /// 对应 JDBC-like 方法 `stores_mixed_case_quoted_identifiers`。
    StoresMixedCaseQuotedIdentifiers,
    /// 对应 JDBC-like 方法 `get_identifier_quote_string`。
    GetIdentifierQuoteString,
    /// 对应 JDBC-like 方法 `get_sql_keywords`。
    GetSqlKeywords,
    /// 对应 JDBC-like 方法 `get_numeric_functions`。
    GetNumericFunctions,
    /// 对应 JDBC-like 方法 `get_string_functions`。
    GetStringFunctions,
    /// 对应 JDBC-like 方法 `get_system_functions`。
    GetSystemFunctions,
    /// 对应 JDBC-like 方法 `get_time_date_functions`。
    GetTimeDateFunctions,
    /// 对应 JDBC-like 方法 `get_search_string_escape`。
    GetSearchStringEscape,
    /// 对应 JDBC-like 方法 `get_extra_name_characters`。
    GetExtraNameCharacters,
    /// 对应 JDBC-like 方法 `supports_alter_table_with_add_column`。
    SupportsAlterTableWithAddColumn,
    /// 对应 JDBC-like 方法 `supports_alter_table_with_drop_column`。
    SupportsAlterTableWithDropColumn,
    /// 对应 JDBC-like 方法 `supports_column_aliasing`。
    SupportsColumnAliasing,
    /// 对应 JDBC-like 方法 `null_plus_non_null_is_null`。
    NullPlusNonNullIsNull,
    /// 对应 JDBC-like 方法 `supports_convert`。
    SupportsConvert,
    /// 对应 JDBC-like 方法 `supports_convert_between`。
    SupportsConvertBetween,
    /// 对应 JDBC-like 方法 `supports_table_correlation_names`。
    SupportsTableCorrelationNames,
    /// 对应 JDBC-like 方法 `supports_different_table_correlation_names`。
    SupportsDifferentTableCorrelationNames,
    /// 对应 JDBC-like 方法 `supports_expressions_in_order_by`。
    SupportsExpressionsInOrderBy,
    /// 对应 JDBC-like 方法 `supports_order_by_unrelated`。
    SupportsOrderByUnrelated,
    /// 对应 JDBC-like 方法 `supports_group_by`。
    SupportsGroupBy,
    /// 对应 JDBC-like 方法 `supports_group_by_unrelated`。
    SupportsGroupByUnrelated,
    /// 对应 JDBC-like 方法 `supports_group_by_beyond_select`。
    SupportsGroupByBeyondSelect,
    /// 对应 JDBC-like 方法 `supports_like_escape_clause`。
    SupportsLikeEscapeClause,
    /// 对应 JDBC-like 方法 `supports_multiple_result_sets`。
    SupportsMultipleResultSets,
    /// 对应 JDBC-like 方法 `supports_multiple_transactions`。
    SupportsMultipleTransactions,
    /// 对应 JDBC-like 方法 `supports_non_nullable_columns`。
    SupportsNonNullableColumns,
    /// 对应 JDBC-like 方法 `supports_minimum_sql_grammar`。
    SupportsMinimumSqlGrammar,
    /// 对应 JDBC-like 方法 `supports_core_sql_grammar`。
    SupportsCoreSqlGrammar,
    /// 对应 JDBC-like 方法 `supports_extended_sql_grammar`。
    SupportsExtendedSqlGrammar,
    /// 对应 JDBC-like 方法 `supports_ansi92_entry_level_sql`。
    SupportsAnsi92EntryLevelSql,
    /// 对应 JDBC-like 方法 `supports_ansi92_intermediate_sql`。
    SupportsAnsi92IntermediateSql,
    /// 对应 JDBC-like 方法 `supports_ansi92_full_sql`。
    SupportsAnsi92FullSql,
    /// 对应 JDBC-like 方法 `supports_integrity_enhancement_facility`。
    SupportsIntegrityEnhancementFacility,
    /// 对应 JDBC-like 方法 `supports_outer_joins`。
    SupportsOuterJoins,
    /// 对应 JDBC-like 方法 `supports_full_outer_joins`。
    SupportsFullOuterJoins,
    /// 对应 JDBC-like 方法 `supports_limited_outer_joins`。
    SupportsLimitedOuterJoins,
    /// 对应 JDBC-like 方法 `get_schema_term`。
    GetSchemaTerm,
    /// 对应 JDBC-like 方法 `get_procedure_term`。
    GetProcedureTerm,
    /// 对应 JDBC-like 方法 `get_catalog_term`。
    GetCatalogTerm,
    /// 对应 JDBC-like 方法 `is_catalog_at_start`。
    IsCatalogAtStart,
    /// 对应 JDBC-like 方法 `get_catalog_separator`。
    GetCatalogSeparator,
    /// 对应 JDBC-like 方法 `supports_schemas_in_data_manipulation`。
    SupportsSchemasInDataManipulation,
    /// 对应 JDBC-like 方法 `supports_schemas_in_procedure_calls`。
    SupportsSchemasInProcedureCalls,
    /// 对应 JDBC-like 方法 `supports_schemas_in_table_definitions`。
    SupportsSchemasInTableDefinitions,
    /// 对应 JDBC-like 方法 `supports_schemas_in_index_definitions`。
    SupportsSchemasInIndexDefinitions,
    /// 对应 JDBC-like 方法 `supports_schemas_in_privilege_definitions`。
    SupportsSchemasInPrivilegeDefinitions,
    /// 对应 JDBC-like 方法 `supports_catalogs_in_data_manipulation`。
    SupportsCatalogsInDataManipulation,
    /// 对应 JDBC-like 方法 `supports_catalogs_in_procedure_calls`。
    SupportsCatalogsInProcedureCalls,
    /// 对应 JDBC-like 方法 `supports_catalogs_in_table_definitions`。
    SupportsCatalogsInTableDefinitions,
    /// 对应 JDBC-like 方法 `supports_catalogs_in_index_definitions`。
    SupportsCatalogsInIndexDefinitions,
    /// 对应 JDBC-like 方法 `supports_catalogs_in_privilege_definitions`。
    SupportsCatalogsInPrivilegeDefinitions,
    /// 对应 JDBC-like 方法 `supports_positioned_delete`。
    SupportsPositionedDelete,
    /// 对应 JDBC-like 方法 `supports_positioned_update`。
    SupportsPositionedUpdate,
    /// 对应 JDBC-like 方法 `supports_select_for_update`。
    SupportsSelectForUpdate,
    /// 对应 JDBC-like 方法 `supports_stored_procedures`。
    SupportsStoredProcedures,
    /// 对应 JDBC-like 方法 `supports_subqueries_in_comparisons`。
    SupportsSubqueriesInComparisons,
    /// 对应 JDBC-like 方法 `supports_subqueries_in_exists`。
    SupportsSubqueriesInExists,
    /// 对应 JDBC-like 方法 `supports_subqueries_in_ins`。
    SupportsSubqueriesInIns,
    /// 对应 JDBC-like 方法 `supports_subqueries_in_quantifieds`。
    SupportsSubqueriesInQuantifieds,
    /// 对应 JDBC-like 方法 `supports_correlated_subqueries`。
    SupportsCorrelatedSubqueries,
    /// 对应 JDBC-like 方法 `supports_union`。
    SupportsUnion,
    /// 对应 JDBC-like 方法 `supports_union_all`。
    SupportsUnionAll,
    /// 对应 JDBC-like 方法 `supports_open_cursors_across_commit`。
    SupportsOpenCursorsAcrossCommit,
    /// 对应 JDBC-like 方法 `supports_open_cursors_across_rollback`。
    SupportsOpenCursorsAcrossRollback,
    /// 对应 JDBC-like 方法 `supports_open_statements_across_commit`。
    SupportsOpenStatementsAcrossCommit,
    /// 对应 JDBC-like 方法 `supports_open_statements_across_rollback`。
    SupportsOpenStatementsAcrossRollback,
    /// 对应 JDBC-like 方法 `get_max_binary_literal_length`。
    GetMaxBinaryLiteralLength,
    /// 对应 JDBC-like 方法 `get_max_char_literal_length`。
    GetMaxCharLiteralLength,
    /// 对应 JDBC-like 方法 `get_max_column_name_length`。
    GetMaxColumnNameLength,
    /// 对应 JDBC-like 方法 `get_max_columns_in_group_by`。
    GetMaxColumnsInGroupBy,
    /// 对应 JDBC-like 方法 `get_max_columns_in_index`。
    GetMaxColumnsInIndex,
    /// 对应 JDBC-like 方法 `get_max_columns_in_order_by`。
    GetMaxColumnsInOrderBy,
    /// 对应 JDBC-like 方法 `get_max_columns_in_select`。
    GetMaxColumnsInSelect,
    /// 对应 JDBC-like 方法 `get_max_columns_in_table`。
    GetMaxColumnsInTable,
    /// 对应 JDBC-like 方法 `get_max_connections`。
    GetMaxConnections,
    /// 对应 JDBC-like 方法 `get_max_cursor_name_length`。
    GetMaxCursorNameLength,
    /// 对应 JDBC-like 方法 `get_max_index_length`。
    GetMaxIndexLength,
    /// 对应 JDBC-like 方法 `get_max_schema_name_length`。
    GetMaxSchemaNameLength,
    /// 对应 JDBC-like 方法 `get_max_procedure_name_length`。
    GetMaxProcedureNameLength,
    /// 对应 JDBC-like 方法 `get_max_catalog_name_length`。
    GetMaxCatalogNameLength,
    /// 对应 JDBC-like 方法 `get_max_row_size`。
    GetMaxRowSize,
    /// 对应 JDBC-like 方法 `does_max_row_size_include_blobs`。
    DoesMaxRowSizeIncludeBlobs,
    /// 对应 JDBC-like 方法 `get_max_statement_length`。
    GetMaxStatementLength,
    /// 对应 JDBC-like 方法 `get_max_statements`。
    GetMaxStatements,
    /// 对应 JDBC-like 方法 `get_max_table_name_length`。
    GetMaxTableNameLength,
    /// 对应 JDBC-like 方法 `get_max_tables_in_select`。
    GetMaxTablesInSelect,
    /// 对应 JDBC-like 方法 `get_max_user_name_length`。
    GetMaxUserNameLength,
    /// 对应 JDBC-like 方法 `get_default_transaction_isolation`。
    GetDefaultTransactionIsolation,
    /// 对应 JDBC-like 方法 `supports_transactions`。
    SupportsTransactions,
    /// 对应 JDBC-like 方法 `supports_transaction_isolation_level`。
    SupportsTransactionIsolationLevel,
    /// 对应 JDBC-like 方法 `supports_data_definition_and_data_manipulation_transactions`。
    SupportsDataDefinitionAndDataManipulationTransactions,
    /// 对应 JDBC-like 方法 `supports_data_manipulation_transactions_only`。
    SupportsDataManipulationTransactionsOnly,
    /// 对应 JDBC-like 方法 `data_definition_causes_transaction_commit`。
    DataDefinitionCausesTransactionCommit,
    /// 对应 JDBC-like 方法 `data_definition_ignored_in_transactions`。
    DataDefinitionIgnoredInTransactions,
    /// 对应 JDBC-like 方法 `get_procedures`。
    GetProcedures,
    /// 对应 JDBC-like 方法 `get_procedure_columns`。
    GetProcedureColumns,
    /// 对应 JDBC-like 方法 `get_tables`。
    GetTables,
    /// 对应 JDBC-like 方法 `get_schemas`。
    GetSchemas,
    /// 对应 JDBC-like 方法 `get_catalogs`。
    GetCatalogs,
    /// 对应 JDBC-like 方法 `get_table_types`。
    GetTableTypes,
    /// 对应 JDBC-like 方法 `get_columns`。
    GetColumns,
    /// 对应 JDBC-like 方法 `get_column_privileges`。
    GetColumnPrivileges,
    /// 对应 JDBC-like 方法 `get_table_privileges`。
    GetTablePrivileges,
    /// 对应 JDBC-like 方法 `get_best_row_identifier`。
    GetBestRowIdentifier,
    /// 对应 JDBC-like 方法 `get_version_columns`。
    GetVersionColumns,
    /// 对应 JDBC-like 方法 `get_primary_keys`。
    GetPrimaryKeys,
    /// 对应 JDBC-like 方法 `get_imported_keys`。
    GetImportedKeys,
    /// 对应 JDBC-like 方法 `get_exported_keys`。
    GetExportedKeys,
    /// 对应 JDBC-like 方法 `get_cross_reference`。
    GetCrossReference,
    /// 对应 JDBC-like 方法 `get_type_info`。
    GetTypeInfo,
    /// 对应 JDBC-like 方法 `get_index_info`。
    GetIndexInfo,
    /// 对应 JDBC-like 方法 `supports_result_set_type`。
    SupportsResultSetType,
    /// 对应 JDBC-like 方法 `supports_result_set_concurrency`。
    SupportsResultSetConcurrency,
    /// 对应 JDBC-like 方法 `own_updates_are_visible`。
    OwnUpdatesAreVisible,
    /// 对应 JDBC-like 方法 `own_deletes_are_visible`。
    OwnDeletesAreVisible,
    /// 对应 JDBC-like 方法 `own_inserts_are_visible`。
    OwnInsertsAreVisible,
    /// 对应 JDBC-like 方法 `others_updates_are_visible`。
    OthersUpdatesAreVisible,
    /// 对应 JDBC-like 方法 `others_deletes_are_visible`。
    OthersDeletesAreVisible,
    /// 对应 JDBC-like 方法 `others_inserts_are_visible`。
    OthersInsertsAreVisible,
    /// 对应 JDBC-like 方法 `updates_are_detected`。
    UpdatesAreDetected,
    /// 对应 JDBC-like 方法 `deletes_are_detected`。
    DeletesAreDetected,
    /// 对应 JDBC-like 方法 `inserts_are_detected`。
    InsertsAreDetected,
    /// 对应 JDBC-like 方法 `supports_batch_updates`。
    SupportsBatchUpdates,
    /// 对应 JDBC-like 方法 `get_ud_ts`。
    GetUdTs,
    /// 对应 JDBC-like 方法 `supports_savepoints`。
    SupportsSavepoints,
    /// 对应 JDBC-like 方法 `supports_named_parameters`。
    SupportsNamedParameters,
    /// 对应 JDBC-like 方法 `supports_multiple_open_results`。
    SupportsMultipleOpenResults,
    /// 对应 JDBC-like 方法 `supports_get_generated_keys`。
    SupportsGetGeneratedKeys,
    /// 对应 JDBC-like 方法 `get_super_types`。
    GetSuperTypes,
    /// 对应 JDBC-like 方法 `get_super_tables`。
    GetSuperTables,
    /// 对应 JDBC-like 方法 `get_attributes`。
    GetAttributes,
    /// 对应 JDBC-like 方法 `supports_result_set_holdability`。
    SupportsResultSetHoldability,
    /// 对应 JDBC-like 方法 `get_result_set_holdability`。
    GetResultSetHoldability,
    /// 对应 JDBC-like 方法 `get_database_major_version`。
    GetDatabaseMajorVersion,
    /// 对应 JDBC-like 方法 `get_database_minor_version`。
    GetDatabaseMinorVersion,
    /// 对应 JDBC-like 方法 `get_rdbc_major_version`。
    GetRdbcMajorVersion,
    /// 对应 JDBC-like 方法 `get_rdbc_minor_version`。
    GetRdbcMinorVersion,
    /// 对应 JDBC-like 方法 `get_sql_state_type`。
    GetSqlStateType,
    /// 对应 JDBC-like 方法 `locators_update_copy`。
    LocatorsUpdateCopy,
    /// 对应 JDBC-like 方法 `supports_statement_pooling`。
    SupportsStatementPooling,
    /// 对应 JDBC-like 方法 `get_row_id_lifetime`。
    GetRowIdLifetime,
    /// 对应 JDBC-like 方法 `get_schemas_with_pattern`。
    GetSchemasWithPattern,
    /// 对应 JDBC-like 方法 `supports_stored_functions_using_call_syntax`。
    SupportsStoredFunctionsUsingCallSyntax,
    /// 对应 JDBC-like 方法 `auto_commit_failure_closes_all_result_sets`。
    AutoCommitFailureClosesAllResultSets,
    /// 对应 JDBC-like 方法 `get_client_info_properties`。
    GetClientInfoProperties,
    /// 对应 JDBC-like 方法 `get_functions`。
    GetFunctions,
    /// 对应 JDBC-like 方法 `get_function_columns`。
    GetFunctionColumns,
    /// 对应 JDBC-like 方法 `get_pseudo_columns`。
    GetPseudoColumns,
    /// 对应 JDBC-like 方法 `generated_key_always_returned`。
    GeneratedKeyAlwaysReturned,
}

impl DatabaseMetaDataOperation {
    /// 返回稳定的 JDBC-like operation name。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AllProceduresAreCallable => "all_procedures_are_callable",
            Self::AllTablesAreSelectable => "all_tables_are_selectable",
            Self::GetUrl => "get_url",
            Self::GetUserName => "get_user_name",
            Self::IsReadOnly => "is_read_only",
            Self::NullsAreSortedHigh => "nulls_are_sorted_high",
            Self::NullsAreSortedLow => "nulls_are_sorted_low",
            Self::NullsAreSortedAtStart => "nulls_are_sorted_at_start",
            Self::NullsAreSortedAtEnd => "nulls_are_sorted_at_end",
            Self::GetDatabaseProductName => "get_database_product_name",
            Self::GetDatabaseProductVersion => "get_database_product_version",
            Self::GetDriverName => "get_driver_name",
            Self::GetDriverVersion => "get_driver_version",
            Self::GetDriverMajorVersion => "get_driver_major_version",
            Self::GetDriverMinorVersion => "get_driver_minor_version",
            Self::UsesLocalFiles => "uses_local_files",
            Self::UsesLocalFilePerTable => "uses_local_file_per_table",
            Self::SupportsMixedCaseIdentifiers => "supports_mixed_case_identifiers",
            Self::StoresUpperCaseIdentifiers => "stores_upper_case_identifiers",
            Self::StoresLowerCaseIdentifiers => "stores_lower_case_identifiers",
            Self::StoresMixedCaseIdentifiers => "stores_mixed_case_identifiers",
            Self::SupportsMixedCaseQuotedIdentifiers => "supports_mixed_case_quoted_identifiers",
            Self::StoresUpperCaseQuotedIdentifiers => "stores_upper_case_quoted_identifiers",
            Self::StoresLowerCaseQuotedIdentifiers => "stores_lower_case_quoted_identifiers",
            Self::StoresMixedCaseQuotedIdentifiers => "stores_mixed_case_quoted_identifiers",
            Self::GetIdentifierQuoteString => "get_identifier_quote_string",
            Self::GetSqlKeywords => "get_sql_keywords",
            Self::GetNumericFunctions => "get_numeric_functions",
            Self::GetStringFunctions => "get_string_functions",
            Self::GetSystemFunctions => "get_system_functions",
            Self::GetTimeDateFunctions => "get_time_date_functions",
            Self::GetSearchStringEscape => "get_search_string_escape",
            Self::GetExtraNameCharacters => "get_extra_name_characters",
            Self::SupportsAlterTableWithAddColumn => "supports_alter_table_with_add_column",
            Self::SupportsAlterTableWithDropColumn => "supports_alter_table_with_drop_column",
            Self::SupportsColumnAliasing => "supports_column_aliasing",
            Self::NullPlusNonNullIsNull => "null_plus_non_null_is_null",
            Self::SupportsConvert => "supports_convert",
            Self::SupportsConvertBetween => "supports_convert_between",
            Self::SupportsTableCorrelationNames => "supports_table_correlation_names",
            Self::SupportsDifferentTableCorrelationNames => {
                "supports_different_table_correlation_names"
            }
            Self::SupportsExpressionsInOrderBy => "supports_expressions_in_order_by",
            Self::SupportsOrderByUnrelated => "supports_order_by_unrelated",
            Self::SupportsGroupBy => "supports_group_by",
            Self::SupportsGroupByUnrelated => "supports_group_by_unrelated",
            Self::SupportsGroupByBeyondSelect => "supports_group_by_beyond_select",
            Self::SupportsLikeEscapeClause => "supports_like_escape_clause",
            Self::SupportsMultipleResultSets => "supports_multiple_result_sets",
            Self::SupportsMultipleTransactions => "supports_multiple_transactions",
            Self::SupportsNonNullableColumns => "supports_non_nullable_columns",
            Self::SupportsMinimumSqlGrammar => "supports_minimum_sql_grammar",
            Self::SupportsCoreSqlGrammar => "supports_core_sql_grammar",
            Self::SupportsExtendedSqlGrammar => "supports_extended_sql_grammar",
            Self::SupportsAnsi92EntryLevelSql => "supports_ansi92_entry_level_sql",
            Self::SupportsAnsi92IntermediateSql => "supports_ansi92_intermediate_sql",
            Self::SupportsAnsi92FullSql => "supports_ansi92_full_sql",
            Self::SupportsIntegrityEnhancementFacility => "supports_integrity_enhancement_facility",
            Self::SupportsOuterJoins => "supports_outer_joins",
            Self::SupportsFullOuterJoins => "supports_full_outer_joins",
            Self::SupportsLimitedOuterJoins => "supports_limited_outer_joins",
            Self::GetSchemaTerm => "get_schema_term",
            Self::GetProcedureTerm => "get_procedure_term",
            Self::GetCatalogTerm => "get_catalog_term",
            Self::IsCatalogAtStart => "is_catalog_at_start",
            Self::GetCatalogSeparator => "get_catalog_separator",
            Self::SupportsSchemasInDataManipulation => "supports_schemas_in_data_manipulation",
            Self::SupportsSchemasInProcedureCalls => "supports_schemas_in_procedure_calls",
            Self::SupportsSchemasInTableDefinitions => "supports_schemas_in_table_definitions",
            Self::SupportsSchemasInIndexDefinitions => "supports_schemas_in_index_definitions",
            Self::SupportsSchemasInPrivilegeDefinitions => {
                "supports_schemas_in_privilege_definitions"
            }
            Self::SupportsCatalogsInDataManipulation => "supports_catalogs_in_data_manipulation",
            Self::SupportsCatalogsInProcedureCalls => "supports_catalogs_in_procedure_calls",
            Self::SupportsCatalogsInTableDefinitions => "supports_catalogs_in_table_definitions",
            Self::SupportsCatalogsInIndexDefinitions => "supports_catalogs_in_index_definitions",
            Self::SupportsCatalogsInPrivilegeDefinitions => {
                "supports_catalogs_in_privilege_definitions"
            }
            Self::SupportsPositionedDelete => "supports_positioned_delete",
            Self::SupportsPositionedUpdate => "supports_positioned_update",
            Self::SupportsSelectForUpdate => "supports_select_for_update",
            Self::SupportsStoredProcedures => "supports_stored_procedures",
            Self::SupportsSubqueriesInComparisons => "supports_subqueries_in_comparisons",
            Self::SupportsSubqueriesInExists => "supports_subqueries_in_exists",
            Self::SupportsSubqueriesInIns => "supports_subqueries_in_ins",
            Self::SupportsSubqueriesInQuantifieds => "supports_subqueries_in_quantifieds",
            Self::SupportsCorrelatedSubqueries => "supports_correlated_subqueries",
            Self::SupportsUnion => "supports_union",
            Self::SupportsUnionAll => "supports_union_all",
            Self::SupportsOpenCursorsAcrossCommit => "supports_open_cursors_across_commit",
            Self::SupportsOpenCursorsAcrossRollback => "supports_open_cursors_across_rollback",
            Self::SupportsOpenStatementsAcrossCommit => "supports_open_statements_across_commit",
            Self::SupportsOpenStatementsAcrossRollback => {
                "supports_open_statements_across_rollback"
            }
            Self::GetMaxBinaryLiteralLength => "get_max_binary_literal_length",
            Self::GetMaxCharLiteralLength => "get_max_char_literal_length",
            Self::GetMaxColumnNameLength => "get_max_column_name_length",
            Self::GetMaxColumnsInGroupBy => "get_max_columns_in_group_by",
            Self::GetMaxColumnsInIndex => "get_max_columns_in_index",
            Self::GetMaxColumnsInOrderBy => "get_max_columns_in_order_by",
            Self::GetMaxColumnsInSelect => "get_max_columns_in_select",
            Self::GetMaxColumnsInTable => "get_max_columns_in_table",
            Self::GetMaxConnections => "get_max_connections",
            Self::GetMaxCursorNameLength => "get_max_cursor_name_length",
            Self::GetMaxIndexLength => "get_max_index_length",
            Self::GetMaxSchemaNameLength => "get_max_schema_name_length",
            Self::GetMaxProcedureNameLength => "get_max_procedure_name_length",
            Self::GetMaxCatalogNameLength => "get_max_catalog_name_length",
            Self::GetMaxRowSize => "get_max_row_size",
            Self::DoesMaxRowSizeIncludeBlobs => "does_max_row_size_include_blobs",
            Self::GetMaxStatementLength => "get_max_statement_length",
            Self::GetMaxStatements => "get_max_statements",
            Self::GetMaxTableNameLength => "get_max_table_name_length",
            Self::GetMaxTablesInSelect => "get_max_tables_in_select",
            Self::GetMaxUserNameLength => "get_max_user_name_length",
            Self::GetDefaultTransactionIsolation => "get_default_transaction_isolation",
            Self::SupportsTransactions => "supports_transactions",
            Self::SupportsTransactionIsolationLevel => "supports_transaction_isolation_level",
            Self::SupportsDataDefinitionAndDataManipulationTransactions => {
                "supports_data_definition_and_data_manipulation_transactions"
            }
            Self::SupportsDataManipulationTransactionsOnly => {
                "supports_data_manipulation_transactions_only"
            }
            Self::DataDefinitionCausesTransactionCommit => {
                "data_definition_causes_transaction_commit"
            }
            Self::DataDefinitionIgnoredInTransactions => "data_definition_ignored_in_transactions",
            Self::GetProcedures => "get_procedures",
            Self::GetProcedureColumns => "get_procedure_columns",
            Self::GetTables => "get_tables",
            Self::GetSchemas => "get_schemas",
            Self::GetCatalogs => "get_catalogs",
            Self::GetTableTypes => "get_table_types",
            Self::GetColumns => "get_columns",
            Self::GetColumnPrivileges => "get_column_privileges",
            Self::GetTablePrivileges => "get_table_privileges",
            Self::GetBestRowIdentifier => "get_best_row_identifier",
            Self::GetVersionColumns => "get_version_columns",
            Self::GetPrimaryKeys => "get_primary_keys",
            Self::GetImportedKeys => "get_imported_keys",
            Self::GetExportedKeys => "get_exported_keys",
            Self::GetCrossReference => "get_cross_reference",
            Self::GetTypeInfo => "get_type_info",
            Self::GetIndexInfo => "get_index_info",
            Self::SupportsResultSetType => "supports_result_set_type",
            Self::SupportsResultSetConcurrency => "supports_result_set_concurrency",
            Self::OwnUpdatesAreVisible => "own_updates_are_visible",
            Self::OwnDeletesAreVisible => "own_deletes_are_visible",
            Self::OwnInsertsAreVisible => "own_inserts_are_visible",
            Self::OthersUpdatesAreVisible => "others_updates_are_visible",
            Self::OthersDeletesAreVisible => "others_deletes_are_visible",
            Self::OthersInsertsAreVisible => "others_inserts_are_visible",
            Self::UpdatesAreDetected => "updates_are_detected",
            Self::DeletesAreDetected => "deletes_are_detected",
            Self::InsertsAreDetected => "inserts_are_detected",
            Self::SupportsBatchUpdates => "supports_batch_updates",
            Self::GetUdTs => "get_ud_ts",
            Self::SupportsSavepoints => "supports_savepoints",
            Self::SupportsNamedParameters => "supports_named_parameters",
            Self::SupportsMultipleOpenResults => "supports_multiple_open_results",
            Self::SupportsGetGeneratedKeys => "supports_get_generated_keys",
            Self::GetSuperTypes => "get_super_types",
            Self::GetSuperTables => "get_super_tables",
            Self::GetAttributes => "get_attributes",
            Self::SupportsResultSetHoldability => "supports_result_set_holdability",
            Self::GetResultSetHoldability => "get_result_set_holdability",
            Self::GetDatabaseMajorVersion => "get_database_major_version",
            Self::GetDatabaseMinorVersion => "get_database_minor_version",
            Self::GetRdbcMajorVersion => "get_rdbc_major_version",
            Self::GetRdbcMinorVersion => "get_rdbc_minor_version",
            Self::GetSqlStateType => "get_sql_state_type",
            Self::LocatorsUpdateCopy => "locators_update_copy",
            Self::SupportsStatementPooling => "supports_statement_pooling",
            Self::GetRowIdLifetime => "get_row_id_lifetime",
            Self::GetSchemasWithPattern => "get_schemas_with_pattern",
            Self::SupportsStoredFunctionsUsingCallSyntax => {
                "supports_stored_functions_using_call_syntax"
            }
            Self::AutoCommitFailureClosesAllResultSets => {
                "auto_commit_failure_closes_all_result_sets"
            }
            Self::GetClientInfoProperties => "get_client_info_properties",
            Self::GetFunctions => "get_functions",
            Self::GetFunctionColumns => "get_function_columns",
            Self::GetPseudoColumns => "get_pseudo_columns",
            Self::GeneratedKeyAlwaysReturned => "generated_key_always_returned",
        }
    }
}

/// Metadata 方法参数；Option 精确保留 null 与 empty 的差别。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DatabaseMetaDataArgument {
    /// 可空字符串或 pattern。
    OptionalString(Option<String>),
    /// JDBC integer 常量。
    Integer(i32),
    /// Boolean 参数。
    Boolean(bool),
    /// 可空字符串 slice snapshot。
    Strings(Option<Vec<String>>),
    /// 可空整数 slice snapshot。
    Integers(Option<Vec<i32>>),
}

/// 一个类型化的 DatabaseMetaData 调用请求。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatabaseMetaDataRequest {
    operation: DatabaseMetaDataOperation,
    arguments: Vec<DatabaseMetaDataArgument>,
}

impl DatabaseMetaDataRequest {
    /// 创建调用请求；arguments 保持 BASE signature 顺序。
    pub fn new(
        operation: DatabaseMetaDataOperation,
        arguments: Vec<DatabaseMetaDataArgument>,
    ) -> Self {
        Self {
            operation,
            arguments,
        }
    }
    /// 返回方法标识。
    pub const fn operation(&self) -> DatabaseMetaDataOperation {
        self.operation
    }
    /// 按 BASE signature 顺序返回 arguments。
    pub fn arguments(&self) -> &[DatabaseMetaDataArgument] {
        &self.arguments
    }
}

impl fmt::Display for DatabaseMetaDataOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
