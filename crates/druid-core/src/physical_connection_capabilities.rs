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
    /// 是否支持读取与设置自动提交状态。
    pub auto_commit: bool,
    /// 是否支持读取与设置只读状态。
    pub read_only: bool,
    /// 是否支持读取与设置事务隔离级别。
    pub transaction_isolation: bool,
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
            auto_commit: false,
            read_only: false,
            transaction_isolation: false,
            catalog: false,
            schema: false,
        }
    }
}
