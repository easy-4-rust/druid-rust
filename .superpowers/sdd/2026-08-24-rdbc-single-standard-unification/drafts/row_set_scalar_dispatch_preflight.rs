//! 独立诊断原完整案例的标量转换缺口，不替代完整迁移案例验收。
use druid_rdbc::extensions::BufferedRowSet;
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetExt};
use druid_rdbc::types::{RdbcBigDecimal, Row, Value};
use std::future::Future;
use std::str::FromStr;
use std::task::{Context, Poll, Waker};

fn positioned_row_set(values: Vec<Value>) -> BufferedRowSet {
    let mut rows = BufferedRowSet::new(vec![Row::new(values)]);
    let mut context = Context::from_waker(Waker::noop());
    let mut next = Box::pin(rows.next());
    assert!(matches!(
        next.as_mut().poll(&mut context),
        Poll::Ready(Ok(true))
    ));
    drop(next);
    rows
}

#[test]
fn original_row_set_long_conversion_preflight() {
    let rows = positioned_row_set(vec![
        Value::Null,
        Value::Bool(true),
        Value::Float(2.5),
        Value::Decimal(RdbcBigDecimal::from_str("3.25").unwrap()),
        Value::String("17".to_owned()),
    ]);
    for (offset, expected) in [0, 1, 2, 3, 17].into_iter().enumerate() {
        assert_eq!(
            rows.long(ColumnIndex::new(offset + 1).unwrap()),
            Ok(expected)
        );
    }
}

#[test]
fn original_row_set_double_conversion_preflight() {
    let rows = positioned_row_set(vec![
        Value::Null,
        Value::Bool(true),
        Value::Int(42),
        Value::Decimal(RdbcBigDecimal::from_str("3.25").unwrap()),
        Value::String("17".to_owned()),
    ]);
    for (offset, expected) in [0.0, 1.0, 42.0, 3.25, 17.0].into_iter().enumerate() {
        assert_eq!(
            rows.double(ColumnIndex::new(offset + 1).unwrap()),
            Ok(expected)
        );
    }
}

#[test]
fn original_row_set_bytes_conversion_preflight() {
    let rows = positioned_row_set(vec![
        Value::Null,
        Value::Bytes(b"bytes".to_vec()),
        Value::Int(42),
    ]);
    assert_eq!(rows.bytes(ColumnIndex::new(1).unwrap()), Ok(None));
    assert_eq!(
        rows.bytes(ColumnIndex::new(2).unwrap()),
        Ok(Some(b"bytes".to_vec()))
    );
    assert_eq!(
        rows.bytes(ColumnIndex::new(3).unwrap()),
        Ok(Some(b"42".to_vec()))
    );
}
