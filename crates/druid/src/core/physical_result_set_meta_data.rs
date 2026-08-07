//! 物理 RDBC `ResultSetMetaData` SPI。
//!
//! 对应 Java：`java.sql.ResultSetMetaData`。所有 getter 保持独立调用，以保留
//! driver 的错误时机、副作用和动态 metadata；同时继承 `Wrapper` 保留底层
//! driver 对象的 `unwrap/isWrapperFor` 语义。

use super::{DruidError, ResultSetColumnType, ResultSetNullability, Wrapper};
use std::fmt;

/// 物理结果集 metadata 的完整对象安全 SPI。
pub trait PhysicalResultSetMetaData: Wrapper + fmt::Debug + Send + Sync {
    /// 返回列数。
    fn column_count(&self) -> Result<usize, DruidError>;

    /// 返回列是否自动递增。
    fn is_auto_increment(&self, column_index: usize) -> Result<bool, DruidError>;

    /// 返回列是否区分大小写。
    fn is_case_sensitive(&self, column_index: usize) -> Result<bool, DruidError>;

    /// 返回列是否可搜索。
    fn is_searchable(&self, column_index: usize) -> Result<bool, DruidError>;

    /// 返回列是否为货币值。
    fn is_currency(&self, column_index: usize) -> Result<bool, DruidError>;

    /// 返回三态可空性。
    fn nullability(&self, column_index: usize) -> Result<ResultSetNullability, DruidError>;

    /// 返回数值列是否有符号。
    fn is_signed(&self, column_index: usize) -> Result<bool, DruidError>;

    /// 返回最大显示字符数。
    fn column_display_size(&self, column_index: usize) -> Result<usize, DruidError>;

    /// 返回列标签。
    fn column_label(&self, column_index: usize) -> Result<String, DruidError>;

    /// 返回底层列名。
    fn column_name(&self, column_index: usize) -> Result<String, DruidError>;

    /// 返回 schema 名。
    fn schema_name(&self, column_index: usize) -> Result<String, DruidError>;

    /// 返回精度。
    fn precision(&self, column_index: usize) -> Result<usize, DruidError>;

    /// 返回 scale。
    fn scale(&self, column_index: usize) -> Result<usize, DruidError>;

    /// 返回表名。
    fn table_name(&self, column_index: usize) -> Result<String, DruidError>;

    /// 返回 catalog 名。
    fn catalog_name(&self, column_index: usize) -> Result<String, DruidError>;

    /// 返回通用列类型。
    fn column_type(&self, column_index: usize) -> Result<ResultSetColumnType, DruidError>;

    /// 返回数据库类型名。
    fn column_type_name(&self, column_index: usize) -> Result<String, DruidError>;

    /// 返回列是否只读。
    fn is_read_only(&self, column_index: usize) -> Result<bool, DruidError>;

    /// 返回列是否可写。
    fn is_writable(&self, column_index: usize) -> Result<bool, DruidError>;

    /// 返回列是否确定可写。
    fn is_definitely_writable(&self, column_index: usize) -> Result<bool, DruidError>;

    /// 返回 typed getter 对应 Java 类名。
    fn column_class_name(&self, column_index: usize) -> Result<String, DruidError>;
}
