//! `CallableStatement` OUT 参数值。
//!
//! 对应 Java 平台对象：`java.sql.CallableStatement#getObject` 的标量返回值，
//! 并为 `BigDecimal`、`Date`、`Time`、`Timestamp` 保留独立类型身份。

use crate::{
    JdbcArray, JdbcBlob, JdbcClob, JdbcNClob, JdbcReader, JdbcRef, JdbcRowId, JdbcSqlXml, JdbcUrl,
    Value,
};
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use std::fmt;

/// `CallableStatement` 标量 OUT 值。
///
/// Java `getObject` 还可能返回 ResultSet、Ref、Blob、Clob、Array 等对象；这些
/// 对象不能伪装成标量。本枚举只承载已迁移的标量族，其他对象由后续独立 SPI
/// 表达。
#[derive(Debug, Clone, PartialEq)]
pub enum CallableOutputValue {
    /// 现有通用标量或 SQL NULL。
    Scalar(Value),
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
}

impl CallableOutputValue {
    /// 返回是否为 SQL NULL。
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Scalar(Value::Null))
    }
}

impl From<Value> for CallableOutputValue {
    fn from(value: Value) -> Self {
        Self::Scalar(value)
    }
}

impl fmt::Display for CallableOutputValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scalar(value) => value.fmt(formatter),
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
        }
    }
}
