//! Statement 最近执行入口类型。
//!
//! 对应 Java：
//! `com.alibaba.druid.proxy.rdbc.StatementExecuteType`。

/// 区分 RDBC Statement 的四种执行入口。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatementExecuteType {
    /// `Statement#execute` / `PreparedStatement#execute`。
    Execute,
    /// `executeQuery`。
    ExecuteQuery,
    /// `executeUpdate`。
    ExecuteUpdate,
    /// `executeBatch`。
    ExecuteBatch,
}
