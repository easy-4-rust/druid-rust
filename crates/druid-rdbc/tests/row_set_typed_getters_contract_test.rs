//! `javax.sql.RowSet` 继承 ResultSet 的类型化取值面：索引与标签两种入口。
//!
//! 通过 `SharedRowSet` 的真实 reader/row-store 路径发布一行 typed 值，再逐项断言
//! getXxx(int) 与 getXxx(String) 读到同一值；Calendar 重载同时覆盖 unspecified/specified。

use bigdecimal::BigDecimal;
use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::{
    RowSet, RowSetInternal, RowSetMetaData, RowSetMetaDataBuffer, RowSetReader, SharedRowSet,
    SharedRowSetMetaData,
};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetExt, ResultSetUpdate};
use druid_rdbc::types::{RdbcCalendarArgument, RdbcNaiveDate, RdbcNaiveTime, RdbcObject};
use std::str::FromStr;

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

fn column(index: usize) -> ColumnIndex {
    ColumnIndex::new(index).unwrap()
}

fn metadata() -> SharedRowSetMetaData {
    let mut meta = RowSetMetaDataBuffer::new();
    meta.set_column_count(LABELS.len() as i32).unwrap();
    for (offset, label) in LABELS.iter().enumerate() {
        meta.set_column_label(column(offset + 1), Some((*label).to_string()))
            .unwrap();
    }
    SharedRowSetMetaData::new(Box::new(meta))
}

/// 每个地址只发布一行；列顺序与 LABELS 一一对应。
fn typed_values() -> Vec<RdbcObject> {
    vec![
        RdbcObject::Boolean(true),
        RdbcObject::Byte(-8),
        RdbcObject::Short(-300),
        RdbcObject::Integer(42),
        RdbcObject::Long(9_000_000_000),
        RdbcObject::Float(1.5),
        RdbcObject::Double(-2.5),
        RdbcObject::String("text".to_owned()),
        RdbcObject::NString("国字".to_owned()),
        RdbcObject::Bytes(vec![1, 2, 3]),
        RdbcObject::BigDecimal(BigDecimal::from_str("123.45").unwrap()),
        RdbcObject::Date(RdbcNaiveDate::from_ymd_opt(2026, 7, 29).unwrap()),
        RdbcObject::Time(RdbcNaiveTime::from_hms_nano_opt(13, 14, 15, 123_000_000).unwrap()),
        RdbcObject::Timestamp(
            RdbcNaiveDate::from_ymd_opt(2026, 7, 29)
                .unwrap()
                .and_time(RdbcNaiveTime::from_hms_nano_opt(13, 14, 15, 123_000_000).unwrap()),
        ),
    ]
}

struct OneRowReader {
    row: Vec<RdbcObject>,
    meta: SharedRowSetMetaData,
}

#[async_trait::async_trait]
impl RowSetReader for OneRowReader {
    async fn read_data(&mut self, caller: &mut dyn RowSetInternal) -> Result<(), RdbcError> {
        caller.set_meta_data(self.meta.clone())?;
        {
            let row_set = caller.row_set();
            row_set.move_to_insert_row()?;
            for (offset, value) in self.row.iter().enumerate() {
                row_set.update_value(column(offset + 1), ResultSetUpdate::Object(value.clone()))?;
            }
            row_set.insert_row()?;
            row_set.move_to_current_row()?;
        }
        Ok(())
    }
}

async fn loaded_row_set() -> SharedRowSet {
    let mut source = SharedRowSet::new();
    source
        .set_reader(Some(Box::new(OneRowReader {
            row: typed_values(),
            meta: metadata(),
        })))
        .unwrap();
    RowSet::execute(&mut source).await.unwrap();
    assert!(
        source.next().await.unwrap(),
        "published row must be current"
    );
    source
}

#[tokio::test]
async fn row_set_typed_getters_match_by_index_and_label() {
    let source = loaded_row_set().await;

    assert!(source.boolean(column(1)).unwrap());
    assert!(source.boolean_by_label("flag").unwrap());
    assert_eq!(source.byte(column(2)).unwrap(), -8);
    assert_eq!(source.byte_by_label("byte_col").unwrap(), -8);
    assert_eq!(source.short(column(3)).unwrap(), -300);
    assert_eq!(source.short_by_label("short_col").unwrap(), -300);
    assert_eq!(source.int(column(4)).unwrap(), 42);
    assert_eq!(source.int_by_label("int_col").unwrap(), 42);
    assert_eq!(source.long(column(5)).unwrap(), 9_000_000_000);
    assert_eq!(source.long_by_label("long_col").unwrap(), 9_000_000_000);
    assert_eq!(source.float(column(6)).unwrap(), 1.5);
    assert_eq!(source.float_by_label("float_col").unwrap(), 1.5);
    assert_eq!(source.double(column(7)).unwrap(), -2.5);
    assert_eq!(source.double_by_label("double_col").unwrap(), -2.5);
    assert_eq!(source.string(column(8)).unwrap().as_deref(), Some("text"));
    assert_eq!(
        source.string_by_label("text").unwrap().as_deref(),
        Some("text")
    );
    assert_eq!(source.n_string(column(9)).unwrap().as_deref(), Some("国字"));
    assert_eq!(
        source
            .n_string_by_label("national_text")
            .unwrap()
            .as_deref(),
        Some("国字")
    );
    assert_eq!(source.bytes(column(10)).unwrap(), Some(vec![1, 2, 3]));
    assert_eq!(
        source.bytes_by_label("bytes_col").unwrap(),
        Some(vec![1, 2, 3])
    );
    assert_eq!(
        source.big_decimal(column(11)).unwrap(),
        Some(BigDecimal::from_str("123.45").unwrap())
    );
    assert_eq!(
        source.big_decimal_by_label("decimal_col").unwrap(),
        Some(BigDecimal::from_str("123.45").unwrap())
    );
    assert!(!source.was_null().unwrap());
}

#[tokio::test]
async fn row_set_temporal_getters_accept_both_calendar_arguments() {
    let source = loaded_row_set().await;
    let date = RdbcNaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
    let time = RdbcNaiveTime::from_hms_nano_opt(13, 14, 15, 123_000_000).unwrap();
    let timestamp = date.and_time(time);

    for calendar in [
        RdbcCalendarArgument::Unspecified,
        RdbcCalendarArgument::Specified(None),
    ] {
        assert_eq!(source.date(column(12), &calendar).unwrap(), Some(date));
        assert_eq!(
            source.date_by_label("date_col", &calendar).unwrap(),
            Some(date)
        );
        assert_eq!(source.time(column(13), &calendar).unwrap(), Some(time));
        assert_eq!(
            source.time_by_label("time_col", &calendar).unwrap(),
            Some(time)
        );
        assert_eq!(
            source.timestamp(column(14), &calendar).unwrap(),
            Some(timestamp)
        );
        assert_eq!(
            source
                .timestamp_by_label("timestamp_col", &calendar)
                .unwrap(),
            Some(timestamp)
        );
    }
}

#[tokio::test]
async fn row_set_typed_getters_preserve_conversion_errors() {
    let source = loaded_row_set().await;
    assert!(
        source.int(column(8)).is_err(),
        "text column must not silently narrow to int"
    );
    assert!(source
        .date(column(8), &RdbcCalendarArgument::Unspecified)
        .is_err());
}
