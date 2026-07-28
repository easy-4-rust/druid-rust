//! 物理连接属性快照。

/// 物理连接属性快照。
///
/// 对应 Java: `java.sql.Connection` 的 auto-commit、read-only、
/// transaction-isolation、catalog 与 schema 状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnState {
    /// 是否自动提交。
    pub auto_commit: bool,
    /// 是否只读。
    pub read_only: bool,
    /// 事务隔离级别，沿用 JDBC 数值语义。
    pub transaction_isolation: u8,
    /// 当前 catalog。
    pub catalog: Option<String>,
    /// 当前 schema。
    pub schema: Option<String>,
}

impl Default for ConnState {
    fn default() -> Self {
        Self {
            auto_commit: true,
            read_only: false,
            transaction_isolation: 2,
            catalog: None,
            schema: None,
        }
    }
}
