use crate::error::RdbcError;
use crate::metadata::{
    DatabaseMetaData, DatabaseMetaDataArgument, DatabaseMetaDataOperation, DatabaseMetaDataRequest,
};

/// DatabaseMetaData 方法族 `DatabaseMetaDataInfoExt`；所有方法委托唯一 typed core。
#[async_trait::async_trait]
pub trait DatabaseMetaDataInfoExt: DatabaseMetaData {
    /// 执行 JDBC-like `all_procedures_are_callable` 语义。
    async fn all_procedures_are_callable(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::AllProceduresAreCallable,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `all_tables_are_selectable` 语义。
    async fn all_tables_are_selectable(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::AllTablesAreSelectable,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_url` 语义。
    async fn get_url(&mut self) -> Result<Option<String>, RdbcError> {
        self.metadata_string(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetUrl,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_user_name` 语义。
    async fn get_user_name(&mut self) -> Result<Option<String>, RdbcError> {
        self.metadata_string(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetUserName,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `is_read_only` 语义。
    async fn is_read_only(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::IsReadOnly,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `nulls_are_sorted_high` 语义。
    async fn nulls_are_sorted_high(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::NullsAreSortedHigh,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `nulls_are_sorted_low` 语义。
    async fn nulls_are_sorted_low(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::NullsAreSortedLow,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `nulls_are_sorted_at_start` 语义。
    async fn nulls_are_sorted_at_start(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::NullsAreSortedAtStart,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `nulls_are_sorted_at_end` 语义。
    async fn nulls_are_sorted_at_end(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::NullsAreSortedAtEnd,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_database_product_name` 语义。
    async fn get_database_product_name(&mut self) -> Result<Option<String>, RdbcError> {
        self.metadata_string(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetDatabaseProductName,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_database_product_version` 语义。
    async fn get_database_product_version(&mut self) -> Result<Option<String>, RdbcError> {
        self.metadata_string(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetDatabaseProductVersion,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_driver_name` 语义。
    async fn get_driver_name(&mut self) -> Result<Option<String>, RdbcError> {
        self.metadata_string(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetDriverName,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_driver_version` 语义。
    async fn get_driver_version(&mut self) -> Result<Option<String>, RdbcError> {
        self.metadata_string(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetDriverVersion,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_driver_major_version` 语义。
    async fn get_driver_major_version(&mut self) -> Result<i32, RdbcError> {
        self.metadata_integer(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetDriverMajorVersion,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_driver_minor_version` 语义。
    async fn get_driver_minor_version(&mut self) -> Result<i32, RdbcError> {
        self.metadata_integer(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetDriverMinorVersion,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `uses_local_files` 语义。
    async fn uses_local_files(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::UsesLocalFiles,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `uses_local_file_per_table` 语义。
    async fn uses_local_file_per_table(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::UsesLocalFilePerTable,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_mixed_case_identifiers` 语义。
    async fn supports_mixed_case_identifiers(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsMixedCaseIdentifiers,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `stores_upper_case_identifiers` 语义。
    async fn stores_upper_case_identifiers(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::StoresUpperCaseIdentifiers,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `stores_lower_case_identifiers` 语义。
    async fn stores_lower_case_identifiers(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::StoresLowerCaseIdentifiers,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `stores_mixed_case_identifiers` 语义。
    async fn stores_mixed_case_identifiers(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::StoresMixedCaseIdentifiers,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_mixed_case_quoted_identifiers` 语义。
    async fn supports_mixed_case_quoted_identifiers(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsMixedCaseQuotedIdentifiers,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `stores_upper_case_quoted_identifiers` 语义。
    async fn stores_upper_case_quoted_identifiers(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::StoresUpperCaseQuotedIdentifiers,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `stores_lower_case_quoted_identifiers` 语义。
    async fn stores_lower_case_quoted_identifiers(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::StoresLowerCaseQuotedIdentifiers,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `stores_mixed_case_quoted_identifiers` 语义。
    async fn stores_mixed_case_quoted_identifiers(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::StoresMixedCaseQuotedIdentifiers,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_identifier_quote_string` 语义。
    async fn get_identifier_quote_string(&mut self) -> Result<Option<String>, RdbcError> {
        self.metadata_string(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetIdentifierQuoteString,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_sql_keywords` 语义。
    async fn get_sql_keywords(&mut self) -> Result<Option<String>, RdbcError> {
        self.metadata_string(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetSqlKeywords,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_numeric_functions` 语义。
    async fn get_numeric_functions(&mut self) -> Result<Option<String>, RdbcError> {
        self.metadata_string(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetNumericFunctions,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_string_functions` 语义。
    async fn get_string_functions(&mut self) -> Result<Option<String>, RdbcError> {
        self.metadata_string(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetStringFunctions,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_system_functions` 语义。
    async fn get_system_functions(&mut self) -> Result<Option<String>, RdbcError> {
        self.metadata_string(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetSystemFunctions,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_time_date_functions` 语义。
    async fn get_time_date_functions(&mut self) -> Result<Option<String>, RdbcError> {
        self.metadata_string(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetTimeDateFunctions,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_search_string_escape` 语义。
    async fn get_search_string_escape(&mut self) -> Result<Option<String>, RdbcError> {
        self.metadata_string(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetSearchStringEscape,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_extra_name_characters` 语义。
    async fn get_extra_name_characters(&mut self) -> Result<Option<String>, RdbcError> {
        self.metadata_string(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetExtraNameCharacters,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_alter_table_with_add_column` 语义。
    async fn supports_alter_table_with_add_column(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsAlterTableWithAddColumn,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_alter_table_with_drop_column` 语义。
    async fn supports_alter_table_with_drop_column(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsAlterTableWithDropColumn,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_column_aliasing` 语义。
    async fn supports_column_aliasing(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsColumnAliasing,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `null_plus_non_null_is_null` 语义。
    async fn null_plus_non_null_is_null(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::NullPlusNonNullIsNull,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_convert` 语义。
    async fn supports_convert(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsConvert,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_convert_between` 语义。
    async fn supports_convert_between(
        &mut self,
        from_type: i32,
        to_type: i32,
    ) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsConvertBetween,
            std::vec![
                DatabaseMetaDataArgument::Integer(from_type),
                DatabaseMetaDataArgument::Integer(to_type),
            ],
        ))
        .await
    }

    /// 执行 JDBC-like `supports_table_correlation_names` 语义。
    async fn supports_table_correlation_names(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsTableCorrelationNames,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_different_table_correlation_names` 语义。
    async fn supports_different_table_correlation_names(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsDifferentTableCorrelationNames,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_expressions_in_order_by` 语义。
    async fn supports_expressions_in_order_by(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsExpressionsInOrderBy,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_order_by_unrelated` 语义。
    async fn supports_order_by_unrelated(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsOrderByUnrelated,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_group_by` 语义。
    async fn supports_group_by(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsGroupBy,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_group_by_unrelated` 语义。
    async fn supports_group_by_unrelated(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsGroupByUnrelated,
            Vec::new(),
        ))
        .await
    }
}

#[async_trait::async_trait]
impl<T: DatabaseMetaData + ?Sized> DatabaseMetaDataInfoExt for T {}
