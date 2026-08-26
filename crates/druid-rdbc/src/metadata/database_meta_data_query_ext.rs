use crate::error::RdbcError;
use crate::metadata::{
    DatabaseMetaData, DatabaseMetaDataArgument, DatabaseMetaDataOperation, DatabaseMetaDataRequest,
    RowIdLifetime,
};
use crate::result_set::ResultSet;

/// DatabaseMetaData 方法族 `DatabaseMetaDataQueryExt`；所有方法委托唯一 typed core。
#[async_trait::async_trait]
pub trait DatabaseMetaDataQueryExt: DatabaseMetaData {
    /// 执行 JDBC-like `get_column_privileges` 语义。
    async fn get_column_privileges(
        &mut self,
        catalog: Option<&str>,
        schema: Option<&str>,
        table: Option<&str>,
        column_name_pattern: Option<&str>,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.metadata_result_set(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetColumnPrivileges,
            std::vec![
                DatabaseMetaDataArgument::OptionalString(catalog.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(schema.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(table.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(column_name_pattern.map(str::to_owned)),
            ],
        ))
        .await
    }

    /// 执行 JDBC-like `get_table_privileges` 语义。
    async fn get_table_privileges(
        &mut self,
        catalog: Option<&str>,
        schema_pattern: Option<&str>,
        table_name_pattern: Option<&str>,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.metadata_result_set(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetTablePrivileges,
            std::vec![
                DatabaseMetaDataArgument::OptionalString(catalog.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(schema_pattern.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(table_name_pattern.map(str::to_owned)),
            ],
        ))
        .await
    }

    /// 执行 JDBC-like `get_best_row_identifier` 语义。
    async fn get_best_row_identifier(
        &mut self,
        catalog: Option<&str>,
        schema: Option<&str>,
        table: Option<&str>,
        scope: i32,
        nullable: bool,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.metadata_result_set(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetBestRowIdentifier,
            std::vec![
                DatabaseMetaDataArgument::OptionalString(catalog.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(schema.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(table.map(str::to_owned)),
                DatabaseMetaDataArgument::Integer(scope),
                DatabaseMetaDataArgument::Boolean(nullable),
            ],
        ))
        .await
    }

    /// 执行 JDBC-like `get_version_columns` 语义。
    async fn get_version_columns(
        &mut self,
        catalog: Option<&str>,
        schema: Option<&str>,
        table: Option<&str>,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.metadata_result_set(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetVersionColumns,
            std::vec![
                DatabaseMetaDataArgument::OptionalString(catalog.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(schema.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(table.map(str::to_owned)),
            ],
        ))
        .await
    }

    /// 执行 JDBC-like `get_primary_keys` 语义。
    async fn get_primary_keys(
        &mut self,
        catalog: Option<&str>,
        schema: Option<&str>,
        table: Option<&str>,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.metadata_result_set(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetPrimaryKeys,
            std::vec![
                DatabaseMetaDataArgument::OptionalString(catalog.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(schema.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(table.map(str::to_owned)),
            ],
        ))
        .await
    }

    /// 执行 JDBC-like `get_imported_keys` 语义。
    async fn get_imported_keys(
        &mut self,
        catalog: Option<&str>,
        schema: Option<&str>,
        table: Option<&str>,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.metadata_result_set(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetImportedKeys,
            std::vec![
                DatabaseMetaDataArgument::OptionalString(catalog.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(schema.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(table.map(str::to_owned)),
            ],
        ))
        .await
    }

    /// 执行 JDBC-like `get_exported_keys` 语义。
    async fn get_exported_keys(
        &mut self,
        catalog: Option<&str>,
        schema: Option<&str>,
        table: Option<&str>,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.metadata_result_set(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetExportedKeys,
            std::vec![
                DatabaseMetaDataArgument::OptionalString(catalog.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(schema.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(table.map(str::to_owned)),
            ],
        ))
        .await
    }

    /// 执行 JDBC-like `get_cross_reference` 语义。
    async fn get_cross_reference(
        &mut self,
        parent_catalog: Option<&str>,
        parent_schema: Option<&str>,
        parent_table: Option<&str>,
        foreign_catalog: Option<&str>,
        foreign_schema: Option<&str>,
        foreign_table: Option<&str>,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.metadata_result_set(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetCrossReference,
            std::vec![
                DatabaseMetaDataArgument::OptionalString(parent_catalog.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(parent_schema.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(parent_table.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(foreign_catalog.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(foreign_schema.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(foreign_table.map(str::to_owned)),
            ],
        ))
        .await
    }

    /// 执行 JDBC-like `get_type_info` 语义。
    async fn get_type_info(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.metadata_result_set(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetTypeInfo,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_index_info` 语义。
    async fn get_index_info(
        &mut self,
        catalog: Option<&str>,
        schema: Option<&str>,
        table: Option<&str>,
        unique: bool,
        approximate: bool,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.metadata_result_set(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetIndexInfo,
            std::vec![
                DatabaseMetaDataArgument::OptionalString(catalog.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(schema.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(table.map(str::to_owned)),
                DatabaseMetaDataArgument::Boolean(unique),
                DatabaseMetaDataArgument::Boolean(approximate),
            ],
        ))
        .await
    }

    /// 执行 JDBC-like `supports_result_set_type` 语义。
    async fn supports_result_set_type(&mut self, result_set_type: i32) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsResultSetType,
            std::vec![DatabaseMetaDataArgument::Integer(result_set_type),],
        ))
        .await
    }

    /// 执行 JDBC-like `supports_result_set_concurrency` 语义。
    async fn supports_result_set_concurrency(
        &mut self,
        result_set_type: i32,
        concurrency: i32,
    ) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsResultSetConcurrency,
            std::vec![
                DatabaseMetaDataArgument::Integer(result_set_type),
                DatabaseMetaDataArgument::Integer(concurrency),
            ],
        ))
        .await
    }

    /// 执行 JDBC-like `own_updates_are_visible` 语义。
    async fn own_updates_are_visible(&mut self, result_set_type: i32) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::OwnUpdatesAreVisible,
            std::vec![DatabaseMetaDataArgument::Integer(result_set_type),],
        ))
        .await
    }

    /// 执行 JDBC-like `own_deletes_are_visible` 语义。
    async fn own_deletes_are_visible(&mut self, result_set_type: i32) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::OwnDeletesAreVisible,
            std::vec![DatabaseMetaDataArgument::Integer(result_set_type),],
        ))
        .await
    }

    /// 执行 JDBC-like `own_inserts_are_visible` 语义。
    async fn own_inserts_are_visible(&mut self, result_set_type: i32) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::OwnInsertsAreVisible,
            std::vec![DatabaseMetaDataArgument::Integer(result_set_type),],
        ))
        .await
    }

    /// 执行 JDBC-like `others_updates_are_visible` 语义。
    async fn others_updates_are_visible(
        &mut self,
        result_set_type: i32,
    ) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::OthersUpdatesAreVisible,
            std::vec![DatabaseMetaDataArgument::Integer(result_set_type),],
        ))
        .await
    }

    /// 执行 JDBC-like `others_deletes_are_visible` 语义。
    async fn others_deletes_are_visible(
        &mut self,
        result_set_type: i32,
    ) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::OthersDeletesAreVisible,
            std::vec![DatabaseMetaDataArgument::Integer(result_set_type),],
        ))
        .await
    }

    /// 执行 JDBC-like `others_inserts_are_visible` 语义。
    async fn others_inserts_are_visible(
        &mut self,
        result_set_type: i32,
    ) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::OthersInsertsAreVisible,
            std::vec![DatabaseMetaDataArgument::Integer(result_set_type),],
        ))
        .await
    }

    /// 执行 JDBC-like `updates_are_detected` 语义。
    async fn updates_are_detected(&mut self, result_set_type: i32) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::UpdatesAreDetected,
            std::vec![DatabaseMetaDataArgument::Integer(result_set_type),],
        ))
        .await
    }

    /// 执行 JDBC-like `deletes_are_detected` 语义。
    async fn deletes_are_detected(&mut self, result_set_type: i32) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::DeletesAreDetected,
            std::vec![DatabaseMetaDataArgument::Integer(result_set_type),],
        ))
        .await
    }

    /// 执行 JDBC-like `inserts_are_detected` 语义。
    async fn inserts_are_detected(&mut self, result_set_type: i32) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::InsertsAreDetected,
            std::vec![DatabaseMetaDataArgument::Integer(result_set_type),],
        ))
        .await
    }

    /// 执行 JDBC-like `supports_batch_updates` 语义。
    async fn supports_batch_updates(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsBatchUpdates,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_ud_ts` 语义。
    async fn get_ud_ts(
        &mut self,
        catalog: Option<&str>,
        schema_pattern: Option<&str>,
        type_name_pattern: Option<&str>,
        types: Option<&[i32]>,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.metadata_result_set(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetUdTs,
            std::vec![
                DatabaseMetaDataArgument::OptionalString(catalog.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(schema_pattern.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(type_name_pattern.map(str::to_owned)),
                DatabaseMetaDataArgument::Integers(types.map(<[i32]>::to_vec)),
            ],
        ))
        .await
    }

    /// 执行 JDBC-like `supports_savepoints` 语义。
    async fn supports_savepoints(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsSavepoints,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_named_parameters` 语义。
    async fn supports_named_parameters(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsNamedParameters,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_multiple_open_results` 语义。
    async fn supports_multiple_open_results(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsMultipleOpenResults,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_get_generated_keys` 语义。
    async fn supports_get_generated_keys(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsGetGeneratedKeys,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_super_types` 语义。
    async fn get_super_types(
        &mut self,
        catalog: Option<&str>,
        schema_pattern: Option<&str>,
        type_name_pattern: Option<&str>,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.metadata_result_set(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetSuperTypes,
            std::vec![
                DatabaseMetaDataArgument::OptionalString(catalog.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(schema_pattern.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(type_name_pattern.map(str::to_owned)),
            ],
        ))
        .await
    }

    /// 执行 JDBC-like `get_super_tables` 语义。
    async fn get_super_tables(
        &mut self,
        catalog: Option<&str>,
        schema_pattern: Option<&str>,
        table_name_pattern: Option<&str>,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.metadata_result_set(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetSuperTables,
            std::vec![
                DatabaseMetaDataArgument::OptionalString(catalog.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(schema_pattern.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(table_name_pattern.map(str::to_owned)),
            ],
        ))
        .await
    }

    /// 执行 JDBC-like `get_attributes` 语义。
    async fn get_attributes(
        &mut self,
        catalog: Option<&str>,
        schema_pattern: Option<&str>,
        type_name_pattern: Option<&str>,
        attribute_name_pattern: Option<&str>,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.metadata_result_set(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetAttributes,
            std::vec![
                DatabaseMetaDataArgument::OptionalString(catalog.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(schema_pattern.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(type_name_pattern.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(attribute_name_pattern.map(str::to_owned)),
            ],
        ))
        .await
    }

    /// 执行 JDBC-like `supports_result_set_holdability` 语义。
    async fn supports_result_set_holdability(
        &mut self,
        holdability: i32,
    ) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsResultSetHoldability,
            std::vec![DatabaseMetaDataArgument::Integer(holdability),],
        ))
        .await
    }

    /// 执行 JDBC-like `get_result_set_holdability` 语义。
    async fn get_result_set_holdability(&mut self) -> Result<i32, RdbcError> {
        self.metadata_integer(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetResultSetHoldability,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_database_major_version` 语义。
    async fn get_database_major_version(&mut self) -> Result<i32, RdbcError> {
        self.metadata_integer(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetDatabaseMajorVersion,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_database_minor_version` 语义。
    async fn get_database_minor_version(&mut self) -> Result<i32, RdbcError> {
        self.metadata_integer(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetDatabaseMinorVersion,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_rdbc_major_version` 语义。
    async fn get_rdbc_major_version(&mut self) -> Result<i32, RdbcError> {
        self.metadata_integer(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetRdbcMajorVersion,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_rdbc_minor_version` 语义。
    async fn get_rdbc_minor_version(&mut self) -> Result<i32, RdbcError> {
        self.metadata_integer(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetRdbcMinorVersion,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_sql_state_type` 语义。
    async fn get_sql_state_type(&mut self) -> Result<i32, RdbcError> {
        self.metadata_integer(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetSqlStateType,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `locators_update_copy` 语义。
    async fn locators_update_copy(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::LocatorsUpdateCopy,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_statement_pooling` 语义。
    async fn supports_statement_pooling(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsStatementPooling,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_row_id_lifetime` 语义。
    async fn get_row_id_lifetime(&mut self) -> Result<RowIdLifetime, RdbcError> {
        self.metadata_row_id_lifetime(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetRowIdLifetime,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_schemas_with_pattern` 语义。
    async fn get_schemas_with_pattern(
        &mut self,
        catalog: Option<&str>,
        schema_pattern: Option<&str>,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.metadata_result_set(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetSchemasWithPattern,
            std::vec![
                DatabaseMetaDataArgument::OptionalString(catalog.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(schema_pattern.map(str::to_owned)),
            ],
        ))
        .await
    }

    /// 执行 JDBC-like `supports_stored_functions_using_call_syntax` 语义。
    async fn supports_stored_functions_using_call_syntax(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsStoredFunctionsUsingCallSyntax,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `auto_commit_failure_closes_all_result_sets` 语义。
    async fn auto_commit_failure_closes_all_result_sets(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::AutoCommitFailureClosesAllResultSets,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_client_info_properties` 语义。
    async fn get_client_info_properties(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.metadata_result_set(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetClientInfoProperties,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_functions` 语义。
    async fn get_functions(
        &mut self,
        catalog: Option<&str>,
        schema_pattern: Option<&str>,
        function_name_pattern: Option<&str>,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.metadata_result_set(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetFunctions,
            std::vec![
                DatabaseMetaDataArgument::OptionalString(catalog.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(schema_pattern.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(function_name_pattern.map(str::to_owned)),
            ],
        ))
        .await
    }

    /// 执行 JDBC-like `get_function_columns` 语义。
    async fn get_function_columns(
        &mut self,
        catalog: Option<&str>,
        schema_pattern: Option<&str>,
        function_name_pattern: Option<&str>,
        column_name_pattern: Option<&str>,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.metadata_result_set(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetFunctionColumns,
            std::vec![
                DatabaseMetaDataArgument::OptionalString(catalog.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(schema_pattern.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(function_name_pattern.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(column_name_pattern.map(str::to_owned)),
            ],
        ))
        .await
    }

    /// 执行 JDBC-like `get_pseudo_columns` 语义。
    async fn get_pseudo_columns(
        &mut self,
        catalog: Option<&str>,
        schema_pattern: Option<&str>,
        table_name_pattern: Option<&str>,
        column_name_pattern: Option<&str>,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.metadata_result_set(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetPseudoColumns,
            std::vec![
                DatabaseMetaDataArgument::OptionalString(catalog.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(schema_pattern.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(table_name_pattern.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(column_name_pattern.map(str::to_owned)),
            ],
        ))
        .await
    }

    /// 执行 JDBC-like `generated_key_always_returned` 语义。
    async fn generated_key_always_returned(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GeneratedKeyAlwaysReturned,
            Vec::new(),
        ))
        .await
    }
}

#[async_trait::async_trait]
impl<T: DatabaseMetaData + ?Sized> DatabaseMetaDataQueryExt for T {}
