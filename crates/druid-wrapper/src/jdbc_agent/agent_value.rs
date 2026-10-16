use base64::Engine;
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid_rdbc::error::RdbcError;
use druid_rdbc::types::RdbcObject;
use druid_rdbc::types::Value;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Druid 与 JVM 之间无损传递的标量值。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum AgentValue {
    Null,
    Bool(bool),
    Int(i64),
    Byte(i8),
    Short(i16),
    Integer(i32),
    Long(i64),
    Float(f64),
    Float32(f32),
    Double(f64),
    Decimal(String),
    Date(String),
    Time(String),
    Timestamp(String),
    String(String),
    Bytes(String),
}

impl AgentValue {
    /// 将保留具体 Java 数字包装类型的标准对象编码为 Array 元素。对应 Java: Connection#createArrayOf。
    pub fn from_object(value: RdbcObject) -> Result<Self, RdbcError> {
        match value {
            RdbcObject::Scalar(value) => Self::from_druid(value),
            RdbcObject::String(value) | RdbcObject::NString(value) => Ok(Self::String(value)),
            RdbcObject::Boolean(value) => Ok(Self::Bool(value)),
            RdbcObject::Byte(value) => Ok(Self::Byte(value)),
            RdbcObject::Short(value) => Ok(Self::Short(value)),
            RdbcObject::Integer(value) => Ok(Self::Integer(value)),
            RdbcObject::Long(value) => Ok(Self::Long(value)),
            RdbcObject::Float(value) if value.is_finite() => Ok(Self::Float32(value)),
            RdbcObject::Double(value) if value.is_finite() => Ok(Self::Double(value)),
            RdbcObject::Float(_) | RdbcObject::Double(_) => Err(RdbcError::InvalidArgument(
                "JDBC Agent protocol rejects non-finite Array elements".to_owned(),
            )),
            RdbcObject::Bytes(value) => Ok(Self::Bytes(
                base64::engine::general_purpose::STANDARD.encode(value),
            )),
            RdbcObject::BigDecimal(value) => Ok(Self::Decimal(value.to_string())),
            RdbcObject::Date(value) => Ok(Self::Date(value.format("%Y-%m-%d").to_string())),
            RdbcObject::Time(value) => Ok(Self::Time(value.format("%H:%M:%S%.f").to_string())),
            RdbcObject::Timestamp(value) => Ok(Self::Timestamp(
                value.format("%Y-%m-%dT%H:%M:%S%.f").to_string(),
            )),
            _ => Err(RdbcError::feature_not_supported(
                "jdbc_agent_create_array_of_complex_element",
            )),
        }
    }

    /// 将 Array 返回元素恢复为带 Java 包装类型身份的标准对象。对应 Java: Array#getArray。
    pub fn into_object(self) -> Result<RdbcObject, RdbcError> {
        match self {
            Self::Byte(value) => Ok(RdbcObject::Byte(value)),
            Self::Short(value) => Ok(RdbcObject::Short(value)),
            Self::Integer(value) => Ok(RdbcObject::Integer(value)),
            Self::Long(value) => Ok(RdbcObject::Long(value)),
            Self::Float32(value) if value.is_finite() => Ok(RdbcObject::Float(value)),
            Self::Double(value) if value.is_finite() => Ok(RdbcObject::Double(value)),
            Self::Float32(_) | Self::Double(_) => Err(Self::invalid("non-finite Array element")),
            other => other.into_druid().map(RdbcObject::Scalar),
        }
    }

    /// 从 Druid 标量创建协议值。
    pub fn from_druid(value: Value) -> Result<Self, RdbcError> {
        match value {
            Value::Null => Ok(Self::Null),
            Value::Bool(value) => Ok(Self::Bool(value)),
            Value::Int(value) => Ok(Self::Int(value)),
            Value::Float(value) if value.is_finite() => Ok(Self::Float(value)),
            Value::Float(_) => Err(RdbcError::InvalidArgument(
                "JDBC Agent protocol rejects non-finite floating-point values".to_owned(),
            )),
            Value::Decimal(value) => Ok(Self::Decimal(value.to_string())),
            Value::Date(value) => Ok(Self::Date(value.format("%Y-%m-%d").to_string())),
            Value::Time(value) => Ok(Self::Time(value.format("%H:%M:%S%.f").to_string())),
            Value::Timestamp(value) => Ok(Self::Timestamp(
                value.format("%Y-%m-%dT%H:%M:%S%.f").to_string(),
            )),
            Value::String(value) => Ok(Self::String(value)),
            Value::Bytes(value) => Ok(Self::Bytes(
                base64::engine::general_purpose::STANDARD.encode(value),
            )),
        }
    }

    /// 转换为 Druid 标量并严格校验日期、数值和 Base64。
    pub fn into_druid(self) -> Result<Value, RdbcError> {
        match self {
            Self::Null => Ok(Value::Null),
            Self::Bool(value) => Ok(Value::Bool(value)),
            Self::Int(value) | Self::Long(value) => Ok(Value::Int(value)),
            Self::Byte(value) => Ok(Value::Int(i64::from(value))),
            Self::Short(value) => Ok(Value::Int(i64::from(value))),
            Self::Integer(value) => Ok(Value::Int(i64::from(value))),
            Self::Float(value) | Self::Double(value) if value.is_finite() => {
                Ok(Value::Float(value))
            }
            Self::Float32(value) if value.is_finite() => Ok(Value::Float(f64::from(value))),
            Self::Float32(_) | Self::Double(_) | Self::Float(_) => {
                Err(Self::invalid("non-finite float"))
            }
            Self::Decimal(value) => BigDecimal::from_str(&value)
                .map(Value::Decimal)
                .map_err(|error| Self::invalid(error.to_string())),
            Self::Date(value) => NaiveDate::parse_from_str(&value, "%Y-%m-%d")
                .map(Value::Date)
                .map_err(|error| Self::invalid(error.to_string())),
            Self::Time(value) => NaiveTime::parse_from_str(&value, "%H:%M:%S%.f")
                .map(Value::Time)
                .map_err(|error| Self::invalid(error.to_string())),
            Self::Timestamp(value) => Self::parse_timestamp(&value).map(Value::Timestamp),
            Self::String(value) => Ok(Value::String(value)),
            Self::Bytes(value) => base64::engine::general_purpose::STANDARD
                .decode(value)
                .map(Value::Bytes)
                .map_err(|error| Self::invalid(error.to_string())),
        }
    }

    fn parse_timestamp(value: &str) -> Result<NaiveDateTime, RdbcError> {
        ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%dT%H:%M"]
            .into_iter()
            .find_map(|format| NaiveDateTime::parse_from_str(value, format).ok())
            .ok_or_else(|| Self::invalid(format!("invalid timestamp '{value}'")))
    }

    fn invalid(message: impl Into<String>) -> RdbcError {
        RdbcError::DriverError(format!(
            "invalid JDBC Agent scalar value: {}",
            message.into()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::AgentValue;
    use druid_rdbc::types::RdbcObject;

    #[test]
    fn array_elements_preserve_java_numeric_wrapper_identity() {
        let cases = [
            (RdbcObject::Byte(7), "byte"),
            (RdbcObject::Short(8), "short"),
            (RdbcObject::Integer(9), "integer"),
            (RdbcObject::Long(10), "long"),
            (RdbcObject::Float(1.5), "float32"),
            (RdbcObject::Double(2.5), "double"),
        ];
        for (object, kind) in cases {
            let encoded = AgentValue::from_object(object.clone()).expect("encode Array element");
            let json = serde_json::to_value(&encoded).expect("serialize Array element");
            assert_eq!(json["kind"], kind);
            let decoded: AgentValue =
                serde_json::from_value(json).expect("deserialize Array element");
            assert_eq!(decoded.into_object().expect("decode Array element"), object);
        }
    }

    #[test]
    fn array_elements_reject_unrepresentable_complex_objects() {
        let unsupported = RdbcObject::Url(druid_rdbc::types::RdbcUrl::new("https://example.com"));
        let error = AgentValue::from_object(unsupported).expect_err("URL is not a JDBC scalar");
        assert_eq!(
            error.sql_exception().and_then(|error| error.sql_state()),
            Some("0A000")
        );
    }
}
