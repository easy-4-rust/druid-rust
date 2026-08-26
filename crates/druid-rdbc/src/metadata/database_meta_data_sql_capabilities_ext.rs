use crate::error::RdbcError;
use crate::metadata::{DatabaseMetaData, DatabaseMetaDataOperation, DatabaseMetaDataRequest};

/// DatabaseMetaData 方法族 `DatabaseMetaDataSqlCapabilitiesExt`；所有方法委托唯一 typed core。
#[async_trait::async_trait]
pub trait DatabaseMetaDataSqlCapabilitiesExt: DatabaseMetaData {
    /// 执行 JDBC-like `supports_group_by_beyond_select` 语义。
    async fn supports_group_by_beyond_select(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsGroupByBeyondSelect,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_like_escape_clause` 语义。
    async fn supports_like_escape_clause(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsLikeEscapeClause,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_multiple_result_sets` 语义。
    async fn supports_multiple_result_sets(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsMultipleResultSets,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_multiple_transactions` 语义。
    async fn supports_multiple_transactions(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsMultipleTransactions,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_non_nullable_columns` 语义。
    async fn supports_non_nullable_columns(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsNonNullableColumns,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_minimum_sql_grammar` 语义。
    async fn supports_minimum_sql_grammar(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsMinimumSqlGrammar,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_core_sql_grammar` 语义。
    async fn supports_core_sql_grammar(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsCoreSqlGrammar,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_extended_sql_grammar` 语义。
    async fn supports_extended_sql_grammar(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsExtendedSqlGrammar,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_ansi92_entry_level_sql` 语义。
    async fn supports_ansi92_entry_level_sql(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsAnsi92EntryLevelSql,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_ansi92_intermediate_sql` 语义。
    async fn supports_ansi92_intermediate_sql(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsAnsi92IntermediateSql,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_ansi92_full_sql` 语义。
    async fn supports_ansi92_full_sql(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsAnsi92FullSql,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_integrity_enhancement_facility` 语义。
    async fn supports_integrity_enhancement_facility(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsIntegrityEnhancementFacility,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_outer_joins` 语义。
    async fn supports_outer_joins(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsOuterJoins,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_full_outer_joins` 语义。
    async fn supports_full_outer_joins(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsFullOuterJoins,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_limited_outer_joins` 语义。
    async fn supports_limited_outer_joins(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsLimitedOuterJoins,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_schema_term` 语义。
    async fn get_schema_term(&mut self) -> Result<Option<String>, RdbcError> {
        self.metadata_string(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetSchemaTerm,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_procedure_term` 语义。
    async fn get_procedure_term(&mut self) -> Result<Option<String>, RdbcError> {
        self.metadata_string(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetProcedureTerm,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_catalog_term` 语义。
    async fn get_catalog_term(&mut self) -> Result<Option<String>, RdbcError> {
        self.metadata_string(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetCatalogTerm,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `is_catalog_at_start` 语义。
    async fn is_catalog_at_start(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::IsCatalogAtStart,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_catalog_separator` 语义。
    async fn get_catalog_separator(&mut self) -> Result<Option<String>, RdbcError> {
        self.metadata_string(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetCatalogSeparator,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_schemas_in_data_manipulation` 语义。
    async fn supports_schemas_in_data_manipulation(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsSchemasInDataManipulation,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_schemas_in_procedure_calls` 语义。
    async fn supports_schemas_in_procedure_calls(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsSchemasInProcedureCalls,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_schemas_in_table_definitions` 语义。
    async fn supports_schemas_in_table_definitions(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsSchemasInTableDefinitions,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_schemas_in_index_definitions` 语义。
    async fn supports_schemas_in_index_definitions(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsSchemasInIndexDefinitions,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_schemas_in_privilege_definitions` 语义。
    async fn supports_schemas_in_privilege_definitions(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsSchemasInPrivilegeDefinitions,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_catalogs_in_data_manipulation` 语义。
    async fn supports_catalogs_in_data_manipulation(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsCatalogsInDataManipulation,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_catalogs_in_procedure_calls` 语义。
    async fn supports_catalogs_in_procedure_calls(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsCatalogsInProcedureCalls,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_catalogs_in_table_definitions` 语义。
    async fn supports_catalogs_in_table_definitions(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsCatalogsInTableDefinitions,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_catalogs_in_index_definitions` 语义。
    async fn supports_catalogs_in_index_definitions(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsCatalogsInIndexDefinitions,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_catalogs_in_privilege_definitions` 语义。
    async fn supports_catalogs_in_privilege_definitions(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsCatalogsInPrivilegeDefinitions,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_positioned_delete` 语义。
    async fn supports_positioned_delete(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsPositionedDelete,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_positioned_update` 语义。
    async fn supports_positioned_update(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsPositionedUpdate,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_select_for_update` 语义。
    async fn supports_select_for_update(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsSelectForUpdate,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_stored_procedures` 语义。
    async fn supports_stored_procedures(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsStoredProcedures,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_subqueries_in_comparisons` 语义。
    async fn supports_subqueries_in_comparisons(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsSubqueriesInComparisons,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_subqueries_in_exists` 语义。
    async fn supports_subqueries_in_exists(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsSubqueriesInExists,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_subqueries_in_ins` 语义。
    async fn supports_subqueries_in_ins(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsSubqueriesInIns,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_subqueries_in_quantifieds` 语义。
    async fn supports_subqueries_in_quantifieds(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsSubqueriesInQuantifieds,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_correlated_subqueries` 语义。
    async fn supports_correlated_subqueries(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsCorrelatedSubqueries,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_union` 语义。
    async fn supports_union(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsUnion,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_union_all` 语义。
    async fn supports_union_all(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsUnionAll,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_open_cursors_across_commit` 语义。
    async fn supports_open_cursors_across_commit(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsOpenCursorsAcrossCommit,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_open_cursors_across_rollback` 语义。
    async fn supports_open_cursors_across_rollback(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsOpenCursorsAcrossRollback,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_open_statements_across_commit` 语义。
    async fn supports_open_statements_across_commit(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsOpenStatementsAcrossCommit,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_open_statements_across_rollback` 语义。
    async fn supports_open_statements_across_rollback(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsOpenStatementsAcrossRollback,
            Vec::new(),
        ))
        .await
    }
}

#[async_trait::async_trait]
impl<T: DatabaseMetaData + ?Sized> DatabaseMetaDataSqlCapabilitiesExt for T {}
