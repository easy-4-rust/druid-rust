//! 语句类型。

/// SQL 语句对象类型。
///
/// 对应 Java: `java.sql.Statement`、`PreparedStatement` 与
/// `CallableStatement`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatementType {
    /// 普通 Statement。
    Statement,
    /// 预编译 Statement 及其 SQL。
    PreparedStatement(String),
    /// 存储过程调用 Statement 及其 SQL。
    CallableStatement(String),
}
