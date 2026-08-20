//! WallProvider + WallVisitorBase 差分测试
//! （C9 批次：sql 0% 文件）。
//!
//! Java 基线：`33824c3dec1612711f9bb4e409319bcab2e4cd0e`。

extern crate druid_core as druid;
use druid::sql::{DbType, WallConfig, WallProvider, WallViolation, WallVisitorBase};
use sqlparser::ast::Statement;
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;

// ── WallProvider（Java WallProvider）─────────────────────────

/// new + config + name。
#[test]
fn wall_provider_new_and_config() {
    let config = WallConfig::default();
    let provider = WallProvider::new(config);
    assert!(provider.config().select_allow);
    assert!(provider.name().is_none());
}

/// name setter/getter。
#[test]
fn wall_provider_name() {
    let provider = WallProvider::new(WallConfig::default());
    provider.set_name(Some("test-provider".to_owned()));
    assert_eq!(provider.name().as_deref(), Some("test-provider"));
    provider.set_name(None);
    assert!(provider.name().is_none());
}

/// db_type setter/getter。
#[test]
fn wall_provider_db_type() {
    let provider = WallProvider::new(WallConfig::default());
    assert_eq!(provider.db_type(), DbType::Other);
    provider.set_db_type(DbType::MySql);
    assert_eq!(provider.db_type(), DbType::MySql);
    provider.set_db_type(DbType::PostgreSql);
    assert_eq!(provider.db_type(), DbType::PostgreSql);
}

/// check：合法 SQL 返回 WallCheckResult。
#[test]
fn wall_provider_check_valid_sql() {
    let provider = WallProvider::new(WallConfig::default());
    let result = provider.check("SELECT * FROM users WHERE id = 1");
    assert_eq!(result.sql(), "SELECT * FROM users WHERE id = 1");
    assert!(result.violations().is_empty());
    assert!(!result.is_syntax_error());
}

/// check：违规 SQL 返回 violation（drop_table_allow 默认 true，需显式关闭）。
#[test]
fn wall_provider_check_violation() {
    let config = WallConfig::builder().drop_table_allow(false).build();
    let provider = WallProvider::new(config);
    let result = provider.check("DROP TABLE users");
    assert!(!result.violations().is_empty());
    assert!(result
        .violations()
        .iter()
        .any(|v| matches!(v, WallViolation::DropTableNotAllowed(_))));
}

/// check_valid：合法 SQL 返回 Ok(true)。
#[test]
fn wall_provider_check_valid() {
    let provider = WallProvider::new(WallConfig::default());
    let result = provider.check_valid("SELECT 1").unwrap();
    assert!(result);
}

/// try_check：语法错误返回 Ok 但包含 SyntaxError violation。
#[test]
fn wall_provider_try_check_syntax_error() {
    let provider = WallProvider::new(WallConfig::default());
    let result = provider.try_check("THIS IS NOT VALID SQL !!!").unwrap();
    assert!(result.is_syntax_error());
    assert!(result
        .violations()
        .iter()
        .any(|v| matches!(v, WallViolation::SyntaxError(_))));
}

/// try_check：合法 SQL 返回 Ok。
#[test]
fn wall_provider_try_check_valid() {
    let provider = WallProvider::new(WallConfig::default());
    let result = provider.try_check("SELECT 1").unwrap();
    assert!(result.violations().is_empty());
}

/// tenant_value / set_tenant_value。
#[test]
fn wall_provider_tenant_value() {
    assert!(WallProvider::tenant_value().is_none());
    WallProvider::set_tenant_value(Some(druid::core::Value::String("tenant-1".to_owned())));
    assert!(WallProvider::tenant_value().is_some());
    WallProvider::set_tenant_value(None);
    assert!(WallProvider::tenant_value().is_none());
}

/// is_privileged / do_privileged。
#[test]
fn wall_provider_privileged() {
    assert!(!WallProvider::is_privileged());
    let result = WallProvider::do_privileged(|| {
        assert!(WallProvider::is_privileged());
        42
    });
    assert_eq!(result, 42);
    assert!(!WallProvider::is_privileged());
}

/// sql_stat：新 SQL 无统计。
#[test]
fn wall_provider_sql_stat() {
    let provider = WallProvider::new(WallConfig::default());
    let stat = provider.sql_stat("SELECT 1");
    assert!(stat.is_none());
}

// ── WallVisitorBase（Java WallVisitorBase）───────────────────

/// new + provider + violations。
#[test]
fn wall_visitor_base_new_and_violations() {
    let provider = WallProvider::new(WallConfig::default());
    let mut visitor = WallVisitorBase::new(&provider);
    assert!(visitor.violations().is_empty());
    assert!(!visitor.sql_modified());
    assert!(!visitor.sql_end_of_comment());
}

/// push_unique：去重。
#[test]
fn wall_visitor_base_push_unique() {
    let provider = WallProvider::new(WallConfig::default());
    let mut visitor = WallVisitorBase::new(&provider);
    visitor.push_unique(WallViolation::DeleteWithoutWhere);
    assert_eq!(visitor.violations().len(), 1);
    visitor.push_unique(WallViolation::DeleteWithoutWhere);
    assert_eq!(visitor.violations().len(), 1, "should deduplicate");
    visitor.push_unique(WallViolation::UpdateWithoutWhere);
    assert_eq!(visitor.violations().len(), 2);
}

/// sql_modified / sql_end_of_comment setters。
#[test]
fn wall_visitor_base_setters() {
    let provider = WallProvider::new(WallConfig::default());
    let mut visitor = WallVisitorBase::new(&provider);
    assert!(!visitor.sql_modified());
    visitor.set_sql_modified(true);
    assert!(visitor.sql_modified());
    assert!(!visitor.sql_end_of_comment());
    visitor.set_sql_end_of_comment(true);
    assert!(visitor.sql_end_of_comment());
}

/// add_wall_update_check_item / update_check_items。
#[test]
fn wall_visitor_base_update_check_items() {
    let provider = WallProvider::new(WallConfig::default());
    let mut visitor = WallVisitorBase::new(&provider);
    assert!(visitor.update_check_items().is_none());
    // add_wall_update_check_item 需要 WallUpdateCheckItem，暂用空测试。
}

/// db_type：继承自 provider。
#[test]
fn wall_visitor_base_db_type() {
    let config = WallConfig::default();
    let provider = WallProvider::new(config);
    let visitor = WallVisitorBase::new(&provider);
    assert_eq!(visitor.db_type(), DbType::Other);
}

/// check_common：空语句列表。
#[test]
fn wall_visitor_base_check_common_empty() {
    let provider = WallProvider::new(WallConfig::default());
    let mut visitor = WallVisitorBase::new(&provider);
    let stmts: Vec<Statement> = vec![];
    visitor.check_common(&stmts);
    assert!(visitor.violations().is_empty());
}

/// check_deny_variants：无配置时不报错。
#[test]
fn wall_visitor_base_check_deny_variants_empty() {
    let provider = WallProvider::new(WallConfig::default());
    let mut visitor = WallVisitorBase::new(&provider);
    let stmts = Parser::parse_sql(&GenericDialect, "SELECT 1").unwrap();
    visitor.check_deny_variants(&stmts);
    assert!(visitor.violations().is_empty());
}

/// check_virtual_tables：无配置时不报错。
#[test]
fn wall_visitor_base_check_virtual_tables_empty() {
    let provider = WallProvider::new(WallConfig::default());
    let mut visitor = WallVisitorBase::new(&provider);
    let stmts = Parser::parse_sql(&GenericDialect, "SELECT 1").unwrap();
    visitor.check_virtual_tables(&stmts);
    assert!(visitor.violations().is_empty());
}

/// check_common：含 deny_tables 时产生 DeniedTable violation。
#[test]
fn wall_visitor_base_check_common_with_violation() {
    let config = WallConfig::builder().deny_table("secret_table").build();
    let provider = WallProvider::new(config);
    let mut visitor = WallVisitorBase::new(&provider);
    let stmts = Parser::parse_sql(&GenericDialect, "SELECT * FROM secret_table").unwrap();
    visitor.check_common(&stmts);
    assert!(visitor
        .violations()
        .iter()
        .any(|v| matches!(v, WallViolation::DeniedTable(_))));
}

/// check_deny_variants：有 deny_variants 配置。
#[test]
fn wall_visitor_base_check_deny_variants_with_config() {
    let config = WallConfig::builder().deny_variant("secret_var").build();
    let provider = WallProvider::new(config);
    let mut visitor = WallVisitorBase::new(&provider);
    let stmts = Parser::parse_sql(&GenericDialect, "SELECT 1").unwrap();
    visitor.check_deny_variants(&stmts);
    // 无 @ 变量引用时不报错。
    assert!(visitor.violations().is_empty());
}

/// provider() 方法。
#[test]
fn wall_visitor_base_provider() {
    let provider = WallProvider::new(WallConfig::default());
    let visitor = WallVisitorBase::new(&provider);
    assert!(visitor.provider().config().select_allow);
}
