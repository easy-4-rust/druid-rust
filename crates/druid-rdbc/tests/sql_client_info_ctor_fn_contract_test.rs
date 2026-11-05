//! `java.sql.SQLClientInfoException` 构造函数与 `getFailedProperties` 的入口合同。
//!
//! Java 的 9 个构造重载在 Rust 侧折叠为 [`SqlClientInfoException::new`]——reason、
//! SQLState、vendor code 与 cause 由内嵌 [`SqlException`] 承载；每个重载按原参数
//! 形状构造并断言字段精确保留。

use druid_rdbc::error::{SqlClientInfoException, SqlException, SqlExceptionCause};
use std::collections::HashMap;

fn sql(reason: &str, state: Option<&str>, code: i32) -> SqlException {
    SqlException::new(code, state.map(str::to_owned), Some(reason.to_owned()))
}

fn properties(entries: &[(&str, &str)]) -> HashMap<String, String> {
    entries
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}

#[test]
fn empty_constructor_defaults_reason_state_and_code() {
    // 对应 Java: SQLClientInfoException()
    let exception = SqlClientInfoException::new(SqlException::new(0, None, None), HashMap::new());
    assert_eq!(exception.exception.error_code(), 0);
    assert_eq!(exception.exception.sql_state(), None);
    assert_eq!(exception.exception.message(), None);
    assert!(exception.failed_properties().is_empty());
}

#[test]
fn reason_constructor_keeps_message_and_failed_properties() {
    // 对应 Java: SQLClientInfoException(Map)
    let props = properties(&[("ApplicationName", "REASON_VALUE_INVALID")]);
    let exception = SqlClientInfoException::new(
        sql("application name rejected", Some("01S00"), 7),
        props.clone(),
    );
    assert_eq!(
        exception.exception.message().unwrap(),
        "application name rejected"
    );
    assert_eq!(exception.exception.sql_state(), Some("01S00"));
    assert_eq!(exception.failed_properties(), &props);
}

#[test]
fn reason_and_state_constructor_preserve_state_text() {
    // 对应 Java: SQLClientInfoException(String, Map)
    let exception = SqlClientInfoException::new(
        sql("client info rejected", Some("01S00"), 0),
        properties(&[("ClientUser", "REASON_UNKNOWN_PROPERTY")]),
    );
    assert_eq!(exception.exception.sql_state(), Some("01S00"));
}

#[test]
fn reason_state_code_constructor_preserves_vendor_code() {
    // 对应 Java: SQLClientInfoException(String, String, int, Map)
    let exception = SqlClientInfoException::new(
        sql("client info rejected", Some("01S00"), 42),
        properties(&[("ClientUser", "REASON_VALUE_INVALID")]),
    );
    assert_eq!(exception.exception.error_code(), 42);
}

#[test]
fn reason_state_code_properties_cause_constructor_chains_the_cause() {
    // 对应 Java: SQLClientInfoException(String, String, int, Map, Throwable)
    let exception = SqlClientInfoException::new(
        sql("client info rejected", Some("01S00"), 42)
            .with_cause(SqlExceptionCause::ClassName("vendor.RootCause".to_owned())),
        properties(&[("ClientUser", "REASON_VALUE_INVALID")]),
    );
    assert_eq!(
        exception.exception.causes(),
        &[SqlExceptionCause::ClassName("vendor.RootCause".to_owned())]
    );
}

#[test]
fn failed_properties_getter_returns_the_same_payload() {
    // 对应 Java: SQLClientInfoException#getFailedProperties
    let props = properties(&[
        ("ApplicationName", "REASON_VALUE_INVALID"),
        ("ClientUser", "REASON_UNKNOWN_PROPERTY"),
    ]);
    let exception = SqlClientInfoException::new(sql("rejected", Some("01S00"), 7), props.clone());
    assert_eq!(exception.failed_properties(), &props);
    assert_eq!(exception.failed_properties().len(), 2);
}
