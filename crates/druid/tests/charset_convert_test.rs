use druid::core::CharsetConvert;

#[test]
fn charset_convert_new_same_encoding() {
    let c = CharsetConvert::new(Some("utf-8"), Some("utf-8")).unwrap();
    assert!(!c.is_enabled());
    assert_eq!(c.client_encoding(), Some("UTF-8"));
    assert_eq!(c.server_encoding(), Some("UTF-8"));
}

#[test]
fn charset_convert_new_different_encoding() {
    let c = CharsetConvert::new(Some("utf-8"), Some("gbk")).unwrap();
    assert!(c.is_enabled());
    assert_eq!(c.client_encoding(), Some("UTF-8"));
    assert_eq!(c.server_encoding(), Some("GBK"));
}

#[test]
fn charset_convert_new_none() {
    let c = CharsetConvert::new(None, None).unwrap();
    assert!(!c.is_enabled());
    assert!(c.client_encoding().is_none());
    assert!(c.server_encoding().is_none());
}

#[test]
fn charset_convert_new_invalid_encoding() {
    let result = CharsetConvert::new(Some("invalid-charset"), Some("utf-8"));
    assert!(result.is_err());
}

#[test]
fn charset_convert_encode_disabled() {
    let c = CharsetConvert::new(None, None).unwrap();
    let result = c.encode("hello").unwrap();
    assert_eq!(result, "hello");
}

#[test]
fn charset_convert_decode_disabled() {
    let c = CharsetConvert::new(None, None).unwrap();
    let result = c.decode("hello").unwrap();
    assert_eq!(result, "hello");
}

#[test]
fn charset_convert_encode_empty() {
    let c = CharsetConvert::new(Some("utf-8"), Some("gbk")).unwrap();
    let result = c.encode("").unwrap();
    assert_eq!(result, "");
}

#[test]
fn charset_convert_decode_empty() {
    let c = CharsetConvert::new(Some("utf-8"), Some("gbk")).unwrap();
    let result = c.decode("").unwrap();
    assert_eq!(result, "");
}

#[test]
fn charset_convert_encode_decode_roundtrip() {
    let c = CharsetConvert::new(Some("utf-8"), Some("gbk")).unwrap();
    let original = "hello world";
    let encoded = c.encode(original).unwrap();
    let decoded = c.decode(&encoded).unwrap();
    assert_eq!(decoded, original);
}

#[test]
fn charset_convert_clone() {
    let c1 = CharsetConvert::new(Some("utf-8"), Some("gbk")).unwrap();
    let c2 = c1.clone();
    assert_eq!(c1.is_enabled(), c2.is_enabled());
}

#[test]
fn charset_convert_debug() {
    let c = CharsetConvert::new(Some("utf-8"), Some("utf-8")).unwrap();
    let dbg = format!("{c:?}");
    assert!(dbg.contains("CharsetConvert"));
}
