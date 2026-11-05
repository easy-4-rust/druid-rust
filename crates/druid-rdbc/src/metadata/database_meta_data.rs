use crate::connection::Connection;
use crate::error::RdbcError;
use crate::metadata::{
    DatabaseMetaDataArgument, DatabaseMetaDataOperation, DatabaseMetaDataRequest, RowIdLifetime,
};
use crate::result_set::ResultSet;
use crate::wrapper::Wrapper;

/// `DatabaseMetaData#getTables` 的 typed pattern descriptor。
///
/// `None` 与空字符串保持不同含义，不在标准层合并。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TablesQuery {
    /// catalog pattern；`None` 保留 Java `null` 语义。
    pub catalog: Option<String>,
    /// schema pattern；`None` 保留 Java `null` 语义。
    pub schema_pattern: Option<String>,
    /// table name pattern；`None` 保留 Java `null` 语义。
    pub table_name_pattern: Option<String>,
    /// table types；`None` 与空集合语义不同。
    pub table_types: Option<Vec<String>>,
}

/// JDBC `DatabaseMetaData` 的 owned live object 标准。
///
/// 对应 Java：`java.sql.DatabaseMetaData`。实现持有自己的 connection actor/native handle，
/// 不借用创建它的 [`crate::connection::Connection`]。
#[async_trait::async_trait]
pub trait DatabaseMetaData: Wrapper + Send + 'static {
    /// 执行 boolean-valued metadata request。
    async fn metadata_boolean(
        &mut self,
        request: DatabaseMetaDataRequest,
    ) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: request.operation().as_str(),
        })
    }

    /// 执行 nullable-string metadata request。
    async fn metadata_string(
        &mut self,
        request: DatabaseMetaDataRequest,
    ) -> Result<Option<String>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: request.operation().as_str(),
        })
    }

    /// 执行 integer-valued metadata request。
    async fn metadata_integer(
        &mut self,
        request: DatabaseMetaDataRequest,
    ) -> Result<i32, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: request.operation().as_str(),
        })
    }

    /// 执行 ResultSet-valued metadata request。
    async fn metadata_result_set(
        &mut self,
        request: DatabaseMetaDataRequest,
    ) -> Result<Box<dyn ResultSet>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: request.operation().as_str(),
        })
    }

    /// 执行 RowId lifetime request。
    async fn metadata_row_id_lifetime(
        &mut self,
        request: DatabaseMetaDataRequest,
    ) -> Result<RowIdLifetime, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: request.operation().as_str(),
        })
    }

    /// 返回数据库产品名。
    async fn database_product_name(&mut self) -> Result<String, RdbcError> {
        self.metadata_string(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetDatabaseProductName,
            Vec::new(),
        ))
        .await?
        .ok_or_else(|| RdbcError::DriverError("database product name is SQL NULL".to_owned()))
    }

    /// 查询 tables 并返回 owned live result set。
    async fn tables(&mut self, _query: TablesQuery) -> Result<Box<dyn ResultSet>, RdbcError> {
        self.metadata_result_set(DatabaseMetaDataRequest::new(
            DatabaseMetaDataOperation::GetTables,
            std::vec![
                DatabaseMetaDataArgument::OptionalString(_query.catalog),
                DatabaseMetaDataArgument::OptionalString(_query.schema_pattern),
                DatabaseMetaDataArgument::OptionalString(_query.table_name_pattern),
                DatabaseMetaDataArgument::Strings(_query.table_types),
            ],
        ))
        .await
    }

    /// 返回创建本 metadata 对象的连接。
    ///
    /// 对应 Java：`java.sql.DatabaseMetaData#getConnection()`。Java 8 规定驱动不支持时
    /// 抛出 `SQLFeatureNotSupportedException`，因此默认入口返回同一个标准错误，
    /// 不伪造连接对象，也不返回借用连接。
    async fn get_connection(&mut self) -> Result<Box<dyn Connection>, RdbcError> {
        Err(RdbcError::feature_not_supported("metadata_get_connection"))
    }

    /// 返回逻辑 LOB 的最大字节数。
    ///
    /// 对应 Java：`java.sql.DatabaseMetaData#getMaxLogicalLobSize()`。Java 8 规定驱动
    /// 不支持时抛出 `SQLFeatureNotSupportedException`，因此默认入口返回同一个标准错误，
    /// 不臆造 LOB 上限。
    async fn get_max_logical_lob_size(&mut self) -> Result<i32, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "metadata_get_max_logical_lob_size",
        ))
    }

    /// 返回驱动是否支持 REF CURSOR。
    ///
    /// 对应 Java：`java.sql.DatabaseMetaData#supportsRefCursors()`。Java 8 规定驱动
    /// 不支持时抛出 `SQLFeatureNotSupportedException`，因此默认入口返回同一个标准错误，
    /// 不返回伪造的能力声明。
    async fn supports_ref_cursors(&mut self) -> Result<bool, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "metadata_supports_ref_cursors",
        ))
    }
}
