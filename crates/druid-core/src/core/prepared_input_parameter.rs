//! `PreparedStatement` 按索引绑定参数。
//!
//! 对应 Java 平台依赖：`java.sql.PreparedStatement` 的 `setXxx` 方法族。
//! 来源语义：
//! `com.alibaba.druid.pool.DruidPooledPreparedStatement`。

use super::{
    DruidError, RdbcArray, RdbcBlob, RdbcCalendarArgument, RdbcCharacterLength, RdbcClob,
    RdbcInputStream, RdbcNClob, RdbcObject, RdbcParameter, RdbcParameterType, RdbcParameterValue,
    RdbcReader, RdbcRef, RdbcRowId, RdbcSqlXml, RdbcStreamLength, RdbcUrl, Value,
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

/// PreparedStatement 参数及其精确 RDBC setter 语义。
///
/// 每个 variant 对应 Java 的一个 setter 家族。流和 LOB 保存资源句柄，不在
/// 池化层提前读取；Calendar、长度和 SQL 类型元数据保持到物理 Adapter 边界。
#[derive(Debug, Clone, PartialEq)]
pub enum PreparedInputParameter {
    /// Rust 显式 `add_batch(Vec<Value>)` 扩展参数。
    ///
    /// 该分支不伪装成 Java `setXxx`；仅用于把 Rust 原有批处理入口与完整
    /// RDBC setter 描述符放进同一个有序批次。
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
        calendar: RdbcCalendarArgument,
    },
    /// `setTime(parameterIndex, x[, cal])`。
    Time {
        /// Java 参数 `x`。
        value: Option<NaiveTime>,
        /// Calendar 重载身份。
        calendar: RdbcCalendarArgument,
    },
    /// `setTimestamp(parameterIndex, x[, cal])`。
    Timestamp {
        /// Java 参数 `x`。
        value: Option<NaiveDateTime>,
        /// Calendar 重载身份。
        calendar: RdbcCalendarArgument,
    },
    /// `setAsciiStream(parameterIndex, x[, int/long])`。
    AsciiStream {
        /// Java 参数 `x`。
        stream: Option<RdbcInputStream>,
        /// 长度重载身份。
        length: RdbcStreamLength,
    },
    /// 已废弃的 `setUnicodeStream(parameterIndex, x, int)`。
    UnicodeStream {
        /// Java 参数 `x`。
        stream: Option<RdbcInputStream>,
        /// Java 参数 `length`，原样保留负值。
        length: i32,
    },
    /// `setBinaryStream(parameterIndex, x[, int/long])`。
    BinaryStream {
        /// Java 参数 `x`。
        stream: Option<RdbcInputStream>,
        /// 长度重载身份。
        length: RdbcStreamLength,
    },
    /// `setCharacterStream(parameterIndex, reader[, int/long])`。
    CharacterStream {
        /// Java 参数 `reader`。
        reader: Option<RdbcReader>,
        /// 长度重载身份。
        length: RdbcCharacterLength,
    },
    /// `setNCharacterStream(parameterIndex, value[, long])`。
    NCharacterStream {
        /// Java 参数 `value`。
        reader: Option<RdbcReader>,
        /// 长度重载身份。
        length: RdbcCharacterLength,
    },
    /// `setObject(parameterIndex, x[, targetSqlType[, scaleOrLength]])`。
    Object {
        /// Java 参数 `x`；`None` 对应 Java null。
        value: Option<RdbcObject>,
        /// Java 参数 `targetSqlType`。
        target_sql_type: Option<i32>,
        /// Java 参数 `scaleOrLength`。
        scale_or_length: Option<i32>,
    },
    /// `setRef(parameterIndex, x)`。
    Ref(Option<RdbcRef>),
    /// `setBlob(parameterIndex, x)`。
    Blob(Option<RdbcBlob>),
    /// `setBlob(parameterIndex, inputStream[, long])`。
    BlobStream {
        /// Java 参数 `inputStream`。
        stream: Option<RdbcInputStream>,
        /// 长度重载身份。
        length: RdbcStreamLength,
    },
    /// `setClob(parameterIndex, x)`。
    Clob(Option<RdbcClob>),
    /// `setClob(parameterIndex, reader[, long])`。
    ClobReader {
        /// Java 参数 `reader`。
        reader: Option<RdbcReader>,
        /// 长度重载身份。
        length: RdbcCharacterLength,
    },
    /// `setNClob(parameterIndex, value)`。
    NClob(Option<RdbcNClob>),
    /// `setNClob(parameterIndex, reader[, long])`。
    NClobReader {
        /// Java 参数 `reader`。
        reader: Option<RdbcReader>,
        /// 长度重载身份。
        length: RdbcCharacterLength,
    },
    /// `setArray(parameterIndex, x)`。
    Array(Option<RdbcArray>),
    /// `setURL(parameterIndex, x)`。
    Url(Option<RdbcUrl>),
    /// `setRowId(parameterIndex, x)`。
    RowId(Option<RdbcRowId>),
    /// `setSQLXML(parameterIndex, xmlObject)`。
    SqlXml(Option<RdbcSqlXml>),
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
    pub fn object(value: Option<RdbcObject>) -> Self {
        Self::Object {
            value,
            target_sql_type: None,
            scale_or_length: None,
        }
    }

    /// 创建 `setObject(int, Object, int)` 描述符。
    pub fn object_with_sql_type(value: Option<RdbcObject>, target_sql_type: i32) -> Self {
        Self::Object {
            value,
            target_sql_type: Some(target_sql_type),
            scale_or_length: None,
        }
    }

    /// 创建 `setObject(int, Object, int, int)` 描述符。
    pub fn object_with_sql_type_and_scale(
        value: Option<RdbcObject>,
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
            } => Self::rdbc_object_scalar(value),
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

    fn rdbc_object_scalar(value: &RdbcObject) -> Result<Value, DruidError> {
        match value {
            RdbcObject::Scalar(value) => Ok(value.clone()),
            RdbcObject::String(value) | RdbcObject::NString(value) => {
                Ok(Value::String(value.clone()))
            }
            RdbcObject::Boolean(value) => Ok(Value::Bool(*value)),
            RdbcObject::Byte(value) => Ok(Value::Int(i64::from(*value))),
            RdbcObject::Short(value) => Ok(Value::Int(i64::from(*value))),
            RdbcObject::Integer(value) => Ok(Value::Int(i64::from(*value))),
            RdbcObject::Long(value) => Ok(Value::Int(*value)),
            RdbcObject::Float(value) => Ok(Value::Float(f64::from(*value))),
            RdbcObject::Double(value) => Ok(Value::Float(*value)),
            RdbcObject::Bytes(value) => Ok(Value::Bytes(value.clone())),
            RdbcObject::BigDecimal(value) => Ok(Value::Decimal(value.clone())),
            RdbcObject::Date(value) => Ok(Value::Date(*value)),
            RdbcObject::Time(value) => Ok(Value::Time(*value)),
            RdbcObject::Timestamp(value) => Ok(Value::Timestamp(*value)),
            RdbcObject::Url(value) => Ok(Value::String(value.external_form().to_string())),
            RdbcObject::Ref(_)
            | RdbcObject::Array(_)
            | RdbcObject::RowId(_)
            | RdbcObject::SqlXml(_)
            | RdbcObject::Blob(_)
            | RdbcObject::Clob(_)
            | RdbcObject::NClob(_)
            | RdbcObject::CharacterStream(_)
            | RdbcObject::NCharacterStream(_)
            | RdbcObject::Custom(_) => Err(DruidError::UnsupportedOperation {
                operation: "prepared_object_requires_native_adapter",
            }),
        }
    }
}

impl RdbcParameter for PreparedInputParameter {
    fn value(&self) -> Option<RdbcParameterValue> {
        let object = |value| Some(RdbcParameterValue::Object(value));
        match self {
            Self::RustValue(value) => object(RdbcObject::Scalar(value.clone())),
            Self::Null { .. } => None,
            Self::Boolean(value) => object(RdbcObject::Boolean(*value)),
            Self::Byte(value) => object(RdbcObject::Byte(*value)),
            Self::Short(value) => object(RdbcObject::Short(*value)),
            Self::Int(value) => object(RdbcObject::Integer(*value)),
            Self::Long(value) => object(RdbcObject::Long(*value)),
            Self::Float(value) => object(RdbcObject::Float(*value)),
            Self::Double(value) => object(RdbcObject::Double(*value)),
            Self::BigDecimal(value) => value
                .clone()
                .map(RdbcObject::BigDecimal)
                .map(RdbcParameterValue::Object),
            Self::String(value) => value
                .clone()
                .map(RdbcObject::String)
                .map(RdbcParameterValue::Object),
            Self::NString(value) => value
                .clone()
                .map(RdbcObject::NString)
                .map(RdbcParameterValue::Object),
            Self::Bytes(value) => value
                .clone()
                .map(RdbcObject::Bytes)
                .map(RdbcParameterValue::Object),
            Self::Date { value, .. } => value.map(RdbcObject::Date).map(RdbcParameterValue::Object),
            Self::Time { value, .. } => value.map(RdbcObject::Time).map(RdbcParameterValue::Object),
            Self::Timestamp { value, .. } => value
                .map(RdbcObject::Timestamp)
                .map(RdbcParameterValue::Object),
            Self::AsciiStream { stream, .. }
            | Self::BinaryStream { stream, .. }
            | Self::BlobStream { stream, .. } => {
                stream.clone().map(RdbcParameterValue::InputStream)
            }
            Self::UnicodeStream { stream, .. } => {
                stream.clone().map(RdbcParameterValue::InputStream)
            }
            Self::CharacterStream { reader, .. }
            | Self::NCharacterStream { reader, .. }
            | Self::ClobReader { reader, .. }
            | Self::NClobReader { reader, .. } => reader.clone().map(RdbcParameterValue::Reader),
            Self::Object { value, .. } => value.clone().map(RdbcParameterValue::Object),
            Self::Ref(value) => value
                .clone()
                .map(RdbcObject::Ref)
                .map(RdbcParameterValue::Object),
            Self::Blob(value) => value
                .clone()
                .map(RdbcObject::Blob)
                .map(RdbcParameterValue::Object),
            Self::Clob(value) => value
                .clone()
                .map(RdbcObject::Clob)
                .map(RdbcParameterValue::Object),
            Self::NClob(value) => value
                .clone()
                .map(RdbcObject::NClob)
                .map(RdbcParameterValue::Object),
            Self::Array(value) => value
                .clone()
                .map(RdbcObject::Array)
                .map(RdbcParameterValue::Object),
            Self::Url(value) => value
                .clone()
                .map(RdbcObject::Url)
                .map(RdbcParameterValue::Object),
            Self::RowId(value) => value
                .clone()
                .map(RdbcObject::RowId)
                .map(RdbcParameterValue::Object),
            Self::SqlXml(value) => value
                .clone()
                .map(RdbcObject::SqlXml)
                .map(RdbcParameterValue::Object),
        }
    }

    fn length(&self) -> i64 {
        match self {
            Self::Null { .. }
            | Self::Int(_)
            | Self::Long(_)
            | Self::BigDecimal(_)
            | Self::String(_) => 0,
            Self::Date { value, calendar } => {
                if value.is_none() || matches!(calendar, RdbcCalendarArgument::Unspecified) {
                    0
                } else {
                    -1
                }
            }
            Self::Timestamp { value, calendar } => {
                if value.is_none() || matches!(calendar, RdbcCalendarArgument::Unspecified) {
                    0
                } else {
                    -1
                }
            }
            Self::Time { value, .. } => {
                if value.is_none() {
                    0
                } else {
                    -1
                }
            }
            Self::AsciiStream { stream, length }
            | Self::BinaryStream { stream, length }
            | Self::BlobStream { stream, length } => {
                stream.as_ref().map_or(0, |_| stream_length(*length))
            }
            Self::UnicodeStream { stream, length } => {
                stream.as_ref().map_or(0, |_| i64::from(*length))
            }
            Self::CharacterStream { reader, length }
            | Self::NCharacterStream { reader, length }
            | Self::ClobReader { reader, length }
            | Self::NClobReader { reader, length } => {
                reader.as_ref().map_or(0, |_| character_length(*length))
            }
            Self::Object { value, .. } => value.as_ref().map_or(0, |_| -1),
            Self::NString(value) => value.as_ref().map_or(0, |_| -1),
            Self::Bytes(value) => value.as_ref().map_or(0, |_| -1),
            Self::Ref(value) => value.as_ref().map_or(0, |_| -1),
            Self::Blob(value) => value.as_ref().map_or(0, |_| -1),
            Self::Clob(value) => value.as_ref().map_or(0, |_| -1),
            Self::NClob(value) => value.as_ref().map_or(0, |_| -1),
            Self::Array(value) => value.as_ref().map_or(0, |_| -1),
            Self::Url(value) => value.as_ref().map_or(0, |_| -1),
            Self::RowId(value) => value.as_ref().map_or(0, |_| -1),
            Self::SqlXml(value) => value.as_ref().map_or(0, |_| -1),
            Self::RustValue(_)
            | Self::Boolean(_)
            | Self::Byte(_)
            | Self::Short(_)
            | Self::Float(_)
            | Self::Double(_) => -1,
        }
    }

    fn calendar(&self) -> Option<super::RdbcCalendar> {
        match self {
            Self::Date {
                value: Some(_),
                calendar: RdbcCalendarArgument::Specified(calendar),
            }
            | Self::Time {
                value: Some(_),
                calendar: RdbcCalendarArgument::Specified(calendar),
            }
            | Self::Timestamp {
                value: Some(_),
                calendar: RdbcCalendarArgument::Specified(calendar),
            } => calendar.clone(),
            _ => None,
        }
    }

    fn sql_type(&self) -> i32 {
        match self {
            Self::RustValue(_) => 1_111,
            Self::Null { sql_type, .. } => {
                if *sql_type == -6 {
                    4
                } else {
                    *sql_type
                }
            }
            Self::Boolean(_) => 16,
            Self::Byte(_) => -6,
            Self::Short(_) => 5,
            Self::Int(_) => 4,
            Self::Long(_) => -5,
            Self::Float(_) => 6,
            Self::Double(_) => 8,
            Self::BigDecimal(_) => 3,
            Self::String(_) => 12,
            Self::NString(_) => -9,
            Self::Bytes(_) => RdbcParameterType::BYTES,
            Self::Date { .. } => 91,
            Self::Time { .. } => 92,
            Self::Timestamp { .. } => 93,
            Self::AsciiStream { .. } => RdbcParameterType::ASCII_INPUT_STREAM,
            Self::UnicodeStream { .. } => RdbcParameterType::UNICODE_STREAM,
            Self::BinaryStream { .. } => RdbcParameterType::BINARY_INPUT_STREAM,
            Self::CharacterStream { .. } => RdbcParameterType::CHARACTER_INPUT_STREAM,
            Self::NCharacterStream { .. } => RdbcParameterType::NCHARACTER_INPUT_STREAM,
            Self::Object {
                target_sql_type, ..
            } => target_sql_type.unwrap_or(1_111),
            Self::Ref(_) => 2_006,
            Self::Blob(_) | Self::BlobStream { .. } => 2_004,
            Self::Clob(_) | Self::ClobReader { .. } => 2_005,
            Self::NClob(_) | Self::NClobReader { .. } => 2_011,
            Self::Array(_) => 2_003,
            Self::Url(_) => RdbcParameterType::URL,
            Self::RowId(_) => -8,
            Self::SqlXml(_) => 2_009,
        }
    }
}

fn stream_length(length: RdbcStreamLength) -> i64 {
    match length {
        RdbcStreamLength::Unspecified => -1,
        RdbcStreamLength::Int(length) => i64::from(length),
        RdbcStreamLength::Long(length) => length,
    }
}

fn character_length(length: RdbcCharacterLength) -> i64 {
    match length {
        RdbcCharacterLength::Unspecified => -1,
        RdbcCharacterLength::Int(length) => i64::from(length),
        RdbcCharacterLength::Long(length) => length,
    }
}
