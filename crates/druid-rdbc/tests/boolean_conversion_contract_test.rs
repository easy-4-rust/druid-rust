use bigdecimal::BigDecimal;
use druid_rdbc::error::RdbcError;
use druid_rdbc::extensions::BufferedRowSet;
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetExt};
use druid_rdbc::types::{Row, Value};
use std::str::FromStr;

#[tokio::test]
async fn boolean_default_preserves_all_base_conversions_and_error_priority() {
    let cases = vec![
        (Value::Null, false),
        (Value::Bool(false), false),
        (Value::Bool(true), true),
        (Value::Int(0), false),
        (Value::Int(-2), true),
        (Value::Int(1), true),
        (Value::Float(0.0), false),
        (Value::Float(-0.0), false),
        (Value::Float(0.25), true),
        (Value::Float(f64::NAN), true),
        (Value::Decimal(BigDecimal::from_str("0.00").unwrap()), false),
        (Value::Decimal(BigDecimal::from_str("-0.01").unwrap()), true),
        (Value::String("1".to_owned()), true),
        (Value::String("TrUe".to_owned()), true),
        (Value::String("0".to_owned()), false),
        (Value::String("false".to_owned()), false),
        (Value::String(" true ".to_owned()), false),
        (Value::String("other".to_owned()), false),
    ];
    for (value, expected) in cases {
        let mut rows = BufferedRowSet::with_column_labels(
            vec![Row::new(vec![value.clone()])],
            vec!["flag".to_owned()],
        );
        assert!(matches!(
            rows.boolean(ColumnIndex::new(1).unwrap()),
            Err(RdbcError::InvalidArgument(_))
        ));
        assert!(rows.next().await.unwrap());
        assert_eq!(
            rows.boolean(ColumnIndex::new(1).unwrap()).unwrap(),
            expected,
            "{value:?}"
        );
        assert_eq!(rows.boolean_by_label("flag").unwrap(), expected);
        assert_eq!(rows.was_null().unwrap(), matches!(value, Value::Null));
        rows.close().await.unwrap();
        assert!(matches!(
            rows.boolean(ColumnIndex::new(1).unwrap()),
            Err(RdbcError::Other(message)) if message == "result set is closed"
        ));
    }
    let mut bytes = BufferedRowSet::new(vec![Row::new(vec![Value::Bytes(vec![1])])]);
    assert!(bytes.next().await.unwrap());
    assert!(matches!(
        bytes.boolean(ColumnIndex::new(1).unwrap()),
        Err(RdbcError::DriverError(_))
    ));
}
