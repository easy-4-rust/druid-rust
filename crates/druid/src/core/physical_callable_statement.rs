//! 物理 `CallableStatement` SPI。
//!
//! 对应 Java 平台依赖：`java.sql.CallableStatement`。

use super::{
    CallableCalendarArgument, CallableInputParameter, CallableOutParameter, CallableParameter,
    DruidError, PhysicalPreparedStatement, RdbcArray, RdbcBlob, RdbcClob, RdbcNClob, RdbcObject,
    RdbcReader, RdbcRef, RdbcRowId, RdbcSqlXml, RdbcTargetType, RdbcTypeMap, RdbcUrl, Value,
};
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

/// 驱动存储过程调用句柄。
///
/// Druid wrapper 保留索引/名称重载、错误计数和缓存生命周期；具体驱动负责参数
/// 注册、类型转换、`wasNull` 与 OUT 值读取。未支持 `CallableStatement` 的 Adapter
/// 必须返回结构化 `UnsupportedOperation`，不得把普通查询伪装成存储过程调用。
pub trait PhysicalCallableStatement: PhysicalPreparedStatement {
    /// 注册 OUT 参数。
    fn register_out_parameter(
        &self,
        parameter: CallableParameter,
        out_parameter: CallableOutParameter,
    ) -> Result<(), DruidError>;

    /// 设置命名 IN 参数。
    ///
    /// 参数 `parameter` 保留 Java setter 身份以及 `sqlType/typeName/scale`，
    /// 具体驱动不得把这些信息静默丢弃。
    fn set_named_parameter(
        &self,
        parameter_name: &str,
        parameter: CallableInputParameter,
    ) -> Result<(), DruidError>;

    /// 读取 OUT 参数值。
    fn out_parameter(&self, parameter: &CallableParameter) -> Result<RdbcObject, DruidError>;

    /// 使用 Java `Map<String, Class<?>>` 类型映射读取 OUT 参数。
    ///
    /// `None` 精确保留 Java null Map，不等价于空 Map。驱动不支持时必须显式报错，
    /// 不得退化为无 Map 的 `getObject`。
    fn out_parameter_with_type_map(
        &self,
        _parameter: &CallableParameter,
        _type_map: Option<&RdbcTypeMap>,
    ) -> Result<RdbcObject, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "callable_out_parameter_with_type_map",
        })
    }

    /// 使用 Java `Class<T>` 对应目标类型读取 OUT 参数。
    fn out_parameter_as(
        &self,
        _parameter: &CallableParameter,
        _target_type: &RdbcTargetType,
    ) -> Result<RdbcObject, DruidError> {
        Err(DruidError::UnsupportedOperation {
            operation: "callable_out_parameter_as",
        })
    }

    /// 返回最近一次 OUT 参数读取是否得到 SQL NULL。
    fn was_null(&self) -> Result<bool, DruidError>;

    /// 读取字符串 OUT 参数。
    ///
    /// 对应 Java：`CallableStatement#getString(int/String)`。
    fn string_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<String>, DruidError> {
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::Scalar(Value::String(value)) => Ok(Some(value)),
            other => Err(callable_type_error("String", &other)),
        }
    }

    /// 读取 national-character 字符串 OUT 参数。
    ///
    /// 对应 Java：`CallableStatement#getNString(int/String)`。
    fn n_string_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<String>, DruidError> {
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
    ) -> Result<Option<RdbcUrl>, DruidError> {
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::Url(value) => Ok(Some(value)),
            other => Err(callable_type_error("URL", &other)),
        }
    }

    /// 读取 `Ref` OUT 参数。
    fn ref_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<RdbcRef>, DruidError> {
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::Ref(value) => Ok(Some(value)),
            other => Err(callable_type_error("Ref", &other)),
        }
    }

    /// 读取 `Array` OUT 参数。
    fn array_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<RdbcArray>, DruidError> {
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::Array(value) => Ok(Some(value)),
            other => Err(callable_type_error("Array", &other)),
        }
    }

    /// 读取 `RowId` OUT 参数。
    fn row_id_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<RdbcRowId>, DruidError> {
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::RowId(value) => Ok(Some(value)),
            other => Err(callable_type_error("RowId", &other)),
        }
    }

    /// 读取 `SQLXML` OUT 参数。
    fn sql_xml_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<RdbcSqlXml>, DruidError> {
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::SqlXml(value) => Ok(Some(value)),
            other => Err(callable_type_error("SQLXML", &other)),
        }
    }

    /// 读取布尔 OUT 参数；SQL NULL 返回 `false`。
    ///
    /// 对应 Java：`CallableStatement#getBoolean(int/String)`。
    fn boolean_out_parameter(&self, parameter: &CallableParameter) -> Result<bool, DruidError> {
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(false),
            RdbcObject::Scalar(Value::Bool(value)) => Ok(value),
            other => Err(callable_type_error("boolean", &other)),
        }
    }

    /// 读取 byte OUT 参数；SQL NULL 返回 `0`。
    ///
    /// 对应 Java：`CallableStatement#getByte(int/String)`。
    fn byte_out_parameter(&self, parameter: &CallableParameter) -> Result<i8, DruidError> {
        let value = self.integer_out_parameter(parameter)?;
        i8::try_from(value)
            .map_err(|_| DruidError::DriverError("OUT parameter exceeds byte".to_string()))
    }

    /// 读取 short OUT 参数；SQL NULL 返回 `0`。
    ///
    /// 对应 Java：`CallableStatement#getShort(int/String)`。
    fn short_out_parameter(&self, parameter: &CallableParameter) -> Result<i16, DruidError> {
        let value = self.integer_out_parameter(parameter)?;
        i16::try_from(value)
            .map_err(|_| DruidError::DriverError("OUT parameter exceeds short".to_string()))
    }

    /// 读取 int OUT 参数；SQL NULL 返回 `0`。
    ///
    /// 对应 Java：`CallableStatement#getInt(int/String)`。
    fn int_out_parameter(&self, parameter: &CallableParameter) -> Result<i32, DruidError> {
        let value = self.integer_out_parameter(parameter)?;
        i32::try_from(value)
            .map_err(|_| DruidError::DriverError("OUT parameter exceeds int".to_string()))
    }

    /// 读取 long OUT 参数；SQL NULL 返回 `0`。
    ///
    /// 对应 Java：`CallableStatement#getLong(int/String)`。
    fn long_out_parameter(&self, parameter: &CallableParameter) -> Result<i64, DruidError> {
        self.integer_out_parameter(parameter)
    }

    /// 读取 float OUT 参数；SQL NULL 返回 `0.0`。
    ///
    /// 对应 Java：`CallableStatement#getFloat(int/String)`。
    #[allow(clippy::cast_possible_truncation)] // Java `getFloat` 定义为向 f32 收窄。
    fn float_out_parameter(&self, parameter: &CallableParameter) -> Result<f32, DruidError> {
        Ok(self.double_out_parameter(parameter)? as f32)
    }

    /// 读取 double OUT 参数；SQL NULL 返回 `0.0`。
    ///
    /// 对应 Java：`CallableStatement#getDouble(int/String)`。
    fn double_out_parameter(&self, parameter: &CallableParameter) -> Result<f64, DruidError> {
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(0.0),
            RdbcObject::Scalar(Value::Float(value)) => Ok(value),
            other => Err(callable_type_error("floating point", &other)),
        }
    }

    /// 读取字节数组 OUT 参数。
    ///
    /// 对应 Java：`CallableStatement#getBytes(int/String)`。
    fn bytes_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<Vec<u8>>, DruidError> {
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::Scalar(Value::Bytes(value)) => Ok(Some(value)),
            other => Err(callable_type_error("bytes", &other)),
        }
    }

    /// 读取 Blob OUT 参数。
    ///
    /// 对应 Java：`CallableStatement#getBlob(int/String)`。SQL NULL 返回 `None`，
    /// 非 Blob 值按驱动类型错误处理。
    fn blob_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<RdbcBlob>, DruidError> {
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::Blob(value) => Ok(Some(value)),
            other => Err(callable_type_error("Blob", &other)),
        }
    }

    /// 读取 Clob OUT 参数。
    ///
    /// 对应 Java：`CallableStatement#getClob(int/String)`。
    fn clob_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<RdbcClob>, DruidError> {
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::Clob(value) => Ok(Some(value)),
            other => Err(callable_type_error("Clob", &other)),
        }
    }

    /// 读取 `NClob` OUT 参数。
    ///
    /// 对应 Java：`CallableStatement#getNClob(int/String)`。
    fn n_clob_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<RdbcNClob>, DruidError> {
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::NClob(value) => Ok(Some(value)),
            other => Err(callable_type_error("NClob", &other)),
        }
    }

    /// 读取普通字符 Reader OUT 参数。
    ///
    /// 对应 Java：`CallableStatement#getCharacterStream(int/String)`。
    fn character_stream_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<RdbcReader>, DruidError> {
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::CharacterStream(value) => Ok(Some(value)),
            other => Err(callable_type_error("CharacterStream", &other)),
        }
    }

    /// 读取 national character Reader OUT 参数。
    ///
    /// 对应 Java：`CallableStatement#getNCharacterStream(int/String)`。
    fn n_character_stream_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<RdbcReader>, DruidError> {
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::NCharacterStream(value) => Ok(Some(value)),
            other => Err(callable_type_error("NCharacterStream", &other)),
        }
    }

    /// 读取任意精度 Decimal OUT 参数。
    ///
    /// 对应 Java：`CallableStatement#getBigDecimal(int/String)`。
    fn big_decimal_out_parameter(
        &self,
        parameter: &CallableParameter,
    ) -> Result<Option<BigDecimal>, DruidError> {
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::BigDecimal(value) => Ok(Some(value)),
            other => Err(callable_type_error("BigDecimal", &other)),
        }
    }

    /// 使用已废弃 RDBC scale 重载读取 Decimal OUT 参数。
    ///
    /// 对应 Java：`CallableStatement#getBigDecimal(int, int)`。真实驱动可覆盖该
    /// 方法以使用自身舍入规则；默认实现保持数值并调整 scale。
    fn big_decimal_out_parameter_with_scale(
        &self,
        parameter: &CallableParameter,
        scale: i32,
    ) -> Result<Option<BigDecimal>, DruidError> {
        Ok(self
            .big_decimal_out_parameter(parameter)?
            .map(|value| value.with_scale(i64::from(scale))))
    }

    /// 读取 Date OUT 参数。
    ///
    /// 对应 Java：`CallableStatement#getDate(int/String[, Calendar])`。
    fn date_out_parameter(
        &self,
        parameter: &CallableParameter,
        _calendar: &CallableCalendarArgument,
    ) -> Result<Option<NaiveDate>, DruidError> {
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::Date(value) => Ok(Some(value)),
            other => Err(callable_type_error("Date", &other)),
        }
    }

    /// 读取 Time OUT 参数。
    ///
    /// 对应 Java：`CallableStatement#getTime(int/String[, Calendar])`。
    fn time_out_parameter(
        &self,
        parameter: &CallableParameter,
        _calendar: &CallableCalendarArgument,
    ) -> Result<Option<NaiveTime>, DruidError> {
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::Time(value) => Ok(Some(value)),
            other => Err(callable_type_error("Time", &other)),
        }
    }

    /// 读取 Timestamp OUT 参数。
    ///
    /// 对应 Java：`CallableStatement#getTimestamp(int/String[, Calendar])`。
    fn timestamp_out_parameter(
        &self,
        parameter: &CallableParameter,
        _calendar: &CallableCalendarArgument,
    ) -> Result<Option<NaiveDateTime>, DruidError> {
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(None),
            RdbcObject::Timestamp(value) => Ok(Some(value)),
            other => Err(callable_type_error("Timestamp", &other)),
        }
    }

    /// 将通用 OUT 值读取为整数。
    fn integer_out_parameter(&self, parameter: &CallableParameter) -> Result<i64, DruidError> {
        match self.out_parameter(parameter)? {
            RdbcObject::Scalar(Value::Null) => Ok(0),
            RdbcObject::Scalar(Value::Int(value)) => Ok(value),
            other => Err(callable_type_error("integer", &other)),
        }
    }
}

fn callable_type_error(expected: &str, actual: &RdbcObject) -> DruidError {
    DruidError::DriverError(format!(
        "CallableStatement expected {expected} OUT parameter, got {actual}"
    ))
}
