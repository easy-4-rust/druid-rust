use druid::core::{RdbcObject, RdbcParameter, RdbcParameterString, RdbcParameterValue};
use druid::rdbc::CommonDataSource;

struct MinimalDataSource;

impl druid::rdbc::CommonDataSource for MinimalDataSource {}

// ── CommonDataSource default impls ─────────────────────────────

#[test]
fn common_datasource_login_timeout_default() {
    let ds = MinimalDataSource;
    assert_eq!(ds.login_timeout(), 0);
    assert_eq!(ds.get_login_timeout(), 0);
}

#[test]
fn common_datasource_set_login_timeout_unsupported() {
    let ds = MinimalDataSource;
    let result = ds.set_login_timeout(30);
    assert!(result.is_err());
}

#[test]
fn common_datasource_log_writer_none() {
    let ds = MinimalDataSource;
    assert!(ds.get_log_writer().is_none());
}

#[test]
fn common_datasource_set_log_writer_unsupported() {
    let ds = MinimalDataSource;
    let result = ds.set_log_writer(None);
    assert!(result.is_err());
}

#[test]
fn common_datasource_parent_logger() {
    let ds = MinimalDataSource;
    assert_eq!(ds.parent_logger(), "druid::rdbc");
    assert_eq!(ds.get_parent_logger(), "druid::rdbc");
}

// ── RdbcParameterString ────────────────────────────────────────

#[test]
fn rdbc_parameter_string_new_some() {
    let p = RdbcParameterString::new(Some("hello".to_owned()));
    match p.value() {
        Some(RdbcParameterValue::Object(RdbcObject::String(s))) => assert_eq!(s, "hello"),
        other => panic!("expected String, got {other:?}"),
    }
}

#[test]
fn rdbc_parameter_string_new_none() {
    let p = RdbcParameterString::new(None);
    assert!(p.value().is_none());
}

#[test]
fn rdbc_parameter_string_empty() {
    let p = RdbcParameterString::empty();
    match p.value() {
        Some(RdbcParameterValue::Object(RdbcObject::String(s))) => assert!(s.is_empty()),
        other => panic!("expected empty String, got {other:?}"),
    }
}

#[test]
fn rdbc_parameter_string_length() {
    let p = RdbcParameterString::new(Some("test".to_owned()));
    assert_eq!(p.length(), 0);
}

#[test]
fn rdbc_parameter_string_sql_type() {
    let p = RdbcParameterString::new(Some("test".to_owned()));
    assert_eq!(p.sql_type(), 12);
}

#[test]
fn rdbc_parameter_string_calendar() {
    let p = RdbcParameterString::new(Some("test".to_owned()));
    assert!(p.calendar().is_none());
}

#[test]
fn rdbc_parameter_string_clone_eq() {
    let p1 = RdbcParameterString::new(Some("hello".to_owned()));
    let p2 = p1.clone();
    assert_eq!(p1, p2);
}

#[test]
fn rdbc_parameter_string_debug() {
    let p = RdbcParameterString::new(Some("test".to_owned()));
    let dbg = format!("{:?}", p);
    assert!(dbg.contains("RdbcParameterString"));
}
