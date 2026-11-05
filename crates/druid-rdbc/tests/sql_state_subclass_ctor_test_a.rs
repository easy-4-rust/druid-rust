//! JDBC `SQLState` 子类异常的构造器契约测试（A 组）。
//!
//! 对应 Java: `java.sql.SQLInvalidAuthorizationSpecException`、
//! `java.sql.SQLNonTransientConnectionException`、
//! `java.sql.SQLNonTransientException`、
//! `java.sql.SQLRecoverableException` 的 8 个 `<init>` 构造器重载，以及这 4 个
//! 子类从 `java.sql.SQLException` 继承的 `getErrorCode()`、`getSQLState()`、
//! `getNextException()`、`setNextException(java.sql.SQLException)`。
//!
//! Rust 侧没有为这些 Java 子类各建一个独立结构体：`SQLException` 家族统一由
//! [`SqlException`] 承载，子类身份由 `with_class_name` 写入并保留
//! `java.sql.SQLException` 的可赋值父类型，因此每个用例同时断言
//! `class_name()`、`is_instance_of("java.sql.SQLException")` 与 `kind()`。
//!
//! 本文件覆盖的 Java 类与 `sql_exception_constructor_contract_test.rs` 完全
//! 不重叠，不重复该文件已覆盖的身份。

use druid_rdbc::error::{SqlException, SqlExceptionCause, SqlExceptionKind};

/// Java `java.sql.SQLException` 的规范类名。
const SQL_EXCEPTION_CLASS: &str = "java.sql.SQLException";
/// Java `java.sql.SQLInvalidAuthorizationSpecException` 的规范类名。
const SQL_INVALID_AUTHORIZATION_SPEC_CLASS: &str = "java.sql.SQLInvalidAuthorizationSpecException";
/// `SQLInvalidAuthorizationSpecException` 的样例 SQLState（授权规范错误）。
const SQL_INVALID_AUTHORIZATION_SPEC_STATE: &str = "28000";
/// `SQLInvalidAuthorizationSpecException` 的样例 vendor code。
const SQL_INVALID_AUTHORIZATION_SPEC_CODE: i32 = 28000;
/// `SQLInvalidAuthorizationSpecException` 的样例 reason。
const SQL_INVALID_AUTHORIZATION_SPEC_REASON: &str = "invalid authorization specification";

/// Java `java.sql.SQLNonTransientConnectionException` 的规范类名。
const SQL_NON_TRANSIENT_CONNECTION_CLASS: &str = "java.sql.SQLNonTransientConnectionException";
/// `SQLNonTransientConnectionException` 的样例 SQLState（连接失败）。
const SQL_NON_TRANSIENT_CONNECTION_STATE: &str = "08001";
/// `SQLNonTransientConnectionException` 的样例 vendor code。
const SQL_NON_TRANSIENT_CONNECTION_CODE: i32 = 8001;
/// `SQLNonTransientConnectionException` 的样例 reason。
const SQL_NON_TRANSIENT_CONNECTION_REASON: &str = "unable to establish a connection";

/// Java `java.sql.SQLNonTransientException` 的规范类名。
const SQL_NON_TRANSIENT_CLASS: &str = "java.sql.SQLNonTransientException";
/// `SQLNonTransientException` 的样例 SQLState（数据错误）。
const SQL_NON_TRANSIENT_STATE: &str = "22000";
/// `SQLNonTransientException` 的样例 vendor code。
const SQL_NON_TRANSIENT_CODE: i32 = 22000;
/// `SQLNonTransientException` 的样例 reason。
const SQL_NON_TRANSIENT_REASON: &str = "data exception";

/// Java `java.sql.SQLRecoverableException` 的规范类名。
const SQL_RECOVERABLE_CLASS: &str = "java.sql.SQLRecoverableException";
/// `SQLRecoverableException` 的样例 SQLState（连接故障，可恢复）。
const SQL_RECOVERABLE_STATE: &str = "08006";
/// `SQLRecoverableException` 的样例 vendor code。
const SQL_RECOVERABLE_CODE: i32 = 8006;
/// `SQLRecoverableException` 的样例 reason。
const SQL_RECOVERABLE_REASON: &str = "connection failure";

/// Java cause 的运行时类名。
const CAUSE_CLASS: &str = "java.io.IOException";
/// Java cause 的消息。
const CAUSE_MESSAGE: &str = "connection reset by peer";

/// Java `Throwable#toString()` 的等价文本：`类名: 消息`。
///
/// Java `SQLException(Throwable)` 会把 reason 设为 `cause.toString()`，
/// Rust 侧必须自行拼出同一文本。
fn throwable_to_string(class_name: &str, message: &str) -> String {
    format!("{class_name}: {message}")
}

/// Java `Throwable` 在 Rust 侧的等价 cause 表示：保留 cause 的运行时类名。
fn java_cause(class_name: &str) -> SqlExceptionCause {
    SqlExceptionCause::ClassName(class_name.to_owned())
}

/// 断言构造器实参完整映射到 Rust 侧的可观察字段。
///
/// `class_name` 与 `expected_kind` 描述 Java 子类身份；其余参数是 Java 构造器
/// 实参在 Rust 侧应当产生的 error code、SQLState、reason 与 cause 链。
fn assert_constructor_mapping(
    exception: &SqlException,
    class_name: &str,
    expected_kind: SqlExceptionKind,
    expected_code: i32,
    expected_state: Option<&str>,
    expected_message: Option<&str>,
    expected_causes: &[SqlExceptionCause],
) {
    assert_eq!(exception.error_code(), expected_code);
    assert_eq!(exception.sql_state(), expected_state);
    assert_eq!(exception.message(), expected_message);
    assert_eq!(exception.class_name(), class_name);
    assert!(exception.is_instance_of(SQL_EXCEPTION_CLASS));
    assert_eq!(exception.causes(), expected_causes);
    assert_eq!(exception.kind(), expected_kind);
}

// ---------------------------------------------------------------------------
// java.sql.SQLInvalidAuthorizationSpecException
// ---------------------------------------------------------------------------

/// Java `SQLInvalidAuthorizationSpecException()`：reason、SQLState、vendor code 均为空。
#[test]
fn sql_invalid_authorization_spec_exception_init_empty() {
    let exception =
        SqlException::new(0, None, None).with_class_name(SQL_INVALID_AUTHORIZATION_SPEC_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_INVALID_AUTHORIZATION_SPEC_CLASS,
        SqlExceptionKind::InvalidAuthorization,
        0,
        None,
        None,
        &[],
    );
}

/// Java `SQLInvalidAuthorizationSpecException(String reason)`：只带 reason。
#[test]
fn sql_invalid_authorization_spec_exception_init_reason() {
    let exception = SqlException::new(
        0,
        None,
        Some(SQL_INVALID_AUTHORIZATION_SPEC_REASON.to_owned()),
    )
    .with_class_name(SQL_INVALID_AUTHORIZATION_SPEC_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_INVALID_AUTHORIZATION_SPEC_CLASS,
        SqlExceptionKind::InvalidAuthorization,
        0,
        None,
        Some(SQL_INVALID_AUTHORIZATION_SPEC_REASON),
        &[],
    );
}

/// Java `SQLInvalidAuthorizationSpecException(String reason, String sqlState)`。
#[test]
fn sql_invalid_authorization_spec_exception_init_reason_state() {
    let exception = SqlException::new(
        0,
        Some(SQL_INVALID_AUTHORIZATION_SPEC_STATE.to_owned()),
        Some(SQL_INVALID_AUTHORIZATION_SPEC_REASON.to_owned()),
    )
    .with_class_name(SQL_INVALID_AUTHORIZATION_SPEC_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_INVALID_AUTHORIZATION_SPEC_CLASS,
        SqlExceptionKind::InvalidAuthorization,
        0,
        Some(SQL_INVALID_AUTHORIZATION_SPEC_STATE),
        Some(SQL_INVALID_AUTHORIZATION_SPEC_REASON),
        &[],
    );
}

/// Java `SQLInvalidAuthorizationSpecException(String reason, String sqlState, int vendorCode)`。
#[test]
fn sql_invalid_authorization_spec_exception_init_reason_state_code() {
    let exception = SqlException::new(
        SQL_INVALID_AUTHORIZATION_SPEC_CODE,
        Some(SQL_INVALID_AUTHORIZATION_SPEC_STATE.to_owned()),
        Some(SQL_INVALID_AUTHORIZATION_SPEC_REASON.to_owned()),
    )
    .with_class_name(SQL_INVALID_AUTHORIZATION_SPEC_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_INVALID_AUTHORIZATION_SPEC_CLASS,
        SqlExceptionKind::InvalidAuthorization,
        SQL_INVALID_AUTHORIZATION_SPEC_CODE,
        Some(SQL_INVALID_AUTHORIZATION_SPEC_STATE),
        Some(SQL_INVALID_AUTHORIZATION_SPEC_REASON),
        &[],
    );
}

/// Java `SQLInvalidAuthorizationSpecException(String, String, int, Throwable)`。
#[test]
fn sql_invalid_authorization_spec_exception_init_reason_state_code_cause() {
    let exception = SqlException::new(
        SQL_INVALID_AUTHORIZATION_SPEC_CODE,
        Some(SQL_INVALID_AUTHORIZATION_SPEC_STATE.to_owned()),
        Some(SQL_INVALID_AUTHORIZATION_SPEC_REASON.to_owned()),
    )
    .with_class_name(SQL_INVALID_AUTHORIZATION_SPEC_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_INVALID_AUTHORIZATION_SPEC_CLASS,
        SqlExceptionKind::InvalidAuthorization,
        SQL_INVALID_AUTHORIZATION_SPEC_CODE,
        Some(SQL_INVALID_AUTHORIZATION_SPEC_STATE),
        Some(SQL_INVALID_AUTHORIZATION_SPEC_REASON),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// Java `SQLInvalidAuthorizationSpecException(String reason, String sqlState, Throwable cause)`。
#[test]
fn sql_invalid_authorization_spec_exception_init_reason_state_cause() {
    let exception = SqlException::new(
        0,
        Some(SQL_INVALID_AUTHORIZATION_SPEC_STATE.to_owned()),
        Some(SQL_INVALID_AUTHORIZATION_SPEC_REASON.to_owned()),
    )
    .with_class_name(SQL_INVALID_AUTHORIZATION_SPEC_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_INVALID_AUTHORIZATION_SPEC_CLASS,
        SqlExceptionKind::InvalidAuthorization,
        0,
        Some(SQL_INVALID_AUTHORIZATION_SPEC_STATE),
        Some(SQL_INVALID_AUTHORIZATION_SPEC_REASON),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// Java `SQLInvalidAuthorizationSpecException(String reason, Throwable cause)`：SQLState 为空。
#[test]
fn sql_invalid_authorization_spec_exception_init_reason_cause() {
    let exception = SqlException::new(
        0,
        None,
        Some(SQL_INVALID_AUTHORIZATION_SPEC_REASON.to_owned()),
    )
    .with_class_name(SQL_INVALID_AUTHORIZATION_SPEC_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_INVALID_AUTHORIZATION_SPEC_CLASS,
        SqlExceptionKind::InvalidAuthorization,
        0,
        None,
        Some(SQL_INVALID_AUTHORIZATION_SPEC_REASON),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// Java `SQLInvalidAuthorizationSpecException(Throwable cause)`：reason 取 `cause.toString()`。
#[test]
fn sql_invalid_authorization_spec_exception_init_cause() {
    let cause_text = throwable_to_string(CAUSE_CLASS, CAUSE_MESSAGE);
    let exception = SqlException::new(0, None, Some(cause_text.clone()))
        .with_class_name(SQL_INVALID_AUTHORIZATION_SPEC_CLASS)
        .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_INVALID_AUTHORIZATION_SPEC_CLASS,
        SqlExceptionKind::InvalidAuthorization,
        0,
        None,
        Some(cause_text.as_str()),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// `SQLInvalidAuthorizationSpecException` 继承自 `SQLException` 的可观察 getter。
#[test]
fn sql_invalid_authorization_spec_exception_inherited_getters() {
    let mut exception = SqlException::new(
        SQL_INVALID_AUTHORIZATION_SPEC_CODE,
        Some(SQL_INVALID_AUTHORIZATION_SPEC_STATE.to_owned()),
        Some(SQL_INVALID_AUTHORIZATION_SPEC_REASON.to_owned()),
    )
    .with_class_name(SQL_INVALID_AUTHORIZATION_SPEC_CLASS);

    // getErrorCode()
    assert_eq!(exception.error_code(), SQL_INVALID_AUTHORIZATION_SPEC_CODE);
    // getSQLState()
    assert_eq!(
        exception.sql_state(),
        Some(SQL_INVALID_AUTHORIZATION_SPEC_STATE)
    );
    // getNextException()
    assert!(exception.next_exception().is_none());
    assert!(exception.next_exceptions().is_empty());

    // setNextException(java.sql.SQLException)
    let next = SqlException::new(
        SQL_INVALID_AUTHORIZATION_SPEC_CODE + 1,
        Some(SQL_INVALID_AUTHORIZATION_SPEC_STATE.to_owned()),
        Some("chained authorization failure".to_owned()),
    )
    .with_class_name(SQL_INVALID_AUTHORIZATION_SPEC_CLASS);
    exception.set_next_exception(next.clone());

    assert_eq!(exception.next_exception(), Some(&next));
    assert_eq!(exception.next_exceptions().len(), 1);
    assert_eq!(
        exception.next_exception().map(SqlException::error_code),
        Some(SQL_INVALID_AUTHORIZATION_SPEC_CODE + 1)
    );
    assert_eq!(
        exception.next_exception().map(SqlException::class_name),
        Some(SQL_INVALID_AUTHORIZATION_SPEC_CLASS)
    );
}

// ---------------------------------------------------------------------------
// java.sql.SQLNonTransientConnectionException
// ---------------------------------------------------------------------------

/// Java `SQLNonTransientConnectionException()`：reason、SQLState、vendor code 均为空。
#[test]
fn sql_non_transient_connection_exception_init_empty() {
    let exception =
        SqlException::new(0, None, None).with_class_name(SQL_NON_TRANSIENT_CONNECTION_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_NON_TRANSIENT_CONNECTION_CLASS,
        SqlExceptionKind::NonTransientConnection,
        0,
        None,
        None,
        &[],
    );
}

/// Java `SQLNonTransientConnectionException(String reason)`：只带 reason。
#[test]
fn sql_non_transient_connection_exception_init_reason() {
    let exception = SqlException::new(
        0,
        None,
        Some(SQL_NON_TRANSIENT_CONNECTION_REASON.to_owned()),
    )
    .with_class_name(SQL_NON_TRANSIENT_CONNECTION_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_NON_TRANSIENT_CONNECTION_CLASS,
        SqlExceptionKind::NonTransientConnection,
        0,
        None,
        Some(SQL_NON_TRANSIENT_CONNECTION_REASON),
        &[],
    );
}

/// Java `SQLNonTransientConnectionException(String reason, String sqlState)`。
#[test]
fn sql_non_transient_connection_exception_init_reason_state() {
    let exception = SqlException::new(
        0,
        Some(SQL_NON_TRANSIENT_CONNECTION_STATE.to_owned()),
        Some(SQL_NON_TRANSIENT_CONNECTION_REASON.to_owned()),
    )
    .with_class_name(SQL_NON_TRANSIENT_CONNECTION_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_NON_TRANSIENT_CONNECTION_CLASS,
        SqlExceptionKind::NonTransientConnection,
        0,
        Some(SQL_NON_TRANSIENT_CONNECTION_STATE),
        Some(SQL_NON_TRANSIENT_CONNECTION_REASON),
        &[],
    );
}

/// Java `SQLNonTransientConnectionException(String reason, String sqlState, int vendorCode)`。
#[test]
fn sql_non_transient_connection_exception_init_reason_state_code() {
    let exception = SqlException::new(
        SQL_NON_TRANSIENT_CONNECTION_CODE,
        Some(SQL_NON_TRANSIENT_CONNECTION_STATE.to_owned()),
        Some(SQL_NON_TRANSIENT_CONNECTION_REASON.to_owned()),
    )
    .with_class_name(SQL_NON_TRANSIENT_CONNECTION_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_NON_TRANSIENT_CONNECTION_CLASS,
        SqlExceptionKind::NonTransientConnection,
        SQL_NON_TRANSIENT_CONNECTION_CODE,
        Some(SQL_NON_TRANSIENT_CONNECTION_STATE),
        Some(SQL_NON_TRANSIENT_CONNECTION_REASON),
        &[],
    );
}

/// Java `SQLNonTransientConnectionException(String, String, int, Throwable)`。
#[test]
fn sql_non_transient_connection_exception_init_reason_state_code_cause() {
    let exception = SqlException::new(
        SQL_NON_TRANSIENT_CONNECTION_CODE,
        Some(SQL_NON_TRANSIENT_CONNECTION_STATE.to_owned()),
        Some(SQL_NON_TRANSIENT_CONNECTION_REASON.to_owned()),
    )
    .with_class_name(SQL_NON_TRANSIENT_CONNECTION_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_NON_TRANSIENT_CONNECTION_CLASS,
        SqlExceptionKind::NonTransientConnection,
        SQL_NON_TRANSIENT_CONNECTION_CODE,
        Some(SQL_NON_TRANSIENT_CONNECTION_STATE),
        Some(SQL_NON_TRANSIENT_CONNECTION_REASON),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// Java `SQLNonTransientConnectionException(String reason, String sqlState, Throwable cause)`。
#[test]
fn sql_non_transient_connection_exception_init_reason_state_cause() {
    let exception = SqlException::new(
        0,
        Some(SQL_NON_TRANSIENT_CONNECTION_STATE.to_owned()),
        Some(SQL_NON_TRANSIENT_CONNECTION_REASON.to_owned()),
    )
    .with_class_name(SQL_NON_TRANSIENT_CONNECTION_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_NON_TRANSIENT_CONNECTION_CLASS,
        SqlExceptionKind::NonTransientConnection,
        0,
        Some(SQL_NON_TRANSIENT_CONNECTION_STATE),
        Some(SQL_NON_TRANSIENT_CONNECTION_REASON),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// Java `SQLNonTransientConnectionException(String reason, Throwable cause)`：SQLState 为空。
#[test]
fn sql_non_transient_connection_exception_init_reason_cause() {
    let exception = SqlException::new(
        0,
        None,
        Some(SQL_NON_TRANSIENT_CONNECTION_REASON.to_owned()),
    )
    .with_class_name(SQL_NON_TRANSIENT_CONNECTION_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_NON_TRANSIENT_CONNECTION_CLASS,
        SqlExceptionKind::NonTransientConnection,
        0,
        None,
        Some(SQL_NON_TRANSIENT_CONNECTION_REASON),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// Java `SQLNonTransientConnectionException(Throwable cause)`：reason 取 `cause.toString()`。
#[test]
fn sql_non_transient_connection_exception_init_cause() {
    let cause_text = throwable_to_string(CAUSE_CLASS, CAUSE_MESSAGE);
    let exception = SqlException::new(0, None, Some(cause_text.clone()))
        .with_class_name(SQL_NON_TRANSIENT_CONNECTION_CLASS)
        .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_NON_TRANSIENT_CONNECTION_CLASS,
        SqlExceptionKind::NonTransientConnection,
        0,
        None,
        Some(cause_text.as_str()),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// `SQLNonTransientConnectionException` 继承自 `SQLException` 的可观察 getter。
#[test]
fn sql_non_transient_connection_exception_inherited_getters() {
    let mut exception = SqlException::new(
        SQL_NON_TRANSIENT_CONNECTION_CODE,
        Some(SQL_NON_TRANSIENT_CONNECTION_STATE.to_owned()),
        Some(SQL_NON_TRANSIENT_CONNECTION_REASON.to_owned()),
    )
    .with_class_name(SQL_NON_TRANSIENT_CONNECTION_CLASS);

    // getErrorCode()
    assert_eq!(exception.error_code(), SQL_NON_TRANSIENT_CONNECTION_CODE);
    // getSQLState()
    assert_eq!(
        exception.sql_state(),
        Some(SQL_NON_TRANSIENT_CONNECTION_STATE)
    );
    // getNextException()
    assert!(exception.next_exception().is_none());
    assert!(exception.next_exceptions().is_empty());

    // setNextException(java.sql.SQLException)
    let next = SqlException::new(
        SQL_NON_TRANSIENT_CONNECTION_CODE + 1,
        Some(SQL_NON_TRANSIENT_CONNECTION_STATE.to_owned()),
        Some("chained connection failure".to_owned()),
    )
    .with_class_name(SQL_NON_TRANSIENT_CONNECTION_CLASS);
    exception.set_next_exception(next.clone());

    assert_eq!(exception.next_exception(), Some(&next));
    assert_eq!(exception.next_exceptions().len(), 1);
    assert_eq!(
        exception.next_exception().map(SqlException::error_code),
        Some(SQL_NON_TRANSIENT_CONNECTION_CODE + 1)
    );
    assert_eq!(
        exception.next_exception().map(SqlException::class_name),
        Some(SQL_NON_TRANSIENT_CONNECTION_CLASS)
    );
}

// ---------------------------------------------------------------------------
// java.sql.SQLNonTransientException
// ---------------------------------------------------------------------------

/// Java `SQLNonTransientException()`：reason、SQLState、vendor code 均为空。
#[test]
fn sql_non_transient_exception_init_empty() {
    let exception = SqlException::new(0, None, None).with_class_name(SQL_NON_TRANSIENT_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_NON_TRANSIENT_CLASS,
        SqlExceptionKind::NonTransient,
        0,
        None,
        None,
        &[],
    );
}

/// Java `SQLNonTransientException(String reason)`：只带 reason。
#[test]
fn sql_non_transient_exception_init_reason() {
    let exception = SqlException::new(0, None, Some(SQL_NON_TRANSIENT_REASON.to_owned()))
        .with_class_name(SQL_NON_TRANSIENT_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_NON_TRANSIENT_CLASS,
        SqlExceptionKind::NonTransient,
        0,
        None,
        Some(SQL_NON_TRANSIENT_REASON),
        &[],
    );
}

/// Java `SQLNonTransientException(String reason, String sqlState)`。
#[test]
fn sql_non_transient_exception_init_reason_state() {
    let exception = SqlException::new(
        0,
        Some(SQL_NON_TRANSIENT_STATE.to_owned()),
        Some(SQL_NON_TRANSIENT_REASON.to_owned()),
    )
    .with_class_name(SQL_NON_TRANSIENT_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_NON_TRANSIENT_CLASS,
        SqlExceptionKind::NonTransient,
        0,
        Some(SQL_NON_TRANSIENT_STATE),
        Some(SQL_NON_TRANSIENT_REASON),
        &[],
    );
}

/// Java `SQLNonTransientException(String reason, String sqlState, int vendorCode)`。
#[test]
fn sql_non_transient_exception_init_reason_state_code() {
    let exception = SqlException::new(
        SQL_NON_TRANSIENT_CODE,
        Some(SQL_NON_TRANSIENT_STATE.to_owned()),
        Some(SQL_NON_TRANSIENT_REASON.to_owned()),
    )
    .with_class_name(SQL_NON_TRANSIENT_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_NON_TRANSIENT_CLASS,
        SqlExceptionKind::NonTransient,
        SQL_NON_TRANSIENT_CODE,
        Some(SQL_NON_TRANSIENT_STATE),
        Some(SQL_NON_TRANSIENT_REASON),
        &[],
    );
}

/// Java `SQLNonTransientException(String, String, int, Throwable)`。
#[test]
fn sql_non_transient_exception_init_reason_state_code_cause() {
    let exception = SqlException::new(
        SQL_NON_TRANSIENT_CODE,
        Some(SQL_NON_TRANSIENT_STATE.to_owned()),
        Some(SQL_NON_TRANSIENT_REASON.to_owned()),
    )
    .with_class_name(SQL_NON_TRANSIENT_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_NON_TRANSIENT_CLASS,
        SqlExceptionKind::NonTransient,
        SQL_NON_TRANSIENT_CODE,
        Some(SQL_NON_TRANSIENT_STATE),
        Some(SQL_NON_TRANSIENT_REASON),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// Java `SQLNonTransientException(String reason, String sqlState, Throwable cause)`。
#[test]
fn sql_non_transient_exception_init_reason_state_cause() {
    let exception = SqlException::new(
        0,
        Some(SQL_NON_TRANSIENT_STATE.to_owned()),
        Some(SQL_NON_TRANSIENT_REASON.to_owned()),
    )
    .with_class_name(SQL_NON_TRANSIENT_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_NON_TRANSIENT_CLASS,
        SqlExceptionKind::NonTransient,
        0,
        Some(SQL_NON_TRANSIENT_STATE),
        Some(SQL_NON_TRANSIENT_REASON),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// Java `SQLNonTransientException(String reason, Throwable cause)`：SQLState 为空。
#[test]
fn sql_non_transient_exception_init_reason_cause() {
    let exception = SqlException::new(0, None, Some(SQL_NON_TRANSIENT_REASON.to_owned()))
        .with_class_name(SQL_NON_TRANSIENT_CLASS)
        .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_NON_TRANSIENT_CLASS,
        SqlExceptionKind::NonTransient,
        0,
        None,
        Some(SQL_NON_TRANSIENT_REASON),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// Java `SQLNonTransientException(Throwable cause)`：reason 取 `cause.toString()`。
#[test]
fn sql_non_transient_exception_init_cause() {
    let cause_text = throwable_to_string(CAUSE_CLASS, CAUSE_MESSAGE);
    let exception = SqlException::new(0, None, Some(cause_text.clone()))
        .with_class_name(SQL_NON_TRANSIENT_CLASS)
        .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_NON_TRANSIENT_CLASS,
        SqlExceptionKind::NonTransient,
        0,
        None,
        Some(cause_text.as_str()),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// `SQLNonTransientException` 继承自 `SQLException` 的可观察 getter。
#[test]
fn sql_non_transient_exception_inherited_getters() {
    let mut exception = SqlException::new(
        SQL_NON_TRANSIENT_CODE,
        Some(SQL_NON_TRANSIENT_STATE.to_owned()),
        Some(SQL_NON_TRANSIENT_REASON.to_owned()),
    )
    .with_class_name(SQL_NON_TRANSIENT_CLASS);

    // getErrorCode()
    assert_eq!(exception.error_code(), SQL_NON_TRANSIENT_CODE);
    // getSQLState()
    assert_eq!(exception.sql_state(), Some(SQL_NON_TRANSIENT_STATE));
    // getNextException()
    assert!(exception.next_exception().is_none());
    assert!(exception.next_exceptions().is_empty());

    // setNextException(java.sql.SQLException)
    let next = SqlException::new(
        SQL_NON_TRANSIENT_CODE + 1,
        Some(SQL_NON_TRANSIENT_STATE.to_owned()),
        Some("chained data failure".to_owned()),
    )
    .with_class_name(SQL_NON_TRANSIENT_CLASS);
    exception.set_next_exception(next.clone());

    assert_eq!(exception.next_exception(), Some(&next));
    assert_eq!(exception.next_exceptions().len(), 1);
    assert_eq!(
        exception.next_exception().map(SqlException::error_code),
        Some(SQL_NON_TRANSIENT_CODE + 1)
    );
    assert_eq!(
        exception.next_exception().map(SqlException::class_name),
        Some(SQL_NON_TRANSIENT_CLASS)
    );
}

// ---------------------------------------------------------------------------
// java.sql.SQLRecoverableException
// ---------------------------------------------------------------------------

/// Java `SQLRecoverableException()`：reason、SQLState、vendor code 均为空。
#[test]
fn sql_recoverable_exception_init_empty() {
    let exception = SqlException::new(0, None, None).with_class_name(SQL_RECOVERABLE_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_RECOVERABLE_CLASS,
        SqlExceptionKind::Recoverable,
        0,
        None,
        None,
        &[],
    );
}

/// Java `SQLRecoverableException(String reason)`：只带 reason。
#[test]
fn sql_recoverable_exception_init_reason() {
    let exception = SqlException::new(0, None, Some(SQL_RECOVERABLE_REASON.to_owned()))
        .with_class_name(SQL_RECOVERABLE_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_RECOVERABLE_CLASS,
        SqlExceptionKind::Recoverable,
        0,
        None,
        Some(SQL_RECOVERABLE_REASON),
        &[],
    );
}

/// Java `SQLRecoverableException(String reason, String sqlState)`。
#[test]
fn sql_recoverable_exception_init_reason_state() {
    let exception = SqlException::new(
        0,
        Some(SQL_RECOVERABLE_STATE.to_owned()),
        Some(SQL_RECOVERABLE_REASON.to_owned()),
    )
    .with_class_name(SQL_RECOVERABLE_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_RECOVERABLE_CLASS,
        SqlExceptionKind::Recoverable,
        0,
        Some(SQL_RECOVERABLE_STATE),
        Some(SQL_RECOVERABLE_REASON),
        &[],
    );
}

/// Java `SQLRecoverableException(String reason, String sqlState, int vendorCode)`。
#[test]
fn sql_recoverable_exception_init_reason_state_code() {
    let exception = SqlException::new(
        SQL_RECOVERABLE_CODE,
        Some(SQL_RECOVERABLE_STATE.to_owned()),
        Some(SQL_RECOVERABLE_REASON.to_owned()),
    )
    .with_class_name(SQL_RECOVERABLE_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_RECOVERABLE_CLASS,
        SqlExceptionKind::Recoverable,
        SQL_RECOVERABLE_CODE,
        Some(SQL_RECOVERABLE_STATE),
        Some(SQL_RECOVERABLE_REASON),
        &[],
    );
}

/// Java `SQLRecoverableException(String, String, int, Throwable)`。
#[test]
fn sql_recoverable_exception_init_reason_state_code_cause() {
    let exception = SqlException::new(
        SQL_RECOVERABLE_CODE,
        Some(SQL_RECOVERABLE_STATE.to_owned()),
        Some(SQL_RECOVERABLE_REASON.to_owned()),
    )
    .with_class_name(SQL_RECOVERABLE_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_RECOVERABLE_CLASS,
        SqlExceptionKind::Recoverable,
        SQL_RECOVERABLE_CODE,
        Some(SQL_RECOVERABLE_STATE),
        Some(SQL_RECOVERABLE_REASON),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// Java `SQLRecoverableException(String reason, String sqlState, Throwable cause)`。
#[test]
fn sql_recoverable_exception_init_reason_state_cause() {
    let exception = SqlException::new(
        0,
        Some(SQL_RECOVERABLE_STATE.to_owned()),
        Some(SQL_RECOVERABLE_REASON.to_owned()),
    )
    .with_class_name(SQL_RECOVERABLE_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_RECOVERABLE_CLASS,
        SqlExceptionKind::Recoverable,
        0,
        Some(SQL_RECOVERABLE_STATE),
        Some(SQL_RECOVERABLE_REASON),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// Java `SQLRecoverableException(String reason, Throwable cause)`：SQLState 为空。
#[test]
fn sql_recoverable_exception_init_reason_cause() {
    let exception = SqlException::new(0, None, Some(SQL_RECOVERABLE_REASON.to_owned()))
        .with_class_name(SQL_RECOVERABLE_CLASS)
        .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_RECOVERABLE_CLASS,
        SqlExceptionKind::Recoverable,
        0,
        None,
        Some(SQL_RECOVERABLE_REASON),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// Java `SQLRecoverableException(Throwable cause)`：reason 取 `cause.toString()`。
#[test]
fn sql_recoverable_exception_init_cause() {
    let cause_text = throwable_to_string(CAUSE_CLASS, CAUSE_MESSAGE);
    let exception = SqlException::new(0, None, Some(cause_text.clone()))
        .with_class_name(SQL_RECOVERABLE_CLASS)
        .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_RECOVERABLE_CLASS,
        SqlExceptionKind::Recoverable,
        0,
        None,
        Some(cause_text.as_str()),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// `SQLRecoverableException` 继承自 `SQLException` 的可观察 getter。
#[test]
fn sql_recoverable_exception_inherited_getters() {
    let mut exception = SqlException::new(
        SQL_RECOVERABLE_CODE,
        Some(SQL_RECOVERABLE_STATE.to_owned()),
        Some(SQL_RECOVERABLE_REASON.to_owned()),
    )
    .with_class_name(SQL_RECOVERABLE_CLASS);

    // getErrorCode()
    assert_eq!(exception.error_code(), SQL_RECOVERABLE_CODE);
    // getSQLState()
    assert_eq!(exception.sql_state(), Some(SQL_RECOVERABLE_STATE));
    // getNextException()
    assert!(exception.next_exception().is_none());
    assert!(exception.next_exceptions().is_empty());

    // setNextException(java.sql.SQLException)
    let next = SqlException::new(
        SQL_RECOVERABLE_CODE + 1,
        Some(SQL_RECOVERABLE_STATE.to_owned()),
        Some("chained recoverable failure".to_owned()),
    )
    .with_class_name(SQL_RECOVERABLE_CLASS);
    exception.set_next_exception(next.clone());

    assert_eq!(exception.next_exception(), Some(&next));
    assert_eq!(exception.next_exceptions().len(), 1);
    assert_eq!(
        exception.next_exception().map(SqlException::error_code),
        Some(SQL_RECOVERABLE_CODE + 1)
    );
    assert_eq!(
        exception.next_exception().map(SqlException::class_name),
        Some(SQL_RECOVERABLE_CLASS)
    );
}
