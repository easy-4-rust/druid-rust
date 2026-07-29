//! JDBC `Object` 平台值。
//!
//! 对应 Java：`ResultSet#getObject`、`CallableStatement#getObject`、
//! `Array#getArray` 与 `Ref#getObject` 的返回对象。该类型属于 JDBC 平台层，
//! 不绑定某一种 Statement。

use super::{
    JdbcArray, JdbcBlob, JdbcClob, JdbcNClob, JdbcOpaqueObject, JdbcReader, JdbcRef, JdbcRowId,
    JdbcSqlXml, JdbcUrl, Value,
};
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use std::fmt;

/// JDBC `Object` 的无损平台表示。
#[derive(Debug, Clone, PartialEq)]
pub enum JdbcObject {
    /// 现有通用标量或 SQL NULL。
    Scalar(Value),
    /// typed `getObject(..., String.class)`。
    String(String),
    /// typed `getObject(..., Boolean.class)`。
    Boolean(bool),
    /// typed `getObject(..., Byte.class)`。
    Byte(i8),
    /// typed `getObject(..., Short.class)`。
    Short(i16),
    /// typed `getObject(..., Integer.class)`。
    Integer(i32),
    /// typed `getObject(..., Long.class)`。
    Long(i64),
    /// typed `getObject(..., Float.class)`。
    Float(f32),
    /// typed `getObject(..., Double.class)`。
    Double(f64),
    /// typed `getObject(..., byte[].class)`。
    Bytes(Vec<u8>),
    /// `java.math.BigDecimal`。
    BigDecimal(BigDecimal),
    /// `java.sql.Date`。
    Date(NaiveDate),
    /// `java.sql.Time`。
    Time(NaiveTime),
    /// `java.sql.Timestamp`，保留纳秒精度。
    Timestamp(NaiveDateTime),
    /// `CallableStatement#getNString` 返回的 national-character 字符串。
    NString(String),
    /// `java.net.URL`。
    Url(JdbcUrl),
    /// `java.sql.Ref`。
    Ref(JdbcRef),
    /// `java.sql.Array`。
    Array(JdbcArray),
    /// `java.sql.RowId`。
    RowId(JdbcRowId),
    /// `java.sql.SQLXML`。
    SqlXml(JdbcSqlXml),
    /// `java.sql.Blob` 资源句柄。
    Blob(JdbcBlob),
    /// `java.sql.Clob` 资源句柄。
    Clob(JdbcClob),
    /// `java.sql.NClob` 资源句柄。
    NClob(JdbcNClob),
    /// `java.io.Reader`，由 `getCharacterStream` 返回。
    CharacterStream(JdbcReader),
    /// `java.io.Reader`，由 `getNCharacterStream` 返回。
    NCharacterStream(JdbcReader),
    /// 任意 driver/vendor 自定义对象；共享句柄保留 Java 引用身份。
    Custom(JdbcOpaqueObject),
}

impl JdbcObject {
    /// 返回是否为 SQL NULL。
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Scalar(Value::Null))
    }
}

impl From<Value> for JdbcObject {
    fn from(value: Value) -> Self {
        Self::Scalar(value)
    }
}

impl fmt::Display for JdbcObject {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scalar(value) => value.fmt(formatter),
            Self::String(value) => value.fmt(formatter),
            Self::Boolean(value) => value.fmt(formatter),
            Self::Byte(value) => value.fmt(formatter),
            Self::Short(value) => value.fmt(formatter),
            Self::Integer(value) => value.fmt(formatter),
            Self::Long(value) => value.fmt(formatter),
            Self::Float(value) => value.fmt(formatter),
            Self::Double(value) => value.fmt(formatter),
            Self::Bytes(value) => write!(formatter, "<{} bytes>", value.len()),
            Self::BigDecimal(value) => value.fmt(formatter),
            Self::Date(value) => value.fmt(formatter),
            Self::Time(value) => value.fmt(formatter),
            Self::Timestamp(value) => value.fmt(formatter),
            Self::NString(value) => value.fmt(formatter),
            Self::Url(value) => value.external_form().fmt(formatter),
            Self::Ref(_) => formatter.write_str("<Ref>"),
            Self::Array(_) => formatter.write_str("<Array>"),
            Self::RowId(_) => formatter.write_str("<RowId>"),
            Self::SqlXml(_) => formatter.write_str("<SQLXML>"),
            Self::Blob(_) => formatter.write_str("<Blob>"),
            Self::Clob(_) => formatter.write_str("<Clob>"),
            Self::NClob(_) => formatter.write_str("<NClob>"),
            Self::CharacterStream(_) => formatter.write_str("<CharacterStream>"),
            Self::NCharacterStream(_) => formatter.write_str("<NCharacterStream>"),
            Self::Custom(value) => write!(formatter, "<{}>", value.class_name()),
        }
    }
}
