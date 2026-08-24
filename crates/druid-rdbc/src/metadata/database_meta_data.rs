use crate::error::RdbcError;
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
    /// 返回数据库产品名。
    async fn database_product_name(&mut self) -> Result<String, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "database_meta_data_product_name",
        })
    }

    /// 查询 tables 并返回 owned live result set。
    async fn tables(&mut self, _query: TablesQuery) -> Result<Box<dyn ResultSet>, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "database_meta_data_tables",
        })
    }
}
