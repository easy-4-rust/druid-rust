//! 对应 Java 类：com.alibaba.druid.wall.Violation
//!
//! Wall 违规类型枚举。

use std::fmt;

/// Wall 违规类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WallViolation {
    /// 基础 SQL 操作被配置拒绝。
    OperationNotAllowed(String),
    /// 一次输入含多条语句。
    MultiStatementNotAllowed,
    /// DROP TABLE 被拒绝
    DropTableNotAllowed(String),
    /// TRUNCATE 被拒绝
    TruncateNotAllowed,
    /// DELETE 无 WHERE
    DeleteWithoutWhere,
    /// UPDATE 无 WHERE
    UpdateWithoutWhere,
    /// SELECT * 被拒绝。
    SelectAllColumnNotAllowed,
    /// 恒真条件绕过 WHERE。
    AlwaysTrueCondition(String),
    /// SQL 必须参数化。
    MustParameterized,
    /// LIMIT 0 被拒绝。
    LimitZeroNotAllowed,
    /// 禁止的表
    DeniedTable(String),
    /// 禁止的 schema
    DeniedSchema(String),
    /// 禁止的函数
    DeniedFunction(String),
    /// SQL 解析错误
    SyntaxError(String),
}

impl fmt::Display for WallViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OperationNotAllowed(operation) => write!(f, "{operation} not allowed"),
            Self::MultiStatementNotAllowed => write!(f, "multi-statement not allowed"),
            Self::DropTableNotAllowed(t) => write!(f, "DROP TABLE not allowed: {t}"),
            Self::TruncateNotAllowed => write!(f, "TRUNCATE not allowed"),
            Self::DeleteWithoutWhere => write!(f, "DELETE without WHERE not allowed"),
            Self::UpdateWithoutWhere => write!(f, "UPDATE without WHERE not allowed"),
            Self::SelectAllColumnNotAllowed => write!(f, "SELECT * not allowed"),
            Self::AlwaysTrueCondition(clause) => {
                write!(f, "always true {clause} condition not allowed")
            }
            Self::MustParameterized => write!(f, "sql must be parameterized"),
            Self::LimitZeroNotAllowed => write!(f, "LIMIT 0 not allowed"),
            Self::DeniedTable(t) => write!(f, "denied table: {t}"),
            Self::DeniedSchema(schema) => write!(f, "denied schema: {schema}"),
            Self::DeniedFunction(fn_) => write!(f, "denied function: {fn_}"),
            Self::SyntaxError(msg) => write!(f, "syntax error: {msg}"),
        }
    }
}

impl std::error::Error for WallViolation {}
