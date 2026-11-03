//! `java.sql.ResultSet` 的标签（列名）取值入口与 Calendar 取值重载合同。
//!
//! 通过 `BufferedRowSet::with_column_labels` 发布一行 14 列 typed 值，逐项断言
//! `getXxx(String)` 标签入口读到与列内容一致的值；`getDate/getTime/getTimestamp`
//! 同时覆盖「无 Calendar 重载」与「Calendar 重载」两种身份；未知标签必须报错。
//!
//! 对应 Java: java.sql.ResultSet#getXxx(java.lang.String...)。

use bigdecimal::BigDecimal;
use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::BufferedRowSet;
use druid_rdbc::result_set::{ResultSet, ResultSetExt};
use druid_rdbc::types::{
    RdbcCalendarArgument, RdbcNaiveDate, RdbcNaiveDateTime, RdbcNaiveTime, Row, Value,
};
use std::str::FromStr;

/// 14 列标签，顺序与 `typed_row()` 的列值一一对应。
const LABELS: [&str; 14] = [
    "flag",
    "byte_col",
    "short_col",
    "int_col",
    "long_col",
    "float_col",
    "double_col",
    "text",
    "national_text",
    "bytes_col",
    "decimal_col",
    "date_col",
    "time_col",
    "timestamp_col",
];

/// 期望的 SQL DATE 值。
fn expected_date() -> RdbcNaiveDate {
    RdbcNaiveDate::from_ymd_opt(2026, 7, 29).unwrap()
}

/// 期望的 SQL TIME 值。
fn expected_time() -> RdbcNaiveTime {
    RdbcNaiveTime::from_hms_nano_opt(13, 14, 15, 123_000_000).unwrap()
}

/// 期望的 SQL TIMESTAMP 值。
fn expected_timestamp() -> RdbcNaiveDateTime {
    expected_date().and_time(expected_time())
}

/// 构造一行 14 列 typed 值；列顺序与 `LABELS` 一一对应。
fn typed_row() -> Row {
    Row::new(vec![
        Value::Bool(true),
        Value::Int(-8),
        Value::Int(-300),
        Value::Int(42),
        Value::Int(9_000_000_000),
        Value::Float(1.5),
        Value::Float(-2.5),
        Value::String("text".to_owned()),
        Value::String("国字".to_owned()),
        Value::Bytes(vec![1, 2, 3]),
        Value::Decimal(BigDecimal::from_str("123.45").unwrap()),
        Value::Date(expected_date()),
        Value::Time(expected_time()),
        Value::Timestamp(expected_timestamp()),
    ])
}

/// 构造带标签的单行缓冲结果集；调用方负责推进游标。
fn labelled_row_set() -> BufferedRowSet {
    BufferedRowSet::with_column_labels(
        vec![typed_row()],
        LABELS.iter().map(|label| (*label).to_owned()).collect(),
    )
}

/// 标签入口读取 typed 标量：boolean/byte/short/int/long/float/double/string/nString/bytes/bigDecimal。
#[tokio::test]
async fn result_set_label_getters_read_typed_values_by_label() {
    let mut result = labelled_row_set();
    assert!(result.next().await.unwrap(), "one row must be current");

    assert!(result.boolean_by_label("flag").unwrap());
    assert_eq!(result.byte_by_label("byte_col").unwrap(), -8);
    assert_eq!(result.short_by_label("short_col").unwrap(), -300);
    assert_eq!(result.int_by_label("int_col").unwrap(), 42);
    assert_eq!(result.long_by_label("long_col").unwrap(), 9_000_000_000);
    assert_eq!(result.float_by_label("float_col").unwrap(), 1.5);
    assert_eq!(result.double_by_label("double_col").unwrap(), -2.5);
    assert_eq!(
        result.string_by_label("text").unwrap().as_deref(),
        Some("text")
    );
    assert_eq!(
        result
            .n_string_by_label("national_text")
            .unwrap()
            .as_deref(),
        Some("国字")
    );
    assert_eq!(
        result.bytes_by_label("bytes_col").unwrap(),
        Some(vec![1, 2, 3])
    );
    assert_eq!(
        result.big_decimal_by_label("decimal_col").unwrap(),
        Some(BigDecimal::from_str("123.45").unwrap())
    );
    assert!(!result.was_null().unwrap());
}

/// 日期/时间/时间戳的标签入口同时覆盖无 Calendar 重载与 Calendar 重载。
#[tokio::test]
async fn result_set_label_temporal_getters_cover_both_calendar_overloads() {
    let mut result = labelled_row_set();
    assert!(result.next().await.unwrap(), "one row must be current");

    let date = expected_date();
    let time = expected_time();
    let timestamp = expected_timestamp();

    // getDate(String)：未指定 Calendar 的重载身份。
    assert_eq!(
        result
            .date_by_label("date_col", &RdbcCalendarArgument::Unspecified)
            .unwrap(),
        Some(date)
    );
    // getDate(String, Calendar)：显式 Calendar 重载身份（Java 传入 null Calendar）。
    assert_eq!(
        result
            .date_by_label("date_col", &RdbcCalendarArgument::Specified(None))
            .unwrap(),
        Some(date)
    );

    // getTime(String)：未指定 Calendar 的重载身份。
    assert_eq!(
        result
            .time_by_label("time_col", &RdbcCalendarArgument::Unspecified)
            .unwrap(),
        Some(time)
    );
    // getTime(String, Calendar)：显式 Calendar 重载身份（Java 传入 null Calendar）。
    assert_eq!(
        result
            .time_by_label("time_col", &RdbcCalendarArgument::Specified(None))
            .unwrap(),
        Some(time)
    );

    // getTimestamp(String)：未指定 Calendar 的重载身份。
    assert_eq!(
        result
            .timestamp_by_label("timestamp_col", &RdbcCalendarArgument::Unspecified)
            .unwrap(),
        Some(timestamp)
    );
    // getTimestamp(String, Calendar)：显式 Calendar 重载身份（Java 传入 null Calendar）。
    assert_eq!(
        result
            .timestamp_by_label("timestamp_col", &RdbcCalendarArgument::Specified(None))
            .unwrap(),
        Some(timestamp)
    );
}

/// 未知标签必须在每个标签入口上报错，不得回退到其它列或返回默认值。
#[tokio::test]
async fn result_set_label_getters_reject_unknown_labels() {
    let mut result = labelled_row_set();
    assert!(result.next().await.unwrap(), "one row must be current");

    assert!(matches!(
        result.boolean_by_label("missing_flag"),
        Err(RdbcError::InvalidArgument(message)) if message.contains("unknown column")
    ));
    assert!(matches!(
        result.byte_by_label("missing_byte_col"),
        Err(RdbcError::InvalidArgument(message)) if message.contains("unknown column")
    ));
    assert!(matches!(
        result.short_by_label("missing_short_col"),
        Err(RdbcError::InvalidArgument(message)) if message.contains("unknown column")
    ));
    assert!(matches!(
        result.int_by_label("missing_int_col"),
        Err(RdbcError::InvalidArgument(message)) if message.contains("unknown column")
    ));
    assert!(matches!(
        result.long_by_label("missing_long_col"),
        Err(RdbcError::InvalidArgument(message)) if message.contains("unknown column")
    ));
    assert!(matches!(
        result.float_by_label("missing_float_col"),
        Err(RdbcError::InvalidArgument(message)) if message.contains("unknown column")
    ));
    assert!(matches!(
        result.double_by_label("missing_double_col"),
        Err(RdbcError::InvalidArgument(message)) if message.contains("unknown column")
    ));
    assert!(matches!(
        result.string_by_label("missing_text"),
        Err(RdbcError::InvalidArgument(message)) if message.contains("unknown column")
    ));
    assert!(matches!(
        result.n_string_by_label("missing_national_text"),
        Err(RdbcError::InvalidArgument(message)) if message.contains("unknown column")
    ));
    assert!(matches!(
        result.bytes_by_label("missing_bytes_col"),
        Err(RdbcError::InvalidArgument(message)) if message.contains("unknown column")
    ));
    assert!(matches!(
        result.big_decimal_by_label("missing_decimal_col"),
        Err(RdbcError::InvalidArgument(message)) if message.contains("unknown column")
    ));
    assert!(matches!(
        result.date_by_label("missing_date_col", &RdbcCalendarArgument::Unspecified),
        Err(RdbcError::InvalidArgument(message)) if message.contains("unknown column")
    ));
    assert!(matches!(
        result.time_by_label("missing_time_col", &RdbcCalendarArgument::Specified(None)),
        Err(RdbcError::InvalidArgument(message)) if message.contains("unknown column")
    ));
    assert!(matches!(
        result.timestamp_by_label("missing_timestamp_col", &RdbcCalendarArgument::Unspecified),
        Err(RdbcError::InvalidArgument(message)) if message.contains("unknown column")
    ));
}
