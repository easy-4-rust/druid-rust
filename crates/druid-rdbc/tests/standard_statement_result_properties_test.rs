//! 独立结果集属性的默认兼容、精确不支持与异常保真。
use druid_rdbc::error::{RdbcError, SqlException};
use druid_rdbc::statement::{
    ResultSetConcurrency, ResultSetHoldability, ResultSetType, Statement, StatementOptions,
};

/// 对应 Java: java.sql.Statement；模拟只实现旧 options 的外部驱动。
struct OptionsOnly(Result<StatementOptions, RdbcError>);
druid_rdbc::impl_standard_wrapper!(OptionsOnly => Statement);
#[async_trait::async_trait]
impl Statement for OptionsOnly {
    fn options(&self) -> Result<StatementOptions, RdbcError> {
        self.0.clone()
    }
}

#[test]
fn independent_defaults_preserve_all_three_non_default_option_values() {
    let statement = OptionsOnly(Ok(StatementOptions::new(
        ResultSetType::ScrollSensitive,
        ResultSetConcurrency::Updatable,
        ResultSetHoldability::HoldOverCommit,
    )));
    assert_eq!(
        statement.result_set_type().unwrap(),
        ResultSetType::ScrollSensitive
    );
    assert_eq!(
        statement.result_set_concurrency().unwrap(),
        ResultSetConcurrency::Updatable
    );
    assert_eq!(
        statement.result_set_holdability().unwrap(),
        ResultSetHoldability::HoldOverCommit
    );
}

#[test]
fn defaults_normalize_only_the_exact_options_unsupported_operation() {
    let statement = OptionsOnly(Err(RdbcError::UnsupportedOperation {
        operation: "statement_options",
    }));
    assert!(matches!(
        statement.result_set_type(),
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_result_set_type"
        })
    ));
    assert!(matches!(
        statement.result_set_concurrency(),
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_result_set_concurrency"
        })
    ));
    assert!(matches!(
        statement.result_set_holdability(),
        Err(RdbcError::UnsupportedOperation {
            operation: "statement_result_set_holdability"
        })
    ));
    let unrelated = RdbcError::UnsupportedOperation {
        operation: "unrelated_driver_operation",
    };
    let statement = OptionsOnly(Err(unrelated.clone()));
    assert_eq!(statement.result_set_type().unwrap_err(), unrelated);
    assert_eq!(statement.result_set_concurrency().unwrap_err(), unrelated);
    assert_eq!(statement.result_set_holdability().unwrap_err(), unrelated);
}

#[test]
fn defaults_preserve_sql_error_code_state_and_message() {
    let error = RdbcError::SqlException(Box::new(SqlException::new(
        19,
        Some("HY000".to_owned()),
        Some("driver details".to_owned()),
    )));
    let statement = OptionsOnly(Err(error.clone()));
    assert_eq!(statement.result_set_type().unwrap_err(), error);
    assert_eq!(statement.result_set_concurrency().unwrap_err(), error);
    assert_eq!(statement.result_set_holdability().unwrap_err(), error);
}
