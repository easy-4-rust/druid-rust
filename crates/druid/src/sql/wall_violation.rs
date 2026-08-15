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
    /// AND 链中非首位恒假条件。
    ///
    /// 对应 Java `ErrorCode.ALWAYS_FALSE`（2113）。
    AlwaysFalseCondition(String),
    /// AND 链中出现两个相邻常量操作数。
    ///
    /// 对应 Java `ErrorCode.DOUBLE_CONST_CONDITION`（2107）。
    DoubleConstCondition,
    /// 条件表达式使用 XOR 运算符。
    ///
    /// 对应 Java `ErrorCode.XOR`（2102）。
    XorNotAllowed,
    /// 条件表达式使用位运算符。
    ///
    /// 对应 Java `ErrorCode.BITWISE`（2103）。
    BitwiseNotAllowed,
    /// 条件表达式包含常量算术运算。
    ///
    /// 对应 Java `ErrorCode.CONST_ARITHMETIC`（2101）。
    ConstArithmeticNotAllowed,
    /// `LIKE` 两侧为相同常量。
    ///
    /// 对应 Java `ErrorCode.SAME_CONST_LIKE`（2108）。
    SameConstLike,
    /// `CASE WHEN` 条件为常量。
    ///
    /// 对应 Java `ErrorCode.CONST_CASE_CONDITION`（2109）。
    ConstCaseCondition,
    /// SQL 必须参数化。
    MustParameterized,
    /// UPDATE 业务一致性检查失败。
    UpdateCheckFailed,
    /// LIMIT 0 被拒绝。
    LimitZeroNotAllowed,
    /// 禁止的表
    DeniedTable(String),
    /// 禁止的 schema
    DeniedSchema(String),
    /// 禁止的函数
    DeniedFunction(String),
    /// 禁止的数据库变量。
    DeniedVariant(String),
    /// 禁止的数据库对象。
    DeniedObject(String),
    /// 只读表被写入。
    ReadOnlyTable(String),
    /// SELECT INTO OUTFILE 被拒绝。
    SelectIntoOutfileNotAllowed,
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
            Self::AlwaysFalseCondition(clause) => {
                write!(f, "always false {clause} condition not allowed")
            }
            Self::DoubleConstCondition => write!(f, "double const condition not allowed"),
            Self::XorNotAllowed => write!(f, "xor operator not allowed"),
            Self::BitwiseNotAllowed => write!(f, "bitwise operator not allowed"),
            Self::ConstArithmeticNotAllowed => write!(f, "const arithmetic not allowed"),
            Self::SameConstLike => write!(f, "same const like not allowed"),
            Self::ConstCaseCondition => write!(f, "const case condition not allowed"),
            Self::MustParameterized => write!(f, "sql must be parameterized"),
            Self::UpdateCheckFailed => write!(f, "update check failed."),
            Self::LimitZeroNotAllowed => write!(f, "LIMIT 0 not allowed"),
            Self::DeniedTable(t) => write!(f, "denied table: {t}"),
            Self::DeniedSchema(schema) => write!(f, "denied schema: {schema}"),
            Self::DeniedFunction(fn_) => write!(f, "denied function: {fn_}"),
            Self::DeniedVariant(variant) => write!(f, "denied variant: {variant}"),
            Self::DeniedObject(object) => write!(f, "denied object: {object}"),
            Self::ReadOnlyTable(table) => write!(f, "read only table: {table}"),
            Self::SelectIntoOutfileNotAllowed => write!(f, "select into outfile not allowed"),
            Self::SyntaxError(msg) => write!(f, "syntax error: {msg}"),
        }
    }
}

impl std::error::Error for WallViolation {}
