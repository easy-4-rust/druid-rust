use std::fmt;

/// 标准枚举名称解析失败，保留 Java valueOf 的两类异常。对应 Java: Enum#valueOf。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientInfoStatusNameError {
    /// 名称为 NULL，对应 NullPointerException。
    NullName,
    /// 不存在的精确名称，对应 IllegalArgumentException。
    UnknownName(String),
}
impl ClientInfoStatusNameError {
    /// 无参数返回对应 Java 异常类名，供调用方保留标准错误分类。
    pub const fn class_name(&self) -> &'static str {
        match self {
            Self::NullName => "java.lang.NullPointerException",
            Self::UnknownName(_) => "java.lang.IllegalArgumentException",
        }
    }
}
impl fmt::Display for ClientInfoStatusNameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NullName => f.write_str("Name is null"),
            Self::UnknownName(name) => {
                write!(f, "No enum constant java.sql.ClientInfoStatus.{name}")
            }
        }
    }
}
impl std::error::Error for ClientInfoStatusNameError {}
