use druid::core::{EncodingConvertFilter, MySQL8DateTimeSqlTypeFilter, Value};

// ── EncodingConvertFilter ──────────────────────────────────────

#[test]
fn encoding_filter_constants() {
    assert_eq!(
        EncodingConvertFilter::ATTR_CHARSET_CONVERTER,
        "ali.charset.converter"
    );
    assert_eq!(EncodingConvertFilter::CLIENT_ENCODING_KEY, "clientEncoding");
    assert_eq!(EncodingConvertFilter::SERVER_ENCODING_KEY, "serverEncoding");
}

#[test]
fn encoding_filter_new_same_encoding() {
    let f = EncodingConvertFilter::new(Some("UTF-8"), Some("UTF-8")).unwrap();
    let encoded = f.encode("hello").unwrap();
    assert_eq!(encoded, "hello");
}

#[test]
fn encoding_filter_new_none_encodings() {
    let f = EncodingConvertFilter::new(None, None).unwrap();
    let encoded = f.encode("hello").unwrap();
    assert_eq!(encoded, "hello");
}

#[test]
fn encoding_filter_encode_decode_roundtrip() {
    let f = EncodingConvertFilter::new(None, None).unwrap();
    let original = "test string";
    let encoded = f.encode(original).unwrap();
    let decoded = f.decode(&encoded).unwrap();
    assert_eq!(decoded, original);
}

#[test]
fn encoding_filter_encode_value_string() {
    let f = EncodingConvertFilter::new(None, None).unwrap();
    let val = Value::String("hello".to_owned());
    let encoded = f.encode_value(val).unwrap();
    match encoded {
        Value::String(s) => assert_eq!(s, "hello"),
        other => panic!("expected String, got {other:?}"),
    }
}

#[test]
fn encoding_filter_encode_value_non_string() {
    let f = EncodingConvertFilter::new(None, None).unwrap();
    let val = Value::Int(42);
    let encoded = f.encode_value(val).unwrap();
    match encoded {
        Value::Int(v) => assert_eq!(v, 42),
        other => panic!("expected Int, got {other:?}"),
    }
}

#[test]
fn encoding_filter_decode_value_string() {
    let f = EncodingConvertFilter::new(None, None).unwrap();
    let val = Value::String("hello".to_owned());
    let decoded = f.decode_value(val).unwrap();
    match decoded {
        Value::String(s) => assert_eq!(s, "hello"),
        other => panic!("expected String, got {other:?}"),
    }
}

#[test]
fn encoding_filter_decode_value_non_string() {
    let f = EncodingConvertFilter::new(None, None).unwrap();
    let val = Value::Int(99);
    let decoded = f.decode_value(val).unwrap();
    match decoded {
        Value::Int(v) => assert_eq!(v, 99),
        other => panic!("expected Long, got {other:?}"),
    }
}

#[test]
fn encoding_filter_debug() {
    let f = EncodingConvertFilter::new(None, None).unwrap();
    let dbg = format!("{:?}", f);
    assert!(dbg.contains("EncodingConvertFilter"));
}

// ── MySQL8DateTimeSqlTypeFilter ────────────────────────────────

#[test]
fn mysql8_filter_new() {
    let f = MySQL8DateTimeSqlTypeFilter::new();
    let _ = f;
}

#[test]
fn mysql8_filter_default() {
    let f = MySQL8DateTimeSqlTypeFilter::default();
    let _ = f;
}

#[test]
fn mysql8_filter_get_object_replace_identity() {
    let val = Value::String("test".to_owned());
    let result = MySQL8DateTimeSqlTypeFilter::get_object_replace_local_date_time(val.clone());
    assert_eq!(format!("{:?}", val), format!("{:?}", result));
}

#[test]
fn mysql8_filter_clone_copy() {
    let f = MySQL8DateTimeSqlTypeFilter::new();
    let f2 = f;
    let _ = f2;
}

#[test]
fn mysql8_filter_debug() {
    let f = MySQL8DateTimeSqlTypeFilter::new();
    let dbg = format!("{:?}", f);
    assert!(dbg.contains("MySQL8DateTimeSqlTypeFilter"));
}
