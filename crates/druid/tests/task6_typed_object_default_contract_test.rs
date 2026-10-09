//! 固定 BASE typed getObject 默认合同的标准接口迁移。
use bigdecimal::BigDecimal;
use bigdecimal::FromPrimitive;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::BufferedRowSet;
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetExt, ResultSetRead};
use druid_rdbc::types::{RdbcCalendarArgument, RdbcObject, RdbcTargetType, Row, Value};
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
#[path = "typed_object_support/custom_target_read_probe.rs"]
mod custom_target_read_probe;
#[path = "typed_object_support/typed_resource_call.rs"]
mod typed_resource_call;
#[path = "typed_object_support/typed_resource_probe.rs"]
mod typed_resource_probe;
use custom_target_read_probe::CustomTargetReadProbe;
use typed_resource_call::TypedResourceCall;
use typed_resource_probe::TypedResourceProbe;
fn task6_column(index: usize) -> ColumnIndex {
    ColumnIndex::new(index).unwrap()
}

#[tokio::test]
async fn resource_read_defaults_preserve_family_errors_nulls_and_driver_priority() {
    let families = [
        (ResultSetRead::Ref, "result_set_ref"),
        (ResultSetRead::Blob, "result_set_blob"),
        (ResultSetRead::Clob, "result_set_clob"),
        (ResultSetRead::Array, "result_set_array"),
        (ResultSetRead::Url, "result_set_url"),
        (ResultSetRead::RowId, "result_set_row_id"),
        (ResultSetRead::NClob, "result_set_n_clob"),
        (ResultSetRead::SqlXml, "result_set_sql_xml"),
    ];
    let probe = CustomTargetReadProbe {
        value_calls: AtomicUsize::new(0),
        failure: None,
    };
    let failure = RdbcError::DriverError("driver column failure".to_owned());
    let failing = CustomTargetReadProbe {
        value_calls: AtomicUsize::new(0),
        failure: Some(failure.clone()),
    };
    let mut nulls = BufferedRowSet::new(vec![Row::new(vec![Value::Null])]);
    assert!(nulls.next().await.unwrap());
    for (request, operation) in families {
        assert_eq!(
            probe.read_object(task6_column(3), &request),
            Err(RdbcError::UnsupportedOperation { operation })
        );
        assert_eq!(
            nulls.read_object(task6_column(1), &request).unwrap(),
            RdbcObject::Scalar(Value::Null)
        );
        assert!(nulls.was_null().unwrap());
        assert_eq!(
            failing.read_object(task6_column(3), &request),
            Err(failure.clone())
        );
    }
    assert_eq!(probe.value_calls.load(Ordering::Relaxed), 8);
    assert_eq!(failing.value_calls.load(Ordering::Relaxed), 8);
}

#[test]
fn type_map_index_default_preserves_error_without_reading_columns() {
    let probe = CustomTargetReadProbe {
        value_calls: AtomicUsize::new(0),
        failure: None,
    };
    assert_eq!(
        probe.object_with_type_map(task6_column(3), None),
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_get_object_with_type_map",
        })
    );
    assert_eq!(probe.value_calls.load(Ordering::Relaxed), 0);
}

#[test]
fn type_map_label_default_preserves_error_without_reading_columns() {
    let probe = CustomTargetReadProbe {
        value_calls: AtomicUsize::new(0),
        failure: None,
    };
    assert_eq!(
        probe.object_by_label_with_type_map("missing", None),
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_get_object_by_label_with_type_map",
        })
    );
    assert_eq!(probe.value_calls.load(Ordering::Relaxed), 0);
}

/// 保留 BASE 使用 from_f64 的二进制浮点 Decimal 精度，不先格式化为字符串。
#[tokio::test]
async fn floating_decimal_preserves_base_binary_precision() {
    let mut rows = BufferedRowSet::new(vec![Row::new(vec![Value::Float(0.1)])]);
    assert!(rows.next().await.unwrap());
    assert_eq!(
        rows.big_decimal(task6_column(1)).unwrap(),
        BigDecimal::from_f64(0.1)
    );
}

#[test]
fn task6_custom_typed_object_checks_the_physical_column_exactly_once() {
    let probe = CustomTargetReadProbe {
        value_calls: AtomicUsize::new(0),
        failure: None,
    };
    assert!(matches!(
        probe.object_as(
            task6_column(3),
            &RdbcTargetType::Custom("vendor.Type".to_string())
        ),
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_get_object_typed_custom"
        })
    ));
    assert_eq!(probe.value_calls.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn task6_default_typed_conversions_cover_alternate_values_and_driver_errors() {
    let mut result_set = task6_typed_conversion_row_set();
    assert!(result_set.next().await.unwrap());
    task6_assert_default_decimal_and_date_conversions(&result_set);
    task6_assert_default_time_and_timestamp_conversions(&result_set);
}

#[test]
fn task6_default_typed_get_object_delegates_every_standard_resource_target() {
    let probe = TypedResourceProbe::new();
    let null = RdbcObject::Scalar(Value::Null);

    assert_eq!(
        probe
            .object_as(task6_column(1), &RdbcTargetType::Blob)
            .unwrap(),
        null
    );
    assert_eq!(
        probe
            .object_as(task6_column(2), &RdbcTargetType::Clob)
            .unwrap(),
        null
    );
    assert_eq!(
        probe
            .object_as(task6_column(3), &RdbcTargetType::NClob)
            .unwrap(),
        null
    );
    assert_eq!(
        probe
            .object_as(task6_column(4), &RdbcTargetType::Array)
            .unwrap(),
        null
    );
    assert_eq!(
        probe
            .object_as(task6_column(5), &RdbcTargetType::Ref)
            .unwrap(),
        null
    );
    assert_eq!(
        probe
            .object_as(task6_column(6), &RdbcTargetType::RowId)
            .unwrap(),
        null
    );
    assert_eq!(
        probe
            .object_as(task6_column(7), &RdbcTargetType::SqlXml)
            .unwrap(),
        null
    );
    assert_eq!(
        probe
            .object_as(task6_column(8), &RdbcTargetType::Url)
            .unwrap(),
        null
    );
    assert_eq!(
        probe
            .object_by_label_as("blob_label", &RdbcTargetType::Blob)
            .unwrap(),
        null
    );
    assert_eq!(
        probe.calls(),
        vec![
            TypedResourceCall::Blob(1),
            TypedResourceCall::Clob(2),
            TypedResourceCall::NClob(3),
            TypedResourceCall::Array(4),
            TypedResourceCall::Ref(5),
            TypedResourceCall::RowId(6),
            TypedResourceCall::SqlXml(7),
            TypedResourceCall::Url(8),
            TypedResourceCall::FindColumn("blob_label".to_string()),
            TypedResourceCall::Blob(41),
        ]
    );
}

fn task6_typed_conversion_row_set() -> BufferedRowSet {
    let date = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
    let time = NaiveTime::from_hms_nano_opt(13, 14, 15, 123_000_000).unwrap();
    BufferedRowSet::new(vec![Row::new(vec![
        Value::Null,
        Value::Int(42),
        Value::Float(2.5),
        Value::Float(f64::NAN),
        Value::String("3.75".to_string()),
        Value::String("invalid".to_string()),
        Value::Bytes(vec![1]),
        Value::Date(date),
        Value::Time(time),
        Value::Timestamp(NaiveDateTime::new(date, time)),
        Value::String("2026-07-30".to_string()),
        Value::String("2026-07-30 01:02:03.456".to_string()),
        Value::String("2026-07-30T01:02:03.456".to_string()),
        Value::String("01:02:03.456".to_string()),
    ])])
}

fn task6_assert_default_decimal_and_date_conversions(result_set: &BufferedRowSet) {
    assert_eq!(result_set.big_decimal(task6_column(1)).unwrap(), None);
    assert_eq!(
        result_set.big_decimal(task6_column(2)).unwrap(),
        Some(BigDecimal::from(42))
    );
    assert_eq!(
        result_set
            .big_decimal_with_scale(task6_column(3), 1)
            .unwrap(),
        Some(BigDecimal::from_str("2.5").unwrap())
    );
    assert_eq!(
        result_set.big_decimal(task6_column(5)).unwrap(),
        Some(BigDecimal::from_str("3.75").unwrap())
    );
    assert!(matches!(
        result_set.big_decimal(task6_column(4)),
        Err(RdbcError::DriverError(_))
    ));
    assert!(matches!(
        result_set.big_decimal(task6_column(6)),
        Err(RdbcError::DriverError(_))
    ));
    assert!(matches!(
        result_set.big_decimal(task6_column(7)),
        Err(RdbcError::DriverError(_))
    ));

    assert_eq!(
        result_set
            .date(task6_column(1), &RdbcCalendarArgument::Unspecified)
            .unwrap(),
        None
    );
    assert!(result_set
        .date(task6_column(10), &RdbcCalendarArgument::Unspecified)
        .unwrap()
        .is_some());
    assert!(result_set
        .date(task6_column(11), &RdbcCalendarArgument::Unspecified)
        .unwrap()
        .is_some());
    assert!(matches!(
        result_set.date(task6_column(6), &RdbcCalendarArgument::Unspecified),
        Err(RdbcError::DriverError(_))
    ));
    assert!(matches!(
        result_set.date(task6_column(7), &RdbcCalendarArgument::Unspecified),
        Err(RdbcError::DriverError(_))
    ));
}

fn task6_assert_default_time_and_timestamp_conversions(result_set: &BufferedRowSet) {
    assert_eq!(
        result_set
            .time(task6_column(1), &RdbcCalendarArgument::Unspecified)
            .unwrap(),
        None
    );
    assert!(result_set
        .time(task6_column(10), &RdbcCalendarArgument::Unspecified)
        .unwrap()
        .is_some());
    assert!(result_set
        .time(task6_column(14), &RdbcCalendarArgument::Unspecified)
        .unwrap()
        .is_some());
    assert!(matches!(
        result_set.time(task6_column(6), &RdbcCalendarArgument::Unspecified),
        Err(RdbcError::DriverError(_))
    ));
    assert!(matches!(
        result_set.time(task6_column(7), &RdbcCalendarArgument::Unspecified),
        Err(RdbcError::DriverError(_))
    ));

    assert_eq!(
        result_set
            .timestamp(task6_column(1), &RdbcCalendarArgument::Unspecified)
            .unwrap(),
        None
    );
    for index in [8, 12, 13] {
        assert!(result_set
            .timestamp(task6_column(index), &RdbcCalendarArgument::Unspecified)
            .unwrap()
            .is_some());
    }
    assert!(matches!(
        result_set.timestamp(task6_column(6), &RdbcCalendarArgument::Unspecified),
        Err(RdbcError::DriverError(_))
    ));
    assert!(matches!(
        result_set.timestamp(task6_column(7), &RdbcCalendarArgument::Unspecified),
        Err(RdbcError::DriverError(_))
    ));
}

/// 自定义目标类型先检查实际列，保留驱动错误并且只读取一次。
#[test]
fn custom_target_preserves_driver_error_priority_and_single_read() {
    let probe = CustomTargetReadProbe {
        value_calls: AtomicUsize::new(0),
        failure: Some(RdbcError::InvalidArgument(
            "original driver column error".to_owned(),
        )),
    };
    assert!(
        matches!(probe.object_as(task6_column(3), &RdbcTargetType::Custom("vendor.Type".to_owned())),
        Err(RdbcError::InvalidArgument(message)) if message == "original driver column error")
    );
    assert_eq!(probe.value_calls.load(Ordering::Relaxed), 1);
}

/// 标准默认入口完整转换标量目标，保留 SQL NULL、溢出和非法 UTF-8 错误。
#[tokio::test]
async fn standard_typed_scalar_targets_preserve_values_nulls_and_errors() {
    let date = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
    let time = NaiveTime::from_hms_nano_opt(13, 14, 15, 123_456_789).unwrap();
    let decimal = BigDecimal::from_str("1234567890.123456789").unwrap();
    let mut rows = BufferedRowSet::new(vec![Row::new(vec![
        Value::String("1".to_owned()),
        Value::Int(127),
        Value::Int(32000),
        Value::Int(i64::from(i32::MAX)),
        Value::Int(i64::MAX),
        Value::Float(1.25),
        Value::Float(2.5),
        Value::Decimal(decimal.clone()),
        Value::Date(date),
        Value::Time(time),
        Value::Timestamp(date.and_time(time)),
        Value::String("druid".to_owned()),
        Value::Bytes(vec![0, 255]),
        Value::Null,
        Value::Int(256),
        Value::Bytes(vec![255]),
    ])]);
    assert!(rows.next().await.unwrap());
    for (column, target, expected) in [
        (1, RdbcTargetType::Boolean, RdbcObject::Boolean(true)),
        (2, RdbcTargetType::Byte, RdbcObject::Byte(127)),
        (3, RdbcTargetType::Short, RdbcObject::Short(32000)),
        (4, RdbcTargetType::Integer, RdbcObject::Integer(i32::MAX)),
        (5, RdbcTargetType::Long, RdbcObject::Long(i64::MAX)),
        (6, RdbcTargetType::Float, RdbcObject::Float(1.25)),
        (7, RdbcTargetType::Double, RdbcObject::Double(2.5)),
        (
            8,
            RdbcTargetType::BigDecimal,
            RdbcObject::BigDecimal(decimal),
        ),
        (9, RdbcTargetType::Date, RdbcObject::Date(date)),
        (10, RdbcTargetType::Time, RdbcObject::Time(time)),
        (
            11,
            RdbcTargetType::Timestamp,
            RdbcObject::Timestamp(date.and_time(time)),
        ),
        (
            4,
            RdbcTargetType::String,
            RdbcObject::String(i32::MAX.to_string()),
        ),
        (
            12,
            RdbcTargetType::Bytes,
            RdbcObject::Bytes(b"druid".to_vec()),
        ),
        (13, RdbcTargetType::Bytes, RdbcObject::Bytes(vec![0, 255])),
    ] {
        assert_eq!(
            rows.object_as(task6_column(column), &target).unwrap(),
            expected
        );
    }
    for target in [
        RdbcTargetType::String,
        RdbcTargetType::Boolean,
        RdbcTargetType::Byte,
        RdbcTargetType::Short,
        RdbcTargetType::Integer,
        RdbcTargetType::Long,
        RdbcTargetType::Float,
        RdbcTargetType::Double,
        RdbcTargetType::Bytes,
        RdbcTargetType::BigDecimal,
        RdbcTargetType::Date,
        RdbcTargetType::Time,
        RdbcTargetType::Timestamp,
    ] {
        assert_eq!(
            rows.object_as(task6_column(14), &target).unwrap(),
            RdbcObject::Scalar(Value::Null)
        );
        assert!(rows.was_null().unwrap());
    }
    assert!(matches!(
        rows.object_as(task6_column(15), &RdbcTargetType::Byte),
        Err(RdbcError::DriverError(_))
    ));
    assert!(matches!(
        rows.object_as(task6_column(16), &RdbcTargetType::String),
        Err(RdbcError::DriverError(_))
    ));
    rows.close().await.unwrap();
    assert!(matches!(
        rows.object_as(task6_column(1), &RdbcTargetType::Boolean),
        Err(RdbcError::Other(message)) if message == "result set is closed"
    ));
}
