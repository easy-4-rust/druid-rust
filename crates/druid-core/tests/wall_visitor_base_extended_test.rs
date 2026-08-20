extern crate druid_core as druid;
use druid_core::sql::{
    DbType, WallConfig, WallProvider, WallUpdateCheckItem, WallViolation, WallVisitorBase,
};
use sqlparser::ast::{Expr, Value};
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;

#[test]
fn check_common_deny_table_in_select() {
    let config = WallConfig::builder().deny_table("secret_table").build();
    let provider = WallProvider::new(config);
    let mut visitor = WallVisitorBase::new(&provider);
    let stmts = Parser::parse_sql(&GenericDialect, "SELECT * FROM secret_table").unwrap();
    visitor.check_common(&stmts);
    assert!(visitor
        .violations()
        .iter()
        .any(|v| matches!(v, WallViolation::DeniedTable(ref name) if name == "secret_table")));
}

#[test]
fn check_common_deny_table_case_insensitive() {
    let config = WallConfig::builder().deny_table("SECRET").build();
    let provider = WallProvider::new(config);
    let mut visitor = WallVisitorBase::new(&provider);
    let stmts = Parser::parse_sql(&GenericDialect, "SELECT * FROM secret").unwrap();
    visitor.check_common(&stmts);
    assert!(visitor
        .violations()
        .iter()
        .any(|v| matches!(v, WallViolation::DeniedTable(_))));
}

#[test]
fn check_common_read_only_table_insert() {
    let config = WallConfig::builder().read_only_table("audit_log").build();
    let provider = WallProvider::new(config);
    let mut visitor = WallVisitorBase::new(&provider);
    let stmts = Parser::parse_sql(&GenericDialect, "INSERT INTO audit_log VALUES (1)").unwrap();
    visitor.check_common(&stmts);
    assert!(visitor
        .violations()
        .iter()
        .any(|v| matches!(v, WallViolation::ReadOnlyTable(ref name) if name == "audit_log")));
}

#[test]
fn check_common_read_only_table_update() {
    let config = WallConfig::builder().read_only_table("config").build();
    let provider = WallProvider::new(config);
    let mut visitor = WallVisitorBase::new(&provider);
    let stmts = Parser::parse_sql(&GenericDialect, "UPDATE config SET val=1").unwrap();
    visitor.check_common(&stmts);
    assert!(visitor
        .violations()
        .iter()
        .any(|v| matches!(v, WallViolation::ReadOnlyTable(_))));
}

#[test]
fn check_common_read_only_table_delete() {
    let config = WallConfig::builder().read_only_table("archive").build();
    let provider = WallProvider::new(config);
    let mut visitor = WallVisitorBase::new(&provider);
    let stmts = Parser::parse_sql(&GenericDialect, "DELETE FROM archive").unwrap();
    visitor.check_common(&stmts);
    assert!(visitor
        .violations()
        .iter()
        .any(|v| matches!(v, WallViolation::ReadOnlyTable(_))));
}

#[test]
fn check_common_no_violation_for_non_mutation() {
    let config = WallConfig::builder().read_only_table("t").build();
    let provider = WallProvider::new(config);
    let mut visitor = WallVisitorBase::new(&provider);
    let stmts = Parser::parse_sql(&GenericDialect, "SELECT * FROM t").unwrap();
    visitor.check_common(&stmts);
    assert!(visitor.violations().is_empty());
}

#[test]
fn check_deny_variants_with_matching_variable() {
    let config = WallConfig::builder().deny_variant("evil_var").build();
    let provider = WallProvider::new(config);
    let mut visitor = WallVisitorBase::new(&provider);
    // sqlparser 的 GenericDialect 将 `evil_var` 解析为 Identifier。
    let stmts = Parser::parse_sql(&GenericDialect, "SELECT evil_var FROM t").unwrap();
    visitor.check_deny_variants(&stmts);
    assert!(visitor
        .violations()
        .iter()
        .any(|v| matches!(v, WallViolation::DeniedVariant(_))));
}

#[test]
fn check_deny_variants_with_system_variable() {
    let config = WallConfig::builder().deny_variant("version").build();
    let provider = WallProvider::new(config);
    let mut visitor = WallVisitorBase::new(&provider);
    let stmts = Parser::parse_sql(&GenericDialect, "SELECT @@version").unwrap();
    visitor.check_deny_variants(&stmts);
    assert!(visitor
        .violations()
        .iter()
        .any(|v| matches!(v, WallViolation::DeniedVariant(_))));
}

#[test]
fn check_deny_variants_variant_check_disabled() {
    let mut config = WallConfig::default();
    config.variant_check = false;
    let provider = WallProvider::new(config);
    let mut visitor = WallVisitorBase::new(&provider);
    let stmts = Parser::parse_sql(&GenericDialect, "SELECT @x").unwrap();
    visitor.check_deny_variants(&stmts);
    assert!(visitor.violations().is_empty());
}

#[test]
fn check_virtual_tables_with_v_prefix() {
    let provider = WallProvider::new(WallConfig::default());
    let mut visitor = WallVisitorBase::new(&provider);
    let stmts = Parser::parse_sql(&GenericDialect, "SELECT * FROM v$instance").unwrap();
    visitor.check_virtual_tables(&stmts);
    assert!(visitor
        .violations()
        .iter()
        .any(|v| matches!(v, WallViolation::DeniedTable(ref name) if name.contains("v$instance"))));
}

#[test]
fn check_virtual_tables_with_v_underscore_prefix() {
    let provider = WallProvider::new(WallConfig::default());
    let mut visitor = WallVisitorBase::new(&provider);
    let stmts = Parser::parse_sql(&GenericDialect, "SELECT * FROM v_$session").unwrap();
    visitor.check_virtual_tables(&stmts);
    assert!(visitor
        .violations()
        .iter()
        .any(|v| matches!(v, WallViolation::DeniedTable(_))));
}

#[test]
fn check_virtual_tables_normal_table_no_violation() {
    let provider = WallProvider::new(WallConfig::default());
    let mut visitor = WallVisitorBase::new(&provider);
    let stmts = Parser::parse_sql(&GenericDialect, "SELECT * FROM users").unwrap();
    visitor.check_virtual_tables(&stmts);
    assert!(visitor.violations().is_empty());
}

#[test]
fn check_virtual_tables_disabled() {
    let mut config = WallConfig::default();
    config.table_check = false;
    let provider = WallProvider::new(config);
    let mut visitor = WallVisitorBase::new(&provider);
    let stmts = Parser::parse_sql(&GenericDialect, "SELECT * FROM v$instance").unwrap();
    visitor.check_virtual_tables(&stmts);
    assert!(visitor.violations().is_empty());
}

#[test]
fn add_wall_update_check_item() {
    let provider = WallProvider::new(WallConfig::default());
    let mut visitor = WallVisitorBase::new(&provider);
    assert!(visitor.update_check_items().is_none());
    visitor.add_wall_update_check_item(WallUpdateCheckItem::new(
        "t",
        "id",
        Expr::Value(Value::Number("1".to_owned(), false)),
        vec![],
    ));
    assert!(visitor.update_check_items().is_some());
    assert_eq!(visitor.update_check_items().unwrap().len(), 1);
}

#[test]
fn check_common_deny_table_in_join() {
    let config = WallConfig::builder().deny_table("forbidden").build();
    let provider = WallProvider::new(config);
    let mut visitor = WallVisitorBase::new(&provider);
    let stmts = Parser::parse_sql(
        &GenericDialect,
        "SELECT a.id FROM allowed a JOIN forbidden f ON a.id = f.id",
    )
    .unwrap();
    visitor.check_common(&stmts);
    assert!(visitor
        .violations()
        .iter()
        .any(|v| matches!(v, WallViolation::DeniedTable(ref name) if name == "forbidden")));
}

#[test]
fn check_common_multiple_statements() {
    let config = WallConfig::builder().deny_table("blocked").build();
    let provider = WallProvider::new(config);
    let mut visitor = WallVisitorBase::new(&provider);
    let stmts =
        Parser::parse_sql(&GenericDialect, "SELECT 1; INSERT INTO blocked VALUES (1)").unwrap();
    visitor.check_common(&stmts);
    assert!(visitor
        .violations()
        .iter()
        .any(|v| matches!(v, WallViolation::DeniedTable(_))));
}
