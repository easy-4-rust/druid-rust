//! 对应 Java 类：java.sql.Types 值类型系统

use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use std::fmt;

/// JDBC 通用标量值。
///
/// 对应 Java：`java.sql.ResultSet#getObject`、PreparedStatement 参数和各驱动
/// 的标量结果。Decimal 与日期时间保持独立类型身份，不能先降级为字符串再由
/// pooled wrapper 猜测类型。
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// SQL NULL。
    Null,
    /// SQL BOOLEAN。
    Bool(bool),
    /// SQL 有符号整数。
    Int(i64),
    /// SQL 浮点数。
    Float(f64),
    /// SQL 任意精度 DECIMAL/NUMERIC。
    Decimal(BigDecimal),
    /// SQL DATE。
    Date(NaiveDate),
    /// SQL TIME。
    Time(NaiveTime),
    /// SQL TIMESTAMP/DATETIME，不含时区且保留纳秒。
    Timestamp(NaiveDateTime),
    /// SQL 字符串。
    String(String),
    /// SQL 二进制值。
    Bytes(Vec<u8>),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => write!(f, "NULL"),
            Self::Bool(v) => write!(f, "{v}"),
            Self::Int(v) => write!(f, "{v}"),
            Self::Float(v) => write!(f, "{v}"),
            Self::Decimal(v) => write!(f, "{v}"),
            Self::Date(v) => write!(f, "{v}"),
            Self::Time(v) => write!(f, "{v}"),
            Self::Timestamp(v) => write!(f, "{v}"),
            Self::String(v) => write!(f, "'{v}'"),
            Self::Bytes(v) => write!(f, "<{} bytes>", v.len()),
        }
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}
impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Self::Int(v)
    }
}
impl From<i32> for Value {
    fn from(v: i32) -> Self {
        Self::Int(v as i64)
    }
}
impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Self::Float(v)
    }
}
impl From<BigDecimal> for Value {
    fn from(v: BigDecimal) -> Self {
        Self::Decimal(v)
    }
}
impl From<NaiveDate> for Value {
    fn from(v: NaiveDate) -> Self {
        Self::Date(v)
    }
}
impl From<NaiveTime> for Value {
    fn from(v: NaiveTime) -> Self {
        Self::Time(v)
    }
}
impl From<NaiveDateTime> for Value {
    fn from(v: NaiveDateTime) -> Self {
        Self::Timestamp(v)
    }
}
impl From<String> for Value {
    fn from(v: String) -> Self {
        Self::String(v)
    }
}
impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Self::String(v.to_string())
    }
}
impl From<Vec<u8>> for Value {
    fn from(v: Vec<u8>) -> Self {
        Self::Bytes(v)
    }
}
