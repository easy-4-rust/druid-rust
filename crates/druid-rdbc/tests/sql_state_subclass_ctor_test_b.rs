//! JDBC `SQLState` 子类异常的构造器契约测试（B 组）。
//!
//! 对应 Java: `java.sql.SQLSyntaxErrorException`、
//! `java.sql.SQLTimeoutException`、
//! `java.sql.SQLTransactionRollbackException`、
//! `java.sql.SQLTransientConnectionException`、
//! `java.sql.SQLTransientException` 的 8 个 `<init>` 构造器重载，以及这 5 个
//! 子类从 `java.sql.SQLException` 继承的 `getErrorCode()`、`getSQLState()`、
//! `getNextException()`、`setNextException(java.sql.SQLException)`。
//!
//! Rust 侧没有为这些 Java 子类各建一个独立结构体：`SQLException` 家族统一由
//! [`SqlException`] 承载，子类身份由 `with_class_name` 写入并保留
//! `java.sql.SQLException` 的可赋值父类型，因此每个用例同时断言
//! `class_name()`、`is_instance_of("java.sql.SQLException")` 与 `kind()`。
//!
//! 本文件覆盖的 Java 类与 `sql_exception_constructor_contract_test.rs`、
//! `sql_state_subclass_ctor_test_a.rs` 完全不重叠，不重复这两个文件已覆盖的身份。

use druid_rdbc::error::{SqlException, SqlExceptionCause, SqlExceptionKind};

/// Java `java.sql.SQLException` 的规范类名。
const SQL_EXCEPTION_CLASS: &str = "java.sql.SQLException";
/// Java `java.sql.SQLSyntaxErrorException` 的规范类名。
const SQL_SYNTAX_ERROR_CLASS: &str = "java.sql.SQLSyntaxErrorException";
/// `SQLSyntaxErrorException` 的样例 SQLState（语法错误）。
const SQL_SYNTAX_ERROR_STATE: &str = "42000";
/// `SQLSyntaxErrorException` 的样例 vendor code。
const SQL_SYNTAX_ERROR_CODE: i32 = 42000;
/// `SQLSyntaxErrorException` 的样例 reason。
const SQL_SYNTAX_ERROR_REASON: &str = "syntax error at or near position 1";

/// Java `java.sql.SQLTimeoutException` 的规范类名。
const SQL_TIMEOUT_CLASS: &str = "java.sql.SQLTimeoutException";
/// `SQLTimeoutException` 的样例 SQLState（操作超时）。
const SQL_TIMEOUT_STATE: &str = "HYT00";
/// `SQLTimeoutException` 的样例 vendor code。
const SQL_TIMEOUT_CODE: i32 = 57014;
/// `SQLTimeoutException` 的样例 reason。
const SQL_TIMEOUT_REASON: &str = "query execution timed out";

/// Java `java.sql.SQLTransactionRollbackException` 的规范类名。
const SQL_TRANSACTION_ROLLBACK_CLASS: &str = "java.sql.SQLTransactionRollbackException";
/// `SQLTransactionRollbackException` 的样例 SQLState（事务回滚）。
const SQL_TRANSACTION_ROLLBACK_STATE: &str = "40001";
/// `SQLTransactionRollbackException` 的样例 vendor code。
const SQL_TRANSACTION_ROLLBACK_CODE: i32 = 40001;
/// `SQLTransactionRollbackException` 的样例 reason。
const SQL_TRANSACTION_ROLLBACK_REASON: &str =
    "transaction rolled back due to serialization failure";

/// Java `java.sql.SQLTransientConnectionException` 的规范类名。
const SQL_TRANSIENT_CONNECTION_CLASS: &str = "java.sql.SQLTransientConnectionException";
/// `SQLTransientConnectionException` 的样例 SQLState（连接失败）。
const SQL_TRANSIENT_CONNECTION_STATE: &str = "08001";
/// `SQLTransientConnectionException` 的样例 vendor code。
const SQL_TRANSIENT_CONNECTION_CODE: i32 = 8001;
/// `SQLTransientConnectionException` 的样例 reason。
const SQL_TRANSIENT_CONNECTION_REASON: &str = "unable to establish a connection";

/// Java `java.sql.SQLTransientException` 的规范类名。
const SQL_TRANSIENT_CLASS: &str = "java.sql.SQLTransientException";
/// `SQLTransientException` 的样例 SQLState（可重试的连接故障）。
const SQL_TRANSIENT_STATE: &str = "08000";
/// `SQLTransientException` 的样例 vendor code。
const SQL_TRANSIENT_CODE: i32 = 8000;
/// `SQLTransientException` 的样例 reason。
const SQL_TRANSIENT_REASON: &str = "connection failure during transient operation";

/// Java cause 的运行时类名。
const CAUSE_CLASS: &str = "java.lang.IllegalStateException";
/// Java cause 的消息。
const CAUSE_MESSAGE: &str = "underlying driver failure";

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
// java.sql.SQLSyntaxErrorException
// ---------------------------------------------------------------------------

/// Java `SQLSyntaxErrorException()`：reason、SQLState、vendor code 均为空。
#[test]
fn sql_syntax_error_exception_init_empty() {
    let exception = SqlException::new(0, None, None).with_class_name(SQL_SYNTAX_ERROR_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_SYNTAX_ERROR_CLASS,
        SqlExceptionKind::Syntax,
        0,
        None,
        None,
        &[],
    );
}

/// Java `SQLSyntaxErrorException(String reason)`：只带 reason。
#[test]
fn sql_syntax_error_exception_init_reason() {
    let exception = SqlException::new(0, None, Some(SQL_SYNTAX_ERROR_REASON.to_owned()))
        .with_class_name(SQL_SYNTAX_ERROR_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_SYNTAX_ERROR_CLASS,
        SqlExceptionKind::Syntax,
        0,
        None,
        Some(SQL_SYNTAX_ERROR_REASON),
        &[],
    );
}

/// Java `SQLSyntaxErrorException(String reason, String sqlState)`。
#[test]
fn sql_syntax_error_exception_init_reason_state() {
    let exception = SqlException::new(
        0,
        Some(SQL_SYNTAX_ERROR_STATE.to_owned()),
        Some(SQL_SYNTAX_ERROR_REASON.to_owned()),
    )
    .with_class_name(SQL_SYNTAX_ERROR_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_SYNTAX_ERROR_CLASS,
        SqlExceptionKind::Syntax,
        0,
        Some(SQL_SYNTAX_ERROR_STATE),
        Some(SQL_SYNTAX_ERROR_REASON),
        &[],
    );
}

/// Java `SQLSyntaxErrorException(String reason, String sqlState, int vendorCode)`。
#[test]
fn sql_syntax_error_exception_init_reason_state_code() {
    let exception = SqlException::new(
        SQL_SYNTAX_ERROR_CODE,
        Some(SQL_SYNTAX_ERROR_STATE.to_owned()),
        Some(SQL_SYNTAX_ERROR_REASON.to_owned()),
    )
    .with_class_name(SQL_SYNTAX_ERROR_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_SYNTAX_ERROR_CLASS,
        SqlExceptionKind::Syntax,
        SQL_SYNTAX_ERROR_CODE,
        Some(SQL_SYNTAX_ERROR_STATE),
        Some(SQL_SYNTAX_ERROR_REASON),
        &[],
    );
}

/// Java `SQLSyntaxErrorException(String reason, String sqlState, int vendorCode, Throwable cause)`。
#[test]
fn sql_syntax_error_exception_init_reason_state_code_cause() {
    let exception = SqlException::new(
        SQL_SYNTAX_ERROR_CODE,
        Some(SQL_SYNTAX_ERROR_STATE.to_owned()),
        Some(SQL_SYNTAX_ERROR_REASON.to_owned()),
    )
    .with_class_name(SQL_SYNTAX_ERROR_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_SYNTAX_ERROR_CLASS,
        SqlExceptionKind::Syntax,
        SQL_SYNTAX_ERROR_CODE,
        Some(SQL_SYNTAX_ERROR_STATE),
        Some(SQL_SYNTAX_ERROR_REASON),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// Java `SQLSyntaxErrorException(String reason, String sqlState, Throwable cause)`。
#[test]
fn sql_syntax_error_exception_init_reason_state_cause() {
    let exception = SqlException::new(
        0,
        Some(SQL_SYNTAX_ERROR_STATE.to_owned()),
        Some(SQL_SYNTAX_ERROR_REASON.to_owned()),
    )
    .with_class_name(SQL_SYNTAX_ERROR_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_SYNTAX_ERROR_CLASS,
        SqlExceptionKind::Syntax,
        0,
        Some(SQL_SYNTAX_ERROR_STATE),
        Some(SQL_SYNTAX_ERROR_REASON),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// Java `SQLSyntaxErrorException(String reason, Throwable cause)`。
#[test]
fn sql_syntax_error_exception_init_reason_cause() {
    let exception = SqlException::new(0, None, Some(SQL_SYNTAX_ERROR_REASON.to_owned()))
        .with_class_name(SQL_SYNTAX_ERROR_CLASS)
        .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_SYNTAX_ERROR_CLASS,
        SqlExceptionKind::Syntax,
        0,
        None,
        Some(SQL_SYNTAX_ERROR_REASON),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// Java `SQLSyntaxErrorException(Throwable cause)`：reason 取 `cause.toString()`。
#[test]
fn sql_syntax_error_exception_init_cause() {
    let cause_text = throwable_to_string(CAUSE_CLASS, CAUSE_MESSAGE);
    let exception = SqlException::new(0, None, Some(cause_text.clone()))
        .with_class_name(SQL_SYNTAX_ERROR_CLASS)
        .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_SYNTAX_ERROR_CLASS,
        SqlExceptionKind::Syntax,
        0,
        None,
        Some(cause_text.as_str()),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// `SQLSyntaxErrorException` 继承自 `SQLException` 的可观察 getter。
#[test]
fn sql_syntax_error_exception_inherited_getters() {
    let mut exception = SqlException::new(
        SQL_SYNTAX_ERROR_CODE,
        Some(SQL_SYNTAX_ERROR_STATE.to_owned()),
        Some(SQL_SYNTAX_ERROR_REASON.to_owned()),
    )
    .with_class_name(SQL_SYNTAX_ERROR_CLASS);

    // getErrorCode()
    assert_eq!(exception.error_code(), SQL_SYNTAX_ERROR_CODE);
    // getSQLState()
    assert_eq!(exception.sql_state(), Some(SQL_SYNTAX_ERROR_STATE));
    // getNextException()
    assert!(exception.next_exception().is_none());
    assert!(exception.next_exceptions().is_empty());

    // setNextException(java.sql.SQLException)
    let next = SqlException::new(
        SQL_SYNTAX_ERROR_CODE + 1,
        Some(SQL_SYNTAX_ERROR_STATE.to_owned()),
        Some("chained syntax failure".to_owned()),
    )
    .with_class_name(SQL_SYNTAX_ERROR_CLASS);
    exception.set_next_exception(next.clone());

    assert_eq!(exception.next_exception(), Some(&next));
    assert_eq!(exception.next_exceptions().len(), 1);
    assert_eq!(
        exception.next_exception().map(SqlException::error_code),
        Some(SQL_SYNTAX_ERROR_CODE + 1)
    );
    assert_eq!(
        exception.next_exception().map(SqlException::class_name),
        Some(SQL_SYNTAX_ERROR_CLASS)
    );
}

// ---------------------------------------------------------------------------
// java.sql.SQLTimeoutException
// ---------------------------------------------------------------------------

/// Java `SQLTimeoutException()`：reason、SQLState、vendor code 均为空。
#[test]
fn sql_timeout_exception_init_empty() {
    let exception = SqlException::new(0, None, None).with_class_name(SQL_TIMEOUT_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_TIMEOUT_CLASS,
        SqlExceptionKind::Timeout,
        0,
        None,
        None,
        &[],
    );
}

/// Java `SQLTimeoutException(String reason)`：只带 reason。
#[test]
fn sql_timeout_exception_init_reason() {
    let exception = SqlException::new(0, None, Some(SQL_TIMEOUT_REASON.to_owned()))
        .with_class_name(SQL_TIMEOUT_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_TIMEOUT_CLASS,
        SqlExceptionKind::Timeout,
        0,
        None,
        Some(SQL_TIMEOUT_REASON),
        &[],
    );
}

/// Java `SQLTimeoutException(String reason, String sqlState)`。
#[test]
fn sql_timeout_exception_init_reason_state() {
    let exception = SqlException::new(
        0,
        Some(SQL_TIMEOUT_STATE.to_owned()),
        Some(SQL_TIMEOUT_REASON.to_owned()),
    )
    .with_class_name(SQL_TIMEOUT_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_TIMEOUT_CLASS,
        SqlExceptionKind::Timeout,
        0,
        Some(SQL_TIMEOUT_STATE),
        Some(SQL_TIMEOUT_REASON),
        &[],
    );
}

/// Java `SQLTimeoutException(String reason, String sqlState, int vendorCode)`。
#[test]
fn sql_timeout_exception_init_reason_state_code() {
    let exception = SqlException::new(
        SQL_TIMEOUT_CODE,
        Some(SQL_TIMEOUT_STATE.to_owned()),
        Some(SQL_TIMEOUT_REASON.to_owned()),
    )
    .with_class_name(SQL_TIMEOUT_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_TIMEOUT_CLASS,
        SqlExceptionKind::Timeout,
        SQL_TIMEOUT_CODE,
        Some(SQL_TIMEOUT_STATE),
        Some(SQL_TIMEOUT_REASON),
        &[],
    );
}

/// Java `SQLTimeoutException(String reason, String sqlState, int vendorCode, Throwable cause)`。
#[test]
fn sql_timeout_exception_init_reason_state_code_cause() {
    let exception = SqlException::new(
        SQL_TIMEOUT_CODE,
        Some(SQL_TIMEOUT_STATE.to_owned()),
        Some(SQL_TIMEOUT_REASON.to_owned()),
    )
    .with_class_name(SQL_TIMEOUT_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_TIMEOUT_CLASS,
        SqlExceptionKind::Timeout,
        SQL_TIMEOUT_CODE,
        Some(SQL_TIMEOUT_STATE),
        Some(SQL_TIMEOUT_REASON),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// Java `SQLTimeoutException(String reason, String sqlState, Throwable cause)`。
#[test]
fn sql_timeout_exception_init_reason_state_cause() {
    let exception = SqlException::new(
        0,
        Some(SQL_TIMEOUT_STATE.to_owned()),
        Some(SQL_TIMEOUT_REASON.to_owned()),
    )
    .with_class_name(SQL_TIMEOUT_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_TIMEOUT_CLASS,
        SqlExceptionKind::Timeout,
        0,
        Some(SQL_TIMEOUT_STATE),
        Some(SQL_TIMEOUT_REASON),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// Java `SQLTimeoutException(String reason, Throwable cause)`。
#[test]
fn sql_timeout_exception_init_reason_cause() {
    let exception = SqlException::new(0, None, Some(SQL_TIMEOUT_REASON.to_owned()))
        .with_class_name(SQL_TIMEOUT_CLASS)
        .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_TIMEOUT_CLASS,
        SqlExceptionKind::Timeout,
        0,
        None,
        Some(SQL_TIMEOUT_REASON),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// Java `SQLTimeoutException(Throwable cause)`：reason 取 `cause.toString()`。
#[test]
fn sql_timeout_exception_init_cause() {
    let cause_text = throwable_to_string(CAUSE_CLASS, CAUSE_MESSAGE);
    let exception = SqlException::new(0, None, Some(cause_text.clone()))
        .with_class_name(SQL_TIMEOUT_CLASS)
        .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_TIMEOUT_CLASS,
        SqlExceptionKind::Timeout,
        0,
        None,
        Some(cause_text.as_str()),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// `SQLTimeoutException` 继承自 `SQLException` 的可观察 getter。
#[test]
fn sql_timeout_exception_inherited_getters() {
    let mut exception = SqlException::new(
        SQL_TIMEOUT_CODE,
        Some(SQL_TIMEOUT_STATE.to_owned()),
        Some(SQL_TIMEOUT_REASON.to_owned()),
    )
    .with_class_name(SQL_TIMEOUT_CLASS);

    // getErrorCode()
    assert_eq!(exception.error_code(), SQL_TIMEOUT_CODE);
    // getSQLState()
    assert_eq!(exception.sql_state(), Some(SQL_TIMEOUT_STATE));
    // getNextException()
    assert!(exception.next_exception().is_none());
    assert!(exception.next_exceptions().is_empty());

    // setNextException(java.sql.SQLException)
    let next = SqlException::new(
        SQL_TIMEOUT_CODE + 1,
        Some(SQL_TIMEOUT_STATE.to_owned()),
        Some("chained timeout failure".to_owned()),
    )
    .with_class_name(SQL_TIMEOUT_CLASS);
    exception.set_next_exception(next.clone());

    assert_eq!(exception.next_exception(), Some(&next));
    assert_eq!(exception.next_exceptions().len(), 1);
    assert_eq!(
        exception.next_exception().map(SqlException::error_code),
        Some(SQL_TIMEOUT_CODE + 1)
    );
    assert_eq!(
        exception.next_exception().map(SqlException::class_name),
        Some(SQL_TIMEOUT_CLASS)
    );
}

// ---------------------------------------------------------------------------
// java.sql.SQLTransactionRollbackException
// ---------------------------------------------------------------------------

/// Java `SQLTransactionRollbackException()`：reason、SQLState、vendor code 均为空。
#[test]
fn sql_transaction_rollback_exception_init_empty() {
    let exception =
        SqlException::new(0, None, None).with_class_name(SQL_TRANSACTION_ROLLBACK_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_TRANSACTION_ROLLBACK_CLASS,
        SqlExceptionKind::TransactionRollback,
        0,
        None,
        None,
        &[],
    );
}

/// Java `SQLTransactionRollbackException(String reason)`：只带 reason。
#[test]
fn sql_transaction_rollback_exception_init_reason() {
    let exception = SqlException::new(0, None, Some(SQL_TRANSACTION_ROLLBACK_REASON.to_owned()))
        .with_class_name(SQL_TRANSACTION_ROLLBACK_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_TRANSACTION_ROLLBACK_CLASS,
        SqlExceptionKind::TransactionRollback,
        0,
        None,
        Some(SQL_TRANSACTION_ROLLBACK_REASON),
        &[],
    );
}

/// Java `SQLTransactionRollbackException(String reason, String sqlState)`。
#[test]
fn sql_transaction_rollback_exception_init_reason_state() {
    let exception = SqlException::new(
        0,
        Some(SQL_TRANSACTION_ROLLBACK_STATE.to_owned()),
        Some(SQL_TRANSACTION_ROLLBACK_REASON.to_owned()),
    )
    .with_class_name(SQL_TRANSACTION_ROLLBACK_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_TRANSACTION_ROLLBACK_CLASS,
        SqlExceptionKind::TransactionRollback,
        0,
        Some(SQL_TRANSACTION_ROLLBACK_STATE),
        Some(SQL_TRANSACTION_ROLLBACK_REASON),
        &[],
    );
}

/// Java `SQLTransactionRollbackException(String reason, String sqlState, int vendorCode)`。
#[test]
fn sql_transaction_rollback_exception_init_reason_state_code() {
    let exception = SqlException::new(
        SQL_TRANSACTION_ROLLBACK_CODE,
        Some(SQL_TRANSACTION_ROLLBACK_STATE.to_owned()),
        Some(SQL_TRANSACTION_ROLLBACK_REASON.to_owned()),
    )
    .with_class_name(SQL_TRANSACTION_ROLLBACK_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_TRANSACTION_ROLLBACK_CLASS,
        SqlExceptionKind::TransactionRollback,
        SQL_TRANSACTION_ROLLBACK_CODE,
        Some(SQL_TRANSACTION_ROLLBACK_STATE),
        Some(SQL_TRANSACTION_ROLLBACK_REASON),
        &[],
    );
}

/// Java `SQLTransactionRollbackException(String reason, String sqlState, int vendorCode, Throwable cause)`。
#[test]
fn sql_transaction_rollback_exception_init_reason_state_code_cause() {
    let exception = SqlException::new(
        SQL_TRANSACTION_ROLLBACK_CODE,
        Some(SQL_TRANSACTION_ROLLBACK_STATE.to_owned()),
        Some(SQL_TRANSACTION_ROLLBACK_REASON.to_owned()),
    )
    .with_class_name(SQL_TRANSACTION_ROLLBACK_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_TRANSACTION_ROLLBACK_CLASS,
        SqlExceptionKind::TransactionRollback,
        SQL_TRANSACTION_ROLLBACK_CODE,
        Some(SQL_TRANSACTION_ROLLBACK_STATE),
        Some(SQL_TRANSACTION_ROLLBACK_REASON),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// Java `SQLTransactionRollbackException(String reason, String sqlState, Throwable cause)`。
#[test]
fn sql_transaction_rollback_exception_init_reason_state_cause() {
    let exception = SqlException::new(
        0,
        Some(SQL_TRANSACTION_ROLLBACK_STATE.to_owned()),
        Some(SQL_TRANSACTION_ROLLBACK_REASON.to_owned()),
    )
    .with_class_name(SQL_TRANSACTION_ROLLBACK_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_TRANSACTION_ROLLBACK_CLASS,
        SqlExceptionKind::TransactionRollback,
        0,
        Some(SQL_TRANSACTION_ROLLBACK_STATE),
        Some(SQL_TRANSACTION_ROLLBACK_REASON),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// Java `SQLTransactionRollbackException(String reason, Throwable cause)`。
#[test]
fn sql_transaction_rollback_exception_init_reason_cause() {
    let exception = SqlException::new(0, None, Some(SQL_TRANSACTION_ROLLBACK_REASON.to_owned()))
        .with_class_name(SQL_TRANSACTION_ROLLBACK_CLASS)
        .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_TRANSACTION_ROLLBACK_CLASS,
        SqlExceptionKind::TransactionRollback,
        0,
        None,
        Some(SQL_TRANSACTION_ROLLBACK_REASON),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// Java `SQLTransactionRollbackException(Throwable cause)`：reason 取 `cause.toString()`。
#[test]
fn sql_transaction_rollback_exception_init_cause() {
    let cause_text = throwable_to_string(CAUSE_CLASS, CAUSE_MESSAGE);
    let exception = SqlException::new(0, None, Some(cause_text.clone()))
        .with_class_name(SQL_TRANSACTION_ROLLBACK_CLASS)
        .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_TRANSACTION_ROLLBACK_CLASS,
        SqlExceptionKind::TransactionRollback,
        0,
        None,
        Some(cause_text.as_str()),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// `SQLTransactionRollbackException` 继承自 `SQLException` 的可观察 getter。
#[test]
fn sql_transaction_rollback_exception_inherited_getters() {
    let mut exception = SqlException::new(
        SQL_TRANSACTION_ROLLBACK_CODE,
        Some(SQL_TRANSACTION_ROLLBACK_STATE.to_owned()),
        Some(SQL_TRANSACTION_ROLLBACK_REASON.to_owned()),
    )
    .with_class_name(SQL_TRANSACTION_ROLLBACK_CLASS);

    // getErrorCode()
    assert_eq!(exception.error_code(), SQL_TRANSACTION_ROLLBACK_CODE);
    // getSQLState()
    assert_eq!(exception.sql_state(), Some(SQL_TRANSACTION_ROLLBACK_STATE));
    // getNextException()
    assert!(exception.next_exception().is_none());
    assert!(exception.next_exceptions().is_empty());

    // setNextException(java.sql.SQLException)
    let next = SqlException::new(
        SQL_TRANSACTION_ROLLBACK_CODE + 1,
        Some(SQL_TRANSACTION_ROLLBACK_STATE.to_owned()),
        Some("chained rollback failure".to_owned()),
    )
    .with_class_name(SQL_TRANSACTION_ROLLBACK_CLASS);
    exception.set_next_exception(next.clone());

    assert_eq!(exception.next_exception(), Some(&next));
    assert_eq!(exception.next_exceptions().len(), 1);
    assert_eq!(
        exception.next_exception().map(SqlException::error_code),
        Some(SQL_TRANSACTION_ROLLBACK_CODE + 1)
    );
    assert_eq!(
        exception.next_exception().map(SqlException::class_name),
        Some(SQL_TRANSACTION_ROLLBACK_CLASS)
    );
}

// ---------------------------------------------------------------------------
// java.sql.SQLTransientConnectionException
// ---------------------------------------------------------------------------

/// Java `SQLTransientConnectionException()`：reason、SQLState、vendor code 均为空。
#[test]
fn sql_transient_connection_exception_init_empty() {
    let exception =
        SqlException::new(0, None, None).with_class_name(SQL_TRANSIENT_CONNECTION_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_TRANSIENT_CONNECTION_CLASS,
        SqlExceptionKind::TransientConnection,
        0,
        None,
        None,
        &[],
    );
}

/// Java `SQLTransientConnectionException(String reason)`：只带 reason。
#[test]
fn sql_transient_connection_exception_init_reason() {
    let exception = SqlException::new(0, None, Some(SQL_TRANSIENT_CONNECTION_REASON.to_owned()))
        .with_class_name(SQL_TRANSIENT_CONNECTION_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_TRANSIENT_CONNECTION_CLASS,
        SqlExceptionKind::TransientConnection,
        0,
        None,
        Some(SQL_TRANSIENT_CONNECTION_REASON),
        &[],
    );
}

/// Java `SQLTransientConnectionException(String reason, String sqlState)`。
#[test]
fn sql_transient_connection_exception_init_reason_state() {
    let exception = SqlException::new(
        0,
        Some(SQL_TRANSIENT_CONNECTION_STATE.to_owned()),
        Some(SQL_TRANSIENT_CONNECTION_REASON.to_owned()),
    )
    .with_class_name(SQL_TRANSIENT_CONNECTION_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_TRANSIENT_CONNECTION_CLASS,
        SqlExceptionKind::TransientConnection,
        0,
        Some(SQL_TRANSIENT_CONNECTION_STATE),
        Some(SQL_TRANSIENT_CONNECTION_REASON),
        &[],
    );
}

/// Java `SQLTransientConnectionException(String reason, String sqlState, int vendorCode)`。
#[test]
fn sql_transient_connection_exception_init_reason_state_code() {
    let exception = SqlException::new(
        SQL_TRANSIENT_CONNECTION_CODE,
        Some(SQL_TRANSIENT_CONNECTION_STATE.to_owned()),
        Some(SQL_TRANSIENT_CONNECTION_REASON.to_owned()),
    )
    .with_class_name(SQL_TRANSIENT_CONNECTION_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_TRANSIENT_CONNECTION_CLASS,
        SqlExceptionKind::TransientConnection,
        SQL_TRANSIENT_CONNECTION_CODE,
        Some(SQL_TRANSIENT_CONNECTION_STATE),
        Some(SQL_TRANSIENT_CONNECTION_REASON),
        &[],
    );
}

/// Java `SQLTransientConnectionException(String reason, String sqlState, int vendorCode, Throwable cause)`。
#[test]
fn sql_transient_connection_exception_init_reason_state_code_cause() {
    let exception = SqlException::new(
        SQL_TRANSIENT_CONNECTION_CODE,
        Some(SQL_TRANSIENT_CONNECTION_STATE.to_owned()),
        Some(SQL_TRANSIENT_CONNECTION_REASON.to_owned()),
    )
    .with_class_name(SQL_TRANSIENT_CONNECTION_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_TRANSIENT_CONNECTION_CLASS,
        SqlExceptionKind::TransientConnection,
        SQL_TRANSIENT_CONNECTION_CODE,
        Some(SQL_TRANSIENT_CONNECTION_STATE),
        Some(SQL_TRANSIENT_CONNECTION_REASON),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// Java `SQLTransientConnectionException(String reason, String sqlState, Throwable cause)`。
#[test]
fn sql_transient_connection_exception_init_reason_state_cause() {
    let exception = SqlException::new(
        0,
        Some(SQL_TRANSIENT_CONNECTION_STATE.to_owned()),
        Some(SQL_TRANSIENT_CONNECTION_REASON.to_owned()),
    )
    .with_class_name(SQL_TRANSIENT_CONNECTION_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_TRANSIENT_CONNECTION_CLASS,
        SqlExceptionKind::TransientConnection,
        0,
        Some(SQL_TRANSIENT_CONNECTION_STATE),
        Some(SQL_TRANSIENT_CONNECTION_REASON),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// Java `SQLTransientConnectionException(String reason, Throwable cause)`。
#[test]
fn sql_transient_connection_exception_init_reason_cause() {
    let exception = SqlException::new(0, None, Some(SQL_TRANSIENT_CONNECTION_REASON.to_owned()))
        .with_class_name(SQL_TRANSIENT_CONNECTION_CLASS)
        .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_TRANSIENT_CONNECTION_CLASS,
        SqlExceptionKind::TransientConnection,
        0,
        None,
        Some(SQL_TRANSIENT_CONNECTION_REASON),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// Java `SQLTransientConnectionException(Throwable cause)`：reason 取 `cause.toString()`。
#[test]
fn sql_transient_connection_exception_init_cause() {
    let cause_text = throwable_to_string(CAUSE_CLASS, CAUSE_MESSAGE);
    let exception = SqlException::new(0, None, Some(cause_text.clone()))
        .with_class_name(SQL_TRANSIENT_CONNECTION_CLASS)
        .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_TRANSIENT_CONNECTION_CLASS,
        SqlExceptionKind::TransientConnection,
        0,
        None,
        Some(cause_text.as_str()),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// `SQLTransientConnectionException` 继承自 `SQLException` 的可观察 getter。
#[test]
fn sql_transient_connection_exception_inherited_getters() {
    let mut exception = SqlException::new(
        SQL_TRANSIENT_CONNECTION_CODE,
        Some(SQL_TRANSIENT_CONNECTION_STATE.to_owned()),
        Some(SQL_TRANSIENT_CONNECTION_REASON.to_owned()),
    )
    .with_class_name(SQL_TRANSIENT_CONNECTION_CLASS);

    // getErrorCode()
    assert_eq!(exception.error_code(), SQL_TRANSIENT_CONNECTION_CODE);
    // getSQLState()
    assert_eq!(exception.sql_state(), Some(SQL_TRANSIENT_CONNECTION_STATE));
    // getNextException()
    assert!(exception.next_exception().is_none());
    assert!(exception.next_exceptions().is_empty());

    // setNextException(java.sql.SQLException)
    let next = SqlException::new(
        SQL_TRANSIENT_CONNECTION_CODE + 1,
        Some(SQL_TRANSIENT_CONNECTION_STATE.to_owned()),
        Some("chained connection failure".to_owned()),
    )
    .with_class_name(SQL_TRANSIENT_CONNECTION_CLASS);
    exception.set_next_exception(next.clone());

    assert_eq!(exception.next_exception(), Some(&next));
    assert_eq!(exception.next_exceptions().len(), 1);
    assert_eq!(
        exception.next_exception().map(SqlException::error_code),
        Some(SQL_TRANSIENT_CONNECTION_CODE + 1)
    );
    assert_eq!(
        exception.next_exception().map(SqlException::class_name),
        Some(SQL_TRANSIENT_CONNECTION_CLASS)
    );
}

// ---------------------------------------------------------------------------
// java.sql.SQLTransientException
// ---------------------------------------------------------------------------

/// Java `SQLTransientException()`：reason、SQLState、vendor code 均为空。
#[test]
fn sql_transient_exception_init_empty() {
    let exception = SqlException::new(0, None, None).with_class_name(SQL_TRANSIENT_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_TRANSIENT_CLASS,
        SqlExceptionKind::Transient,
        0,
        None,
        None,
        &[],
    );
}

/// Java `SQLTransientException(String reason)`：只带 reason。
#[test]
fn sql_transient_exception_init_reason() {
    let exception = SqlException::new(0, None, Some(SQL_TRANSIENT_REASON.to_owned()))
        .with_class_name(SQL_TRANSIENT_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_TRANSIENT_CLASS,
        SqlExceptionKind::Transient,
        0,
        None,
        Some(SQL_TRANSIENT_REASON),
        &[],
    );
}

/// Java `SQLTransientException(String reason, String sqlState)`。
#[test]
fn sql_transient_exception_init_reason_state() {
    let exception = SqlException::new(
        0,
        Some(SQL_TRANSIENT_STATE.to_owned()),
        Some(SQL_TRANSIENT_REASON.to_owned()),
    )
    .with_class_name(SQL_TRANSIENT_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_TRANSIENT_CLASS,
        SqlExceptionKind::Transient,
        0,
        Some(SQL_TRANSIENT_STATE),
        Some(SQL_TRANSIENT_REASON),
        &[],
    );
}

/// Java `SQLTransientException(String reason, String sqlState, int vendorCode)`。
#[test]
fn sql_transient_exception_init_reason_state_code() {
    let exception = SqlException::new(
        SQL_TRANSIENT_CODE,
        Some(SQL_TRANSIENT_STATE.to_owned()),
        Some(SQL_TRANSIENT_REASON.to_owned()),
    )
    .with_class_name(SQL_TRANSIENT_CLASS);

    assert_constructor_mapping(
        &exception,
        SQL_TRANSIENT_CLASS,
        SqlExceptionKind::Transient,
        SQL_TRANSIENT_CODE,
        Some(SQL_TRANSIENT_STATE),
        Some(SQL_TRANSIENT_REASON),
        &[],
    );
}

/// Java `SQLTransientException(String reason, String sqlState, int vendorCode, Throwable cause)`。
#[test]
fn sql_transient_exception_init_reason_state_code_cause() {
    let exception = SqlException::new(
        SQL_TRANSIENT_CODE,
        Some(SQL_TRANSIENT_STATE.to_owned()),
        Some(SQL_TRANSIENT_REASON.to_owned()),
    )
    .with_class_name(SQL_TRANSIENT_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_TRANSIENT_CLASS,
        SqlExceptionKind::Transient,
        SQL_TRANSIENT_CODE,
        Some(SQL_TRANSIENT_STATE),
        Some(SQL_TRANSIENT_REASON),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// Java `SQLTransientException(String reason, String sqlState, Throwable cause)`。
#[test]
fn sql_transient_exception_init_reason_state_cause() {
    let exception = SqlException::new(
        0,
        Some(SQL_TRANSIENT_STATE.to_owned()),
        Some(SQL_TRANSIENT_REASON.to_owned()),
    )
    .with_class_name(SQL_TRANSIENT_CLASS)
    .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_TRANSIENT_CLASS,
        SqlExceptionKind::Transient,
        0,
        Some(SQL_TRANSIENT_STATE),
        Some(SQL_TRANSIENT_REASON),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// Java `SQLTransientException(String reason, Throwable cause)`。
#[test]
fn sql_transient_exception_init_reason_cause() {
    let exception = SqlException::new(0, None, Some(SQL_TRANSIENT_REASON.to_owned()))
        .with_class_name(SQL_TRANSIENT_CLASS)
        .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_TRANSIENT_CLASS,
        SqlExceptionKind::Transient,
        0,
        None,
        Some(SQL_TRANSIENT_REASON),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// Java `SQLTransientException(Throwable cause)`：reason 取 `cause.toString()`。
#[test]
fn sql_transient_exception_init_cause() {
    let cause_text = throwable_to_string(CAUSE_CLASS, CAUSE_MESSAGE);
    let exception = SqlException::new(0, None, Some(cause_text.clone()))
        .with_class_name(SQL_TRANSIENT_CLASS)
        .with_cause(java_cause(CAUSE_CLASS));

    assert_constructor_mapping(
        &exception,
        SQL_TRANSIENT_CLASS,
        SqlExceptionKind::Transient,
        0,
        None,
        Some(cause_text.as_str()),
        &[SqlExceptionCause::ClassName(CAUSE_CLASS.to_owned())],
    );
}

/// `SQLTransientException` 继承自 `SQLException` 的可观察 getter。
#[test]
fn sql_transient_exception_inherited_getters() {
    let mut exception = SqlException::new(
        SQL_TRANSIENT_CODE,
        Some(SQL_TRANSIENT_STATE.to_owned()),
        Some(SQL_TRANSIENT_REASON.to_owned()),
    )
    .with_class_name(SQL_TRANSIENT_CLASS);

    // getErrorCode()
    assert_eq!(exception.error_code(), SQL_TRANSIENT_CODE);
    // getSQLState()
    assert_eq!(exception.sql_state(), Some(SQL_TRANSIENT_STATE));
    // getNextException()
    assert!(exception.next_exception().is_none());
    assert!(exception.next_exceptions().is_empty());

    // setNextException(java.sql.SQLException)
    let next = SqlException::new(
        SQL_TRANSIENT_CODE + 1,
        Some(SQL_TRANSIENT_STATE.to_owned()),
        Some("chained transient failure".to_owned()),
    )
    .with_class_name(SQL_TRANSIENT_CLASS);
    exception.set_next_exception(next.clone());

    assert_eq!(exception.next_exception(), Some(&next));
    assert_eq!(exception.next_exceptions().len(), 1);
    assert_eq!(
        exception.next_exception().map(SqlException::error_code),
        Some(SQL_TRANSIENT_CODE + 1)
    );
    assert_eq!(
        exception.next_exception().map(SqlException::class_name),
        Some(SQL_TRANSIENT_CLASS)
    );
}
