//! `PreparedStatement` 按索引绑定参数。
//!
//! 对应 Java 平台依赖：`java.sql.PreparedStatement` 的 `setXxx` 方法族。
//! 来源语义：
//! `com.alibaba.druid.pool.DruidPooledPreparedStatement`。

use super::{
    DruidError, JdbcArray, JdbcBlob, JdbcCalendarArgument, JdbcCharacterLength, JdbcClob,
    JdbcInputStream, JdbcNClob, JdbcObject, JdbcReader, JdbcRef, JdbcRowId, JdbcSqlXml,
    JdbcStreamLength, JdbcUrl, Value,
};
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

/// `setNull(..., typeName)` 重载状态。
///
/// Java 无 `typeName` 重载与显式传入 `null` 是不同调用，必须独立保留。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PreparedTypeNameArgument {
    /// 调用了 `setNull(int, int)`。
    #[default]
    Unspecified,
    /// 调用了 `setNull(int, int, String)`；内部 `None` 对应 Java null。
    Specified(Option<String>),
}

/// PreparedStatement 参数及其精确 JDBC setter 语义。
///
/// 每个 variant 对应 Java 的一个 setter 家族。流和 LOB 保存资源句柄，不在
/// 池化层提前读取；Calendar、长度和 SQL 类型元数据保持到物理 Adapter 边界。
#[derive(Debug, Clone, PartialEq)]
pub enum PreparedInputParameter {
    /// Rust 显式 `add_batch(Vec<Value>)` 扩展参数。
    ///
    /// 该分支不伪装成 Java `setXxx`；仅用于把 Rust 原有批处理入口与完整
    /// JDBC setter 描述符放进同一个有序批次。
    RustValue(Value),
    /// `setNull(parameterIndex, sqlType[, typeName])`。
    Null {
        /// Java 参数 `sqlType`。
        sql_type: i32,
        /// 是否调用了带 `typeName` 的重载及其 nullable 参数。
        type_name: PreparedTypeNameArgument,
    },
    /// `setBoolean(parameterIndex, x)`。
    Boolean(bool),
    /// `setByte(parameterIndex, x)`。
    Byte(i8),
    /// `setShort(parameterIndex, x)`。
    Short(i16),
    /// `setInt(parameterIndex, x)`。
    Int(i32),
    /// `setLong(parameterIndex, x)`。
    Long(i64),
    /// `setFloat(parameterIndex, x)`。
    Float(f32),
    /// `setDouble(parameterIndex, x)`。
    Double(f64),
    /// `setBigDecimal(parameterIndex, x)`；`None` 对应 Java null。
    BigDecimal(Option<BigDecimal>),
    /// `setString(parameterIndex, x)`；`None` 对应 Java null。
    String(Option<String>),
    /// `setNString(parameterIndex, value)`；`None` 对应 Java null。
    NString(Option<String>),
    /// `setBytes(parameterIndex, x)`；`None` 对应 Java null。
    Bytes(Option<Vec<u8>>),
    /// `setDate(parameterIndex, x[, cal])`。
    Date {
        /// Java 参数 `x`。
        value: Option<NaiveDate>,
        /// Calendar 重载身份。
        calendar: JdbcCalendarArgument,
    },
    /// `setTime(parameterIndex, x[, cal])`。
    Time {
        /// Java 参数 `x`。
        value: Option<NaiveTime>,
        /// Calendar 重载身份。
        calendar: JdbcCalendarArgument,
    },
    /// `setTimestamp(parameterIndex, x[, cal])`。
    Timestamp {
        /// Java 参数 `x`。
        value: Option<NaiveDateTime>,
        /// Calendar 重载身份。
        calendar: JdbcCalendarArgument,
    },
    /// `setAsciiStream(parameterIndex, x[, int/long])`。
    AsciiStream {
        /// Java 参数 `x`。
        stream: Option<JdbcInputStream>,
        /// 长度重载身份。
        length: JdbcStreamLength,
    },
    /// 已废弃的 `setUnicodeStream(parameterIndex, x, int)`。
    UnicodeStream {
        /// Java 参数 `x`。
        stream: Option<JdbcInputStream>,
        /// Java 参数 `length`，原样保留负值。
        length: i32,
    },
    /// `setBinaryStream(parameterIndex, x[, int/long])`。
    BinaryStream {
        /// Java 参数 `x`。
        stream: Option<JdbcInputStream>,
        /// 长度重载身份。
        length: JdbcStreamLength,
    },
    /// `setCharacterStream(parameterIndex, reader[, int/long])`。
    CharacterStream {
        /// Java 参数 `reader`。
        reader: Option<JdbcReader>,
        /// 长度重载身份。
        length: JdbcCharacterLength,
    },
    /// `setNCharacterStream(parameterIndex, value[, long])`。
    NCharacterStream {
        /// Java 参数 `value`。
        reader: Option<JdbcReader>,
        /// 长度重载身份。
        length: JdbcCharacterLength,
    },
    /// `setObject(parameterIndex, x[, targetSqlType[, scaleOrLength]])`。
    Object {
        /// Java 参数 `x`；`None` 对应 Java null。
        value: Option<JdbcObject>,
        /// Java 参数 `targetSqlType`。
        target_sql_type: Option<i32>,
        /// Java 参数 `scaleOrLength`。
        scale_or_length: Option<i32>,
    },
    /// `setRef(parameterIndex, x)`。
    Ref(Option<JdbcRef>),
    /// `setBlob(parameterIndex, x)`。
    Blob(Option<JdbcBlob>),
    /// `setBlob(parameterIndex, inputStream[, long])`。
    BlobStream {
        /// Java 参数 `inputStream`。
        stream: Option<JdbcInputStream>,
        /// 长度重载身份。
        length: JdbcStreamLength,
    },
    /// `setClob(parameterIndex, x)`。
    Clob(Option<JdbcClob>),
    /// `setClob(parameterIndex, reader[, long])`。
    ClobReader {
        /// Java 参数 `reader`。
        reader: Option<JdbcReader>,
        /// 长度重载身份。
        length: JdbcCharacterLength,
    },
    /// `setNClob(parameterIndex, value)`。
    NClob(Option<JdbcNClob>),
    /// `setNClob(parameterIndex, reader[, long])`。
    NClobReader {
        /// Java 参数 `reader`。
        reader: Option<JdbcReader>,
        /// 长度重载身份。
        length: JdbcCharacterLength,
    },
    /// `setArray(parameterIndex, x)`。
    Array(Option<JdbcArray>),
    /// `setURL(parameterIndex, x)`。
    Url(Option<JdbcUrl>),
    /// `setRowId(parameterIndex, x)`。
    RowId(Option<JdbcRowId>),
    /// `setSQLXML(parameterIndex, xmlObject)`。
    SqlXml(Option<JdbcSqlXml>),
}

impl PreparedInputParameter {
    /// 创建 `setNull(int, int)` 描述符。
    pub fn null(sql_type: i32) -> Self {
        Self::Null {
            sql_type,
            type_name: PreparedTypeNameArgument::Unspecified,
        }
    }

    /// 创建 `setNull(int, int, String)` 描述符。
    pub fn null_with_type_name(sql_type: i32, type_name: Option<String>) -> Self {
        Self::Null {
            sql_type,
            type_name: PreparedTypeNameArgument::Specified(type_name),
        }
    }

    /// 创建 `setObject(int, Object)` 描述符。
    pub fn object(value: Option<JdbcObject>) -> Self {
        Self::Object {
            value,
            target_sql_type: None,
            scale_or_length: None,
        }
    }

    /// 创建 `setObject(int, Object, int)` 描述符。
    pub fn object_with_sql_type(value: Option<JdbcObject>, target_sql_type: i32) -> Self {
        Self::Object {
            value,
            target_sql_type: Some(target_sql_type),
            scale_or_length: None,
        }
    }

    /// 创建 `setObject(int, Object, int, int)` 描述符。
    pub fn object_with_sql_type_and_scale(
        value: Option<JdbcObject>,
        target_sql_type: i32,
        scale_or_length: i32,
    ) -> Self {
        Self::Object {
            value,
            target_sql_type: Some(target_sql_type),
            scale_or_length: Some(scale_or_length),
        }
    }

    /// 将无需资源读取的参数转换为当前通用驱动执行值。
    ///
    /// LOB、流、Ref、Array、RowId、SQLXML 和 vendor object 必须由覆盖
    /// `PhysicalConnection` 参数执行入口的 Adapter 原生处理，不能在池化层物化。
    pub fn scalar_value(&self) -> Result<Value, DruidError> {
        match self {
            Self::RustValue(value) => Ok(value.clone()),
            Self::Null { .. } => Ok(Value::Null),
            Self::Boolean(value) => Ok(Value::Bool(*value)),
            Self::Byte(value) => Ok(Value::Int(i64::from(*value))),
            Self::Short(value) => Ok(Value::Int(i64::from(*value))),
            Self::Int(value) => Ok(Value::Int(i64::from(*value))),
            Self::Long(value) => Ok(Value::Int(*value)),
            Self::Float(value) => Ok(Value::Float(f64::from(*value))),
            Self::Double(value) => Ok(Value::Float(*value)),
            Self::BigDecimal(value) => Ok(value.clone().map_or(Value::Null, Value::Decimal)),
            Self::String(value) | Self::NString(value) => {
                Ok(value.clone().map_or(Value::Null, Value::String))
            }
            Self::Bytes(value) => Ok(value.clone().map_or(Value::Null, Value::Bytes)),
            Self::Date { value, .. } => Ok(value.map_or(Value::Null, Value::Date)),
            Self::Time { value, .. } => Ok(value.map_or(Value::Null, Value::Time)),
            Self::Timestamp { value, .. } => Ok(value.map_or(Value::Null, Value::Timestamp)),
            Self::Object { value: None, .. } => Ok(Value::Null),
            Self::Object {
                value: Some(value), ..
            } => Self::jdbc_object_scalar(value),
            Self::Url(Some(value)) => Ok(Value::String(value.external_form().to_string())),
            Self::Url(None)
            | Self::RowId(None)
            | Self::SqlXml(None)
            | Self::Ref(None)
            | Self::Array(None)
            | Self::Blob(None)
            | Self::Clob(None)
            | Self::NClob(None) => Ok(Value::Null),
            Self::AsciiStream { .. }
            | Self::UnicodeStream { .. }
            | Self::BinaryStream { .. }
            | Self::CharacterStream { .. }
            | Self::NCharacterStream { .. }
            | Self::Ref(Some(_))
            | Self::Blob(Some(_))
            | Self::BlobStream { .. }
            | Self::Clob(Some(_))
            | Self::ClobReader { .. }
            | Self::NClob(Some(_))
            | Self::NClobReader { .. }
            | Self::Array(Some(_))
            | Self::RowId(Some(_))
            | Self::SqlXml(Some(_)) => Err(DruidError::UnsupportedOperation {
                operation: "prepared_parameter_requires_native_adapter",
            }),
        }
    }

    fn jdbc_object_scalar(value: &JdbcObject) -> Result<Value, DruidError> {
        match value {
            JdbcObject::Scalar(value) => Ok(value.clone()),
            JdbcObject::String(value) | JdbcObject::NString(value) => {
                Ok(Value::String(value.clone()))
            }
            JdbcObject::Boolean(value) => Ok(Value::Bool(*value)),
            JdbcObject::Byte(value) => Ok(Value::Int(i64::from(*value))),
            JdbcObject::Short(value) => Ok(Value::Int(i64::from(*value))),
            JdbcObject::Integer(value) => Ok(Value::Int(i64::from(*value))),
            JdbcObject::Long(value) => Ok(Value::Int(*value)),
            JdbcObject::Float(value) => Ok(Value::Float(f64::from(*value))),
            JdbcObject::Double(value) => Ok(Value::Float(*value)),
            JdbcObject::Bytes(value) => Ok(Value::Bytes(value.clone())),
            JdbcObject::BigDecimal(value) => Ok(Value::Decimal(value.clone())),
            JdbcObject::Date(value) => Ok(Value::Date(*value)),
            JdbcObject::Time(value) => Ok(Value::Time(*value)),
            JdbcObject::Timestamp(value) => Ok(Value::Timestamp(*value)),
            JdbcObject::Url(value) => Ok(Value::String(value.external_form().to_string())),
            JdbcObject::Ref(_)
            | JdbcObject::Array(_)
            | JdbcObject::RowId(_)
            | JdbcObject::SqlXml(_)
            | JdbcObject::Blob(_)
            | JdbcObject::Clob(_)
            | JdbcObject::NClob(_)
            | JdbcObject::CharacterStream(_)
            | JdbcObject::NCharacterStream(_)
            | JdbcObject::Custom(_) => Err(DruidError::UnsupportedOperation {
                operation: "prepared_object_requires_native_adapter",
            }),
        }
    }
}
