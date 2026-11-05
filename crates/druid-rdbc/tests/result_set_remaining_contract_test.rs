//! `java.sql.ResultSet` 剩余标签 getter、状态/属性与 NULL 资源更新。
//!
//! 覆盖 BufferedRowSet 的真实实现：typed 标签读取、NULL 资源返回 None、
//! before-first/first/last/after-last 谓词、fetch/并发/保留方式、Wrapper，
//! 以及 type-map getObject 的精确 UnsupportedOperation。

use bigdecimal::BigDecimal;
use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::BufferedRowSet;
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetExt, ResultSetUpdate};
use druid_rdbc::statement::{FetchDirection, ResultSetConcurrency, ResultSetHoldability};
use druid_rdbc::types::{
    RdbcCalendarArgument, RdbcNaiveDate, RdbcNaiveTime, RdbcObject, Row, Value,
};
use druid_rdbc::wrapper::{StandardUnwrapped, Wrapper};
use std::any::TypeId;
use std::str::FromStr;

const LABELS: [&str; 20] = [
    "byte_col",
    "short_col",
    "float_col",
    "time_col",
    "timestamp_col",
    "decimal_col",
    "text",
    "array_col",
    "blob_col",
    "clob_col",
    "nclob_col",
    "ref_col",
    "rowid_col",
    "xml_col",
    "url_col",
    "ascii_col",
    "unicode_col",
    "binary_col",
    "character_col",
    "ncharacter_col",
];

fn column(index: usize) -> ColumnIndex {
    ColumnIndex::new(index).unwrap()
}

fn row_set() -> BufferedRowSet {
    let date = RdbcNaiveDate::from_ymd_opt(2026, 11, 4).unwrap();
    let time = RdbcNaiveTime::from_hms_nano_opt(8, 9, 10, 111_000_000).unwrap();
    let timestamp = date.and_time(time);
    let mut values = vec![
        Value::Int(-8),
        Value::Int(-300),
        Value::Float(1.5),
        Value::Time(time),
        Value::Timestamp(timestamp),
        Value::Decimal(BigDecimal::from_str("123.45").unwrap()),
        Value::String("text".to_owned()),
    ];
    values.extend(std::iter::repeat_n(Value::Null, 13));
    BufferedRowSet::with_column_labels(
        vec![Row::new(values)],
        LABELS.iter().map(|label| (*label).to_owned()).collect(),
    )
}

#[tokio::test]
async fn result_set_remaining_label_getters_return_typed_values_and_none() {
    let mut rows = row_set();
    assert!(rows.next().await.unwrap());

    assert_eq!(rows.byte(column(1)).unwrap(), -8);
    assert_eq!(rows.short(column(2)).unwrap(), -300);
    assert_eq!(rows.float(column(3)).unwrap(), 1.5);
    assert_eq!(
        rows.time(column(4), &RdbcCalendarArgument::Unspecified)
            .unwrap()
            .unwrap(),
        RdbcNaiveTime::from_hms_nano_opt(8, 9, 10, 111_000_000).unwrap()
    );
    assert!(rows
        .timestamp(column(5), &RdbcCalendarArgument::Unspecified)
        .unwrap()
        .is_some());
    assert_eq!(
        rows.big_decimal_by_label_with_scale("decimal_col", 2)
            .unwrap(),
        Some(BigDecimal::from_str("123.45").unwrap())
    );
    assert_eq!(
        rows.object_by_label("text").unwrap(),
        RdbcObject::Scalar(Value::String("text".to_owned()))
    );

    assert!(rows.array_by_label("array_col").unwrap().is_none());
    assert!(rows.blob_by_label("blob_col").unwrap().is_none());
    assert!(rows.clob_by_label("clob_col").unwrap().is_none());
    assert!(rows.n_clob_by_label("nclob_col").unwrap().is_none());
    assert!(rows.reference_by_label("ref_col").unwrap().is_none());
    assert!(rows.row_id_by_label("rowid_col").unwrap().is_none());
    assert!(rows.sql_xml_by_label("xml_col").unwrap().is_none());
    assert!(ResultSetExt::url_by_label(&rows, "url_col")
        .unwrap()
        .is_none());
    assert!(rows.ascii_stream_by_label("ascii_col").unwrap().is_none());
    assert!(rows
        .unicode_stream_by_label("unicode_col")
        .unwrap()
        .is_none());
    assert!(rows.binary_stream_by_label("binary_col").unwrap().is_none());
    assert!(rows
        .character_stream_by_label("character_col")
        .unwrap()
        .is_none());
    assert!(rows
        .n_character_stream_by_label("ncharacter_col")
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn result_set_remaining_state_properties_and_wrapper_are_observable() {
    let mut rows = row_set();

    assert!(rows.is_before_first().unwrap());
    assert!(!rows.is_first().unwrap());
    assert!(!rows.is_last().unwrap());
    assert!(!rows.is_after_last().unwrap());
    assert!(rows.next().await.unwrap());
    assert!(!rows.is_before_first().unwrap());
    assert!(rows.is_first().unwrap());
    assert!(rows.is_last().unwrap());
    assert!(!rows.is_after_last().unwrap());
    assert!(!rows.next().await.unwrap());
    assert!(rows.is_after_last().unwrap());

    assert_eq!(rows.concurrency().unwrap(), ResultSetConcurrency::ReadOnly);
    assert_eq!(
        rows.holdability().unwrap(),
        ResultSetHoldability::HoldOverCommit
    );
    assert!(rows.cursor_name().unwrap().is_none());
    assert!(rows.warnings().unwrap().is_none());
    rows.clear_warnings().unwrap();
    assert!(rows.warnings().unwrap().is_none());

    assert_eq!(rows.fetch_direction().unwrap(), FetchDirection::Forward);
    rows.set_fetch_direction(FetchDirection::Reverse).unwrap();
    assert_eq!(rows.fetch_direction().unwrap(), FetchDirection::Reverse);
    assert_eq!(rows.fetch_size().unwrap(), 0);
    rows.set_fetch_size(25).unwrap();
    assert_eq!(rows.fetch_size().unwrap(), 25);

    let wrapper: &dyn Wrapper = &rows;
    assert!(wrapper
        .is_wrapper_for(Some(TypeId::of::<dyn ResultSet>()))
        .unwrap());
    match wrapper.unwrap(Some(TypeId::of::<dyn ResultSet>())).unwrap() {
        StandardUnwrapped::ResultSet(view) => {
            assert!(std::ptr::addr_eq(view, &rows as &dyn ResultSet))
        }
        other => panic!("expected ResultSet view, got {other:?}"),
    }
}

#[tokio::test]
async fn result_set_type_map_get_object_reports_exact_unsupported_operation() {
    let mut rows = row_set();
    assert!(rows.next().await.unwrap());
    assert!(matches!(
        rows.object_with_type_map(column(7), None),
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_get_object_with_type_map"
        })
    ));
    assert!(matches!(
        rows.object_with_type_map_by_label("text", None),
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_get_object_by_label_with_type_map"
        })
    ));
    assert_eq!(
        rows.object_by_label("text").unwrap(),
        RdbcObject::Scalar(Value::String("text".to_owned()))
    );
}

#[tokio::test]
async fn result_set_null_resource_handle_updates_publish_null() {
    let mut rows = row_set();
    assert!(rows.next().await.unwrap());
    for (offset, update) in [
        ResultSetUpdate::Array(None),
        ResultSetUpdate::Blob(None),
        ResultSetUpdate::Clob(None),
        ResultSetUpdate::NClob(None),
        ResultSetUpdate::Ref(None),
        ResultSetUpdate::RowId(None),
        ResultSetUpdate::SqlXml(None),
    ]
    .into_iter()
    .enumerate()
    {
        let target = column(8 + offset);
        rows.update_value(target, update).unwrap();
        assert_eq!(rows.value(target).unwrap(), Value::Null);
        assert!(rows.was_null().unwrap());
    }
}
