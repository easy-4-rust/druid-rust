use crate::error::RdbcError;
use crate::metadata::{
    DatabaseMetaData, DatabaseMetaDataArgument, DatabaseMetaDataOperation, DatabaseMetaDataRequest,
};
use crate::result_set::ResultSet;

/// DatabaseMetaData 方法族 `DatabaseMetaDataLimitsExt`；所有方法委托唯一 typed core。
#[async_trait::async_trait]
pub trait DatabaseMetaDataLimitsExt: DatabaseMetaData {
    /// 执行 JDBC-like `get_max_binary_literal_length` 语义。
    async fn get_max_binary_literal_length(&mut self) -> Result<i32, RdbcError> {
        self.metadata_integer(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetMaxBinaryLiteralLength,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_max_char_literal_length` 语义。
    async fn get_max_char_literal_length(&mut self) -> Result<i32, RdbcError> {
        self.metadata_integer(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetMaxCharLiteralLength,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_max_column_name_length` 语义。
    async fn get_max_column_name_length(&mut self) -> Result<i32, RdbcError> {
        self.metadata_integer(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetMaxColumnNameLength,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_max_columns_in_group_by` 语义。
    async fn get_max_columns_in_group_by(&mut self) -> Result<i32, RdbcError> {
        self.metadata_integer(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetMaxColumnsInGroupBy,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_max_columns_in_index` 语义。
    async fn get_max_columns_in_index(&mut self) -> Result<i32, RdbcError> {
        self.metadata_integer(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetMaxColumnsInIndex,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_max_columns_in_order_by` 语义。
    async fn get_max_columns_in_order_by(&mut self) -> Result<i32, RdbcError> {
        self.metadata_integer(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetMaxColumnsInOrderBy,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_max_columns_in_select` 语义。
    async fn get_max_columns_in_select(&mut self) -> Result<i32, RdbcError> {
        self.metadata_integer(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetMaxColumnsInSelect,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_max_columns_in_table` 语义。
    async fn get_max_columns_in_table(&mut self) -> Result<i32, RdbcError> {
        self.metadata_integer(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetMaxColumnsInTable,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_max_connections` 语义。
    async fn get_max_connections(&mut self) -> Result<i32, RdbcError> {
        self.metadata_integer(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetMaxConnections,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_max_cursor_name_length` 语义。
    async fn get_max_cursor_name_length(&mut self) -> Result<i32, RdbcError> {
        self.metadata_integer(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetMaxCursorNameLength,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_max_index_length` 语义。
    async fn get_max_index_length(&mut self) -> Result<i32, RdbcError> {
        self.metadata_integer(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetMaxIndexLength,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_max_schema_name_length` 语义。
    async fn get_max_schema_name_length(&mut self) -> Result<i32, RdbcError> {
        self.metadata_integer(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetMaxSchemaNameLength,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_max_procedure_name_length` 语义。
    async fn get_max_procedure_name_length(&mut self) -> Result<i32, RdbcError> {
        self.metadata_integer(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetMaxProcedureNameLength,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_max_catalog_name_length` 语义。
    async fn get_max_catalog_name_length(&mut self) -> Result<i32, RdbcError> {
        self.metadata_integer(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetMaxCatalogNameLength,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_max_row_size` 语义。
    async fn get_max_row_size(&mut self) -> Result<i32, RdbcError> {
        self.metadata_integer(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetMaxRowSize,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `does_max_row_size_include_blobs` 语义。
    async fn does_max_row_size_include_blobs(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::DoesMaxRowSizeIncludeBlobs,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_max_statement_length` 语义。
    async fn get_max_statement_length(&mut self) -> Result<i32, RdbcError> {
        self.metadata_integer(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetMaxStatementLength,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_max_statements` 语义。
    async fn get_max_statements(&mut self) -> Result<i32, RdbcError> {
        self.metadata_integer(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetMaxStatements,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_max_table_name_length` 语义。
    async fn get_max_table_name_length(&mut self) -> Result<i32, RdbcError> {
        self.metadata_integer(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetMaxTableNameLength,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_max_tables_in_select` 语义。
    async fn get_max_tables_in_select(&mut self) -> Result<i32, RdbcError> {
        self.metadata_integer(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetMaxTablesInSelect,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_max_user_name_length` 语义。
    async fn get_max_user_name_length(&mut self) -> Result<i32, RdbcError> {
        self.metadata_integer(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetMaxUserNameLength,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_default_transaction_isolation` 语义。
    async fn get_default_transaction_isolation(&mut self) -> Result<i32, RdbcError> {
        self.metadata_integer(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetDefaultTransactionIsolation,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_transactions` 语义。
    async fn supports_transactions(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsTransactions,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_transaction_isolation_level` 语义。
    async fn supports_transaction_isolation_level(
        &mut self,
        level: i32,
    ) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsTransactionIsolationLevel,
            std::vec![DatabaseMetaDataArgument::Integer(level),],
        ))
        .await
    }

    /// 执行 JDBC-like `supports_data_definition_and_data_manipulation_transactions` 语义。
    async fn supports_data_definition_and_data_manipulation_transactions(
        &mut self,
    ) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsDataDefinitionAndDataManipulationTransactions,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `supports_data_manipulation_transactions_only` 语义。
    async fn supports_data_manipulation_transactions_only(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::SupportsDataManipulationTransactionsOnly,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `data_definition_causes_transaction_commit` 语义。
    async fn data_definition_causes_transaction_commit(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::DataDefinitionCausesTransactionCommit,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `data_definition_ignored_in_transactions` 语义。
    async fn data_definition_ignored_in_transactions(&mut self) -> Result<bool, RdbcError> {
        self.metadata_boolean(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::DataDefinitionIgnoredInTransactions,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_procedures` 语义。
    async fn get_procedures(
        &mut self,
        catalog: Option<&str>,
        schema_pattern: Option<&str>,
        procedure_name_pattern: Option<&str>,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.metadata_result_set(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetProcedures,
            std::vec![
                DatabaseMetaDataArgument::OptionalString(catalog.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(schema_pattern.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(procedure_name_pattern.map(str::to_owned)),
            ],
        ))
        .await
    }

    /// 执行 JDBC-like `get_procedure_columns` 语义。
    async fn get_procedure_columns(
        &mut self,
        catalog: Option<&str>,
        schema_pattern: Option<&str>,
        procedure_name_pattern: Option<&str>,
        column_name_pattern: Option<&str>,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.metadata_result_set(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetProcedureColumns,
            std::vec![
                DatabaseMetaDataArgument::OptionalString(catalog.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(schema_pattern.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(procedure_name_pattern.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(column_name_pattern.map(str::to_owned)),
            ],
        ))
        .await
    }

    /// 执行 JDBC-like `get_tables` 语义。
    async fn get_tables(
        &mut self,
        catalog: Option<&str>,
        schema_pattern: Option<&str>,
        table_name_pattern: Option<&str>,
        types: Option<&[String]>,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.metadata_result_set(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetTables,
            std::vec![
                DatabaseMetaDataArgument::OptionalString(catalog.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(schema_pattern.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(table_name_pattern.map(str::to_owned)),
                DatabaseMetaDataArgument::Strings(types.map(<[String]>::to_vec)),
            ],
        ))
        .await
    }

    /// 执行 JDBC-like `get_schemas` 语义。
    async fn get_schemas(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.metadata_result_set(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetSchemas,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_catalogs` 语义。
    async fn get_catalogs(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.metadata_result_set(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetCatalogs,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_table_types` 语义。
    async fn get_table_types(&mut self) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.metadata_result_set(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetTableTypes,
            Vec::new(),
        ))
        .await
    }

    /// 执行 JDBC-like `get_columns` 语义。
    async fn get_columns(
        &mut self,
        catalog: Option<&str>,
        schema_pattern: Option<&str>,
        table_name_pattern: Option<&str>,
        column_name_pattern: Option<&str>,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.metadata_result_set(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetColumns,
            std::vec![
                DatabaseMetaDataArgument::OptionalString(catalog.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(schema_pattern.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(table_name_pattern.map(str::to_owned)),
                DatabaseMetaDataArgument::OptionalString(column_name_pattern.map(str::to_owned)),
            ],
        ))
        .await
    }
}

#[async_trait::async_trait]
impl<T: DatabaseMetaData + ?Sized> DatabaseMetaDataLimitsExt for T {}
