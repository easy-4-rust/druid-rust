//! 旧版公开 JDBC 常量仍可从池化语句类型直接引用。

use druid::core::DruidPooledStatement;

#[test]
fn pooled_statement_retains_public_jdbc_constants() {
    assert_eq!(DruidPooledStatement::CLOSE_CURRENT_RESULT, 1);
    assert_eq!(DruidPooledStatement::KEEP_CURRENT_RESULT, 2);
    assert_eq!(DruidPooledStatement::CLOSE_ALL_RESULTS, 3);
    assert_eq!(DruidPooledStatement::SUCCESS_NO_INFO, -2);
    assert_eq!(DruidPooledStatement::EXECUTE_FAILED, -3);
    assert_eq!(DruidPooledStatement::RETURN_GENERATED_KEYS, 1);
    assert_eq!(DruidPooledStatement::NO_GENERATED_KEYS, 2);
}
