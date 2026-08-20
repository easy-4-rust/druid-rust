//! `WallCheckResult` + `RdbcParameterImpl` + `WallProviderStatValue` 差分测试
//! （C9 批次：sql + core 0% 文件）。
//!
//! Java 基线：`33824c3dec1612711f9bb4e409319bcab2e4cd0e`。

extern crate druid_core as druid;
use druid_core::core::RdbcParameterImpl;
use druid_core::sql::{WallCheckResult, WallProviderStatValue, WallSqlStat, WallViolation};
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;
use std::sync::Arc;

// ── WallCheckResult（Java WallProvider.checkInternal 返回类型）────

/// new：完整构造。
#[test]
fn wall_check_result_new() {
    let stmts = Parser::parse_sql(&GenericDialect, "SELECT 1").unwrap();
    let violations = vec![WallViolation::DeleteWithoutWhere];
    let sql_stat = Arc::new(WallSqlStat::new("SELECT 1".to_owned(), vec![], false));

    let result = WallCheckResult::new(
        "SELECT 1".to_owned(),
        stmts.clone(),
        violations.clone(),
        false,
        sql_stat,
    );

    assert_eq!(result.sql(), "SELECT 1");
    assert_eq!(result.statements().len(), 1);
    assert_eq!(result.violations().len(), 1);
    assert!(!result.is_syntax_error());
    assert!(result.sql_stat().is_some());
    assert!(result.update_check_items().is_none());
}

/// privileged：快速通行，不产生 AST/违规/统计。
#[test]
fn wall_check_result_privileged() {
    let result = WallCheckResult::privileged("SELECT 1".to_owned());
    assert_eq!(result.sql(), "SELECT 1");
    assert!(result.statements().is_empty());
    assert!(result.violations().is_empty());
    assert!(!result.is_syntax_error());
    assert!(result.sql_stat().is_none());
    assert!(result.update_check_items().is_none());
}

/// `syntax_error` 标记。
#[test]
fn wall_check_result_syntax_error() {
    let sql_stat = Arc::new(WallSqlStat::new(
        "INVALID".to_owned(),
        vec![WallViolation::SyntaxError("parse error".to_owned())],
        true,
    ));
    let result = WallCheckResult::new(
        "INVALID".to_owned(),
        vec![],
        vec![WallViolation::SyntaxError("parse error".to_owned())],
        true,
        sql_stat,
    );
    assert!(result.is_syntax_error());
}

/// `set_update_check_items`。
#[test]
fn wall_check_result_update_check_items() {
    let sql_stat = Arc::new(WallSqlStat::new("SELECT 1".to_owned(), vec![], false));
    let mut result = WallCheckResult::new("SELECT 1".to_owned(), vec![], vec![], false, sql_stat);
    assert!(result.update_check_items().is_none());
    result.set_update_check_items(Some(vec![]));
    assert!(result.update_check_items().is_some());
    assert!(result.update_check_items().unwrap().is_empty());
}

// ── WallProviderStatValue（Java WallProvider 管理快照）─────────

/// `to_map：字段映射`。
#[test]
fn wall_provider_stat_value_to_map() {
    let value = WallProviderStatValue {
        name: Some("test".to_owned()),
        check_count: 100,
        hard_check_count: 50,
        violation_count: 5,
        white_list_hit_count: 10,
        black_list_hit_count: 3,
        syntax_error_count: 2,
        violation_effect_row_count: 1,
        tables: vec![],
        functions: vec![],
        white_list: vec![],
        black_list: vec![],
    };
    let map = value.to_map();
    assert!(map.contains_key("checkCount"));
    assert!(map.contains_key("hardCheckCount"));
    assert!(map.contains_key("violationCount"));
    assert!(map.contains_key("whiteListHitCount"));
    assert!(map.contains_key("blackListHitCount"));
    assert!(map.contains_key("syntaxErrorCount"));
    assert!(map.contains_key("violationEffectRowCount"));
}

// ── RdbcParameterImpl（Java PreparedStatement 参数实现）────────

/// new：完整构造。
#[test]
fn rdbc_parameter_impl_new() {
    let param = RdbcParameterImpl::new(
        4, // VARCHAR
        None, 0, None, 0,
    );
    // 验证构造成功（pub 字段不直接暴露）。
    // 通过 Debug 验证。
    let debug = format!("{param:?}");
    assert!(debug.contains("sql_type"));
}

/// `with_value：简化构造`。
#[test]
fn rdbc_parameter_impl_with_value() {
    let param = RdbcParameterImpl::with_value(4, None);
    let debug = format!("{param:?}");
    assert!(debug.contains("sql_type"));
}

/// `with_length：带长度构造`。
#[test]
fn rdbc_parameter_impl_with_length() {
    let param = RdbcParameterImpl::with_length(4, None, 100);
    let debug = format!("{param:?}");
    assert!(debug.contains("sql_type"));
}

/// `with_calendar：带日历构造`。
#[test]
fn rdbc_parameter_impl_with_calendar() {
    let param = RdbcParameterImpl::with_calendar(4, None, None);
    let debug = format!("{param:?}");
    assert!(debug.contains("sql_type"));
}
