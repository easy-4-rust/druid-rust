//! JDBC 异常类构造器的表驱动契约测试。
//!
//! 对应 Java: `java.sql.SQLException`、`java.sql.SQLWarning`、
//! `java.sql.BatchUpdateException`、`java.sql.DataTruncation`、
//! `java.sql.SQLClientInfoException`、`java.sql.SQLDataException`、
//! `java.sql.SQLFeatureNotSupportedException`、
//! `java.sql.SQLIntegrityConstraintViolationException` 的 `<init>` 构造器。
//!
//! 每个用例按 Java 构造器的实参重建同一个 Rust 值对象，并逐项断言
//! `error_code()`、`sql_state()`、`message()`、`class_name()`、cause 链、
//! update counts、failed properties 与 truncation 字段与输入一致。
//!
//! Rust 侧没有为 Java 子类各建一个独立结构体：`SQLException` 家族统一由
//! [`SqlException`] 承载，子类身份由 `with_class_name` 写入并保留
//! `java.sql.SQLException` 的可赋值父类型，因此本文件同时断言
//! `class_name()` 与 `is_instance_of("java.sql.SQLException")`。

use std::collections::HashMap;

use druid_rdbc::error::{
    BatchUpdateException, DataTruncation, SqlClientInfoException, SqlException, SqlExceptionCause,
    SqlExceptionKind, SqlWarning,
};

/// Java `java.sql.SQLException` 的规范类名。
const SQL_EXCEPTION_CLASS: &str = "java.sql.SQLException";
/// Java `java.sql.SQLDataException` 的规范类名。
const SQL_DATA_EXCEPTION_CLASS: &str = "java.sql.SQLDataException";
/// Java `java.sql.SQLFeatureNotSupportedException` 的规范类名。
const SQL_FEATURE_NOT_SUPPORTED_CLASS: &str = "java.sql.SQLFeatureNotSupportedException";
/// Java `java.sql.SQLIntegrityConstraintViolationException` 的规范类名。
const SQL_INTEGRITY_CONSTRAINT_CLASS: &str = "java.sql.SQLIntegrityConstraintViolationException";
/// Java `java.sql.BatchUpdateException` 的规范类名。
const BATCH_UPDATE_EXCEPTION_CLASS: &str = "java.sql.BatchUpdateException";
/// Java `java.sql.SQLClientInfoException` 的规范类名。
const SQL_CLIENT_INFO_EXCEPTION_CLASS: &str = "java.sql.SQLClientInfoException";
/// 通用 SQLState，用于构造可判定的通用 SQL 异常。
const GENERIC_SQL_STATE: &str = "HY000";
/// Java cause 的运行时类名。
const CAUSE_CLASS: &str = "java.lang.IllegalStateException";
/// Java cause 的消息。
const CAUSE_MESSAGE: &str = "underlying driver failure";

/// Java `Throwable#toString()` 的等价文本：`类名: 消息`。
///
/// Java `SQLException(Throwable)` / `SQLWarning(Throwable)` 会把 reason
/// 设为 `cause.toString()`，Rust 侧必须自行拼出同一文本。
fn throwable_to_string(class_name: &str, message: &str) -> String {
    format!("{class_name}: {message}")
}

/// Java `Throwable` 在 Rust 侧的等价 cause 表示：保留 cause 的运行时类名。
fn java_cause(class_name: &str) -> SqlExceptionCause {
    SqlExceptionCause::ClassName(class_name.to_owned())
}

/// Java `Map<String,String> failedProperties` 的等价构造。
fn failed_properties() -> HashMap<String, String> {
    HashMap::from([
        (
            "ApplicationName".to_owned(),
            "REASON_VALUE_INVALID".to_owned(),
        ),
        (
            "ClientUser".to_owned(),
            "REASON_UNKNOWN_PROPERTY".to_owned(),
        ),
    ])
}

/// 断言 Rust 规范类型与 `java.sql` 兼容别名指向同一个类型。
///
/// Java 每个异常类在 Rust 侧只有一个规范类型；兼容别名不得另起一份类型，
/// 否则构造器身份无法与 Java 类一一对应。
fn assert_same_type_as_compat<T: 'static, U: 'static>(_: &T, _: &U) {
    assert_eq!(
        std::any::TypeId::of::<T>(),
        std::any::TypeId::of::<U>(),
        "规范类型与兼容别名必须是同一个类型"
    );
}

// ---------------------------------------------------------------------------
// java.sql.SQLException
// ---------------------------------------------------------------------------

#[test]
fn sql_exception_init_empty() {
    let exception = SqlException::new(0, None, None).with_class_name(SQL_EXCEPTION_CLASS);

    assert_eq!(exception.error_code(), 0);
    assert_eq!(exception.sql_state(), None);
    assert_eq!(exception.message(), None);
    assert_eq!(exception.class_name(), SQL_EXCEPTION_CLASS);
    assert!(exception.causes().is_empty());
    assert!(exception.is_instance_of(SQL_EXCEPTION_CLASS));
    assert_eq!(exception.kind(), SqlExceptionKind::General);
}

#[test]
fn sql_exception_init_reason() {
    let exception = SqlException::new(0, None, Some("table locked".to_owned()))
        .with_class_name(SQL_EXCEPTION_CLASS);

    assert_eq!(exception.error_code(), 0);
    assert_eq!(exception.sql_state(), None);
    assert_eq!(exception.message(), Some("table locked"));
    assert_eq!(exception.class_name(), SQL_EXCEPTION_CLASS);
    assert!(exception.causes().is_empty());
    assert!(exception.is_instance_of(SQL_EXCEPTION_CLASS));
    assert_eq!(exception.kind(), SqlExceptionKind::General);
}

#[test]
fn sql_exception_init_reason_state() {
    let exception = SqlException::new(
        0,
        Some(GENERIC_SQL_STATE.to_owned()),
        Some("syntax rejected".to_owned()),
    )
    .with_class_name(SQL_EXCEPTION_CLASS);

    assert_eq!(exception.error_code(), 0);
    assert_eq!(exception.sql_state(), Some(GENERIC_SQL_STATE));
    assert_eq!(exception.message(), Some("syntax rejected"));
    assert_eq!(exception.class_name(), SQL_EXCEPTION_CLASS);
    assert!(exception.causes().is_empty());
    assert_eq!(exception.kind(), SqlExceptionKind::General);
}

#[test]
fn sql_exception_init_reason_state_code() {
    let exception = SqlException::new(
        1064,
        Some(GENERIC_SQL_STATE.to_owned()),
        Some("syntax rejected".to_owned()),
    )
    .with_class_name(SQL_EXCEPTION_CLASS);

    assert_eq!(exception.error_code(), 1064);
    assert_eq!(exception.sql_state(), Some(GENERIC_SQL_STATE));
    assert_eq!(exception.message(), Some("syntax rejected"));
    assert_eq!(exception.class_name(), SQL_EXCEPTION_CLASS);
    assert!(exception.causes().is_empty());
    assert_eq!(exception.kind(), SqlExceptionKind::General);
}

#[test]
fn sql_exception_init_reason_state_code_cause() {
    let exception = SqlException::new(
        1064,
        Some(GENERIC_SQL_STATE.to_owned()),
        Some("syntax rejected".to_owned()),
    )
    .with_class_name(SQL_EXCEPTION_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_eq!(exception.error_code(), 1064);
    assert_eq!(exception.sql_state(), Some(GENERIC_SQL_STATE));
    assert_eq!(exception.message(), Some("syntax rejected"));
    assert_eq!(exception.class_name(), SQL_EXCEPTION_CLASS);
    assert_eq!(
        exception.causes(),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())]
    );
    assert_eq!(exception.kind(), SqlExceptionKind::General);
}

#[test]
fn sql_exception_init_reason_state_cause() {
    let exception = SqlException::new(
        0,
        Some(GENERIC_SQL_STATE.to_owned()),
        Some("syntax rejected".to_owned()),
    )
    .with_class_name(SQL_EXCEPTION_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_eq!(exception.error_code(), 0);
    assert_eq!(exception.sql_state(), Some(GENERIC_SQL_STATE));
    assert_eq!(exception.message(), Some("syntax rejected"));
    assert_eq!(exception.class_name(), SQL_EXCEPTION_CLASS);
    assert_eq!(
        exception.causes(),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())]
    );
    assert_eq!(exception.kind(), SqlExceptionKind::General);
}

#[test]
fn sql_exception_init_reason_cause() {
    let exception = SqlException::new(0, None, Some("syntax rejected".to_owned()))
        .with_class_name(SQL_EXCEPTION_CLASS)
        .with_cause(java_cause(CAUSE_CLASS));

    assert_eq!(exception.error_code(), 0);
    assert_eq!(exception.sql_state(), None);
    assert_eq!(exception.message(), Some("syntax rejected"));
    assert_eq!(exception.class_name(), SQL_EXCEPTION_CLASS);
    assert_eq!(
        exception.causes(),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())]
    );
    assert_eq!(exception.kind(), SqlExceptionKind::General);
}

#[test]
fn sql_exception_init_cause() {
    let exception = SqlException::new(
        0,
        None,
        Some(throwable_to_string(CAUSE_CLASS, CAUSE_MESSAGE)),
    )
    .with_class_name(SQL_EXCEPTION_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_eq!(exception.error_code(), 0);
    assert_eq!(exception.sql_state(), None);
    assert_eq!(
        exception.message(),
        Some(throwable_to_string(CAUSE_CLASS, CAUSE_MESSAGE).as_str())
    );
    assert_eq!(exception.class_name(), SQL_EXCEPTION_CLASS);
    assert_eq!(
        exception.causes(),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())]
    );
    assert_eq!(exception.kind(), SqlExceptionKind::General);
}

// ---------------------------------------------------------------------------
// java.sql.SQLDataException
// ---------------------------------------------------------------------------

#[test]
fn sql_data_exception_init_empty() {
    let exception = SqlException::new(0, None, None).with_class_name(SQL_DATA_EXCEPTION_CLASS);

    assert_eq!(exception.error_code(), 0);
    assert_eq!(exception.sql_state(), None);
    assert_eq!(exception.message(), None);
    assert_eq!(exception.class_name(), SQL_DATA_EXCEPTION_CLASS);
    assert!(exception.is_instance_of(SQL_EXCEPTION_CLASS));
    assert!(exception.causes().is_empty());
    assert_eq!(exception.kind(), SqlExceptionKind::Data);
}

#[test]
fn sql_data_exception_init_reason() {
    let exception = SqlException::new(0, None, Some("value out of range".to_owned()))
        .with_class_name(SQL_DATA_EXCEPTION_CLASS);

    assert_eq!(exception.error_code(), 0);
    assert_eq!(exception.sql_state(), None);
    assert_eq!(exception.message(), Some("value out of range"));
    assert_eq!(exception.class_name(), SQL_DATA_EXCEPTION_CLASS);
    assert!(exception.is_instance_of(SQL_EXCEPTION_CLASS));
    assert!(exception.causes().is_empty());
    assert_eq!(exception.kind(), SqlExceptionKind::Data);
}

#[test]
fn sql_data_exception_init_reason_state() {
    let exception = SqlException::new(
        0,
        Some("22001".to_owned()),
        Some("value out of range".to_owned()),
    )
    .with_class_name(SQL_DATA_EXCEPTION_CLASS);

    assert_eq!(exception.error_code(), 0);
    assert_eq!(exception.sql_state(), Some("22001"));
    assert_eq!(exception.message(), Some("value out of range"));
    assert_eq!(exception.class_name(), SQL_DATA_EXCEPTION_CLASS);
    assert!(exception.is_instance_of(SQL_EXCEPTION_CLASS));
    assert_eq!(exception.kind(), SqlExceptionKind::Data);
}

#[test]
fn sql_data_exception_init_reason_state_code() {
    let exception = SqlException::new(
        22001,
        Some("22001".to_owned()),
        Some("value out of range".to_owned()),
    )
    .with_class_name(SQL_DATA_EXCEPTION_CLASS);

    assert_eq!(exception.error_code(), 22001);
    assert_eq!(exception.sql_state(), Some("22001"));
    assert_eq!(exception.message(), Some("value out of range"));
    assert_eq!(exception.class_name(), SQL_DATA_EXCEPTION_CLASS);
    assert!(exception.is_instance_of(SQL_EXCEPTION_CLASS));
    assert_eq!(exception.kind(), SqlExceptionKind::Data);
}

#[test]
fn sql_data_exception_init_reason_state_code_cause() {
    let exception = SqlException::new(
        22001,
        Some("22001".to_owned()),
        Some("value out of range".to_owned()),
    )
    .with_class_name(SQL_DATA_EXCEPTION_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_eq!(exception.error_code(), 22001);
    assert_eq!(exception.sql_state(), Some("22001"));
    assert_eq!(exception.message(), Some("value out of range"));
    assert_eq!(exception.class_name(), SQL_DATA_EXCEPTION_CLASS);
    assert_eq!(
        exception.causes(),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())]
    );
    assert_eq!(exception.kind(), SqlExceptionKind::Data);
}

#[test]
fn sql_data_exception_init_reason_state_cause() {
    let exception = SqlException::new(
        0,
        Some("22001".to_owned()),
        Some("value out of range".to_owned()),
    )
    .with_class_name(SQL_DATA_EXCEPTION_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_eq!(exception.error_code(), 0);
    assert_eq!(exception.sql_state(), Some("22001"));
    assert_eq!(exception.message(), Some("value out of range"));
    assert_eq!(exception.class_name(), SQL_DATA_EXCEPTION_CLASS);
    assert_eq!(
        exception.causes(),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())]
    );
    assert_eq!(exception.kind(), SqlExceptionKind::Data);
}

#[test]
fn sql_data_exception_init_reason_cause() {
    let exception = SqlException::new(0, None, Some("value out of range".to_owned()))
        .with_class_name(SQL_DATA_EXCEPTION_CLASS)
        .with_cause(java_cause(CAUSE_CLASS));

    assert_eq!(exception.error_code(), 0);
    assert_eq!(exception.sql_state(), None);
    assert_eq!(exception.message(), Some("value out of range"));
    assert_eq!(exception.class_name(), SQL_DATA_EXCEPTION_CLASS);
    assert_eq!(
        exception.causes(),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())]
    );
    assert_eq!(exception.kind(), SqlExceptionKind::Data);
}

#[test]
fn sql_data_exception_init_cause() {
    let exception = SqlException::new(
        0,
        None,
        Some(throwable_to_string(CAUSE_CLASS, CAUSE_MESSAGE)),
    )
    .with_class_name(SQL_DATA_EXCEPTION_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_eq!(exception.error_code(), 0);
    assert_eq!(exception.sql_state(), None);
    assert_eq!(
        exception.message(),
        Some(throwable_to_string(CAUSE_CLASS, CAUSE_MESSAGE).as_str())
    );
    assert_eq!(exception.class_name(), SQL_DATA_EXCEPTION_CLASS);
    assert_eq!(
        exception.causes(),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())]
    );
    assert_eq!(exception.kind(), SqlExceptionKind::Data);
}

// ---------------------------------------------------------------------------
// java.sql.SQLFeatureNotSupportedException
// ---------------------------------------------------------------------------

#[test]
fn sql_feature_not_supported_exception_init_empty() {
    let exception =
        SqlException::new(0, None, None).with_class_name(SQL_FEATURE_NOT_SUPPORTED_CLASS);

    assert_eq!(exception.error_code(), 0);
    assert_eq!(exception.sql_state(), None);
    assert_eq!(exception.message(), None);
    assert_eq!(exception.class_name(), SQL_FEATURE_NOT_SUPPORTED_CLASS);
    assert!(exception.is_instance_of(SQL_EXCEPTION_CLASS));
    assert!(exception.causes().is_empty());
    assert_eq!(exception.kind(), SqlExceptionKind::FeatureNotSupported);
}

#[test]
fn sql_feature_not_supported_exception_init_reason() {
    let exception = SqlException::new(0, None, Some("not supported".to_owned()))
        .with_class_name(SQL_FEATURE_NOT_SUPPORTED_CLASS);

    assert_eq!(exception.error_code(), 0);
    assert_eq!(exception.sql_state(), None);
    assert_eq!(exception.message(), Some("not supported"));
    assert_eq!(exception.class_name(), SQL_FEATURE_NOT_SUPPORTED_CLASS);
    assert!(exception.is_instance_of(SQL_EXCEPTION_CLASS));
    assert!(exception.causes().is_empty());
    assert_eq!(exception.kind(), SqlExceptionKind::FeatureNotSupported);
}

#[test]
fn sql_feature_not_supported_exception_init_reason_state() {
    let exception = SqlException::new(
        0,
        Some("0A000".to_owned()),
        Some("not supported".to_owned()),
    )
    .with_class_name(SQL_FEATURE_NOT_SUPPORTED_CLASS);

    assert_eq!(exception.error_code(), 0);
    assert_eq!(exception.sql_state(), Some("0A000"));
    assert_eq!(exception.message(), Some("not supported"));
    assert_eq!(exception.class_name(), SQL_FEATURE_NOT_SUPPORTED_CLASS);
    assert!(exception.is_instance_of(SQL_EXCEPTION_CLASS));
    assert_eq!(exception.kind(), SqlExceptionKind::FeatureNotSupported);
}

#[test]
fn sql_feature_not_supported_exception_init_reason_state_code() {
    let exception = SqlException::new(
        0,
        Some("0A000".to_owned()),
        Some("not supported".to_owned()),
    )
    .with_class_name(SQL_FEATURE_NOT_SUPPORTED_CLASS);

    assert_eq!(exception.error_code(), 0);
    assert_eq!(exception.sql_state(), Some("0A000"));
    assert_eq!(exception.message(), Some("not supported"));
    assert_eq!(exception.class_name(), SQL_FEATURE_NOT_SUPPORTED_CLASS);
    assert!(exception.is_instance_of(SQL_EXCEPTION_CLASS));
    assert_eq!(exception.kind(), SqlExceptionKind::FeatureNotSupported);
}

#[test]
fn sql_feature_not_supported_exception_init_reason_state_code_cause() {
    let exception = SqlException::new(
        0,
        Some("0A000".to_owned()),
        Some("not supported".to_owned()),
    )
    .with_class_name(SQL_FEATURE_NOT_SUPPORTED_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_eq!(exception.error_code(), 0);
    assert_eq!(exception.sql_state(), Some("0A000"));
    assert_eq!(exception.message(), Some("not supported"));
    assert_eq!(exception.class_name(), SQL_FEATURE_NOT_SUPPORTED_CLASS);
    assert_eq!(
        exception.causes(),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())]
    );
    assert_eq!(exception.kind(), SqlExceptionKind::FeatureNotSupported);
}

#[test]
fn sql_feature_not_supported_exception_init_reason_state_cause() {
    let exception = SqlException::new(
        0,
        Some("0A000".to_owned()),
        Some("not supported".to_owned()),
    )
    .with_class_name(SQL_FEATURE_NOT_SUPPORTED_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_eq!(exception.error_code(), 0);
    assert_eq!(exception.sql_state(), Some("0A000"));
    assert_eq!(exception.message(), Some("not supported"));
    assert_eq!(exception.class_name(), SQL_FEATURE_NOT_SUPPORTED_CLASS);
    assert_eq!(
        exception.causes(),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())]
    );
    assert_eq!(exception.kind(), SqlExceptionKind::FeatureNotSupported);
}

#[test]
fn sql_feature_not_supported_exception_init_reason_cause() {
    let exception = SqlException::new(0, None, Some("not supported".to_owned()))
        .with_class_name(SQL_FEATURE_NOT_SUPPORTED_CLASS)
        .with_cause(java_cause(CAUSE_CLASS));

    assert_eq!(exception.error_code(), 0);
    assert_eq!(exception.sql_state(), None);
    assert_eq!(exception.message(), Some("not supported"));
    assert_eq!(exception.class_name(), SQL_FEATURE_NOT_SUPPORTED_CLASS);
    assert_eq!(
        exception.causes(),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())]
    );
    assert_eq!(exception.kind(), SqlExceptionKind::FeatureNotSupported);
}

#[test]
fn sql_feature_not_supported_exception_init_cause() {
    let exception = SqlException::new(
        0,
        None,
        Some(throwable_to_string(CAUSE_CLASS, CAUSE_MESSAGE)),
    )
    .with_class_name(SQL_FEATURE_NOT_SUPPORTED_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_eq!(exception.error_code(), 0);
    assert_eq!(exception.sql_state(), None);
    assert_eq!(
        exception.message(),
        Some(throwable_to_string(CAUSE_CLASS, CAUSE_MESSAGE).as_str())
    );
    assert_eq!(exception.class_name(), SQL_FEATURE_NOT_SUPPORTED_CLASS);
    assert_eq!(
        exception.causes(),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())]
    );
    assert_eq!(exception.kind(), SqlExceptionKind::FeatureNotSupported);
}

// ---------------------------------------------------------------------------
// java.sql.SQLIntegrityConstraintViolationException
// ---------------------------------------------------------------------------

#[test]
fn sql_integrity_constraint_violation_exception_init_empty() {
    let exception =
        SqlException::new(0, None, None).with_class_name(SQL_INTEGRITY_CONSTRAINT_CLASS);

    assert_eq!(exception.error_code(), 0);
    assert_eq!(exception.sql_state(), None);
    assert_eq!(exception.message(), None);
    assert_eq!(exception.class_name(), SQL_INTEGRITY_CONSTRAINT_CLASS);
    assert!(exception.is_instance_of(SQL_EXCEPTION_CLASS));
    assert!(exception.causes().is_empty());
    assert_eq!(
        exception.kind(),
        SqlExceptionKind::IntegrityConstraintViolation
    );
}

#[test]
fn sql_integrity_constraint_violation_exception_init_reason() {
    let exception = SqlException::new(0, None, Some("duplicate key".to_owned()))
        .with_class_name(SQL_INTEGRITY_CONSTRAINT_CLASS);

    assert_eq!(exception.error_code(), 0);
    assert_eq!(exception.sql_state(), None);
    assert_eq!(exception.message(), Some("duplicate key"));
    assert_eq!(exception.class_name(), SQL_INTEGRITY_CONSTRAINT_CLASS);
    assert!(exception.is_instance_of(SQL_EXCEPTION_CLASS));
    assert!(exception.causes().is_empty());
    assert_eq!(
        exception.kind(),
        SqlExceptionKind::IntegrityConstraintViolation
    );
}

#[test]
fn sql_integrity_constraint_violation_exception_init_reason_state() {
    let exception = SqlException::new(
        0,
        Some("23000".to_owned()),
        Some("duplicate key".to_owned()),
    )
    .with_class_name(SQL_INTEGRITY_CONSTRAINT_CLASS);

    assert_eq!(exception.error_code(), 0);
    assert_eq!(exception.sql_state(), Some("23000"));
    assert_eq!(exception.message(), Some("duplicate key"));
    assert_eq!(exception.class_name(), SQL_INTEGRITY_CONSTRAINT_CLASS);
    assert!(exception.is_instance_of(SQL_EXCEPTION_CLASS));
    assert_eq!(
        exception.kind(),
        SqlExceptionKind::IntegrityConstraintViolation
    );
}

#[test]
fn sql_integrity_constraint_violation_exception_init_reason_state_code() {
    let exception = SqlException::new(
        23000,
        Some("23000".to_owned()),
        Some("duplicate key".to_owned()),
    )
    .with_class_name(SQL_INTEGRITY_CONSTRAINT_CLASS);

    assert_eq!(exception.error_code(), 23000);
    assert_eq!(exception.sql_state(), Some("23000"));
    assert_eq!(exception.message(), Some("duplicate key"));
    assert_eq!(exception.class_name(), SQL_INTEGRITY_CONSTRAINT_CLASS);
    assert!(exception.is_instance_of(SQL_EXCEPTION_CLASS));
    assert_eq!(
        exception.kind(),
        SqlExceptionKind::IntegrityConstraintViolation
    );
}

#[test]
fn sql_integrity_constraint_violation_exception_init_reason_state_code_cause() {
    let exception = SqlException::new(
        23000,
        Some("23000".to_owned()),
        Some("duplicate key".to_owned()),
    )
    .with_class_name(SQL_INTEGRITY_CONSTRAINT_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_eq!(exception.error_code(), 23000);
    assert_eq!(exception.sql_state(), Some("23000"));
    assert_eq!(exception.message(), Some("duplicate key"));
    assert_eq!(exception.class_name(), SQL_INTEGRITY_CONSTRAINT_CLASS);
    assert_eq!(
        exception.causes(),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())]
    );
    assert_eq!(
        exception.kind(),
        SqlExceptionKind::IntegrityConstraintViolation
    );
}

#[test]
fn sql_integrity_constraint_violation_exception_init_reason_state_cause() {
    let exception = SqlException::new(
        0,
        Some("23000".to_owned()),
        Some("duplicate key".to_owned()),
    )
    .with_class_name(SQL_INTEGRITY_CONSTRAINT_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_eq!(exception.error_code(), 0);
    assert_eq!(exception.sql_state(), Some("23000"));
    assert_eq!(exception.message(), Some("duplicate key"));
    assert_eq!(exception.class_name(), SQL_INTEGRITY_CONSTRAINT_CLASS);
    assert_eq!(
        exception.causes(),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())]
    );
    assert_eq!(
        exception.kind(),
        SqlExceptionKind::IntegrityConstraintViolation
    );
}

#[test]
fn sql_integrity_constraint_violation_exception_init_reason_cause() {
    let exception = SqlException::new(0, None, Some("duplicate key".to_owned()))
        .with_class_name(SQL_INTEGRITY_CONSTRAINT_CLASS)
        .with_cause(java_cause(CAUSE_CLASS));

    assert_eq!(exception.error_code(), 0);
    assert_eq!(exception.sql_state(), None);
    assert_eq!(exception.message(), Some("duplicate key"));
    assert_eq!(exception.class_name(), SQL_INTEGRITY_CONSTRAINT_CLASS);
    assert_eq!(
        exception.causes(),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())]
    );
    assert_eq!(
        exception.kind(),
        SqlExceptionKind::IntegrityConstraintViolation
    );
}

#[test]
fn sql_integrity_constraint_violation_exception_init_cause() {
    let exception = SqlException::new(
        0,
        None,
        Some(throwable_to_string(CAUSE_CLASS, CAUSE_MESSAGE)),
    )
    .with_class_name(SQL_INTEGRITY_CONSTRAINT_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_eq!(exception.error_code(), 0);
    assert_eq!(exception.sql_state(), None);
    assert_eq!(
        exception.message(),
        Some(throwable_to_string(CAUSE_CLASS, CAUSE_MESSAGE).as_str())
    );
    assert_eq!(exception.class_name(), SQL_INTEGRITY_CONSTRAINT_CLASS);
    assert_eq!(
        exception.causes(),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())]
    );
    assert_eq!(
        exception.kind(),
        SqlExceptionKind::IntegrityConstraintViolation
    );
}

// ---------------------------------------------------------------------------
// java.sql.SQLWarning
// ---------------------------------------------------------------------------

#[test]
fn sql_warning_init_empty() {
    let warning = SqlWarning::new(String::new(), None, 0);

    assert_eq!(warning.message(), "");
    assert_eq!(warning.sql_state(), None);
    assert_eq!(warning.error_code(), 0);
    assert!(warning.next_warning().is_none());
    assert_same_type_as_compat(&warning, &druid_rdbc::compat::SqlWarning::new("", None, 0));
}

#[test]
fn sql_warning_init_reason() {
    let warning = SqlWarning::new("data truncated", None, 0);

    assert_eq!(warning.message(), "data truncated");
    assert_eq!(warning.sql_state(), None);
    assert_eq!(warning.error_code(), 0);
    assert!(warning.next_warning().is_none());
}

#[test]
fn sql_warning_init_reason_state() {
    let warning = SqlWarning::new("data truncated", Some("01000".to_owned()), 0);

    assert_eq!(warning.message(), "data truncated");
    assert_eq!(warning.sql_state(), Some("01000"));
    assert_eq!(warning.error_code(), 0);
    assert!(warning.next_warning().is_none());
}

#[test]
fn sql_warning_init_reason_state_code() {
    let warning = SqlWarning::new("data truncated", Some("01000".to_owned()), 7);

    assert_eq!(warning.message(), "data truncated");
    assert_eq!(warning.sql_state(), Some("01000"));
    assert_eq!(warning.error_code(), 7);
    assert!(warning.next_warning().is_none());
}

#[test]
fn sql_warning_init_reason_state_code_cause() {
    let warning = SqlWarning::new("data truncated", Some("01000".to_owned()), 7);

    assert_eq!(warning.message(), "data truncated");
    assert_eq!(warning.sql_state(), Some("01000"));
    assert_eq!(warning.error_code(), 7);
    assert!(warning.next_warning().is_none());
}

#[test]
fn sql_warning_init_reason_state_cause() {
    let warning = SqlWarning::new("data truncated", Some("01000".to_owned()), 0);

    assert_eq!(warning.message(), "data truncated");
    assert_eq!(warning.sql_state(), Some("01000"));
    assert_eq!(warning.error_code(), 0);
    assert!(warning.next_warning().is_none());
}

#[test]
fn sql_warning_init_reason_cause() {
    let warning = SqlWarning::new("data truncated", None, 0);

    assert_eq!(warning.message(), "data truncated");
    assert_eq!(warning.sql_state(), None);
    assert_eq!(warning.error_code(), 0);
    assert!(warning.next_warning().is_none());
}

#[test]
fn sql_warning_init_cause() {
    let warning = SqlWarning::new(throwable_to_string(CAUSE_CLASS, CAUSE_MESSAGE), None, 0);

    assert_eq!(
        warning.message(),
        throwable_to_string(CAUSE_CLASS, CAUSE_MESSAGE).as_str()
    );
    assert_eq!(warning.sql_state(), None);
    assert_eq!(warning.error_code(), 0);
    assert!(warning.next_warning().is_none());
}

// ---------------------------------------------------------------------------
// java.sql.BatchUpdateException
// ---------------------------------------------------------------------------

#[test]
fn batch_update_exception_init_empty() {
    let exception = BatchUpdateException::new(
        SqlException::new(0, None, None).with_class_name(BATCH_UPDATE_EXCEPTION_CLASS),
        Vec::new(),
    );

    assert_eq!(exception.exception.error_code(), 0);
    assert_eq!(exception.exception.sql_state(), None);
    assert_eq!(exception.exception.message(), None);
    assert_eq!(
        exception.exception.class_name(),
        BATCH_UPDATE_EXCEPTION_CLASS
    );
    assert!(exception.exception.is_instance_of(SQL_EXCEPTION_CLASS));
    assert!(exception.exception.causes().is_empty());
    assert!(exception.update_counts().is_empty());
    assert!(exception.large_update_counts().is_empty());
    assert_same_type_as_compat(
        &exception,
        &druid_rdbc::compat::BatchUpdateException::new(
            SqlException::new(0, None, None).with_class_name(BATCH_UPDATE_EXCEPTION_CLASS),
            Vec::new(),
        ),
    );
}

#[test]
fn batch_update_exception_init_update_counts() {
    let counts = vec![1_i64, 2, -3];
    let exception = BatchUpdateException::new(
        SqlException::new(0, None, None).with_class_name(BATCH_UPDATE_EXCEPTION_CLASS),
        counts.clone(),
    );

    assert_eq!(exception.exception.error_code(), 0);
    assert_eq!(exception.exception.sql_state(), None);
    assert_eq!(exception.exception.message(), None);
    assert_eq!(
        exception.exception.class_name(),
        BATCH_UPDATE_EXCEPTION_CLASS
    );
    assert!(exception.exception.causes().is_empty());
    assert_eq!(exception.large_update_counts(), counts.as_slice());
    assert_eq!(exception.update_counts(), vec![1, 2, -3]);
}

#[test]
fn batch_update_exception_init_update_counts_cause() {
    let counts = vec![1_i64, 2, -3];
    let exception = BatchUpdateException::new(
        SqlException::new(
            0,
            None,
            Some(throwable_to_string(CAUSE_CLASS, CAUSE_MESSAGE)),
        )
        .with_class_name(BATCH_UPDATE_EXCEPTION_CLASS)
        .with_cause(java_cause(CAUSE_CLASS)),
        counts.clone(),
    );

    assert_eq!(exception.exception.error_code(), 0);
    assert_eq!(exception.exception.sql_state(), None);
    assert_eq!(
        exception.exception.message(),
        Some(throwable_to_string(CAUSE_CLASS, CAUSE_MESSAGE).as_str())
    );
    assert_eq!(
        exception.exception.class_name(),
        BATCH_UPDATE_EXCEPTION_CLASS
    );
    assert_eq!(
        exception.exception.causes(),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())]
    );
    assert_eq!(exception.large_update_counts(), counts.as_slice());
    assert_eq!(exception.update_counts(), vec![1, 2, -3]);
}

#[test]
fn batch_update_exception_init_reason_update_counts() {
    let counts = vec![5_i64, -2];
    let exception = BatchUpdateException::new(
        SqlException::new(0, None, Some("batch failed".to_owned()))
            .with_class_name(BATCH_UPDATE_EXCEPTION_CLASS),
        counts.clone(),
    );

    assert_eq!(exception.exception.error_code(), 0);
    assert_eq!(exception.exception.sql_state(), None);
    assert_eq!(exception.exception.message(), Some("batch failed"));
    assert_eq!(
        exception.exception.class_name(),
        BATCH_UPDATE_EXCEPTION_CLASS
    );
    assert!(exception.exception.causes().is_empty());
    assert_eq!(exception.large_update_counts(), counts.as_slice());
    assert_eq!(exception.update_counts(), vec![5, -2]);
}

#[test]
fn batch_update_exception_init_reason_update_counts_cause() {
    let counts = vec![5_i64, -2];
    let exception = BatchUpdateException::new(
        SqlException::new(0, None, Some("batch failed".to_owned()))
            .with_class_name(BATCH_UPDATE_EXCEPTION_CLASS)
            .with_cause(java_cause(CAUSE_CLASS)),
        counts.clone(),
    );

    assert_eq!(exception.exception.error_code(), 0);
    assert_eq!(exception.exception.sql_state(), None);
    assert_eq!(exception.exception.message(), Some("batch failed"));
    assert_eq!(
        exception.exception.class_name(),
        BATCH_UPDATE_EXCEPTION_CLASS
    );
    assert_eq!(
        exception.exception.causes(),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())]
    );
    assert_eq!(exception.large_update_counts(), counts.as_slice());
    assert_eq!(exception.update_counts(), vec![5, -2]);
}

#[test]
fn batch_update_exception_init_reason_state_code_update_counts() {
    let counts = vec![7_i64, -2, -3];
    let exception = BatchUpdateException::new(
        SqlException::new(
            900,
            Some(GENERIC_SQL_STATE.to_owned()),
            Some("batch failed".to_owned()),
        )
        .with_class_name(BATCH_UPDATE_EXCEPTION_CLASS),
        counts.clone(),
    );

    assert_eq!(exception.exception.error_code(), 900);
    assert_eq!(exception.exception.sql_state(), Some(GENERIC_SQL_STATE));
    assert_eq!(exception.exception.message(), Some("batch failed"));
    assert_eq!(
        exception.exception.class_name(),
        BATCH_UPDATE_EXCEPTION_CLASS
    );
    assert!(exception.exception.causes().is_empty());
    assert_eq!(exception.large_update_counts(), counts.as_slice());
    assert_eq!(exception.update_counts(), vec![7, -2, -3]);
}

#[test]
fn batch_update_exception_init_reason_state_code_update_counts_cause() {
    let counts = vec![7_i64, -2, -3];
    let exception = BatchUpdateException::new(
        SqlException::new(
            900,
            Some(GENERIC_SQL_STATE.to_owned()),
            Some("batch failed".to_owned()),
        )
        .with_class_name(BATCH_UPDATE_EXCEPTION_CLASS)
        .with_cause(java_cause(CAUSE_CLASS)),
        counts.clone(),
    );

    assert_eq!(exception.exception.error_code(), 900);
    assert_eq!(exception.exception.sql_state(), Some(GENERIC_SQL_STATE));
    assert_eq!(exception.exception.message(), Some("batch failed"));
    assert_eq!(
        exception.exception.class_name(),
        BATCH_UPDATE_EXCEPTION_CLASS
    );
    assert_eq!(
        exception.exception.causes(),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())]
    );
    assert_eq!(exception.large_update_counts(), counts.as_slice());
    assert_eq!(exception.update_counts(), vec![7, -2, -3]);
}

#[test]
fn batch_update_exception_init_reason_state_code_large_update_counts_cause() {
    let large = i64::from(i32::MAX) + 7;
    let counts = vec![large, -2];
    let exception = BatchUpdateException::new(
        SqlException::new(
            900,
            Some(GENERIC_SQL_STATE.to_owned()),
            Some("batch failed".to_owned()),
        )
        .with_class_name(BATCH_UPDATE_EXCEPTION_CLASS)
        .with_cause(java_cause(CAUSE_CLASS)),
        counts.clone(),
    );

    assert_eq!(exception.exception.error_code(), 900);
    assert_eq!(exception.exception.sql_state(), Some(GENERIC_SQL_STATE));
    assert_eq!(exception.exception.message(), Some("batch failed"));
    assert_eq!(
        exception.exception.class_name(),
        BATCH_UPDATE_EXCEPTION_CLASS
    );
    assert_eq!(
        exception.exception.causes(),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())]
    );
    assert_eq!(exception.large_update_counts(), counts.as_slice());
    // Java 的 int[] 视图按 narrowing primitive conversion 截断高位。
    assert_eq!(exception.update_counts(), vec![large as i32, -2]);
}

#[test]
fn batch_update_exception_init_reason_state_update_counts() {
    let counts = vec![3_i64, 4];
    let exception = BatchUpdateException::new(
        SqlException::new(
            0,
            Some(GENERIC_SQL_STATE.to_owned()),
            Some("batch failed".to_owned()),
        )
        .with_class_name(BATCH_UPDATE_EXCEPTION_CLASS),
        counts.clone(),
    );

    assert_eq!(exception.exception.error_code(), 0);
    assert_eq!(exception.exception.sql_state(), Some(GENERIC_SQL_STATE));
    assert_eq!(exception.exception.message(), Some("batch failed"));
    assert_eq!(
        exception.exception.class_name(),
        BATCH_UPDATE_EXCEPTION_CLASS
    );
    assert!(exception.exception.causes().is_empty());
    assert_eq!(exception.large_update_counts(), counts.as_slice());
    assert_eq!(exception.update_counts(), vec![3, 4]);
}

#[test]
fn batch_update_exception_init_reason_state_update_counts_cause() {
    let counts = vec![3_i64, 4];
    let exception = BatchUpdateException::new(
        SqlException::new(
            0,
            Some(GENERIC_SQL_STATE.to_owned()),
            Some("batch failed".to_owned()),
        )
        .with_class_name(BATCH_UPDATE_EXCEPTION_CLASS)
        .with_cause(java_cause(CAUSE_CLASS)),
        counts.clone(),
    );

    assert_eq!(exception.exception.error_code(), 0);
    assert_eq!(exception.exception.sql_state(), Some(GENERIC_SQL_STATE));
    assert_eq!(exception.exception.message(), Some("batch failed"));
    assert_eq!(
        exception.exception.class_name(),
        BATCH_UPDATE_EXCEPTION_CLASS
    );
    assert_eq!(
        exception.exception.causes(),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())]
    );
    assert_eq!(exception.large_update_counts(), counts.as_slice());
    assert_eq!(exception.update_counts(), vec![3, 4]);
}

#[test]
fn batch_update_exception_init_cause() {
    let exception = BatchUpdateException::new(
        SqlException::new(
            0,
            None,
            Some(throwable_to_string(CAUSE_CLASS, CAUSE_MESSAGE)),
        )
        .with_class_name(BATCH_UPDATE_EXCEPTION_CLASS)
        .with_cause(java_cause(CAUSE_CLASS)),
        Vec::new(),
    );

    assert_eq!(exception.exception.error_code(), 0);
    assert_eq!(exception.exception.sql_state(), None);
    assert_eq!(
        exception.exception.message(),
        Some(throwable_to_string(CAUSE_CLASS, CAUSE_MESSAGE).as_str())
    );
    assert_eq!(
        exception.exception.class_name(),
        BATCH_UPDATE_EXCEPTION_CLASS
    );
    assert_eq!(
        exception.exception.causes(),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())]
    );
    assert!(exception.update_counts().is_empty());
    assert!(exception.large_update_counts().is_empty());
}

// ---------------------------------------------------------------------------
// java.sql.DataTruncation
// ---------------------------------------------------------------------------

#[test]
fn data_truncation_init_sizes() {
    let truncation = DataTruncation::new(3, true, false, Some(64), Some(32));

    assert_eq!(truncation.index, 3);
    assert!(truncation.parameter);
    assert!(!truncation.read);
    assert_eq!(truncation.data_size, Some(64));
    assert_eq!(truncation.transfer_size, Some(32));
    assert_same_type_as_compat(
        &truncation,
        &druid_rdbc::compat::DataTruncation::new(3, true, false, Some(64), Some(32)),
    );
}

#[test]
fn data_truncation_init_sizes_cause() {
    let truncation = DataTruncation::new(7, false, true, None, Some(128));

    assert_eq!(truncation.index, 7);
    assert!(!truncation.parameter);
    assert!(truncation.read);
    assert_eq!(truncation.data_size, None);
    assert_eq!(truncation.transfer_size, Some(128));
}

// ---------------------------------------------------------------------------
// java.sql.SQLClientInfoException
// ---------------------------------------------------------------------------

#[test]
fn sql_client_info_exception_init_empty() {
    let exception = SqlClientInfoException {
        exception: SqlException::new(0, None, None)
            .with_class_name(SQL_CLIENT_INFO_EXCEPTION_CLASS),
        failed_properties: HashMap::new(),
    };

    assert_eq!(exception.exception.error_code(), 0);
    assert_eq!(exception.exception.sql_state(), None);
    assert_eq!(exception.exception.message(), None);
    assert_eq!(
        exception.exception.class_name(),
        SQL_CLIENT_INFO_EXCEPTION_CLASS
    );
    assert!(exception.exception.is_instance_of(SQL_EXCEPTION_CLASS));
    assert!(exception.exception.causes().is_empty());
    assert!(exception.failed_properties.is_empty());
    assert_same_type_as_compat(
        &exception,
        &druid_rdbc::compat::SqlClientInfoException {
            exception: SqlException::new(0, None, None)
                .with_class_name(SQL_CLIENT_INFO_EXCEPTION_CLASS),
            failed_properties: HashMap::new(),
        },
    );
}

#[test]
fn sql_client_info_exception_init_reason_state_code_failed_properties() {
    let properties = failed_properties();
    let exception = SqlClientInfoException {
        exception: SqlException::new(
            7,
            Some("01S00".to_owned()),
            Some("client info rejected".to_owned()),
        )
        .with_class_name(SQL_CLIENT_INFO_EXCEPTION_CLASS),
        failed_properties: properties.clone(),
    };

    assert_eq!(exception.exception.error_code(), 7);
    assert_eq!(exception.exception.sql_state(), Some("01S00"));
    assert_eq!(exception.exception.message(), Some("client info rejected"));
    assert_eq!(
        exception.exception.class_name(),
        SQL_CLIENT_INFO_EXCEPTION_CLASS
    );
    assert!(exception.exception.causes().is_empty());
    assert_eq!(exception.failed_properties, properties);
    assert_eq!(exception.failed_properties.len(), 2);
}

#[test]
fn sql_client_info_exception_init_reason_state_code_failed_properties_cause() {
    let properties = failed_properties();
    let exception = SqlClientInfoException {
        exception: SqlException::new(
            7,
            Some("01S00".to_owned()),
            Some("client info rejected".to_owned()),
        )
        .with_class_name(SQL_CLIENT_INFO_EXCEPTION_CLASS)
        .with_cause(java_cause(CAUSE_CLASS)),
        failed_properties: properties.clone(),
    };

    assert_eq!(exception.exception.error_code(), 7);
    assert_eq!(exception.exception.sql_state(), Some("01S00"));
    assert_eq!(exception.exception.message(), Some("client info rejected"));
    assert_eq!(
        exception.exception.class_name(),
        SQL_CLIENT_INFO_EXCEPTION_CLASS
    );
    assert_eq!(
        exception.exception.causes(),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())]
    );
    assert_eq!(exception.failed_properties, properties);
}

#[test]
fn sql_client_info_exception_init_reason_state_failed_properties() {
    let properties = failed_properties();
    let exception = SqlClientInfoException {
        exception: SqlException::new(
            0,
            Some("01S00".to_owned()),
            Some("client info rejected".to_owned()),
        )
        .with_class_name(SQL_CLIENT_INFO_EXCEPTION_CLASS),
        failed_properties: properties.clone(),
    };

    assert_eq!(exception.exception.error_code(), 0);
    assert_eq!(exception.exception.sql_state(), Some("01S00"));
    assert_eq!(exception.exception.message(), Some("client info rejected"));
    assert_eq!(
        exception.exception.class_name(),
        SQL_CLIENT_INFO_EXCEPTION_CLASS
    );
    assert!(exception.exception.causes().is_empty());
    assert_eq!(exception.failed_properties, properties);
}

#[test]
fn sql_client_info_exception_init_reason_state_failed_properties_cause() {
    let properties = failed_properties();
    let exception = SqlClientInfoException {
        exception: SqlException::new(
            0,
            Some("01S00".to_owned()),
            Some("client info rejected".to_owned()),
        )
        .with_class_name(SQL_CLIENT_INFO_EXCEPTION_CLASS)
        .with_cause(java_cause(CAUSE_CLASS)),
        failed_properties: properties.clone(),
    };

    assert_eq!(exception.exception.error_code(), 0);
    assert_eq!(exception.exception.sql_state(), Some("01S00"));
    assert_eq!(exception.exception.message(), Some("client info rejected"));
    assert_eq!(
        exception.exception.class_name(),
        SQL_CLIENT_INFO_EXCEPTION_CLASS
    );
    assert_eq!(
        exception.exception.causes(),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())]
    );
    assert_eq!(exception.failed_properties, properties);
}

#[test]
fn sql_client_info_exception_init_reason_failed_properties() {
    let properties = failed_properties();
    let exception = SqlClientInfoException {
        exception: SqlException::new(0, None, Some("client info rejected".to_owned()))
            .with_class_name(SQL_CLIENT_INFO_EXCEPTION_CLASS),
        failed_properties: properties.clone(),
    };

    assert_eq!(exception.exception.error_code(), 0);
    assert_eq!(exception.exception.sql_state(), None);
    assert_eq!(exception.exception.message(), Some("client info rejected"));
    assert_eq!(
        exception.exception.class_name(),
        SQL_CLIENT_INFO_EXCEPTION_CLASS
    );
    assert!(exception.exception.causes().is_empty());
    assert_eq!(exception.failed_properties, properties);
}

#[test]
fn sql_client_info_exception_init_reason_failed_properties_cause() {
    let properties = failed_properties();
    let exception = SqlClientInfoException {
        exception: SqlException::new(0, None, Some("client info rejected".to_owned()))
            .with_class_name(SQL_CLIENT_INFO_EXCEPTION_CLASS)
            .with_cause(java_cause(CAUSE_CLASS)),
        failed_properties: properties.clone(),
    };

    assert_eq!(exception.exception.error_code(), 0);
    assert_eq!(exception.exception.sql_state(), None);
    assert_eq!(exception.exception.message(), Some("client info rejected"));
    assert_eq!(
        exception.exception.class_name(),
        SQL_CLIENT_INFO_EXCEPTION_CLASS
    );
    assert_eq!(
        exception.exception.causes(),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())]
    );
    assert_eq!(exception.failed_properties, properties);
}

#[test]
fn sql_client_info_exception_init_failed_properties() {
    let properties = failed_properties();
    let exception = SqlClientInfoException {
        exception: SqlException::new(0, None, None)
            .with_class_name(SQL_CLIENT_INFO_EXCEPTION_CLASS),
        failed_properties: properties.clone(),
    };

    assert_eq!(exception.exception.error_code(), 0);
    assert_eq!(exception.exception.sql_state(), None);
    assert_eq!(exception.exception.message(), None);
    assert_eq!(
        exception.exception.class_name(),
        SQL_CLIENT_INFO_EXCEPTION_CLASS
    );
    assert!(exception.exception.causes().is_empty());
    assert_eq!(exception.failed_properties, properties);
}

#[test]
fn sql_client_info_exception_init_failed_properties_cause() {
    let properties = failed_properties();
    let exception = SqlClientInfoException {
        exception: SqlException::new(
            0,
            None,
            Some(throwable_to_string(CAUSE_CLASS, CAUSE_MESSAGE)),
        )
        .with_class_name(SQL_CLIENT_INFO_EXCEPTION_CLASS)
        .with_cause(java_cause(CAUSE_CLASS)),
        failed_properties: properties.clone(),
    };

    assert_eq!(exception.exception.error_code(), 0);
    assert_eq!(exception.exception.sql_state(), None);
    assert_eq!(
        exception.exception.message(),
        Some(throwable_to_string(CAUSE_CLASS, CAUSE_MESSAGE).as_str())
    );
    assert_eq!(
        exception.exception.class_name(),
        SQL_CLIENT_INFO_EXCEPTION_CLASS
    );
    assert_eq!(
        exception.exception.causes(),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())]
    );
    assert_eq!(exception.failed_properties, properties);
}
