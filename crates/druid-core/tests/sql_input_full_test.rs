#![allow(clippy::approx_constant)]
//! Comprehensive tests for `SqlInput` (Java `SQLInput` 语义对照)。
//!
//! Java 基线：`33824c3dec1612711f9bb4e409319bcab2e4cd0e`。
//! 目标：覆盖 `sql_input.rs` 全部公共方法，包括 `read_value` 耗尽、
//! 各类型读取、NULL `行为、类型转换错误、read_url` 等路径。

extern crate druid_core as druid;
use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid_core::core::Value;
use druid_core::sql::SqlInput;

// ══════════════════════════════════════════════════════════════════
// 1. new + read_value 基础路径
// ══════════════════════════════════════════════════════════════════

/// new 构造 + `read_value` 顺序消费。
#[test]
fn sql_input_new_and_read_value() {
    let mut input = SqlInput::new(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
    assert_eq!(input.read_value().unwrap(), Value::Int(1));
    assert_eq!(input.read_value().unwrap(), Value::Int(2));
    assert_eq!(input.read_value().unwrap(), Value::Int(3));
}

/// `read_value` 耗尽后返回 `InvalidArgument`。
#[test]
fn sql_input_read_value_exhaustion() {
    let mut input = SqlInput::new(vec![Value::Int(1)]);
    let _ = input.read_value().unwrap();
    let result = input.read_value();
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err}").contains("no remaining attributes"));
}

/// 空 values 列表立即耗尽。
#[test]
fn sql_input_empty_values_exhaustion() {
    let mut input = SqlInput::new(vec![]);
    assert!(input.read_value().is_err());
}

// ══════════════════════════════════════════════════════════════════
// 2. was_null
// ══════════════════════════════════════════════════════════════════

/// 初始 `was_null` 为 false。
#[test]
fn sql_input_initial_was_null_false() {
    let input = SqlInput::new(vec![]);
    assert!(!input.was_null());
}

/// 读取非 NULL 值后 `was_null` 为 false。
#[test]
fn sql_input_was_null_false_for_non_null() {
    let mut input = SqlInput::new(vec![Value::Int(42)]);
    let _ = input.read_value().unwrap();
    assert!(!input.was_null());
}

/// 读取 NULL 值后 `was_null` 为 true。
#[test]
fn sql_input_was_null_true_for_null() {
    let mut input = SqlInput::new(vec![Value::Null]);
    let _ = input.read_value().unwrap();
    assert!(input.was_null());
}

/// 连续读取不同 NULL 状态。
#[test]
fn sql_input_was_null_alternating() {
    let mut input = SqlInput::new(vec![Value::Null, Value::Int(1), Value::Null]);
    let _ = input.read_value().unwrap();
    assert!(input.was_null());
    let _ = input.read_value().unwrap();
    assert!(!input.was_null());
    let _ = input.read_value().unwrap();
    assert!(input.was_null());
}

// ══════════════════════════════════════════════════════════════════
// 3. read_boolean
// ══════════════════════════════════════════════════════════════════

/// Bool 值正常读取。
#[test]
fn read_boolean_true() {
    let mut input = SqlInput::new(vec![Value::Bool(true)]);
    assert!(input.read_boolean().unwrap());
    assert!(!input.was_null());
}

#[test]
fn read_boolean_false() {
    let mut input = SqlInput::new(vec![Value::Bool(false)]);
    assert!(!input.read_boolean().unwrap());
}

/// NULL → false + `was_null=true`。
#[test]
fn read_boolean_null() {
    let mut input = SqlInput::new(vec![Value::Null]);
    assert!(!input.read_boolean().unwrap());
    assert!(input.was_null());
}

/// 非 Bool 类型 → 转换错误。
#[test]
fn read_boolean_type_mismatch() {
    let mut input = SqlInput::new(vec![Value::Int(1)]);
    assert!(input.read_boolean().is_err());
}

// ══════════════════════════════════════════════════════════════════
// 4. read_long
// ══════════════════════════════════════════════════════════════════

/// Int 值正常读取。
#[test]
fn read_long_normal() {
    let mut input = SqlInput::new(vec![Value::Int(42)]);
    assert_eq!(input.read_long().unwrap(), 42);
}

/// NULL → 0 + `was_null=true`。
#[test]
fn read_long_null() {
    let mut input = SqlInput::new(vec![Value::Null]);
    assert_eq!(input.read_long().unwrap(), 0);
    assert!(input.was_null());
}

/// 非 Int 类型 → 转换错误。
#[test]
fn read_long_type_mismatch() {
    let mut input = SqlInput::new(vec![Value::String("abc".to_owned())]);
    assert!(input.read_long().is_err());
}

// ══════════════════════════════════════════════════════════════════
// 5. read_byte
// ══════════════════════════════════════════════════════════════════

/// i8 范围内正常读取。
#[test]
fn read_byte_normal() {
    let mut input = SqlInput::new(vec![Value::Int(127)]);
    assert_eq!(input.read_byte().unwrap(), 127);
}

/// NULL → 0。
#[test]
fn read_byte_null() {
    let mut input = SqlInput::new(vec![Value::Null]);
    assert_eq!(input.read_byte().unwrap(), 0);
}

/// 超出 i8 范围 → 转换错误。
#[test]
fn read_byte_overflow() {
    let mut input = SqlInput::new(vec![Value::Int(200)]);
    assert!(input.read_byte().is_err());
}

/// 负值在 i8 范围内。
#[test]
fn read_byte_negative() {
    let mut input = SqlInput::new(vec![Value::Int(-1)]);
    assert_eq!(input.read_byte().unwrap(), -1);
}

// ══════════════════════════════════════════════════════════════════
// 6. read_short
// ══════════════════════════════════════════════════════════════════

/// i16 范围内正常读取。
#[test]
fn read_short_normal() {
    let mut input = SqlInput::new(vec![Value::Int(1000)]);
    assert_eq!(input.read_short().unwrap(), 1000);
}

/// NULL → 0。
#[test]
fn read_short_null() {
    let mut input = SqlInput::new(vec![Value::Null]);
    assert_eq!(input.read_short().unwrap(), 0);
}

/// 超出 i16 范围 → 转换错误。
#[test]
fn read_short_overflow() {
    let mut input = SqlInput::new(vec![Value::Int(40000)]);
    assert!(input.read_short().is_err());
}

// ══════════════════════════════════════════════════════════════════
// 7. read_int
// ══════════════════════════════════════════════════════════════════

/// i32 范围内正常读取。
#[test]
fn read_int_normal() {
    let mut input = SqlInput::new(vec![Value::Int(100000)]);
    assert_eq!(input.read_int().unwrap(), 100000);
}

/// NULL → 0。
#[test]
fn read_int_null() {
    let mut input = SqlInput::new(vec![Value::Null]);
    assert_eq!(input.read_int().unwrap(), 0);
}

/// 超出 i32 范围 → 转换错误。
#[test]
fn read_int_overflow() {
    let mut input = SqlInput::new(vec![Value::Int(i64::from(i32::MAX) + 1)]);
    assert!(input.read_int().is_err());
}

// ══════════════════════════════════════════════════════════════════
// 8. read_double
// ══════════════════════════════════════════════════════════════════

/// Float 值正常读取。
#[test]
fn read_double_from_float() {
    let mut input = SqlInput::new(vec![Value::Float(3.14)]);
    let v = input.read_double().unwrap();
    assert!((v - 3.14).abs() < f64::EPSILON);
}

/// Int → f64 转换。
#[test]
fn read_double_from_int() {
    let mut input = SqlInput::new(vec![Value::Int(42)]);
    let v = input.read_double().unwrap();
    assert!((v - 42.0).abs() < f64::EPSILON);
}

/// NULL → 0.0。
#[test]
fn read_double_null() {
    let mut input = SqlInput::new(vec![Value::Null]);
    assert_eq!(input.read_double().unwrap(), 0.0);
}

/// 非数值类型 → 转换错误。
#[test]
fn read_double_type_mismatch() {
    let mut input = SqlInput::new(vec![Value::String("abc".to_owned())]);
    assert!(input.read_double().is_err());
}

// ══════════════════════════════════════════════════════════════════
// 9. read_float
// ══════════════════════════════════════════════════════════════════

/// Float 值正常读取。
#[test]
fn read_float_normal() {
    let mut input = SqlInput::new(vec![Value::Float(1.5)]);
    let v = input.read_float().unwrap();
    assert!((v - 1.5_f32).abs() < f32::EPSILON);
}

/// NULL → 0.0。
#[test]
fn read_float_null() {
    let mut input = SqlInput::new(vec![Value::Null]);
    assert_eq!(input.read_float().unwrap(), 0.0);
}

// ══════════════════════════════════════════════════════════════════
// 10. read_big_decimal
// ══════════════════════════════════════════════════════════════════

/// Decimal 值正常读取。
#[test]
fn read_big_decimal_normal() {
    let bd = BigDecimal::from(12345);
    let mut input = SqlInput::new(vec![Value::Decimal(bd.clone())]);
    let result = input.read_big_decimal().unwrap();
    assert_eq!(result.unwrap(), bd);
}

/// Int → `BigDecimal` 转换。
#[test]
fn read_big_decimal_from_int() {
    let mut input = SqlInput::new(vec![Value::Int(42)]);
    let result = input.read_big_decimal().unwrap();
    assert_eq!(result.unwrap(), BigDecimal::from(42));
}

/// NULL → None。
#[test]
fn read_big_decimal_null() {
    let mut input = SqlInput::new(vec![Value::Null]);
    assert!(input.read_big_decimal().unwrap().is_none());
}

/// 非 Decimal/Int 类型 → 转换错误。
#[test]
fn read_big_decimal_type_mismatch() {
    let mut input = SqlInput::new(vec![Value::String("abc".to_owned())]);
    assert!(input.read_big_decimal().is_err());
}

// ══════════════════════════════════════════════════════════════════
// 11. read_string
// ══════════════════════════════════════════════════════════════════

/// String 值正常读取。
#[test]
fn read_string_normal() {
    let mut input = SqlInput::new(vec![Value::String("hello".to_owned())]);
    let result = input.read_string().unwrap();
    assert_eq!(result.unwrap(), "hello");
}

/// NULL → None。
#[test]
fn read_string_null() {
    let mut input = SqlInput::new(vec![Value::Null]);
    assert!(input.read_string().unwrap().is_none());
}

/// 非 String 类型 → 转换错误。
#[test]
fn read_string_type_mismatch() {
    let mut input = SqlInput::new(vec![Value::Int(42)]);
    assert!(input.read_string().is_err());
}

// ══════════════════════════════════════════════════════════════════
// 12. read_bytes
// ══════════════════════════════════════════════════════════════════

/// Bytes 值正常读取。
#[test]
fn read_bytes_normal() {
    let bytes = vec![1u8, 2, 3];
    let mut input = SqlInput::new(vec![Value::Bytes(bytes.clone())]);
    let result = input.read_bytes().unwrap();
    assert_eq!(result.unwrap(), bytes);
}

/// NULL → None。
#[test]
fn read_bytes_null() {
    let mut input = SqlInput::new(vec![Value::Null]);
    assert!(input.read_bytes().unwrap().is_none());
}

/// 非 Bytes 类型 → 转换错误。
#[test]
fn read_bytes_type_mismatch() {
    let mut input = SqlInput::new(vec![Value::Int(42)]);
    assert!(input.read_bytes().is_err());
}

// ══════════════════════════════════════════════════════════════════
// 13. read_date
// ══════════════════════════════════════════════════════════════════

/// Date 值正常读取。
#[test]
fn read_date_normal() {
    let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
    let mut input = SqlInput::new(vec![Value::Date(date)]);
    let result = input.read_date().unwrap();
    assert_eq!(result.unwrap(), date);
}

/// NULL → None。
#[test]
fn read_date_null() {
    let mut input = SqlInput::new(vec![Value::Null]);
    assert!(input.read_date().unwrap().is_none());
}

/// 非 Date 类型 → 转换错误。
#[test]
fn read_date_type_mismatch() {
    let mut input = SqlInput::new(vec![Value::Int(42)]);
    assert!(input.read_date().is_err());
}

// ══════════════════════════════════════════════════════════════════
// 14. read_time
// ══════════════════════════════════════════════════════════════════

/// Time 值正常读取。
#[test]
fn read_time_normal() {
    let time = NaiveTime::from_hms_milli_opt(14, 30, 0, 500).unwrap();
    let mut input = SqlInput::new(vec![Value::Time(time)]);
    let result = input.read_time().unwrap();
    assert_eq!(result.unwrap(), time);
}

/// NULL → None。
#[test]
fn read_time_null() {
    let mut input = SqlInput::new(vec![Value::Null]);
    assert!(input.read_time().unwrap().is_none());
}

/// 非 Time 类型 → 转换错误。
#[test]
fn read_time_type_mismatch() {
    let mut input = SqlInput::new(vec![Value::Int(42)]);
    assert!(input.read_time().is_err());
}

// ══════════════════════════════════════════════════════════════════
// 15. read_timestamp
// ══════════════════════════════════════════════════════════════════

/// Timestamp 值正常读取。
#[test]
fn read_timestamp_normal() {
    let ts = NaiveDateTime::parse_from_str("2024-01-15 14:30:00", "%Y-%m-%d %H:%M:%S").unwrap();
    let mut input = SqlInput::new(vec![Value::Timestamp(ts)]);
    let result = input.read_timestamp().unwrap();
    assert_eq!(result.unwrap(), ts);
}

/// NULL → None。
#[test]
fn read_timestamp_null() {
    let mut input = SqlInput::new(vec![Value::Null]);
    assert!(input.read_timestamp().unwrap().is_none());
}

/// 非 Timestamp 类型 → 转换错误。
#[test]
fn read_timestamp_type_mismatch() {
    let mut input = SqlInput::new(vec![Value::Int(42)]);
    assert!(input.read_timestamp().is_err());
}

// ══════════════════════════════════════════════════════════════════
// 16. read_object
// ══════════════════════════════════════════════════════════════════

/// `read_object` 返回任意 Value。
#[test]
fn read_object_returns_value() {
    let mut input = SqlInput::new(vec![Value::Int(42)]);
    let v = input.read_object().unwrap();
    assert_eq!(v, Value::Int(42));
}

/// `read_object` NULL。
#[test]
fn read_object_null() {
    let mut input = SqlInput::new(vec![Value::Null]);
    let v = input.read_object().unwrap();
    assert_eq!(v, Value::Null);
}

// ══════════════════════════════════════════════════════════════════
// 17. read_url
// ══════════════════════════════════════════════════════════════════

/// 合法 URL 字符串。
#[test]
fn read_url_valid() {
    let mut input = SqlInput::new(vec![Value::String("https://example.com/path".to_owned())]);
    let result = input.read_url().unwrap();
    let url = result.unwrap();
    assert_eq!(url.host_str(), Some("example.com"));
    assert_eq!(url.path(), "/path");
}

/// NULL → None。
#[test]
fn read_url_null() {
    let mut input = SqlInput::new(vec![Value::Null]);
    assert!(input.read_url().unwrap().is_none());
}

/// 非法 URL → 转换错误。
#[test]
fn read_url_invalid() {
    let mut input = SqlInput::new(vec![Value::String("not a url".to_owned())]);
    let result = input.read_url();
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(format!("{err}").contains("DATALINK"));
}

/// 非 String 类型 → `read_string` 先报错。
#[test]
fn read_url_type_mismatch() {
    let mut input = SqlInput::new(vec![Value::Int(42)]);
    assert!(input.read_url().is_err());
}

// ══════════════════════════════════════════════════════════════════
// 18. 转换错误消息格式
// ══════════════════════════════════════════════════════════════════

/// 转换错误包含目标类型和值信息。
#[test]
fn conversion_error_message_format() {
    let mut input = SqlInput::new(vec![Value::Bool(true)]);
    let result = input.read_long();
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("BIGINT"));
}

/// 转换错误包含 `SQLDataException` 类名。
#[test]
fn conversion_error_class_name() {
    let mut input = SqlInput::new(vec![Value::String("x".to_owned())]);
    let result = input.read_boolean();
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("SQLDataException") || msg.contains("22005"));
}

// ══════════════════════════════════════════════════════════════════
// 19. 多类型混合消费
// ══════════════════════════════════════════════════════════════════

/// 混合类型按序消费。
#[test]
fn sql_input_mixed_type_consumption() {
    let mut input = SqlInput::new(vec![
        Value::Bool(true),
        Value::Int(42),
        Value::String("hello".to_owned()),
        Value::Null,
        Value::Float(3.14),
    ]);
    assert!(input.read_boolean().unwrap());
    assert_eq!(input.read_long().unwrap(), 42);
    assert_eq!(input.read_string().unwrap().unwrap(), "hello");
    // read_string for Null → None + was_null
    assert!(input.read_string().unwrap().is_none());
    assert!(input.was_null());
    let v = input.read_double().unwrap();
    assert!((v - 3.14).abs() < f64::EPSILON);
}

/// `read_byte` → `read_short` → `read_int` 链式消费。
#[test]
fn sql_input_integer_chain() {
    let mut input = SqlInput::new(vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
    assert_eq!(input.read_byte().unwrap(), 1);
    assert_eq!(input.read_short().unwrap(), 2);
    assert_eq!(input.read_int().unwrap(), 3);
}

// ══════════════════════════════════════════════════════════════════
// 20. 边界值
// ══════════════════════════════════════════════════════════════════

/// `read_byte` `边界：i8::MAX`。
#[test]
fn read_byte_i8_max() {
    let mut input = SqlInput::new(vec![Value::Int(i64::from(i8::MAX))]);
    assert_eq!(input.read_byte().unwrap(), i8::MAX);
}

/// `read_byte` `边界：i8::MIN`。
#[test]
fn read_byte_i8_min() {
    let mut input = SqlInput::new(vec![Value::Int(i64::from(i8::MIN))]);
    assert_eq!(input.read_byte().unwrap(), i8::MIN);
}

/// `read_short` `边界：i16::MAX`。
#[test]
fn read_short_i16_max() {
    let mut input = SqlInput::new(vec![Value::Int(i64::from(i16::MAX))]);
    assert_eq!(input.read_short().unwrap(), i16::MAX);
}

/// `read_int` `边界：i32::MAX`。
#[test]
fn read_int_i32_max() {
    let mut input = SqlInput::new(vec![Value::Int(i64::from(i32::MAX))]);
    assert_eq!(input.read_int().unwrap(), i32::MAX);
}

/// `read_long` `边界：i64::MAX`。
#[test]
fn read_long_i64_max() {
    let mut input = SqlInput::new(vec![Value::Int(i64::MAX)]);
    assert_eq!(input.read_long().unwrap(), i64::MAX);
}
