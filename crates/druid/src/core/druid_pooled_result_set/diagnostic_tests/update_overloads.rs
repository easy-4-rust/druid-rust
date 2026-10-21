//! 结果集诊断：update_overloads 行为族。
//!
//! 自 `diagnostic_tests.rs` 按行为族拆分；共享夹具见 `harness`。

use super::harness::*;
use super::*;

/// 完整保留 BASE 56 次更新及流身份，重载映射为标准描述符。
/// 对应 Java: java.sql.ResultSet#updateXxx。
#[tokio::test]
async fn task6_all_scalar_and_stream_update_overloads_preserve_raw_spi_argument_identity() {
    let (statement, mut connection) = task6_sqlite_diagnostic_statement_with_filters(None).await;
    let probe = Arc::new(std::sync::Mutex::new(Vec::new()));
    let admission = statement.inner.begin_standard_admission().unwrap();
    let mut result_set = DruidPooledResultSet::new_standard(
        Arc::clone(&statement.inner),
        Box::new(update_identity_probe::UpdateIdentityProbe::new(Arc::clone(
            &probe,
        ))),
        &admission,
    )
    .await
    .unwrap();
    drop(admission);
    let date = NaiveDate::from_ymd_opt(2026, 7, 29).unwrap();
    let time = NaiveTime::from_hms_nano_opt(13, 14, 15, 123_456_789).unwrap();
    let timestamp = date.and_time(time);
    let input = RdbcInputStream::from_bytes([1, 2, 3]);
    let reader = RdbcReader::from_string("严格流身份");
    let vendor_object =
        RdbcOpaqueObject::new(Arc::new(vendor_object_probe::VendorObjectProbe { id: 99 }));
    assert_eq!(
        vendor_object.downcast_ref::<vendor_object_probe::VendorObjectProbe>(),
        Some(&vendor_object_probe::VendorObjectProbe { id: 99 })
    );

    macro_rules! assert_last_update {
        ($expression:expr, $expected:expr) => {{
            $expression.unwrap();
            assert_eq!(
                probe.lock().unwrap().clone().last().cloned(),
                Some($expected)
            );
        }};
    }

    assert_last_update!(
        result_set.update_value(ColumnIndex::new(1).unwrap(), ResultSetUpdate::Null),
        update_call::UpdateCall::Index(1, ResultSetUpdate::Null)
    );
    assert_last_update!(
        result_set.update_value_by_label("null_value", ResultSetUpdate::Null),
        update_call::UpdateCall::Label("null_value".to_string(), ResultSetUpdate::Null)
    );

    macro_rules! assert_scalar_pair {
        (
            $variant:ident, $index_method:ident, $label_method:ident, $index:expr, $label:literal,
            $index_value:expr, $label_value:expr, $index_update:expr, $label_update:expr
        ) => {
            assert_last_update!(
                result_set.update_value(
                    ColumnIndex::new($index).unwrap(),
                    ResultSetUpdate::$variant($index_value)
                ),
                update_call::UpdateCall::Index($index, $index_update)
            );
            assert_last_update!(
                result_set.update_value_by_label($label, ResultSetUpdate::$variant($label_value)),
                update_call::UpdateCall::Label($label.to_string(), $label_update)
            );
        };
    }

    assert_scalar_pair!(
        Boolean,
        update_boolean,
        update_boolean_by_label,
        2,
        "boolean_value",
        true,
        false,
        ResultSetUpdate::Boolean(true),
        ResultSetUpdate::Boolean(false)
    );
    assert_scalar_pair!(
        Byte,
        update_byte,
        update_byte_by_label,
        3,
        "byte_value",
        -8,
        8,
        ResultSetUpdate::Byte(-8),
        ResultSetUpdate::Byte(8)
    );
    assert_scalar_pair!(
        Short,
        update_short,
        update_short_by_label,
        4,
        "short_value",
        -16,
        16,
        ResultSetUpdate::Short(-16),
        ResultSetUpdate::Short(16)
    );
    assert_scalar_pair!(
        Int,
        update_int,
        update_int_by_label,
        5,
        "int_value",
        -32,
        32,
        ResultSetUpdate::Int(-32),
        ResultSetUpdate::Int(32)
    );
    assert_scalar_pair!(
        Long,
        update_long,
        update_long_by_label,
        6,
        "long_value",
        -64,
        64,
        ResultSetUpdate::Long(-64),
        ResultSetUpdate::Long(64)
    );
    assert_scalar_pair!(
        Float,
        update_float,
        update_float_by_label,
        7,
        "float_value",
        1.25,
        -1.25,
        ResultSetUpdate::Float(1.25),
        ResultSetUpdate::Float(-1.25)
    );
    assert_scalar_pair!(
        Double,
        update_double,
        update_double_by_label,
        8,
        "double_value",
        2.5,
        -2.5,
        ResultSetUpdate::Double(2.5),
        ResultSetUpdate::Double(-2.5)
    );
    let decimal = BigDecimal::from_str("1234567890.123456789").unwrap();
    assert_scalar_pair!(
        BigDecimal,
        update_big_decimal,
        update_big_decimal_by_label,
        9,
        "decimal_value",
        Some(decimal.clone()),
        None,
        ResultSetUpdate::BigDecimal(Some(decimal)),
        ResultSetUpdate::BigDecimal(None)
    );
    assert_scalar_pair!(
        String,
        update_string,
        update_string_by_label,
        10,
        "string_value",
        Some("index".to_string()),
        None,
        ResultSetUpdate::String(Some("index".to_string())),
        ResultSetUpdate::String(None)
    );
    assert_scalar_pair!(
        Bytes,
        update_bytes,
        update_bytes_by_label,
        11,
        "bytes_value",
        Some(vec![0, 255]),
        None,
        ResultSetUpdate::Bytes(Some(vec![0, 255])),
        ResultSetUpdate::Bytes(None)
    );
    assert_scalar_pair!(
        Date,
        update_date,
        update_date_by_label,
        12,
        "date_value",
        Some(date),
        None,
        ResultSetUpdate::Date(Some(date)),
        ResultSetUpdate::Date(None)
    );
    assert_scalar_pair!(
        Time,
        update_time,
        update_time_by_label,
        13,
        "time_value",
        Some(time),
        None,
        ResultSetUpdate::Time(Some(time)),
        ResultSetUpdate::Time(None)
    );
    assert_scalar_pair!(
        Timestamp,
        update_timestamp,
        update_timestamp_by_label,
        14,
        "timestamp_value",
        Some(timestamp),
        None,
        ResultSetUpdate::Timestamp(Some(timestamp)),
        ResultSetUpdate::Timestamp(None)
    );
    assert_last_update!(
        result_set.update_value(
            ColumnIndex::new(15).unwrap(),
            ResultSetUpdate::Object(RdbcObject::Custom(vendor_object.clone()))
        ),
        update_call::UpdateCall::Index(
            15,
            ResultSetUpdate::Object(RdbcObject::Custom(vendor_object))
        )
    );
    assert_last_update!(
        result_set
            .update_value_by_label("object_value", ResultSetUpdate::Object(Value::Null.into())),
        update_call::UpdateCall::Label(
            "object_value".to_string(),
            ResultSetUpdate::Object(Value::Null.into())
        )
    );
    assert_last_update!(
        result_set.update_value(
            ColumnIndex::new(16).unwrap(),
            ResultSetUpdate::ObjectWithScaleOrLength {
                value: Value::Decimal(BigDecimal::from(7)).into(),
                scale_or_length: -3
            }
        ),
        update_call::UpdateCall::Index(
            16,
            ResultSetUpdate::ObjectWithScaleOrLength {
                value: Value::Decimal(BigDecimal::from(7)).into(),
                scale_or_length: -3,
            }
        )
    );
    assert_last_update!(
        result_set.update_value_by_label(
            "scaled_object",
            ResultSetUpdate::ObjectWithScaleOrLength {
                value: Value::Bytes(vec![9]).into(),
                scale_or_length: 99
            }
        ),
        update_call::UpdateCall::Label(
            "scaled_object".to_string(),
            ResultSetUpdate::ObjectWithScaleOrLength {
                value: Value::Bytes(vec![9]).into(),
                scale_or_length: 99,
            }
        )
    );
    assert_scalar_pair!(
        NString,
        update_n_string,
        update_n_string_by_label,
        17,
        "n_string_value",
        Some("国家".to_string()),
        None,
        ResultSetUpdate::NString(Some("国家".to_string())),
        ResultSetUpdate::NString(None)
    );

    macro_rules! assert_stream_call {
        ($expression:expr, $target:expr, $variant:ident, $length:expr) => {
            assert_last_update!(
                $expression,
                $target(ResultSetUpdate::$variant {
                    stream: Some(input.clone()),
                    length: $length,
                })
            );
        };
    }
    assert_stream_call!(
        result_set.update_value(
            ColumnIndex::new(18).unwrap(),
            ResultSetUpdate::AsciiStream {
                stream: Some(&input).cloned(),
                length: RdbcStreamLength::Unspecified
            }
        ),
        |update| update_call::UpdateCall::Index(18, update),
        AsciiStream,
        RdbcStreamLength::Unspecified
    );
    assert_stream_call!(
        result_set.update_value_by_label(
            "ascii",
            ResultSetUpdate::AsciiStream {
                stream: Some(&input).cloned(),
                length: RdbcStreamLength::Unspecified
            }
        ),
        |update| update_call::UpdateCall::Label("ascii".to_string(), update),
        AsciiStream,
        RdbcStreamLength::Unspecified
    );
    assert_stream_call!(
        result_set.update_value(
            ColumnIndex::new(19).unwrap(),
            ResultSetUpdate::AsciiStream {
                stream: Some(&input).cloned(),
                length: RdbcStreamLength::Int(-19)
            }
        ),
        |update| update_call::UpdateCall::Index(19, update),
        AsciiStream,
        RdbcStreamLength::Int(-19)
    );
    assert_stream_call!(
        result_set.update_value_by_label(
            "ascii_int",
            ResultSetUpdate::AsciiStream {
                stream: Some(&input).cloned(),
                length: RdbcStreamLength::Int(20)
            }
        ),
        |update| update_call::UpdateCall::Label("ascii_int".to_string(), update),
        AsciiStream,
        RdbcStreamLength::Int(20)
    );
    assert_stream_call!(
        result_set.update_value(
            ColumnIndex::new(20).unwrap(),
            ResultSetUpdate::AsciiStream {
                stream: Some(&input).cloned(),
                length: RdbcStreamLength::Long(-21)
            }
        ),
        |update| update_call::UpdateCall::Index(20, update),
        AsciiStream,
        RdbcStreamLength::Long(-21)
    );
    assert_stream_call!(
        result_set.update_value_by_label(
            "ascii_long",
            ResultSetUpdate::AsciiStream {
                stream: Some(&input).cloned(),
                length: RdbcStreamLength::Long(22)
            }
        ),
        |update| update_call::UpdateCall::Label("ascii_long".to_string(), update),
        AsciiStream,
        RdbcStreamLength::Long(22)
    );
    assert_stream_call!(
        result_set.update_value(
            ColumnIndex::new(21).unwrap(),
            ResultSetUpdate::BinaryStream {
                stream: Some(&input).cloned(),
                length: RdbcStreamLength::Unspecified
            }
        ),
        |update| update_call::UpdateCall::Index(21, update),
        BinaryStream,
        RdbcStreamLength::Unspecified
    );
    assert_stream_call!(
        result_set.update_value_by_label(
            "binary",
            ResultSetUpdate::BinaryStream {
                stream: Some(&input).cloned(),
                length: RdbcStreamLength::Unspecified
            }
        ),
        |update| update_call::UpdateCall::Label("binary".to_string(), update),
        BinaryStream,
        RdbcStreamLength::Unspecified
    );
    assert_stream_call!(
        result_set.update_value(
            ColumnIndex::new(22).unwrap(),
            ResultSetUpdate::BinaryStream {
                stream: Some(&input).cloned(),
                length: RdbcStreamLength::Int(-23)
            }
        ),
        |update| update_call::UpdateCall::Index(22, update),
        BinaryStream,
        RdbcStreamLength::Int(-23)
    );
    assert_stream_call!(
        result_set.update_value_by_label(
            "binary_int",
            ResultSetUpdate::BinaryStream {
                stream: Some(&input).cloned(),
                length: RdbcStreamLength::Int(24)
            }
        ),
        |update| update_call::UpdateCall::Label("binary_int".to_string(), update),
        BinaryStream,
        RdbcStreamLength::Int(24)
    );
    assert_stream_call!(
        result_set.update_value(
            ColumnIndex::new(23).unwrap(),
            ResultSetUpdate::BinaryStream {
                stream: Some(&input).cloned(),
                length: RdbcStreamLength::Long(-25)
            }
        ),
        |update| update_call::UpdateCall::Index(23, update),
        BinaryStream,
        RdbcStreamLength::Long(-25)
    );
    assert_stream_call!(
        result_set.update_value_by_label(
            "binary_long",
            ResultSetUpdate::BinaryStream {
                stream: Some(&input).cloned(),
                length: RdbcStreamLength::Long(26)
            }
        ),
        |update| update_call::UpdateCall::Label("binary_long".to_string(), update),
        BinaryStream,
        RdbcStreamLength::Long(26)
    );

    macro_rules! assert_reader_call {
        ($expression:expr, $target:expr, $variant:ident, $length:expr) => {
            assert_last_update!(
                $expression,
                $target(ResultSetUpdate::$variant {
                    reader: Some(reader.clone()),
                    length: $length,
                })
            );
        };
    }
    assert_reader_call!(
        result_set.update_value(
            ColumnIndex::new(24).unwrap(),
            ResultSetUpdate::CharacterStream {
                reader: Some(&reader).cloned(),
                length: RdbcCharacterLength::Unspecified
            }
        ),
        |update| update_call::UpdateCall::Index(24, update),
        CharacterStream,
        RdbcCharacterLength::Unspecified
    );
    assert_reader_call!(
        result_set.update_value_by_label(
            "character",
            ResultSetUpdate::CharacterStream {
                reader: Some(&reader).cloned(),
                length: RdbcCharacterLength::Unspecified
            }
        ),
        |update| update_call::UpdateCall::Label("character".to_string(), update),
        CharacterStream,
        RdbcCharacterLength::Unspecified
    );
    assert_reader_call!(
        result_set.update_value(
            ColumnIndex::new(25).unwrap(),
            ResultSetUpdate::CharacterStream {
                reader: Some(&reader).cloned(),
                length: RdbcCharacterLength::Int(-27)
            }
        ),
        |update| update_call::UpdateCall::Index(25, update),
        CharacterStream,
        RdbcCharacterLength::Int(-27)
    );
    assert_reader_call!(
        result_set.update_value_by_label(
            "character_int",
            ResultSetUpdate::CharacterStream {
                reader: Some(&reader).cloned(),
                length: RdbcCharacterLength::Int(28)
            }
        ),
        |update| update_call::UpdateCall::Label("character_int".to_string(), update),
        CharacterStream,
        RdbcCharacterLength::Int(28)
    );
    assert_reader_call!(
        result_set.update_value(
            ColumnIndex::new(26).unwrap(),
            ResultSetUpdate::CharacterStream {
                reader: Some(&reader).cloned(),
                length: RdbcCharacterLength::Long(-29)
            }
        ),
        |update| update_call::UpdateCall::Index(26, update),
        CharacterStream,
        RdbcCharacterLength::Long(-29)
    );
    assert_reader_call!(
        result_set.update_value_by_label(
            "character_long",
            ResultSetUpdate::CharacterStream {
                reader: Some(&reader).cloned(),
                length: RdbcCharacterLength::Long(30)
            }
        ),
        |update| update_call::UpdateCall::Label("character_long".to_string(), update),
        CharacterStream,
        RdbcCharacterLength::Long(30)
    );
    assert_reader_call!(
        result_set.update_value(
            ColumnIndex::new(27).unwrap(),
            ResultSetUpdate::NCharacterStream {
                reader: Some(&reader).cloned(),
                length: RdbcCharacterLength::Unspecified
            }
        ),
        |update| update_call::UpdateCall::Index(27, update),
        NCharacterStream,
        RdbcCharacterLength::Unspecified
    );
    assert_reader_call!(
        result_set.update_value_by_label(
            "n_character",
            ResultSetUpdate::NCharacterStream {
                reader: Some(&reader).cloned(),
                length: RdbcCharacterLength::Unspecified
            }
        ),
        |update| update_call::UpdateCall::Label("n_character".to_string(), update),
        NCharacterStream,
        RdbcCharacterLength::Unspecified
    );
    assert_reader_call!(
        result_set.update_value(
            ColumnIndex::new(28).unwrap(),
            ResultSetUpdate::NCharacterStream {
                reader: Some(&reader).cloned(),
                length: RdbcCharacterLength::Long(-31)
            }
        ),
        |update| update_call::UpdateCall::Index(28, update),
        NCharacterStream,
        RdbcCharacterLength::Long(-31)
    );
    assert_reader_call!(
        result_set.update_value_by_label(
            "n_character_long",
            ResultSetUpdate::NCharacterStream {
                reader: Some(&reader).cloned(),
                length: RdbcCharacterLength::Long(32)
            }
        ),
        |update| update_call::UpdateCall::Label("n_character_long".to_string(), update),
        NCharacterStream,
        RdbcCharacterLength::Long(32)
    );

    assert_eq!(probe.lock().unwrap().clone().len(), 56);
    assert!(!input.is_closed());
    assert!(!reader.is_closed());
    result_set.close().await.unwrap();
    let mut statement = statement;
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}

/// 动态标准入口保留列名，同时拒绝隐藏列、缺失映射和关闭租约。
#[tokio::test]
async fn task6_update_label_preserves_dynamic_identity_and_column_governance() {
    let (mut statement, mut connection) =
        task6_sqlite_diagnostic_statement_with_filters(None).await;
    let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
    let admission = statement.inner.begin_standard_admission().unwrap();
    let mut rows = DruidPooledResultSet::new_standard(
        Arc::clone(&statement.inner),
        Box::new(update_identity_probe::UpdateIdentityProbe::new(Arc::clone(
            &calls,
        ))),
        &admission,
    )
    .await
    .unwrap();
    drop(admission);
    rows.set_hidden_columns(Some(vec![2]));
    assert!(
        matches!(rows.update_value_by_label("mapped", ResultSetUpdate::Null), Err(druid_rdbc::error::RdbcError::InvalidArgument(message)) if message == "column is hidden")
    );
    assert!(calls.lock().unwrap().is_empty());
    rows.set_hidden_columns(None);
    rows.set_logic_column_map(Some(HashMap::from([(7, 2)])));
    assert!(
        matches!(rows.update_value_by_label("mapped", ResultSetUpdate::Null), Err(druid_rdbc::error::RdbcError::InvalidArgument(message)) if message == "column is hidden")
    );
    assert!(calls.lock().unwrap().is_empty());
    rows.set_physical_column_map(Some(HashMap::from([(2, 7)])));
    rows.update_value_by_label("mapped", ResultSetUpdate::Int(31))
        .unwrap();
    assert_eq!(
        calls.lock().unwrap().last().cloned(),
        Some(update_call::UpdateCall::Label(
            "mapped".to_owned(),
            ResultSetUpdate::Int(31)
        ))
    );
    rows.set_logic_column_map(None);
    rows.set_physical_column_map(None);
    let alias = DruidPooledResultSet::from_tracked_resource(
        Arc::clone(&statement.inner),
        Arc::clone(&rows.resource),
    )
    .unwrap();
    let mut standard: Box<dyn ResultSet> = Box::new(alias);
    standard
        .update_value_by_label("raw_label", ResultSetUpdate::String(None))
        .unwrap();
    assert_eq!(
        calls.lock().unwrap().last().cloned(),
        Some(update_call::UpdateCall::Label(
            "raw_label".to_owned(),
            ResultSetUpdate::String(None)
        ))
    );
    standard.close().await.unwrap();
    assert!(matches!(
        rows.update_value_by_label("raw_label", ResultSetUpdate::Null),
        Err(druid_rdbc::error::RdbcError::ConnectionDiscarded)
    ));
    assert_eq!(calls.lock().unwrap().len(), 2);
    statement.close().await.unwrap();
    connection.close().await.unwrap();
}
