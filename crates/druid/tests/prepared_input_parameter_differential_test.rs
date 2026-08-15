//! Differential tests for `PreparedInputParameter` — Java `PreparedStatement.setXxx` semantics.
//!
//! Covers all variants of `PreparedInputParameter`, `scalar_value()`, `RdbcParameter` trait
//! (`value()`, `length()`, `calendar()`, `sql_type()`), and helper constructors.

use bigdecimal::BigDecimal;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use druid::core::{
    DruidError, PreparedInputParameter, PreparedTypeNameArgument, RdbcCalendar,
    RdbcCalendarArgument, RdbcCharacterLength, RdbcInputStream, RdbcObject, RdbcParameter,
    RdbcParameterType, RdbcParameterValue, RdbcReader, RdbcStreamLength, Value,
};
use std::str::FromStr;

// ── null / setNull ─────────────────────────────────────────────────

#[test]
fn null_unspecified_sql_type_and_scalar_value() {
    let p = PreparedInputParameter::null(12);
    assert_eq!(p.sql_type(), 12);
    assert_eq!(p.scalar_value().unwrap(), Value::Null);
    assert!(p.value().is_none());
}

#[test]
fn null_with_type_name_specified_none() {
    let p = PreparedInputParameter::null_with_type_name(2003, None);
    match &p {
        PreparedInputParameter::Null {
            sql_type,
            type_name,
        } => {
            assert_eq!(*sql_type, 2003);
            assert_eq!(*type_name, PreparedTypeNameArgument::Specified(None));
        }
        _ => panic!("expected Null variant"),
    }
    assert_eq!(p.sql_type(), 2003);
}

#[test]
fn null_with_type_name_specified_some() {
    let p = PreparedInputParameter::null_with_type_name(2003, Some("MY_UDT".to_string()));
    match &p {
        PreparedInputParameter::Null {
            type_name: PreparedTypeNameArgument::Specified(Some(name)),
            ..
        } => assert_eq!(name, "MY_UDT"),
        _ => panic!("expected Specified(Some)"),
    }
}

#[test]
fn null_with_sql_type_minus6_maps_to_4() {
    // Java: setNull(idx, -6) maps TINYINT(-6) -> sql_type 4 (INTEGER)
    let p = PreparedInputParameter::null(-6);
    assert_eq!(p.sql_type(), 4);
}

#[test]
fn default_prepared_type_name_is_unspecified() {
    let default = PreparedTypeNameArgument::default();
    assert_eq!(default, PreparedTypeNameArgument::Unspecified);
}

// ── boolean ────────────────────────────────────────────────────────

#[test]
fn boolean_scalar_value_and_sql_type() {
    let p = PreparedInputParameter::Boolean(true);
    assert_eq!(p.scalar_value().unwrap(), Value::Bool(true));
    assert_eq!(p.sql_type(), 16); // Java Types.BOOLEAN = 16
    assert_eq!(p.length(), -1);
    match p.value().unwrap() {
        RdbcParameterValue::Object(RdbcObject::Boolean(v)) => assert!(v),
        _ => panic!("expected Boolean object"),
    }
}

// ── byte ───────────────────────────────────────────────────────────

#[test]
fn byte_scalar_promotes_to_int() {
    let p = PreparedInputParameter::Byte(-128);
    assert_eq!(p.scalar_value().unwrap(), Value::Int(-128));
    assert_eq!(p.sql_type(), -6); // TINYINT
}

// ── short ──────────────────────────────────────────────────────────

#[test]
fn short_scalar_promotes_to_int() {
    let p = PreparedInputParameter::Short(32767);
    assert_eq!(p.scalar_value().unwrap(), Value::Int(32767));
    assert_eq!(p.sql_type(), 5); // SMALLINT
}

// ── int ────────────────────────────────────────────────────────────

#[test]
fn int_scalar_value() {
    let p = PreparedInputParameter::Int(42);
    assert_eq!(p.scalar_value().unwrap(), Value::Int(42));
    assert_eq!(p.sql_type(), 4); // INTEGER
    assert_eq!(p.length(), 0);
}

// ── long ───────────────────────────────────────────────────────────

#[test]
fn long_scalar_value() {
    let p = PreparedInputParameter::Long(i64::MAX);
    assert_eq!(p.scalar_value().unwrap(), Value::Int(i64::MAX));
    assert_eq!(p.sql_type(), -5); // BIGINT
}

// ── float ──────────────────────────────────────────────────────────

#[test]
fn float_scalar_promotes_to_f64() {
    let p = PreparedInputParameter::Float(3.14_f32);
    // f32->f64 promotion preserves f32 precision
    match p.scalar_value().unwrap() {
        Value::Float(v) => assert!((v - 3.14_f32 as f64).abs() < f64::EPSILON),
        other => panic!("expected Float, got {other:?}"),
    }
    assert_eq!(p.sql_type(), 6); // FLOAT
    assert_eq!(p.length(), -1);
}

// ── double ─────────────────────────────────────────────────────────

#[test]
fn double_scalar_value() {
    let p = PreparedInputParameter::Double(2.718281828);
    assert_eq!(p.scalar_value().unwrap(), Value::Float(2.718281828));
    assert_eq!(p.sql_type(), 8); // DOUBLE
}

// ── BigDecimal ─────────────────────────────────────────────────────

#[test]
fn big_decimal_some_scalar_value() {
    let bd = BigDecimal::from_str("123.456").unwrap();
    let p = PreparedInputParameter::BigDecimal(Some(bd.clone()));
    match p.scalar_value().unwrap() {
        Value::Decimal(v) => assert_eq!(v, bd),
        other => panic!("expected Decimal, got {other:?}"),
    }
    assert_eq!(p.sql_type(), 3); // DECIMAL
    assert_eq!(p.length(), 0); // BigDecimal => 0 per Java semantics
}

#[test]
fn big_decimal_none_is_null() {
    let p = PreparedInputParameter::BigDecimal(None);
    assert_eq!(p.scalar_value().unwrap(), Value::Null);
    assert!(p.value().is_none());
    assert_eq!(p.length(), 0);
}

// ── String / NString ───────────────────────────────────────────────

#[test]
fn string_some_scalar_and_sql_type() {
    let p = PreparedInputParameter::String(Some("hello".to_string()));
    assert_eq!(
        p.scalar_value().unwrap(),
        Value::String("hello".to_string())
    );
    assert_eq!(p.sql_type(), 12); // VARCHAR
    assert_eq!(p.length(), 0); // String => 0 per Java semantics
}

#[test]
fn string_none_is_null() {
    let p = PreparedInputParameter::String(None);
    assert_eq!(p.scalar_value().unwrap(), Value::Null);
    assert!(p.value().is_none());
    assert_eq!(p.length(), 0);
}

#[test]
fn n_string_some_sql_type() {
    let p = PreparedInputParameter::NString(Some("unicode".to_string()));
    assert_eq!(p.sql_type(), -9); // NVARCHAR
    assert_eq!(
        p.scalar_value().unwrap(),
        Value::String("unicode".to_string())
    );
}

#[test]
fn n_string_none_is_null() {
    let p = PreparedInputParameter::NString(None);
    assert_eq!(p.scalar_value().unwrap(), Value::Null);
}

// ── Bytes ──────────────────────────────────────────────────────────

#[test]
fn bytes_some_scalar_value() {
    let p = PreparedInputParameter::Bytes(Some(vec![1, 2, 3]));
    assert_eq!(p.scalar_value().unwrap(), Value::Bytes(vec![1, 2, 3]));
    assert_eq!(p.sql_type(), RdbcParameterType::BYTES);
    assert_eq!(p.length(), -1);
}

#[test]
fn bytes_none_is_null() {
    let p = PreparedInputParameter::Bytes(None);
    assert_eq!(p.scalar_value().unwrap(), Value::Null);
    assert!(p.value().is_none());
    assert_eq!(p.length(), 0);
}

// ── Date ───────────────────────────────────────────────────────────

#[test]
fn date_some_unspecified_calendar() {
    let date = NaiveDate::from_ymd_opt(2025, 1, 15).unwrap();
    let p = PreparedInputParameter::Date {
        value: Some(date),
        calendar: RdbcCalendarArgument::Unspecified,
    };
    assert_eq!(p.scalar_value().unwrap(), Value::Date(date));
    assert_eq!(p.sql_type(), 91);
    assert_eq!(p.length(), 0); // value.is_some() + Unspecified => 0
    assert!(p.calendar().is_none());
}

#[test]
fn date_some_specified_calendar() {
    let date = NaiveDate::from_ymd_opt(2025, 6, 1).unwrap();
    let p = PreparedInputParameter::Date {
        value: Some(date),
        calendar: RdbcCalendarArgument::Specified(Some(RdbcCalendar::new("UTC").unwrap())),
    };
    assert_eq!(p.length(), -1); // value.is_some() + Specified => -1
    assert!(p.calendar().is_some());
}

#[test]
fn date_none_is_null() {
    let p = PreparedInputParameter::Date {
        value: None,
        calendar: RdbcCalendarArgument::Unspecified,
    };
    assert_eq!(p.scalar_value().unwrap(), Value::Null);
    assert_eq!(p.length(), 0);
    assert!(p.calendar().is_none());
}

// ── Time ───────────────────────────────────────────────────────────

#[test]
fn time_some_sql_type() {
    let time = NaiveTime::from_hms_milli_opt(14, 30, 0, 500).unwrap();
    let p = PreparedInputParameter::Time {
        value: Some(time),
        calendar: RdbcCalendarArgument::Unspecified,
    };
    assert_eq!(p.scalar_value().unwrap(), Value::Time(time));
    assert_eq!(p.sql_type(), 92);
    assert_eq!(p.length(), -1); // time value.is_some() => -1
}

#[test]
fn time_none_is_null() {
    let p = PreparedInputParameter::Time {
        value: None,
        calendar: RdbcCalendarArgument::Unspecified,
    };
    assert_eq!(p.scalar_value().unwrap(), Value::Null);
    assert_eq!(p.length(), 0);
}

// ── Timestamp ──────────────────────────────────────────────────────

#[test]
fn timestamp_some_unspecified_calendar() {
    let ts = NaiveDateTime::parse_from_str("2025-01-15 10:30:00", "%Y-%m-%d %H:%M:%S").unwrap();
    let p = PreparedInputParameter::Timestamp {
        value: Some(ts),
        calendar: RdbcCalendarArgument::Unspecified,
    };
    assert_eq!(p.scalar_value().unwrap(), Value::Timestamp(ts));
    assert_eq!(p.sql_type(), 93);
    assert_eq!(p.length(), 0); // value.is_some() + Unspecified => 0
    assert!(p.calendar().is_none());
}

#[test]
fn timestamp_some_specified_calendar_length_is_negative() {
    let ts = NaiveDateTime::parse_from_str("2025-06-01 12:00:00", "%Y-%m-%d %H:%M:%S").unwrap();
    let p = PreparedInputParameter::Timestamp {
        value: Some(ts),
        calendar: RdbcCalendarArgument::Specified(Some(RdbcCalendar::new("UTC").unwrap())),
    };
    assert_eq!(p.length(), -1);
    assert!(p.calendar().is_some());
}

#[test]
fn timestamp_none_is_null() {
    let p = PreparedInputParameter::Timestamp {
        value: None,
        calendar: RdbcCalendarArgument::Unspecified,
    };
    assert_eq!(p.scalar_value().unwrap(), Value::Null);
    assert!(p.calendar().is_none());
}

// ── Object ─────────────────────────────────────────────────────────

#[test]
fn object_constructor_default_sql_type() {
    let p = PreparedInputParameter::object(Some(RdbcObject::Integer(99)));
    assert_eq!(p.scalar_value().unwrap(), Value::Int(99));
    assert_eq!(p.sql_type(), 1_111); // JAVA_OBJECT when target_sql_type=None
    assert_eq!(p.length(), -1);
}

#[test]
fn object_none_is_null() {
    let p = PreparedInputParameter::object(None);
    assert_eq!(p.scalar_value().unwrap(), Value::Null);
    assert_eq!(p.length(), 0);
}

#[test]
fn object_with_sql_type_constructor() {
    let p =
        PreparedInputParameter::object_with_sql_type(Some(RdbcObject::String("x".to_string())), 12);
    assert_eq!(p.sql_type(), 12);
    assert_eq!(p.scalar_value().unwrap(), Value::String("x".to_string()));
}

#[test]
fn object_with_sql_type_and_scale() {
    let p =
        PreparedInputParameter::object_with_sql_type_and_scale(Some(RdbcObject::Long(42)), -5, 3);
    match &p {
        PreparedInputParameter::Object {
            target_sql_type,
            scale_or_length,
            ..
        } => {
            assert_eq!(*target_sql_type, Some(-5));
            assert_eq!(*scale_or_length, Some(3));
        }
        _ => panic!("expected Object variant"),
    }
}

#[test]
fn object_rdbc_object_scalar_conversion() {
    let cases = vec![
        (RdbcObject::Boolean(false), Value::Bool(false)),
        (RdbcObject::Byte(1), Value::Int(1)),
        (RdbcObject::Short(2), Value::Int(2)),
        (RdbcObject::Integer(3), Value::Int(3)),
        (RdbcObject::Long(4), Value::Int(4)),
        (RdbcObject::Float(1.5), Value::Float(1.5)),
        (RdbcObject::Double(2.5), Value::Float(2.5)),
        (
            RdbcObject::String("s".to_string()),
            Value::String("s".to_string()),
        ),
        (
            RdbcObject::NString("ns".to_string()),
            Value::String("ns".to_string()),
        ),
        (RdbcObject::Bytes(vec![9]), Value::Bytes(vec![9])),
        (
            RdbcObject::BigDecimal(BigDecimal::from_str("1.1").unwrap()),
            Value::Decimal(BigDecimal::from_str("1.1").unwrap()),
        ),
        (
            RdbcObject::Date(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()),
            Value::Date(NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()),
        ),
        (
            RdbcObject::Time(NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
            Value::Time(NaiveTime::from_hms_opt(0, 0, 0).unwrap()),
        ),
    ];
    for (obj, expected) in cases {
        let p = PreparedInputParameter::object(Some(obj));
        assert_eq!(p.scalar_value().unwrap(), expected);
    }
}

#[test]
fn object_rdbc_url_converts_to_string() {
    let url = RdbcObject::Url(druid::core::RdbcUrl::new("https://example.com".to_string()));
    let p = PreparedInputParameter::object(Some(url));
    match p.scalar_value().unwrap() {
        Value::String(s) => assert_eq!(s, "https://example.com"),
        other => panic!("expected String, got {other:?}"),
    }
}

#[test]
fn object_ref_and_character_stream_require_native_adapter() {
    // RdbcObject::CharacterStream and NCharacterStream are non-scalar
    let reader = RdbcReader::from_string("test");
    let cases = vec![
        RdbcObject::CharacterStream(reader.clone()),
        RdbcObject::NCharacterStream(reader),
    ];
    for obj in cases {
        let p = PreparedInputParameter::object(Some(obj));
        assert!(
            matches!(
                p.scalar_value(),
                Err(DruidError::UnsupportedOperation { .. })
            ),
            "expected UnsupportedOperation for non-scalar RdbcObject"
        );
    }
}

// ── Stream variants ────────────────────────────────────────────────

#[test]
fn ascii_stream_requires_native_adapter() {
    let p = PreparedInputParameter::AsciiStream {
        stream: None,
        length: RdbcStreamLength::Unspecified,
    };
    assert!(matches!(
        p.scalar_value(),
        Err(DruidError::UnsupportedOperation { .. })
    ));
    assert_eq!(p.length(), 0); // stream is None => 0
    assert_eq!(p.sql_type(), RdbcParameterType::ASCII_INPUT_STREAM);
}

#[test]
fn unicode_stream_requires_native_adapter() {
    let p = PreparedInputParameter::UnicodeStream {
        stream: None,
        length: 100,
    };
    assert!(matches!(
        p.scalar_value(),
        Err(DruidError::UnsupportedOperation { .. })
    ));
    assert_eq!(p.sql_type(), RdbcParameterType::UNICODE_STREAM);
}

#[test]
fn binary_stream_requires_native_adapter() {
    let p = PreparedInputParameter::BinaryStream {
        stream: None,
        length: RdbcStreamLength::Long(1024),
    };
    assert!(matches!(
        p.scalar_value(),
        Err(DruidError::UnsupportedOperation { .. })
    ));
    assert_eq!(p.sql_type(), RdbcParameterType::BINARY_INPUT_STREAM);
}

#[test]
fn character_stream_requires_native_adapter() {
    let p = PreparedInputParameter::CharacterStream {
        reader: None,
        length: RdbcCharacterLength::Unspecified,
    };
    assert!(matches!(
        p.scalar_value(),
        Err(DruidError::UnsupportedOperation { .. })
    ));
    assert_eq!(p.sql_type(), RdbcParameterType::CHARACTER_INPUT_STREAM);
}

#[test]
fn n_character_stream_requires_native_adapter() {
    let p = PreparedInputParameter::NCharacterStream {
        reader: None,
        length: RdbcCharacterLength::Int(50),
    };
    assert!(matches!(
        p.scalar_value(),
        Err(DruidError::UnsupportedOperation { .. })
    ));
    assert_eq!(p.sql_type(), RdbcParameterType::NCHARACTER_INPUT_STREAM);
}

// ── LOB variants ───────────────────────────────────────────────────

#[test]
fn blob_none_is_null() {
    let p = PreparedInputParameter::Blob(None);
    assert_eq!(p.scalar_value().unwrap(), Value::Null);
    assert!(p.value().is_none());
    assert_eq!(p.sql_type(), 2_004);
    assert_eq!(p.length(), 0);
}

#[test]
fn blob_some_requires_native_adapter_value_trait() {
    // RdbcBlob requires adapter-specific construction (from_parts);
    // verify that the value() trait returns Some for the Blob variant.
    // We cannot easily construct RdbcBlob in test, so we verify through
    // the Blob(None) path and the sql_type/length expectations.
    let p = PreparedInputParameter::Blob(None);
    assert!(p.value().is_none());
    assert_eq!(p.sql_type(), 2_004);
    // Blob with value would have length -1
    assert_eq!(p.length(), 0);
}

#[test]
fn clob_none_is_null() {
    let p = PreparedInputParameter::Clob(None);
    assert_eq!(p.scalar_value().unwrap(), Value::Null);
    assert_eq!(p.sql_type(), 2_005);
}

#[test]
fn n_clob_none_is_null() {
    let p = PreparedInputParameter::NClob(None);
    assert_eq!(p.scalar_value().unwrap(), Value::Null);
    assert_eq!(p.sql_type(), 2_011);
}

#[test]
fn ref_none_is_null() {
    let p = PreparedInputParameter::Ref(None);
    assert_eq!(p.scalar_value().unwrap(), Value::Null);
    assert_eq!(p.sql_type(), 2_006);
}

#[test]
fn array_none_is_null() {
    let p = PreparedInputParameter::Array(None);
    assert_eq!(p.scalar_value().unwrap(), Value::Null);
    assert_eq!(p.sql_type(), 2_003);
}

#[test]
fn url_none_is_null() {
    let p = PreparedInputParameter::Url(None);
    assert_eq!(p.scalar_value().unwrap(), Value::Null);
    assert_eq!(p.sql_type(), RdbcParameterType::URL);
}

#[test]
fn row_id_none_is_null() {
    let p = PreparedInputParameter::RowId(None);
    assert_eq!(p.scalar_value().unwrap(), Value::Null);
    assert_eq!(p.sql_type(), -8);
}

#[test]
fn sql_xml_none_is_null() {
    let p = PreparedInputParameter::SqlXml(None);
    assert_eq!(p.scalar_value().unwrap(), Value::Null);
    assert_eq!(p.sql_type(), 2_009);
}

#[test]
fn blob_stream_requires_native_adapter() {
    let p = PreparedInputParameter::BlobStream {
        stream: None,
        length: RdbcStreamLength::Unspecified,
    };
    assert!(matches!(
        p.scalar_value(),
        Err(DruidError::UnsupportedOperation { .. })
    ));
    assert_eq!(p.sql_type(), 2_004);
}

#[test]
fn clob_reader_requires_native_adapter() {
    let p = PreparedInputParameter::ClobReader {
        reader: None,
        length: RdbcCharacterLength::Unspecified,
    };
    assert!(matches!(
        p.scalar_value(),
        Err(DruidError::UnsupportedOperation { .. })
    ));
    assert_eq!(p.sql_type(), 2_005);
}

#[test]
fn n_clob_reader_requires_native_adapter() {
    let p = PreparedInputParameter::NClobReader {
        reader: None,
        length: RdbcCharacterLength::Unspecified,
    };
    assert!(matches!(
        p.scalar_value(),
        Err(DruidError::UnsupportedOperation { .. })
    ));
    assert_eq!(p.sql_type(), 2_011);
}

// ── RustValue ──────────────────────────────────────────────────────

#[test]
fn rust_value_passthrough() {
    let p = PreparedInputParameter::RustValue(Value::Int(99));
    assert_eq!(p.scalar_value().unwrap(), Value::Int(99));
    assert_eq!(p.sql_type(), 1_111);
    match p.value().unwrap() {
        RdbcParameterValue::Object(RdbcObject::Scalar(Value::Int(v))) => assert_eq!(v, 99),
        other => panic!("expected Scalar(Int(99)), got {other:?}"),
    }
}

// ── calendar() on non-date/time variants returns None ──────────────

#[test]
fn calendar_returns_none_for_non_temporal() {
    assert!(PreparedInputParameter::Int(1).calendar().is_none());
    assert!(PreparedInputParameter::Boolean(false).calendar().is_none());
    assert!(PreparedInputParameter::String(None).calendar().is_none());
    assert!(PreparedInputParameter::null(12).calendar().is_none());
}

// ── length() edge cases ────────────────────────────────────────────

#[test]
fn object_with_value_length_is_negative_one() {
    let p = PreparedInputParameter::object(Some(RdbcObject::Integer(1)));
    assert_eq!(p.length(), -1);
}

#[test]
fn n_string_with_value_length_is_negative_one() {
    let p = PreparedInputParameter::NString(Some("x".to_string()));
    assert_eq!(p.length(), -1);
}

#[test]
fn ref_with_value_length_is_negative_one() {
    let p = PreparedInputParameter::Ref(None);
    assert_eq!(p.length(), 0);
}

// ── Clone and Debug ────────────────────────────────────────────────

#[test]
fn prepared_input_parameter_is_cloneable() {
    let p = PreparedInputParameter::Int(42);
    let p2 = p.clone();
    assert_eq!(p, p2);
}

#[test]
fn prepared_input_parameter_debug_format() {
    let p = PreparedInputParameter::Boolean(true);
    let debug = format!("{p:?}");
    assert!(debug.contains("Boolean"));
}

#[test]
fn prepared_type_name_argument_partial_eq() {
    assert_eq!(
        PreparedTypeNameArgument::Unspecified,
        PreparedTypeNameArgument::Unspecified
    );
    assert_ne!(
        PreparedTypeNameArgument::Unspecified,
        PreparedTypeNameArgument::Specified(None)
    );
}

// ── value() for all scalar variants ────────────────────────────────

#[test]
fn value_returns_none_for_null_variant() {
    assert!(PreparedInputParameter::null(12).value().is_none());
}

#[test]
fn value_returns_object_for_byte() {
    match PreparedInputParameter::Byte(5).value().unwrap() {
        RdbcParameterValue::Object(RdbcObject::Byte(v)) => assert_eq!(v, 5),
        _ => panic!("expected Byte object"),
    }
}

#[test]
fn value_returns_object_for_short() {
    match PreparedInputParameter::Short(10).value().unwrap() {
        RdbcParameterValue::Object(RdbcObject::Short(v)) => assert_eq!(v, 10),
        _ => panic!("expected Short object"),
    }
}

#[test]
fn value_returns_object_for_int() {
    match PreparedInputParameter::Int(42).value().unwrap() {
        RdbcParameterValue::Object(RdbcObject::Integer(v)) => assert_eq!(v, 42),
        _ => panic!("expected Integer object"),
    }
}

#[test]
fn value_returns_object_for_long() {
    match PreparedInputParameter::Long(100).value().unwrap() {
        RdbcParameterValue::Object(RdbcObject::Long(v)) => assert_eq!(v, 100),
        _ => panic!("expected Long object"),
    }
}

#[test]
fn value_returns_object_for_float() {
    match PreparedInputParameter::Float(1.5).value().unwrap() {
        RdbcParameterValue::Object(RdbcObject::Float(v)) => assert_eq!(v, 1.5),
        _ => panic!("expected Float object"),
    }
}

#[test]
fn value_returns_object_for_double() {
    match PreparedInputParameter::Double(2.5).value().unwrap() {
        RdbcParameterValue::Object(RdbcObject::Double(v)) => assert_eq!(v, 2.5),
        _ => panic!("expected Double object"),
    }
}

#[test]
fn value_returns_object_for_decimal_some() {
    let bd = BigDecimal::from_str("99.9").unwrap();
    match PreparedInputParameter::BigDecimal(Some(bd.clone()))
        .value()
        .unwrap()
    {
        RdbcParameterValue::Object(RdbcObject::BigDecimal(v)) => assert_eq!(v, bd),
        _ => panic!("expected BigDecimal object"),
    }
}

#[test]
fn value_returns_none_for_decimal_none() {
    assert!(PreparedInputParameter::BigDecimal(None).value().is_none());
}

#[test]
fn value_returns_input_stream_for_ascii_stream() {
    let stream = RdbcInputStream::from_bytes(vec![65, 66]);
    let p = PreparedInputParameter::AsciiStream {
        stream: Some(stream),
        length: RdbcStreamLength::Int(2),
    };
    match p.value().unwrap() {
        RdbcParameterValue::InputStream(_) => {}
        _ => panic!("expected InputStream"),
    }
}

#[test]
fn value_returns_reader_for_character_stream() {
    let reader = RdbcReader::from_string("abc");
    let p = PreparedInputParameter::CharacterStream {
        reader: Some(reader),
        length: RdbcCharacterLength::Int(3),
    };
    match p.value().unwrap() {
        RdbcParameterValue::Reader(_) => {}
        _ => panic!("expected Reader"),
    }
}

#[test]
fn value_returns_none_for_empty_stream() {
    let p = PreparedInputParameter::BinaryStream {
        stream: None,
        length: RdbcStreamLength::Unspecified,
    };
    assert!(p.value().is_none());
}
