//! 完整原 RowSet getter、流及独立共享转发身份合同草案；尚未接入 Cargo。
use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::{BufferedRowSet, SharedResultSet};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetExt};
use druid_rdbc::types::{
    RdbcBigDecimal as BigDecimal, RdbcNaiveDate as NaiveDate, RdbcNaiveDateTime as NaiveDateTime,
    RdbcNaiveTime as NaiveTime, Row, Value,
};
use std::str::FromStr;

fn task6_scalar_default_row_set() -> BufferedRowSet {
    let date = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
    let time = NaiveTime::from_hms_opt(13, 14, 15).unwrap();
    BufferedRowSet::with_column_labels(
        vec![Row::new(vec![
            Value::Null,
            Value::Bool(true),
            Value::Int(42),
            Value::Float(2.5),
            Value::Decimal(BigDecimal::from_str("3.25").unwrap()),
            Value::String("17".to_string()),
            Value::Bytes(b"bytes".to_vec()),
            Value::Date(date),
            Value::Time(time),
            Value::Timestamp(NaiveDateTime::new(date, time)),
            Value::Bytes(vec![0xff]),
            Value::String("not-a-number".to_string()),
            Value::Int(i64::MAX),
            Value::Bytes(vec![1, 2]),
        ])],
        (1..=14).map(|index| format!("c{index}")).collect(),
    )
}

fn task6_assert_default_scalar_successes(result_set: &BufferedRowSet) {
    assert_eq!(
        result_set.string(ColumnIndex::new(1).unwrap()).unwrap(),
        None
    );
    for index in 2..=10 {
        assert!(result_set
            .string(ColumnIndex::new(index).unwrap())
            .unwrap()
            .is_some());
    }
    assert_eq!(
        result_set.string_by_label("c3").unwrap().as_deref(),
        Some("42")
    );
    assert!(!result_set.boolean(ColumnIndex::new(1).unwrap()).unwrap());
    assert!(result_set.boolean(ColumnIndex::new(2).unwrap()).unwrap());
    assert!(result_set.boolean(ColumnIndex::new(3).unwrap()).unwrap());
    assert!(result_set.boolean(ColumnIndex::new(4).unwrap()).unwrap());
    assert!(result_set.boolean(ColumnIndex::new(5).unwrap()).unwrap());
    assert!(!result_set.boolean_by_label("c6").unwrap());
    assert_eq!(result_set.long(ColumnIndex::new(1).unwrap()).unwrap(), 0);
    assert_eq!(result_set.long(ColumnIndex::new(2).unwrap()).unwrap(), 1);
    assert_eq!(result_set.long(ColumnIndex::new(3).unwrap()).unwrap(), 42);
    assert_eq!(result_set.long(ColumnIndex::new(4).unwrap()).unwrap(), 2);
    assert_eq!(result_set.long(ColumnIndex::new(5).unwrap()).unwrap(), 3);
    assert_eq!(result_set.long_by_label("c6").unwrap(), 17);
    assert_eq!(result_set.int_by_label("c3").unwrap(), 42);
    assert_eq!(result_set.short_by_label("c3").unwrap(), 42);
    assert_eq!(result_set.byte_by_label("c3").unwrap(), 42);
    assert_eq!(
        result_set.double(ColumnIndex::new(1).unwrap()).unwrap(),
        0.0
    );
    assert_eq!(
        result_set.double(ColumnIndex::new(2).unwrap()).unwrap(),
        1.0
    );
    assert_eq!(
        result_set.double(ColumnIndex::new(3).unwrap()).unwrap(),
        42.0
    );
    assert_eq!(
        result_set.double(ColumnIndex::new(4).unwrap()).unwrap(),
        2.5
    );
    assert_eq!(
        result_set.double(ColumnIndex::new(5).unwrap()).unwrap(),
        3.25
    );
    assert_eq!(result_set.double_by_label("c6").unwrap(), 17.0);
    assert_eq!(result_set.float_by_label("c4").unwrap(), 2.5);
    assert_eq!(
        result_set.bytes(ColumnIndex::new(1).unwrap()).unwrap(),
        None
    );
    assert_eq!(
        result_set.bytes(ColumnIndex::new(7).unwrap()).unwrap(),
        Some(b"bytes".to_vec())
    );
    assert_eq!(
        result_set.bytes_by_label("c3").unwrap(),
        Some(b"42".to_vec())
    );
    assert_eq!(result_set.value_by_label("c3").unwrap(), Value::Int(42));
}

fn task6_assert_default_scalar_failures(result_set: &BufferedRowSet) {
    assert!(matches!(
        result_set.string(ColumnIndex::new(11).unwrap()),
        Err(RdbcError::DriverError(_))
    ));
    assert!(matches!(
        result_set.boolean(ColumnIndex::new(7).unwrap()),
        Err(RdbcError::DriverError(_))
    ));
    assert!(matches!(
        result_set.long(ColumnIndex::new(12).unwrap()),
        Err(RdbcError::DriverError(_))
    ));
    assert!(matches!(
        result_set.long(ColumnIndex::new(14).unwrap()),
        Err(RdbcError::DriverError(_))
    ));
    assert!(matches!(
        result_set.double(ColumnIndex::new(12).unwrap()),
        Err(RdbcError::DriverError(_))
    ));
    assert!(matches!(
        result_set.double(ColumnIndex::new(14).unwrap()),
        Err(RdbcError::DriverError(_))
    ));
    assert!(matches!(
        result_set.int(ColumnIndex::new(13).unwrap()),
        Err(RdbcError::DriverError(_))
    ));
    assert!(matches!(
        result_set.short(ColumnIndex::new(13).unwrap()),
        Err(RdbcError::DriverError(_))
    ));
    assert!(matches!(
        result_set.byte(ColumnIndex::new(13).unwrap()),
        Err(RdbcError::DriverError(_))
    ));
    assert!(matches!(
        result_set.value(ColumnIndex::new(15).unwrap()),
        Err(RdbcError::InvalidArgument(message)) if message.contains("exceeds row width")
    ));
}

fn task6_assert_default_stream_conversions(result_set: &BufferedRowSet) {
    assert!(result_set
        .binary_stream(ColumnIndex::new(1).unwrap())
        .unwrap()
        .is_none());
    assert!(result_set
        .character_stream(ColumnIndex::new(1).unwrap())
        .unwrap()
        .is_none());
    for index in 2..=10 {
        assert!(result_set
            .binary_stream(ColumnIndex::new(index).unwrap())
            .unwrap()
            .is_some());
        assert!(result_set
            .character_stream(ColumnIndex::new(index).unwrap())
            .unwrap()
            .is_some());
    }
    assert!(matches!(
        result_set.character_stream(ColumnIndex::new(11).unwrap()),
        Err(RdbcError::DriverError(_))
    ));
}

/// 与原稀疏后端对应的独立实例；明确拒绝推进，不冒充成功行。
#[derive(Debug)]
struct SparseSharedResultSetBackend;
druid_rdbc::impl_standard_wrapper!(SparseSharedResultSetBackend => ResultSet);
#[async_trait::async_trait]
impl ResultSet for SparseSharedResultSetBackend {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        Err(RdbcError::UnsupportedOperation {
            operation: "result_set_next",
        })
    }
}

/// 保留全部原 getter/stream helpers、导航及后端指针身份观察。
/// 对应 Java: java.sql.ResultSet；原共享句柄的 concrete forwarding handoff。
#[tokio::test]
async fn task6_row_set_default_getters_cover_null_label_conversion_and_stream_contracts() {
    let mut result_set = task6_scalar_default_row_set();
    assert!(result_set.next().await.unwrap());
    task6_assert_default_scalar_successes(&result_set);
    task6_assert_default_scalar_failures(&result_set);
    task6_assert_default_stream_conversions(&result_set);
    assert!(!result_set.absolute(0).await.unwrap());
    assert!(result_set.absolute(1).await.unwrap());
    let raw: Box<dyn ResultSet> = Box::new(result_set);
    let raw_identity = std::ptr::from_ref(raw.as_ref());
    let first = SharedResultSet::new(raw);
    let same = first.clone();
    let other = SharedResultSet::new(Box::new(SparseSharedResultSetBackend));
    assert_eq!(first, same);
    assert_ne!(first, other);
    assert!(std::ptr::eq(
        std::ptr::from_ref(&*first.try_result_set().unwrap()),
        raw_identity
    ));
    assert!(format!("{first:?}").contains("SharedResultSet"));
    first.close().await.unwrap();
    assert!(first.is_closed().unwrap());
    assert!(same.is_closed().unwrap());
}
