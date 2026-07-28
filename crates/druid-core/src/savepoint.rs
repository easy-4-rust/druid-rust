//! 事务保存点。

/// 事务保存点句柄。
///
/// 对应 Java: `java.sql.Savepoint`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Savepoint {
    /// 保存点 ID。
    pub id: u64,
    /// 命名保存点的名称；匿名保存点为 `None`。
    pub name: Option<String>,
}
