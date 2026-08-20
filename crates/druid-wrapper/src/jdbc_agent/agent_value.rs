use base64::Engine;
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid_core::core::{DruidError, Value};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Druid 与 JVM 之间无损传递的标量值。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum AgentValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Decimal(String),
    Date(String),
    Time(String),
    Timestamp(String),
    String(String),
    Bytes(String),
}

impl AgentValue {
    /// 从 Druid 标量创建协议值。
    pub fn from_druid(value: Value) -> Result<Self, DruidError> {
        match value {
            Value::Null => Ok(Self::Null),
            Value::Bool(value) => Ok(Self::Bool(value)),
            Value::Int(value) => Ok(Self::Int(value)),
            Value::Float(value) if value.is_finite() => Ok(Self::Float(value)),
            Value::Float(_) => Err(DruidError::InvalidArgument(
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
    pub fn into_druid(self) -> Result<Value, DruidError> {
        match self {
            Self::Null => Ok(Value::Null),
            Self::Bool(value) => Ok(Value::Bool(value)),
            Self::Int(value) => Ok(Value::Int(value)),
            Self::Float(value) if value.is_finite() => Ok(Value::Float(value)),
            Self::Float(_) => Err(Self::invalid("non-finite float")),
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

    fn parse_timestamp(value: &str) -> Result<NaiveDateTime, DruidError> {
        ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%dT%H:%M"]
            .into_iter()
            .find_map(|format| NaiveDateTime::parse_from_str(value, format).ok())
            .ok_or_else(|| Self::invalid(format!("invalid timestamp '{value}'")))
    }

    fn invalid(message: impl Into<String>) -> DruidError {
        DruidError::DriverError(format!(
            "invalid JDBC Agent scalar value: {}",
            message.into()
        ))
    }
}
