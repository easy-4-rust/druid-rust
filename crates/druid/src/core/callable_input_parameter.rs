//! `CallableStatement` 命名 IN 参数。
//!
//! 对应 Java 平台依赖：`java.sql.CallableStatement` 的命名参数 setter 方法族。
//! 来源语义：
//! `DruidPooledCallableStatement#setNull/setObject/setBoolean/...`。

use super::{
    CallableCalendarArgument, RdbcBlob, RdbcCharacterLength, RdbcClob, RdbcInputStream, RdbcNClob,
    RdbcReader, RdbcRowId, RdbcSqlXml, RdbcStreamLength, RdbcUrl, Value,
};
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

/// 命名 IN 参数及其精确 RDBC setter 语义。
///
/// 不同 variant 对应不同的 Java `CallableStatement` setter，避免把
/// `setByte`、`setNString` 或带 `targetSqlType/scale` 的 `setObject`
/// 压缩成无法还原的通用值。
#[derive(Debug, Clone, PartialEq)]
pub enum CallableInputParameter {
    /// `setNull(parameterName, sqlType[, typeName])`。
    Null {
        /// Java 参数 `sqlType`。
        sql_type: i32,
        /// Java 参数 `typeName`。
        type_name: Option<String>,
    },
    /// `setBoolean(parameterName, x)`。
    Boolean(bool),
    /// `setByte(parameterName, x)`。
    Byte(i8),
    /// `setShort(parameterName, x)`。
    Short(i16),
    /// `setInt(parameterName, x)`。
    Int(i32),
    /// `setLong(parameterName, x)`。
    Long(i64),
    /// `setFloat(parameterName, x)`。
    Float(f32),
    /// `setDouble(parameterName, x)`。
    Double(f64),
    /// `setString(parameterName, x)`；`None` 对应 Java null。
    String(Option<String>),
    /// `setNString(parameterName, value)`；`None` 对应 Java null。
    NString(Option<String>),
    /// `setBytes(parameterName, x)`；`None` 对应 Java null。
    Bytes(Option<Vec<u8>>),
    /// `setURL(parameterName, val)`；`None` 对应 Java null。
    Url(Option<RdbcUrl>),
    /// `setRowId(parameterName, x)`；`None` 对应 Java null。
    RowId(Option<RdbcRowId>),
    /// `setSQLXML(parameterName, xmlObject)`；`None` 对应 Java null。
    SqlXml(Option<RdbcSqlXml>),
    /// `setAsciiStream(parameterName, stream[, int/long])`。
    AsciiStream {
        /// Java 参数 `x`；`None` 对应 Java null。
        stream: Option<RdbcInputStream>,
        /// 精确保留无长度、int 或 long 重载。
        length: RdbcStreamLength,
    },
    /// `setBinaryStream(parameterName, stream[, int/long])`。
    BinaryStream {
        /// Java 参数 `x`；`None` 对应 Java null。
        stream: Option<RdbcInputStream>,
        /// 精确保留无长度、int 或 long 重载。
        length: RdbcStreamLength,
    },
    /// `setBlob(parameterName, Blob)`；`None` 对应通过该 setter 传入 Java null。
    Blob(Option<RdbcBlob>),
    /// `setBlob(parameterName, InputStream[, long])`。
    BlobStream {
        /// Java 参数 `inputStream`；`None` 对应 Java null。
        stream: Option<RdbcInputStream>,
        /// 区分无 length 重载和显式 long length。
        length: RdbcStreamLength,
    },
    /// `setClob(parameterName, Clob)`；`None` 对应 Java null。
    Clob(Option<RdbcClob>),
    /// `setClob(parameterName, Reader[, long])`。
    ClobReader {
        /// Java 参数 `reader`。
        reader: Option<RdbcReader>,
        /// 无长度或 long 长度重载。
        length: RdbcCharacterLength,
    },
    /// `setNClob(parameterName, NClob)`；`None` 对应 Java null。
    NClob(Option<RdbcNClob>),
    /// `setNClob(parameterName, Reader[, long])`。
    NClobReader {
        /// Java 参数 `reader`。
        reader: Option<RdbcReader>,
        /// 无长度或 long 长度重载。
        length: RdbcCharacterLength,
    },
    /// `setCharacterStream(parameterName, Reader[, int/long])`。
    CharacterStream {
        /// Java 参数 `reader`。
        reader: Option<RdbcReader>,
        /// 精确保留 int、long 或无长度重载。
        length: RdbcCharacterLength,
    },
    /// `setNCharacterStream(parameterName, Reader[, long])`。
    NCharacterStream {
        /// Java 参数 `value`。
        reader: Option<RdbcReader>,
        /// 无长度或 long 长度重载。
        length: RdbcCharacterLength,
    },
    /// `setBigDecimal(parameterName, x)`；`None` 对应 Java null。
    BigDecimal(Option<BigDecimal>),
    /// `setDate(parameterName, x[, cal])`。
    Date {
        /// Java 参数 `x`；`None` 对应 Java null。
        value: Option<NaiveDate>,
        /// 区分无 Calendar 重载、显式 null 和实际 Calendar。
        calendar: CallableCalendarArgument,
    },
    /// `setTime(parameterName, x[, cal])`。
    Time {
        /// Java 参数 `x`；`None` 对应 Java null。
        value: Option<NaiveTime>,
        /// 区分无 Calendar 重载、显式 null 和实际 Calendar。
        calendar: CallableCalendarArgument,
    },
    /// `setTimestamp(parameterName, x[, cal])`。
    Timestamp {
        /// Java 参数 `x`；`None` 对应 Java null。
        value: Option<NaiveDateTime>,
        /// 区分无 Calendar 重载、显式 null 和实际 Calendar。
        calendar: CallableCalendarArgument,
    },
    /// `setObject(parameterName, x[, targetSqlType[, scale]])`。
    Object {
        /// Java 参数 `x`。
        value: Value,
        /// Java 参数 `targetSqlType`。
        target_sql_type: Option<i32>,
        /// Java 参数 `scale`。
        scale: Option<i32>,
    },
}

impl CallableInputParameter {
    /// 创建 `setNull(String, int)` 参数。
    ///
    /// # 参数
    /// - `sql_type`：Java 参数 `sqlType`。
    pub fn null(sql_type: i32) -> Self {
        Self::Null {
            sql_type,
            type_name: None,
        }
    }

    /// 创建 `setNull(String, int, String)` 参数。
    ///
    /// # 参数
    /// - `sql_type`：Java 参数 `sqlType`。
    /// - `type_name`：Java 参数 `typeName`。
    pub fn null_with_type_name(sql_type: i32, type_name: impl Into<String>) -> Self {
        Self::Null {
            sql_type,
            type_name: Some(type_name.into()),
        }
    }

    /// 创建 `setObject(String, Object)` 参数。
    ///
    /// # 参数
    /// - `value`：Java 参数 `x` 的 Rust 值。
    pub fn object(value: Value) -> Self {
        Self::Object {
            value,
            target_sql_type: None,
            scale: None,
        }
    }

    /// 创建 `setObject(String, Object, int)` 参数。
    ///
    /// # 参数
    /// - `value`：Java 参数 `x` 的 Rust 值。
    /// - `target_sql_type`：Java 参数 `targetSqlType`。
    pub fn object_with_sql_type(value: Value, target_sql_type: i32) -> Self {
        Self::Object {
            value,
            target_sql_type: Some(target_sql_type),
            scale: None,
        }
    }

    /// 创建 `setObject(String, Object, int, int)` 参数。
    ///
    /// # 参数
    /// - `value`：Java 参数 `x` 的 Rust 值。
    /// - `target_sql_type`：Java 参数 `targetSqlType`。
    /// - `scale`：Java 参数 `scale`。
    pub fn object_with_sql_type_and_scale(value: Value, target_sql_type: i32, scale: i32) -> Self {
        Self::Object {
            value,
            target_sql_type: Some(target_sql_type),
            scale: Some(scale),
        }
    }
}
