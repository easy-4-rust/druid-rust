//! 物理连接能力集合。

/// 物理连接可选能力集合。
///
/// 对应 Java: JDBC 驱动通过 `DatabaseMetaData` 和
/// `SQLFeatureNotSupportedException` 暴露的能力差异。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalConnectionCapabilities {
    /// 是否支持事务。
    pub transactions: bool,
    /// 是否支持保存点。
    pub savepoints: bool,
    /// 是否支持只读、自动提交和隔离级别等连接属性。
    pub connection_attributes: bool,
    /// 是否支持 catalog。
    pub catalog: bool,
    /// 是否支持 schema。
    pub schema: bool,
}

impl Default for PhysicalConnectionCapabilities {
    fn default() -> Self {
        Self {
            transactions: true,
            savepoints: false,
            connection_attributes: false,
            catalog: false,
            schema: false,
        }
    }
}
