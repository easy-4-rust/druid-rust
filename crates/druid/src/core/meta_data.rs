//! 数据库与驱动元数据。

/// 数据库与驱动元数据。
///
/// 对应 Java: `java.sql.DatabaseMetaData` 中 Druid 使用的字段。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MetaData {
    /// 数据库产品名称。
    pub database_product_name: String,
    /// 数据库产品版本。
    pub database_product_version: String,
    /// 驱动名称。
    pub driver_name: String,
    /// 驱动版本。
    pub driver_version: String,
    /// 驱动主版本号。
    pub driver_major_version: i32,
    /// 驱动次版本号。
    pub driver_minor_version: i32,
}
