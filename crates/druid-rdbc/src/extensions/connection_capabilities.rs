/// 连接可选能力的 immutable descriptor。
///
/// 对应 Java：`DatabaseMetaData` capability queries 的 Rust 快照。
///
/// # Examples
///
/// ```
/// use druid_rdbc::extensions::ConnectionCapabilities;
/// let capabilities = ConnectionCapabilities::default();
/// assert!(capabilities.transactions);
/// assert!(!capabilities.savepoints);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionCapabilities {
    /// 是否支持事务。
    pub transactions: bool,
    /// 是否支持保存点。
    pub savepoints: bool,
    /// 是否支持 catalog。
    pub catalogs: bool,
    /// 是否支持 schema。
    pub schemas: bool,
    /// 是否支持 callable statements。
    pub callable_statements: bool,
}

impl Default for ConnectionCapabilities {
    fn default() -> Self {
        Self {
            transactions: true,
            savepoints: false,
            catalogs: false,
            schemas: false,
            callable_statements: false,
        }
    }
}
