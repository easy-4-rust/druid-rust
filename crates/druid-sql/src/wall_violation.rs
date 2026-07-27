//! 对应 Java 类：com.alibaba.druid.wall.Violation
//!
//! Wall 违规类型枚举。

use std::fmt;

/// Wall 违规类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WallViolation {
    /// DROP TABLE 被拒绝
    DropTableNotAllowed(String),
    /// TRUNCATE 被拒绝
    TruncateNotAllowed,
    /// DELETE 无 WHERE
    DeleteWithoutWhere,
    /// UPDATE 无 WHERE
    UpdateWithoutWhere,
    /// 禁止的表
    DeniedTable(String),
    /// 禁止的函数
    DeniedFunction(String),
    /// SQL 解析错误
    SyntaxError(String),
}

impl fmt::Display for WallViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DropTableNotAllowed(t) => write!(f, "DROP TABLE not allowed: {t}"),
            Self::TruncateNotAllowed => write!(f, "TRUNCATE not allowed"),
            Self::DeleteWithoutWhere => write!(f, "DELETE without WHERE not allowed"),
            Self::UpdateWithoutWhere => write!(f, "UPDATE without WHERE not allowed"),
            Self::DeniedTable(t) => write!(f, "denied table: {t}"),
            Self::DeniedFunction(fn_) => write!(f, "denied function: {fn_}"),
            Self::SyntaxError(msg) => write!(f, "syntax error: {msg}"),
        }
    }
}

impl std::error::Error for WallViolation {}
