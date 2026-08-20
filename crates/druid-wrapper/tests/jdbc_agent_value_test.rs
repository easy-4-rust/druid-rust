#![cfg(feature = "jdbc-agent")]

use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid_core::core::Value;
use druid_wrapper::jdbc_agent::AgentValue;
use std::str::FromStr;

#[test]
fn agent_values_round_trip_without_scalar_type_loss() {
    let values = vec![
        Value::Null,
        Value::Bool(true),
        Value::Int(42),
        Value::Float(3.5),
        Value::Decimal(BigDecimal::from_str("123456789.012300").expect("decimal")),
        Value::Date(NaiveDate::from_ymd_opt(2026, 8, 7).expect("date")),
        Value::Time(NaiveTime::from_hms_nano_opt(12, 34, 56, 789).expect("time")),
        Value::Timestamp(
            NaiveDateTime::parse_from_str("2026-08-07 12:34:56.123456789", "%F %T%.f")
                .expect("timestamp"),
        ),
        Value::String("数据库".to_owned()),
        Value::Bytes(vec![0, 1, 2, 254, 255]),
    ];

    for value in values {
        let encoded = AgentValue::from_druid(value.clone()).expect("必须可编码");
        let serialized = serde_json::to_vec(&encoded).expect("必须可序列化");
        let decoded: AgentValue = serde_json::from_slice(&serialized).expect("必须可反序列化");
        assert_eq!(decoded.into_druid().expect("必须可恢复"), value);
    }
}

#[test]
fn agent_protocol_rejects_non_finite_float() {
    assert!(AgentValue::from_druid(Value::Float(f64::NAN)).is_err());
    assert!(AgentValue::from_druid(Value::Float(f64::INFINITY)).is_err());
}
