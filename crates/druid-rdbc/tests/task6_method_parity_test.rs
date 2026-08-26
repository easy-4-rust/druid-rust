use druid_rdbc::extensions::BufferedRowSet;
use druid_rdbc::metadata::{ColumnNullability, ResultSetColumnType};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetExt, ResultSetUpdate};
use druid_rdbc::statement::CallableInputParameter;
use druid_rdbc::types::{RdbcCalendarArgument, RdbcCharacterLength, RdbcStreamLength, Row, Value};

#[tokio::test]
async fn buffered_row_set_preserves_navigation_labels_null_streams_and_updates() {
    let rows = vec![
        Row::new(vec![
            Value::Int(7),
            Value::Null,
            Value::String("alpha".to_owned()),
        ]),
        Row::new(vec![
            Value::Int(9),
            Value::Bytes(vec![1, 2]),
            Value::String("beta".to_owned()),
        ]),
    ];
    let mut result_set = BufferedRowSet::with_column_labels(
        rows,
        vec!["id".to_owned(), "payload".to_owned(), "name".to_owned()],
    );

    assert!(result_set.is_before_first().expect("before-first"));
    assert!(result_set.next().await.expect("first row"));
    assert_eq!(result_set.int(ColumnIndex::new(1).unwrap()).unwrap(), 7);
    assert_eq!(
        result_set.string_by_label("name").unwrap().as_deref(),
        Some("alpha")
    );
    assert_eq!(
        result_set.bytes(ColumnIndex::new(2).unwrap()).unwrap(),
        None
    );
    assert!(result_set.was_null().unwrap());

    result_set
        .update_value(ColumnIndex::new(1).unwrap(), ResultSetUpdate::Long(8))
        .unwrap();
    assert_eq!(result_set.long(ColumnIndex::new(1).unwrap()).unwrap(), 8);
    assert!(result_set.row_updated().unwrap());

    assert!(result_set.next().await.expect("second row"));
    assert_eq!(
        result_set
            .binary_stream(ColumnIndex::new(2).unwrap())
            .unwrap()
            .unwrap()
            .read_to_end()
            .unwrap(),
        vec![1, 2]
    );
    assert!(result_set.previous().await.unwrap());
    assert_eq!(result_set.row().unwrap(), 1);
    assert!(result_set.absolute(-1).await.unwrap());
    assert_eq!(result_set.row().unwrap(), 2);
    result_set.after_last().await.unwrap();
    assert!(result_set.is_after_last().unwrap());
}

#[test]
fn buffered_row_set_exposes_the_complete_result_set_metadata_contract() {
    let result_set = BufferedRowSet::with_column_labels(
        vec![Row::new(vec![Value::Int(1), Value::Null])],
        vec!["id".to_owned(), "optional".to_owned()],
    );
    let metadata = result_set.meta_data().unwrap();
    let first = ColumnIndex::new(1).unwrap();
    let second = ColumnIndex::new(2).unwrap();

    assert_eq!(metadata.column_count(), 2);
    assert_eq!(metadata.column_label(first).unwrap(), "id");
    assert_eq!(metadata.column_name(first).unwrap(), "id");
    assert_eq!(metadata.column_type_name(first).unwrap(), "BIGINT");
    assert_eq!(
        metadata.column_type(first).unwrap(),
        ResultSetColumnType::Integer
    );
    assert_eq!(
        metadata.nullability(first).unwrap(),
        ColumnNullability::NoNulls
    );
    assert_eq!(
        metadata.nullability(second).unwrap(),
        ColumnNullability::Nullable
    );
    assert!(metadata.is_searchable(first).unwrap());
    assert!(metadata.is_read_only(first).unwrap());
    assert!(!metadata.is_writable(first).unwrap());
    assert!(!metadata.is_definitely_writable(first).unwrap());
    assert_eq!(metadata.schema_name(first).unwrap(), "");
    assert_eq!(metadata.table_name(first).unwrap(), "");
    assert_eq!(metadata.catalog_name(first).unwrap(), "");
    assert_eq!(metadata.precision(first).unwrap(), 0);
    assert_eq!(metadata.scale(first).unwrap(), 0);
    assert_eq!(metadata.column_display_size(first).unwrap(), 0);
    assert!(!metadata.is_auto_increment(first).unwrap());
    assert!(!metadata.is_case_sensitive(first).unwrap());
    assert!(!metadata.is_currency(first).unwrap());
    assert!(!metadata.is_signed(first).unwrap());
    assert_eq!(metadata.column_class_name(first).unwrap(), "");
}

#[test]
fn callable_input_parameter_preserves_every_jdbc_setter_family() {
    let parameters = vec![
        CallableInputParameter::null(4),
        CallableInputParameter::null_with_type_name(2_002, "CUSTOM"),
        CallableInputParameter::Boolean(true),
        CallableInputParameter::Byte(1),
        CallableInputParameter::Short(2),
        CallableInputParameter::Int(3),
        CallableInputParameter::Long(4),
        CallableInputParameter::Float(5.0),
        CallableInputParameter::Double(6.0),
        CallableInputParameter::String(Some("text".to_owned())),
        CallableInputParameter::NString(None),
        CallableInputParameter::Bytes(Some(vec![1, 2])),
        CallableInputParameter::Url(None),
        CallableInputParameter::RowId(None),
        CallableInputParameter::SqlXml(None),
        CallableInputParameter::AsciiStream {
            stream: None,
            length: RdbcStreamLength::Int(7),
        },
        CallableInputParameter::BinaryStream {
            stream: None,
            length: RdbcStreamLength::Long(8),
        },
        CallableInputParameter::Blob(None),
        CallableInputParameter::BlobStream {
            stream: None,
            length: RdbcStreamLength::Unspecified,
        },
        CallableInputParameter::Clob(None),
        CallableInputParameter::ClobReader {
            reader: None,
            length: RdbcCharacterLength::Long(9),
        },
        CallableInputParameter::NClob(None),
        CallableInputParameter::NClobReader {
            reader: None,
            length: RdbcCharacterLength::Unspecified,
        },
        CallableInputParameter::CharacterStream {
            reader: None,
            length: RdbcCharacterLength::Int(10),
        },
        CallableInputParameter::NCharacterStream {
            reader: None,
            length: RdbcCharacterLength::Long(11),
        },
        CallableInputParameter::BigDecimal(None),
        CallableInputParameter::Date {
            value: None,
            calendar: RdbcCalendarArgument::Unspecified,
        },
        CallableInputParameter::Time {
            value: None,
            calendar: RdbcCalendarArgument::Specified(None),
        },
        CallableInputParameter::Timestamp {
            value: None,
            calendar: RdbcCalendarArgument::Unspecified,
        },
        CallableInputParameter::object(Value::String("object".to_owned())),
        CallableInputParameter::object_with_sql_type(Value::Int(12), 4),
        CallableInputParameter::object_with_sql_type_and_scale(Value::Float(13.5), 8, 2),
    ];

    assert_eq!(parameters, parameters.clone());
    assert!(matches!(
        &parameters[1],
        CallableInputParameter::Null {
            sql_type: 2_002,
            type_name: Some(type_name)
        } if type_name == "CUSTOM"
    ));
    assert!(matches!(
        parameters.last(),
        Some(CallableInputParameter::Object {
            target_sql_type: Some(8),
            scale: Some(2),
            ..
        })
    ));
}

#[tokio::test]
async fn buffered_row_set_converts_finite_numeric_values_to_big_decimal_deterministically() {
    let mut rows = BufferedRowSet::new(vec![Row::new(vec![
        Value::Float(987_654_321.125),
        Value::Int(42),
        Value::String("12.50".to_owned()),
        Value::Float(f64::INFINITY),
        Value::Float(f64::NAN),
        Value::String("not-a-decimal".to_owned()),
    ])]);
    assert!(rows.next().await.unwrap());

    assert_eq!(
        rows.big_decimal(ColumnIndex::new(1).unwrap()).unwrap(),
        Some("987654321.125".parse().unwrap())
    );
    assert_eq!(
        rows.big_decimal(ColumnIndex::new(2).unwrap()).unwrap(),
        Some("42".parse().unwrap())
    );
    assert_eq!(
        rows.big_decimal(ColumnIndex::new(3).unwrap()).unwrap(),
        Some("12.50".parse().unwrap())
    );
    assert!(rows.big_decimal(ColumnIndex::new(4).unwrap()).is_err());
    assert!(rows.big_decimal(ColumnIndex::new(5).unwrap()).is_err());
    assert!(rows.big_decimal(ColumnIndex::new(6).unwrap()).is_err());
}
