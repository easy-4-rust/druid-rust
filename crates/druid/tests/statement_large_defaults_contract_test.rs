//! `java.sql.Statement` 的 JDBC 4.2 large 方法默认入口契约。
//!
//! JDK 8 中 `executeLargeUpdate`、`executeLargeBatch`、`getLargeUpdateCount`、
//! `getLargeMaxRows`、`setLargeMaxRows` 都是 `java.sql.Statement` 的默认方法，
//! 未被实现时抛出 `AbstractMethodError`。Rust 侧必须在 `Statement` trait 上提供
//! 同等的默认入口，使既有实现无须修改即可保持 Java 8 的调用结果。
//!
//! 本测试用一个只实现必需方法的最小 Statement 句柄，对 `dyn Statement` 调用每个
//! 默认方法，断言返回 `RdbcError::Other`，且错误消息同时包含 `AbstractMethodError`
//! 与该方法的 Rust 全名。

use druid_rdbc::error::RdbcError;
use druid_rdbc::statement::{GeneratedKeys, Statement};

/// 只实现必需方法的最小 Statement 句柄：所有 trait 默认方法保持继承。
struct MinimalStatement;

druid_rdbc::impl_standard_wrapper!(MinimalStatement => Statement);

impl Statement for MinimalStatement {}

/// 提取 `RdbcError::Other` 的消息文本；其他变体视为契约失败。
fn other_message(error: &RdbcError) -> &str {
    match error {
        RdbcError::Other(message) => message.as_str(),
        other => panic!("expected RdbcError::Other, got {other:?}"),
    }
}

/// 断言错误消息同时报告 `AbstractMethodError`、Rust 方法全名与 Java 方法名。
fn assert_abstract_method_error(error: &RdbcError, method: &str, java_method: &str) {
    let message = other_message(error);
    assert!(
        message.contains("AbstractMethodError"),
        "{method} must report AbstractMethodError, got: {message}"
    );
    let rust_full_name = format!("druid_rdbc::statement::Statement::{method}");
    assert!(
        message.contains(rust_full_name.as_str()),
        "{method} message must contain the Rust full name {rust_full_name}, got: {message}"
    );
    assert!(
        message.contains(java_method),
        "{method} message must contain the Java method name {java_method}, got: {message}"
    );
}

/// `executeLargeUpdate` 四个重载默认抛出 `AbstractMethodError`。
#[test]
fn execute_large_update_reports_abstract_method_error() {
    let mut statement: Box<dyn Statement> = Box::new(MinimalStatement);
    let error = statement
        .execute_large_update("UPDATE t SET a = 1", GeneratedKeys::None)
        .expect_err("execute_large_update must not succeed by default");
    assert_abstract_method_error(&error, "execute_large_update", "executeLargeUpdate");
}

/// `executeLargeBatch` 默认抛出 `AbstractMethodError`。
#[test]
fn execute_large_batch_reports_abstract_method_error() {
    let mut statement: Box<dyn Statement> = Box::new(MinimalStatement);
    let error = statement
        .execute_large_batch()
        .expect_err("execute_large_batch must not succeed by default");
    assert_abstract_method_error(&error, "execute_large_batch", "executeLargeBatch");
}

/// `getLargeUpdateCount` 默认抛出 `AbstractMethodError`。
#[test]
fn get_large_update_count_reports_abstract_method_error() {
    let statement: Box<dyn Statement> = Box::new(MinimalStatement);
    let error = statement
        .get_large_update_count()
        .expect_err("get_large_update_count must not succeed by default");
    assert_abstract_method_error(&error, "get_large_update_count", "getLargeUpdateCount");
}

/// `getLargeMaxRows` 默认抛出 `AbstractMethodError`。
#[test]
fn get_large_max_rows_reports_abstract_method_error() {
    let statement: Box<dyn Statement> = Box::new(MinimalStatement);
    let error = statement
        .get_large_max_rows()
        .expect_err("get_large_max_rows must not succeed by default");
    assert_abstract_method_error(&error, "get_large_max_rows", "getLargeMaxRows");
}

/// `setLargeMaxRows` 默认抛出 `AbstractMethodError`。
#[test]
fn set_large_max_rows_reports_abstract_method_error() {
    let mut statement: Box<dyn Statement> = Box::new(MinimalStatement);
    let error = statement
        .set_large_max_rows(1024)
        .expect_err("set_large_max_rows must not succeed by default");
    assert_abstract_method_error(&error, "set_large_max_rows", "setLargeMaxRows");
}

/// 五个默认入口都不得改变既有实现的其它默认行为。
#[test]
fn large_defaults_do_not_alter_existing_defaults() {
    let statement: Box<dyn Statement> = Box::new(MinimalStatement);
    let error = statement
        .get_update_count()
        .expect_err("get_update_count must keep its own default error");
    assert!(
        matches!(error, RdbcError::UnsupportedOperation { .. }),
        "get_update_count default must stay UnsupportedOperation, got: {error:?}"
    );
    let error = statement
        .max_rows()
        .expect_err("max_rows must keep its own default error");
    assert!(
        matches!(error, RdbcError::UnsupportedOperation { .. }),
        "max_rows default must stay UnsupportedOperation, got: {error:?}"
    );
}
