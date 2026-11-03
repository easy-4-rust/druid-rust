//! `java.sql.ResultSet` 标量 `updateXxx` 族：索引与标签入口都写入同一列状态。
//!
//! Rust 将各 JDBC setter family 折叠为 `ResultSetUpdate` variant；本测试逐族调用
//! `update_value` / `update_value_by_label`，断言最终存储值与 `row_updated` 状态。

use bigdecimal::BigDecimal;
use druid_rdbc::extensions::BufferedRowSet;
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetUpdate};
use druid_rdbc::types::{RdbcNaiveDate, RdbcNaiveTime, RdbcObject, Row, Value};
use std::str::FromStr;

const LABELS: [&str; 16] = [
    "null_col",
    "bool_col",
    "byte_col",
    "short_col",
    "int_col",
    "long_col",
    "float_col",
    "double_col",
    "string_col",
    "nstring_col",
    "bytes_col",
    "decimal_col",
    "date_col",
    "time_col",
    "timestamp_col",
    "object_col",
];

fn column(index: usize) -> ColumnIndex {
    ColumnIndex::new(index).unwrap()
}

fn row_set() -> BufferedRowSet {
    BufferedRowSet::with_column_labels(
        vec![Row::new(vec![Value::Null; LABELS.len()])],
        LABELS.iter().map(|label| (*label).to_owned()).collect(),
    )
}

fn apply(rows: &mut BufferedRowSet, index: usize, update: ResultSetUpdate, expected: Value) {
    let target = column(index);
    rows.update_value(target, update.clone()).unwrap();
    assert_eq!(rows.value(target).unwrap(), expected, "index entry");
    assert!(rows.row_updated().unwrap());

    rows.update_value(target, ResultSetUpdate::Null).unwrap();
    assert_eq!(rows.value(target).unwrap(), Value::Null);
    rows.update_value_by_label(LABELS[index - 1], update)
        .unwrap();
    assert_eq!(rows.value(target).unwrap(), expected, "label entry");
    assert!(rows.row_updated().unwrap());
}

#[tokio::test]
async fn scalar_update_families_write_through_index_and_label() {
    let mut rows = row_set();
    assert!(rows.next().await.unwrap());

    let date = RdbcNaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
    let time = RdbcNaiveTime::from_hms_nano_opt(13, 14, 15, 123_000_000).unwrap();
    let timestamp = date.and_time(time);

    apply(
        &mut rows,
        2,
        ResultSetUpdate::Boolean(true),
        Value::Bool(true),
    );
    apply(&mut rows, 3, ResultSetUpdate::Byte(-8), Value::Int(-8));
    apply(&mut rows, 4, ResultSetUpdate::Short(-300), Value::Int(-300));
    apply(&mut rows, 5, ResultSetUpdate::Int(42), Value::Int(42));
    apply(
        &mut rows,
        6,
        ResultSetUpdate::Long(9_000_000_000),
        Value::Int(9_000_000_000),
    );
    apply(&mut rows, 7, ResultSetUpdate::Float(1.5), Value::Float(1.5));
    apply(
        &mut rows,
        8,
        ResultSetUpdate::Double(-2.5),
        Value::Float(-2.5),
    );
    apply(
        &mut rows,
        9,
        ResultSetUpdate::String(Some("text".to_owned())),
        Value::String("text".to_owned()),
    );
    apply(
        &mut rows,
        10,
        ResultSetUpdate::NString(Some("国字".to_owned())),
        Value::String("国字".to_owned()),
    );
    apply(
        &mut rows,
        11,
        ResultSetUpdate::Bytes(Some(vec![1, 2, 3])),
        Value::Bytes(vec![1, 2, 3]),
    );
    apply(
        &mut rows,
        12,
        ResultSetUpdate::BigDecimal(Some(BigDecimal::from_str("123.45").unwrap())),
        Value::Decimal(BigDecimal::from_str("123.45").unwrap()),
    );
    apply(
        &mut rows,
        13,
        ResultSetUpdate::Date(Some(date)),
        Value::Date(date),
    );
    apply(
        &mut rows,
        14,
        ResultSetUpdate::Time(Some(time)),
        Value::Time(time),
    );
    apply(
        &mut rows,
        15,
        ResultSetUpdate::Timestamp(Some(timestamp)),
        Value::Timestamp(timestamp),
    );
    apply(
        &mut rows,
        16,
        ResultSetUpdate::Object(RdbcObject::Scalar(Value::Int(77))),
        Value::Int(77),
    );
}

#[tokio::test]
async fn null_update_clears_value_and_sets_was_null() {
    let mut rows = row_set();
    assert!(rows.next().await.unwrap());
    rows.update_value(column(1), ResultSetUpdate::Int(7))
        .unwrap();
    assert!(!rows.was_null().unwrap());
    rows.update_value(column(1), ResultSetUpdate::Null).unwrap();
    assert_eq!(rows.value(column(1)).unwrap(), Value::Null);
    assert!(rows.was_null().unwrap());
    rows.update_value_by_label("null_col", ResultSetUpdate::Null)
        .unwrap();
    assert_eq!(rows.value(column(1)).unwrap(), Value::Null);
    assert!(rows.was_null().unwrap());
}
