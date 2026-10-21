//! JDBC 默认标量读取的数据异常、单次访问和脱敏合同。
use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::BufferedRowSet;
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetExt};
use druid_rdbc::types::{RdbcCalendarArgument, RdbcObject, RdbcTargetType, Row, Value};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// 单行后端，用于验证唯一标准 ResultSet 默认 getter；对应 Java: java.sql.ResultSet。
struct ScalarResultSet {
    value: Value,
    reads: AtomicUsize,
    fail: bool,
    last_was_null: AtomicBool,
}
druid_rdbc::impl_standard_wrapper!(ScalarResultSet => ResultSet);
#[async_trait::async_trait]
impl ResultSet for ScalarResultSet {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        Ok(false)
    }
    fn value(&self, column: ColumnIndex) -> Result<Value, RdbcError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            return Err(RdbcError::DriverError(
                "original driver read failure".to_owned(),
            ));
        }
        if column.get() != 1 {
            return Err(RdbcError::InvalidArgument("unknown column".to_owned()));
        }
        self.last_was_null
            .store(matches!(self.value, Value::Null), Ordering::SeqCst);
        Ok(self.value.clone())
    }
    fn find_column(&self, label: &str) -> Result<ColumnIndex, RdbcError> {
        if label.eq_ignore_ascii_case("value") {
            ColumnIndex::new(1)
        } else {
            Err(RdbcError::InvalidArgument("unknown label".to_owned()))
        }
    }
    fn was_null(&self) -> Result<bool, RdbcError> {
        Ok(self.last_was_null.load(Ordering::SeqCst))
    }
}
fn scalar(value: Value) -> ScalarResultSet {
    ScalarResultSet {
        value,
        reads: AtomicUsize::new(0),
        fail: false,
        last_was_null: AtomicBool::new(false),
    }
}
fn data_error(error: RdbcError, state: &str) {
    let RdbcError::SqlException(error) = error else {
        panic!("expected SQLDataException")
    };
    assert_eq!(error.class_name(), "java.sql.SQLDataException");
    assert_eq!(error.sql_state(), Some(state));
    assert!(!format!("{error:?}").contains("private_"));
}
#[test]
fn default_numeric_getters_convert_once_and_keep_original_driver_failures() {
    let index = ColumnIndex::new(1).unwrap();
    for (value, expected) in [
        (Value::Bool(true), 1),
        (Value::Float(-12.9), -12),
        (Value::String("42".to_owned()), 42),
        (Value::Null, 0),
    ] {
        let result = scalar(value);
        assert_eq!(result.long(index).unwrap(), expected);
        assert_eq!(result.reads.load(Ordering::SeqCst), 1);
    }
    let result = scalar(Value::Int(42));
    assert_eq!(result.double(index).unwrap(), 42.0);
    assert_eq!(result.reads.load(Ordering::SeqCst), 1);
    let mut failed = scalar(Value::String("private_invalid".to_owned()));
    failed.fail = true;
    assert!(
        matches!(failed.long(index), Err(RdbcError::DriverError(message)) if message == "original driver read failure")
    );
    assert_eq!(failed.reads.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn decimal_double_and_temporal_errors_have_exact_states_without_values() {
    let index = ColumnIndex::new(1).unwrap();
    for (value, target, state) in [
        (
            Value::String("private_invalid".to_owned()),
            RdbcTargetType::BigDecimal,
            "22018",
        ),
        (
            Value::String("private_invalid".to_owned()),
            RdbcTargetType::Double,
            "22018",
        ),
        (
            Value::String("1e9999".to_owned()),
            RdbcTargetType::Double,
            "22003",
        ),
        (
            Value::Float(f64::INFINITY),
            RdbcTargetType::BigDecimal,
            "22003",
        ),
        (
            Value::String("private_date".to_owned()),
            RdbcTargetType::Date,
            "22007",
        ),
        (
            Value::String("private_time".to_owned()),
            RdbcTargetType::Time,
            "22007",
        ),
        (
            Value::String("private_timestamp".to_owned()),
            RdbcTargetType::Timestamp,
            "22007",
        ),
        (Value::Bytes(vec![0xff]), RdbcTargetType::String, "22021"),
        (Value::Bytes(vec![1]), RdbcTargetType::Long, "22005"),
    ] {
        let mut row = BufferedRowSet::with_column_labels(
            vec![Row::new(vec![value])],
            vec!["value".to_owned()],
        );
        assert!(row.next().await.unwrap());
        data_error(row.object_as(index, &target).unwrap_err(), state);
        data_error(row.object_as_by_label("value", &target).unwrap_err(), state);
        if target == RdbcTargetType::String && state == "22021" {
            data_error(row.character_stream(index).unwrap_err(), "22021");
            data_error(row.string(index).unwrap_err(), "22021");
        }
        assert!(!row.was_null().unwrap());
        row.close().await.unwrap();
    }
}
#[tokio::test]
async fn ordinary_temporal_and_decimal_getters_share_typed_error_categories() {
    let mut row = BufferedRowSet::new(vec![Row::new(vec![Value::String(
        "private_invalid".to_owned(),
    )])]);
    assert!(row.next().await.unwrap());
    let index = ColumnIndex::new(1).unwrap();
    data_error(row.big_decimal(index).unwrap_err(), "22018");
    data_error(
        row.date(index, &RdbcCalendarArgument::Unspecified)
            .unwrap_err(),
        "22007",
    );
    data_error(
        row.time(index, &RdbcCalendarArgument::Unspecified)
            .unwrap_err(),
        "22007",
    );
    data_error(
        row.timestamp(index, &RdbcCalendarArgument::Unspecified)
            .unwrap_err(),
        "22007",
    );
    row.close().await.unwrap();
}
#[test]
fn explicit_ieee_values_and_finite_double_text_keep_their_value_semantics() {
    let index = ColumnIndex::new(1).unwrap();
    for text in ["NaN", "Infinity", "-Infinity"] {
        let value = scalar(Value::String(text.to_owned()))
            .object_as(index, &RdbcTargetType::Double)
            .unwrap();
        let RdbcObject::Double(value) = value else {
            panic!("Double expected")
        };
        assert!(!value.is_finite());
    }
    assert_eq!(
        scalar(Value::String("1.25".to_owned()))
            .double(index)
            .unwrap(),
        1.25
    );
}

#[test]
fn default_string_getters_convert_all_scalar_families_once() {
    use chrono::{NaiveDate, NaiveTime};
    let date = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
    let time = NaiveTime::from_hms_opt(1, 2, 3).unwrap();
    for (value, expected) in [
        (Value::Bool(true), Some("true")),
        (Value::Bool(false), Some("false")),
        (Value::Int(i64::MIN), Some("-9223372036854775808")),
        (Value::Float(2.5), Some("2.5")),
        (Value::Decimal("3.250".parse().unwrap()), Some("3.250")),
        (Value::Date(date), Some("2026-10-21")),
        (Value::Time(time), Some("01:02:03")),
        (
            Value::Timestamp(date.and_time(time)),
            Some("2026-10-21 01:02:03"),
        ),
        (Value::String("你好😀".to_owned()), Some("你好😀")),
        (Value::Bytes("UTF-8😀".as_bytes().to_vec()), Some("UTF-8😀")),
        (Value::Null, None),
    ] {
        let result = scalar(value);
        let standard: &dyn ResultSet = &result;
        assert_eq!(
            standard
                .string(ColumnIndex::new(1).unwrap())
                .unwrap()
                .as_deref(),
            expected
        );
        assert_eq!(result.reads.load(Ordering::SeqCst), 1);
        assert_eq!(standard.was_null().unwrap(), expected.is_none());
        assert_eq!(
            standard.string_by_label("VALUE").unwrap().as_deref(),
            expected
        );
        assert_eq!(result.reads.load(Ordering::SeqCst), 2);
        assert_eq!(standard.was_null().unwrap(), expected.is_none());
    }
}

#[test]
fn default_string_failures_preserve_original_error_priority_and_single_read() {
    let index = ColumnIndex::new(1).unwrap();
    let result = scalar(Value::Bytes(vec![0xff]));
    data_error(result.string(index).unwrap_err(), "22021");
    assert_eq!(result.reads.load(Ordering::SeqCst), 1);
    data_error(result.string_by_label("value").unwrap_err(), "22021");
    assert_eq!(result.reads.load(Ordering::SeqCst), 2);
    assert!(!result.was_null().unwrap());
    let mut failed = scalar(Value::Bytes(vec![0xff]));
    failed.fail = true;
    let original = RdbcError::DriverError("original driver read failure".to_owned());
    assert_eq!(failed.string(index), Err(original.clone()));
    assert_eq!(failed.string_by_label("value"), Err(original));
    assert_eq!(failed.reads.load(Ordering::SeqCst), 2);
    assert!(matches!(
        failed.string_by_label("missing"),
        Err(RdbcError::InvalidArgument(_))
    ));
    assert_eq!(failed.reads.load(Ordering::SeqCst), 2);
}

#[path = "string_read_support/native_string_result_set.rs"]
mod native_string_result_set;

#[test]
fn native_string_overrides_bypass_default_scalar_conversion_and_preserve_errors() {
    use native_string_result_set::NativeStringResultSet;
    let mut result = NativeStringResultSet::new();
    let index = ColumnIndex::new(1).unwrap();
    let standard: &dyn ResultSet = &result;
    assert_eq!(standard.string(index).unwrap().as_deref(), Some("native:7"));
    assert_eq!(
        standard.string_by_label("value").unwrap().as_deref(),
        Some("native:7")
    );
    assert_eq!(result.scalar_reads.load(Ordering::SeqCst), 0);
    assert_eq!(result.string_reads.load(Ordering::SeqCst), 2);
    result.fail = true;
    let original = RdbcError::DriverError("native string failure".to_owned());
    assert_eq!(result.string(index), Err(original.clone()));
    assert_eq!(result.string_by_label("value"), Err(original));
    assert_eq!(result.scalar_reads.load(Ordering::SeqCst), 0);
    assert_eq!(result.string_reads.load(Ordering::SeqCst), 4);
}
