extern crate druid_core as druid;
use druid_core::core::{OracleValidConnectionChecker, ValidConnectionChecker};
use druid_core::sql::{
    CkWallProvider, DbType, WallConfig, WallProvider, WallViolation, WallVisitor,
};
use sqlparser::ast::Statement;
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;
use std::time::Duration;

// ── CkWallProvider ─────────────────────────────────────────────

#[test]
fn ck_wall_provider_new() {
    let p = CkWallProvider::new();
    assert_eq!(p.db_type(), DbType::ClickHouse);
}

#[test]
fn ck_wall_provider_default() {
    let p = CkWallProvider::default();
    assert_eq!(p.db_type(), DbType::ClickHouse);
}

#[test]
fn ck_wall_provider_with_config() {
    let config = WallConfig::builder().select_allow(false).build();
    let p = CkWallProvider::with_config(config);
    assert!(!p.config().select_allow);
}

#[test]
fn ck_wall_provider_into_inner() {
    let p = CkWallProvider::new();
    let inner = p.into_inner();
    assert_eq!(inner.db_type(), DbType::ClickHouse);
}

#[test]
fn ck_wall_provider_check_valid() {
    let p = CkWallProvider::new();
    assert!(p.check_valid("SELECT 1").unwrap());
}

// ── ClickhouseWallVisitor ──────────────────────────────────────

#[test]
fn clickhouse_wall_visitor_new() {
    let provider = WallProvider::new(WallConfig::default());
    provider.set_db_type(DbType::ClickHouse);
    let visitor = druid_core::sql::ClickhouseWallVisitor::new(&provider);
    assert_eq!(visitor.db_type(), DbType::ClickHouse);
}

#[test]
fn clickhouse_wall_visitor_check_valid() {
    let provider = WallProvider::new(WallConfig::default());
    provider.set_db_type(DbType::ClickHouse);
    let mut visitor = druid_core::sql::ClickhouseWallVisitor::new(&provider);
    let stmts = Parser::parse_sql(&GenericDialect, "SELECT 1").unwrap();
    visitor.check("SELECT 1", &stmts);
    assert!(visitor.violations().is_empty());
}

#[test]
fn clickhouse_wall_visitor_check_deny_table() {
    let config = WallConfig::builder().deny_table("blocked").build();
    let provider = WallProvider::new(config);
    provider.set_db_type(DbType::ClickHouse);
    let mut visitor = druid_core::sql::ClickhouseWallVisitor::new(&provider);
    let stmts = Parser::parse_sql(&GenericDialect, "SELECT * FROM blocked").unwrap();
    visitor.check("SELECT * FROM blocked", &stmts);
    assert!(visitor
        .violations()
        .iter()
        .any(|v| matches!(v, WallViolation::DeniedTable(_))));
}

#[test]
fn clickhouse_wall_visitor_setters() {
    let provider = WallProvider::new(WallConfig::default());
    provider.set_db_type(DbType::ClickHouse);
    let mut visitor = druid_core::sql::ClickhouseWallVisitor::new(&provider);
    assert!(!visitor.sql_modified());
    visitor.set_sql_modified(true);
    assert!(visitor.sql_modified());
    assert!(!visitor.sql_end_of_comment());
    visitor.set_sql_end_of_comment(true);
    assert!(visitor.sql_end_of_comment());
}

// ── SQLiteWallVisitor ──────────────────────────────────────────

#[test]
fn sqlite_wall_visitor_new() {
    let provider = WallProvider::new(WallConfig::default());
    let visitor = druid_core::sql::SQLiteWallVisitor::new(&provider);
    assert_eq!(visitor.db_type(), DbType::PostgreSql);
}

#[test]
fn sqlite_wall_visitor_check_valid() {
    let provider = WallProvider::new(WallConfig::default());
    let mut visitor = druid_core::sql::SQLiteWallVisitor::new(&provider);
    let stmts = Parser::parse_sql(&GenericDialect, "SELECT 1").unwrap();
    visitor.check("SELECT 1", &stmts);
    assert!(visitor.violations().is_empty());
}

#[test]
fn sqlite_wall_visitor_check_deny_table() {
    let config = WallConfig::builder().deny_table("forbidden").build();
    let provider = WallProvider::new(config);
    let mut visitor = druid_core::sql::SQLiteWallVisitor::new(&provider);
    let stmts = Parser::parse_sql(&GenericDialect, "SELECT * FROM forbidden").unwrap();
    visitor.check("SELECT * FROM forbidden", &stmts);
    assert!(visitor
        .violations()
        .iter()
        .any(|v| matches!(v, WallViolation::DeniedTable(_))));
}

#[test]
fn sqlite_wall_visitor_setters() {
    let provider = WallProvider::new(WallConfig::default());
    let mut visitor = druid_core::sql::SQLiteWallVisitor::new(&provider);
    visitor.set_sql_modified(true);
    assert!(visitor.sql_modified());
    visitor.set_sql_end_of_comment(true);
    assert!(visitor.sql_end_of_comment());
}

// ── OracleValidConnectionChecker ───────────────────────────────

#[test]
fn oracle_checker_new() {
    let c = OracleValidConnectionChecker::new();
    assert_eq!(
        OracleValidConnectionChecker::DEFAULT_VALIDATE_QUERY,
        "SELECT 'x' FROM DUAL"
    );
}

#[test]
fn oracle_checker_default() {
    let c = OracleValidConnectionChecker::default();
    let _ = c;
}

#[test]
fn oracle_checker_set_timeout() {
    let mut c = OracleValidConnectionChecker::new();
    c.set_timeout(30);
}

#[test]
fn oracle_checker_config_from_properties() {
    let mut c = OracleValidConnectionChecker::new();
    let mut props = std::collections::HashMap::new();
    props.insert("druid.oracle.pingTimeout".to_owned(), "5".to_owned());
    c.config_from_properties(&props);
}

#[test]
fn oracle_checker_config_from_properties_empty() {
    let mut c = OracleValidConnectionChecker::new();
    let props = std::collections::HashMap::new();
    c.config_from_properties(&props);
}

#[test]
fn oracle_checker_clone_copy_debug() {
    let c = OracleValidConnectionChecker::new();
    let c2 = c;
    let _ = format!("{:?}", c2);
}
