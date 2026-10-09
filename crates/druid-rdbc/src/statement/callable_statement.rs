use crate::error::RdbcError;
use crate::statement::{ParameterIndex, PreparedStatement};
use crate::types::{
    RdbcArray, RdbcBigDecimal as BigDecimal, RdbcBlob, RdbcCalendarArgument, RdbcCharacterLength,
    RdbcClob, RdbcInputStream, RdbcNClob, RdbcNaiveDate as NaiveDate,
    RdbcNaiveDateTime as NaiveDateTime, RdbcNaiveTime as NaiveTime, RdbcObject, RdbcReader,
    RdbcRef, RdbcRowId, RdbcSqlXml, RdbcStreamLength, RdbcTargetType, RdbcTypeMap, RdbcUrl, Value,
};

/// Callable 参数定位；index 保持 JDBC 1-based，name 不允许为空。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CallableParameter {
    /// 1-based 参数索引。
    Index(ParameterIndex),
    /// 驱动识别的参数名。
    Name(String),
}

impl CallableParameter {
    /// 创建 index selector。
    pub fn by_index(index: usize) -> Result<Self, RdbcError> {
        Ok(Self::Index(ParameterIndex::new(index)?))
    }

    /// 创建 name selector。
    pub fn by_name(name: impl Into<String>) -> Result<Self, RdbcError> {
        let name = name.into();
        if name.is_empty() {
            Err(RdbcError::InvalidArgument(
                "callable parameter name is empty".to_owned(),
            ))
        } else {
            Ok(Self::Name(name))
        }
    }
}

/// Callable OUT 参数 SQL 类型描述。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallableOutParameter {
    sql_type: i32,
    scale: Option<i32>,
    type_name: Option<String>,
}

impl CallableOutParameter {
    /// 创建只包含 SQL type 的 OUT 描述。
    pub const fn new(sql_type: i32) -> Self {
        Self {
            sql_type,
            scale: None,
            type_name: None,
        }
    }

    /// 创建带 scale 的 OUT 描述。
    pub const fn with_scale(sql_type: i32, scale: i32) -> Self {
        Self {
            sql_type,
            scale: Some(scale),
            type_name: None,
        }
    }

    /// 创建带 vendor type name 的 OUT 描述。
    pub fn with_type_name(sql_type: i32, type_name: impl Into<String>) -> Self {
        Self {
            sql_type,
            scale: None,
            type_name: Some(type_name.into()),
        }
    }

    /// 返回 JDBC SQL type code。
    pub const fn sql_type(&self) -> i32 {
        self.sql_type
    }

    /// 返回可选 scale。
    pub const fn scale(&self) -> Option<i32> {
        self.scale
    }

    /// 返回可选 vendor type name。
    pub fn type_name(&self) -> Option<&str> {
        self.type_name.as_deref()
    }
}

/// Callable 命名 IN 参数及其精确 JDBC setter 语义。
///
/// 不同 variant 保留 setter 身份以及 SQL type、type name、scale、Calendar 和
/// stream/reader length，adapter 不得把这些信息静默压缩成一个标量值。
#[derive(Debug, Clone, PartialEq)]
pub enum CallableInputParameter {
    /// `setNull(parameterName, sqlType[, typeName])`。
    Null {
        /// JDBC SQL type code。
        sql_type: i32,
        /// 可选 vendor type name。
        type_name: Option<String>,
    },
    /// `setBoolean(parameterName, value)`。
    Boolean(bool),
    /// `setByte(parameterName, value)`。
    Byte(i8),
    /// `setShort(parameterName, value)`。
    Short(i16),
    /// `setInt(parameterName, value)`。
    Int(i32),
    /// `setLong(parameterName, value)`。
    Long(i64),
    /// `setFloat(parameterName, value)`。
    Float(f32),
    /// `setDouble(parameterName, value)`。
    Double(f64),
    /// `setString(parameterName, value)`；`None` 保持 SQL NULL。
    String(Option<String>),
    /// `setNString(parameterName, value)`；`None` 保持 SQL NULL。
    NString(Option<String>),
    /// `setBytes(parameterName, value)`；`None` 保持 SQL NULL。
    Bytes(Option<Vec<u8>>),
    /// `setURL(parameterName, value)`；`None` 保持 SQL NULL。
    Url(Option<RdbcUrl>),
    /// `setRowId(parameterName, value)`；`None` 保持 SQL NULL。
    RowId(Option<RdbcRowId>),
    /// `setSQLXML(parameterName, value)`；`None` 保持 SQL NULL。
    SqlXml(Option<RdbcSqlXml>),
    /// `setAsciiStream(parameterName, stream[, length])`。
    AsciiStream {
        /// 可空输入流。
        stream: Option<RdbcInputStream>,
        /// 无长度、int 或 long length 重载身份。
        length: RdbcStreamLength,
    },
    /// `setBinaryStream(parameterName, stream[, length])`。
    BinaryStream {
        /// 可空输入流。
        stream: Option<RdbcInputStream>,
        /// 无长度、int 或 long length 重载身份。
        length: RdbcStreamLength,
    },
    /// `setBlob(parameterName, value)`；`None` 保持 SQL NULL。
    Blob(Option<RdbcBlob>),
    /// `setBlob(parameterName, stream[, length])`。
    BlobStream {
        /// 可空输入流。
        stream: Option<RdbcInputStream>,
        /// 无长度或 long length 重载身份。
        length: RdbcStreamLength,
    },
    /// `setClob(parameterName, value)`；`None` 保持 SQL NULL。
    Clob(Option<RdbcClob>),
    /// `setClob(parameterName, reader[, length])`。
    ClobReader {
        /// 可空 reader。
        reader: Option<RdbcReader>,
        /// 无长度或 long length 重载身份。
        length: RdbcCharacterLength,
    },
    /// `setNClob(parameterName, value)`；`None` 保持 SQL NULL。
    NClob(Option<RdbcNClob>),
    /// `setNClob(parameterName, reader[, length])`。
    NClobReader {
        /// 可空 reader。
        reader: Option<RdbcReader>,
        /// 无长度或 long length 重载身份。
        length: RdbcCharacterLength,
    },
    /// `setCharacterStream(parameterName, reader[, length])`。
    CharacterStream {
        /// 可空 reader。
        reader: Option<RdbcReader>,
        /// 无长度、int 或 long length 重载身份。
        length: RdbcCharacterLength,
    },
    /// `setNCharacterStream(parameterName, reader[, length])`。
    NCharacterStream {
        /// 可空 reader。
        reader: Option<RdbcReader>,
        /// 无长度或 long length 重载身份。
        length: RdbcCharacterLength,
    },
    /// `setBigDecimal(parameterName, value)`；`None` 保持 SQL NULL。
    BigDecimal(Option<BigDecimal>),
    /// `setDate(parameterName, value[, calendar])`。
    Date {
        /// 可空日期值。
        value: Option<NaiveDate>,
        /// 未指定、显式 null 或实际 Calendar。
        calendar: RdbcCalendarArgument,
    },
    /// `setTime(parameterName, value[, calendar])`。
    Time {
        /// 可空时间值。
        value: Option<NaiveTime>,
        /// 未指定、显式 null 或实际 Calendar。
        calendar: RdbcCalendarArgument,
    },
    /// `setTimestamp(parameterName, value[, calendar])`。
    Timestamp {
        /// 可空时间戳值。
        value: Option<NaiveDateTime>,
        /// 未指定、显式 null 或实际 Calendar。
        calendar: RdbcCalendarArgument,
    },
    /// `setObject(parameterName, value[, targetSqlType[, scale]])`。
    Object {
        /// 原始 JDBC object 对应值。
        value: Value,
        /// 可选 target SQL type code。
        target_sql_type: Option<i32>,
        /// 可选 scale。
        scale: Option<i32>,
    },
}

impl CallableInputParameter {
    /// 创建 `setNull(String, int)` 描述。
    pub const fn null(sql_type: i32) -> Self {
        Self::Null {
            sql_type,
            type_name: None,
        }
    }

    /// 创建 `setNull(String, int, String)` 描述。
    pub fn null_with_type_name(sql_type: i32, type_name: impl Into<String>) -> Self {
        Self::Null {
            sql_type,
            type_name: Some(type_name.into()),
        }
    }

    /// 创建 `setObject(String, Object)` 描述。
    pub const fn object(value: Value) -> Self {
        Self::Object {
            value,
            target_sql_type: None,
            scale: None,
        }
    }

    /// 创建 `setObject(String, Object, int)` 描述。
    pub const fn object_with_sql_type(value: Value, target_sql_type: i32) -> Self {
        Self::Object {
            value,
            target_sql_type: Some(target_sql_type),
            scale: None,
        }
    }

    /// 创建 `setObject(String, Object, int, int)` 描述。
    pub const fn object_with_sql_type_and_scale(
        value: Value,
        target_sql_type: i32,
        scale: i32,
    ) -> Self {
        Self::Object {
            value,
            target_sql_type: Some(target_sql_type),
            scale: Some(scale),
        }
    }
}

/// JDBC `CallableStatement` 标准。
#[async_trait::async_trait]
pub trait CallableStatement: PreparedStatement {
    /// 在当前调用语句中校验参数索引并返回严格的 1-based selector。
    /// 参数 parameter_index 为原调用者索引；返回有效 selector 或原参数错误。
    /// 对应 Java: DruidPooledCallableStatement#index_parameter；池化实现记录所属语句错误。
    fn index_parameter(&self, parameter_index: usize) -> Result<CallableParameter, RdbcError> {
        CallableParameter::by_index(parameter_index)
    }

    /// 注册 indexed/named OUT 参数及其完整 SQL type 描述。
    fn register_out_parameter(
        &mut self,
        _parameter: CallableParameter,
        _out_parameter: CallableOutParameter,
    ) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "callable_statement_register_out_parameter",
        })
    }

    /// 设置命名 IN 参数。
    fn set_named_parameter(
        &mut self,
        _parameter_name: &str,
        _parameter: CallableInputParameter,
    ) -> Result<(), RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "callable_statement_set_named_parameter",
        })
    }

    /// 读取 indexed/named OUT object。
    fn out_parameter(&self, parameter: &CallableParameter) -> Result<RdbcObject, RdbcError> {
        match parameter {
            CallableParameter::Index(index) => self.out_value(*index).map(RdbcObject::from),
            CallableParameter::Name(_) => Err(RdbcError::UnsupportedOperation {
                operation: "callable_statement_named_out_parameter",
            }),
        }
    }

    /// 使用可空 type map 读取 OUT object；`None` 与空 map 保持不同语义。
    fn out_parameter_with_type_map(
        &self,
        _parameter: &CallableParameter,
        _type_map: Option<&RdbcTypeMap>,
    ) -> Result<RdbcObject, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "callable_statement_out_parameter_with_type_map",
        })
    }

    /// 使用 target type 读取 OUT object。
    fn out_parameter_as(
        &self,
        _parameter: &CallableParameter,
        _target_type: &RdbcTargetType,
    ) -> Result<RdbcObject, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "callable_statement_out_parameter_as",
        })
    }

    /// 兼容 index-only adapter 的 value core。
    fn out_value(&self, _index: ParameterIndex) -> Result<Value, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "callable_statement_out_value",
        })
    }

    /// 返回最近一次 OUT getter 是否读取 SQL NULL。
    fn was_null(&self) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "callable_statement_was_null",
        })
    }

    /// 读取 String OUT 参数。
    fn string_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<String>, RdbcError> {
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::Scalar(Value::String(value)) => Ok(Some(value)),
            other => Err(callable_type_error("String", &other)),
        }
    }

    /// 读取 national String OUT 参数。
    fn n_string_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<String>, RdbcError> {
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::NString(value) => Ok(Some(value)),
            other => Err(callable_type_error("NString", &other)),
        }
    }

    /// 读取 URL OUT 参数。
    fn url_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<RdbcUrl>, RdbcError> {
        optional_object(self.out_parameter(parameter)?, "URL", |value| match value {
            RdbcObject::Url(value) => Some(value),
            _ => None,
        })
    }

    /// 读取 Ref OUT 参数。
    fn ref_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<RdbcRef>, RdbcError> {
        optional_object(self.out_parameter(parameter)?, "Ref", |value| match value {
            RdbcObject::Ref(value) => Some(value),
            _ => None,
        })
    }

    /// 读取 Array OUT 参数。
    fn array_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<RdbcArray>, RdbcError> {
        optional_object(
            self.out_parameter(parameter)?,
            "Array",
            |value| match value {
                RdbcObject::Array(value) => Some(value),
                _ => None,
            },
        )
    }

    /// 读取 RowId OUT 参数。
    fn row_id_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<RdbcRowId>, RdbcError> {
        optional_object(
            self.out_parameter(parameter)?,
            "RowId",
            |value| match value {
                RdbcObject::RowId(value) => Some(value),
                _ => None,
            },
        )
    }

    /// 读取 SQLXML OUT 参数。
    fn sql_xml_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<RdbcSqlXml>, RdbcError> {
        optional_object(
            self.out_parameter(parameter)?,
            "SQLXML",
            |value| match value {
                RdbcObject::SqlXml(value) => Some(value),
                _ => None,
            },
        )
    }

    /// 读取 boolean OUT 参数；SQL NULL 返回 false。
    fn boolean_out_parameter(&self, parameter: &CallableParameter) -> Result<bool, RdbcError> {
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(false),
            RdbcObject::Scalar(Value::Bool(value)) => Ok(value),
            other => Err(callable_type_error("boolean", &other)),
        }
    }

    /// 读取 byte OUT 参数；超出 i8 范围返回 typed error。
    fn byte_out_parameter(&self, parameter: &CallableParameter) -> Result<i8, RdbcError> {
        i8::try_from(self.integer_out_parameter(parameter)?)
            .map_err(|_| RdbcError::DriverError("OUT parameter exceeds byte".to_owned()))
    }

    /// 读取 short OUT 参数。
    fn short_out_parameter(&self, parameter: &CallableParameter) -> Result<i16, RdbcError> {
        i16::try_from(self.integer_out_parameter(parameter)?)
            .map_err(|_| RdbcError::DriverError("OUT parameter exceeds short".to_owned()))
    }

    /// 读取 int OUT 参数。
    fn int_out_parameter(&self, parameter: &CallableParameter) -> Result<i32, RdbcError> {
        i32::try_from(self.integer_out_parameter(parameter)?)
            .map_err(|_| RdbcError::DriverError("OUT parameter exceeds int".to_owned()))
    }

    /// 读取 long OUT 参数。
    fn long_out_parameter(&self, parameter: &CallableParameter) -> Result<i64, RdbcError> {
        self.integer_out_parameter(parameter)
    }

    /// 读取 float OUT 参数；保持 Java narrowing conversion。
    #[allow(clippy::cast_possible_truncation)]
    fn float_out_parameter(&self, parameter: &CallableParameter) -> Result<f32, RdbcError> {
        Ok(self.double_out_parameter(parameter)? as f32)
    }

    /// 读取 double OUT 参数；SQL NULL 返回 0.0。
    fn double_out_parameter(&self, parameter: &CallableParameter) -> Result<f64, RdbcError> {
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(0.0),
            RdbcObject::Scalar(Value::Float(value)) => Ok(value),
            other => Err(callable_type_error("floating point", &other)),
        }
    }

    /// 读取 bytes OUT 参数。
    fn bytes_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<Vec<u8>>, RdbcError> {
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::Scalar(Value::Bytes(value)) => Ok(Some(value)),
            other => Err(callable_type_error("bytes", &other)),
        }
    }

    /// 读取 Blob OUT 参数。
    fn blob_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<RdbcBlob>, RdbcError> {
        optional_object(
            self.out_parameter(parameter)?,
            "Blob",
            |value| match value {
                RdbcObject::Blob(value) => Some(value),
                _ => None,
            },
        )
    }

    /// 读取 Clob OUT 参数。
    fn clob_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<RdbcClob>, RdbcError> {
        optional_object(
            self.out_parameter(parameter)?,
            "Clob",
            |value| match value {
                RdbcObject::Clob(value) => Some(value),
                _ => None,
            },
        )
    }

    /// 读取 NClob OUT 参数。
    fn n_clob_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<RdbcNClob>, RdbcError> {
        optional_object(
            self.out_parameter(parameter)?,
            "NClob",
            |value| match value {
                RdbcObject::NClob(value) => Some(value),
                _ => None,
            },
        )
    }

    /// 读取 character Reader OUT 参数。
    fn character_stream_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<RdbcReader>, RdbcError> {
        optional_object(
            self.out_parameter(parameter)?,
            "CharacterStream",
            |value| match value {
                RdbcObject::CharacterStream(value) => Some(value),
                _ => None,
            },
        )
    }

    /// 读取 national-character Reader OUT 参数。
    fn n_character_stream_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<RdbcReader>, RdbcError> {
        optional_object(
            self.out_parameter(parameter)?,
            "NCharacterStream",
            |value| match value {
                RdbcObject::NCharacterStream(value) => Some(value),
                _ => None,
            },
        )
    }

    /// 读取 arbitrary-precision decimal OUT 参数。
    fn big_decimal_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<BigDecimal>, RdbcError> {
        optional_object(
            self.out_parameter(parameter)?,
            "BigDecimal",
            |value| match value {
                RdbcObject::BigDecimal(value) => Some(value),
                _ => None,
            },
        )
    }

    /// 使用 legacy scale 投影 decimal OUT 参数。
    fn big_decimal_out_parameter_with_scale(
        &self,
        parameter: &CallableParameter,
        scale: i32,
    ) -> Result<Option<BigDecimal>, RdbcError> {
        Ok(self
            .big_decimal_out_parameter(parameter)?
            .map(|value| value.with_scale(i64::from(scale))))
    }

    /// 读取 Date OUT 参数并保留 Calendar argument identity。
    fn date_out_parameter(
        &self,
        parameter: &CallableParameter,
        _calendar: &RdbcCalendarArgument,
    ) -> Result<Option<NaiveDate>, RdbcError> {
        optional_object(
            self.out_parameter(parameter)?,
            "Date",
            |value| match value {
                RdbcObject::Date(value) => Some(value),
                _ => None,
            },
        )
    }

    /// 读取 Time OUT 参数并保留 Calendar argument identity。
    fn time_out_parameter(
        &self,
        parameter: &CallableParameter,
        _calendar: &RdbcCalendarArgument,
    ) -> Result<Option<NaiveTime>, RdbcError> {
        optional_object(
            self.out_parameter(parameter)?,
            "Time",
            |value| match value {
                RdbcObject::Time(value) => Some(value),
                _ => None,
            },
        )
    }

    /// 读取 Timestamp OUT 参数并保留 Calendar argument identity。
    fn timestamp_out_parameter(
        &self,
        parameter: &CallableParameter,
        _calendar: &RdbcCalendarArgument,
    ) -> Result<Option<NaiveDateTime>, RdbcError> {
        optional_object(
            self.out_parameter(parameter)?,
            "Timestamp",
            |value| match value {
                RdbcObject::Timestamp(value) => Some(value),
                _ => None,
            },
        )
    }

    /// 把通用 OUT object 投影为 integer。
    fn integer_out_parameter(&self, parameter: &CallableParameter) -> Result<i64, RdbcError> {
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(0),
            RdbcObject::Scalar(Value::Int(value)) => Ok(value),
            other => Err(callable_type_error("integer", &other)),
        }
    }
}

fn optional_object<T>(
    object: RdbcObject,
    expected: &str,
    convert: impl FnOnce(RdbcObject) -> Option<T>,
) -> Result<Option<T>, RdbcError> {
    if std::matches!(object, RdbcObject::Scalar(Value::Null)) {
        return Ok(None);
    }
    convert(object.clone())
        .map(Some)
        .ok_or_else(|| callable_type_error(expected, &object))
}

fn callable_type_error(expected: &str, actual: &RdbcObject) -> RdbcError {
    RdbcError::DriverError(format!(
        "CallableStatement expected {expected} OUT parameter, got {actual}"
    ))
}
