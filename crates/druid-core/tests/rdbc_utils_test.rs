extern crate druid_core as druid;
use druid::sql::{DbType, RdbcUtils};

// ── infer_db_type ──────────────────────────────────────────────

#[test]
fn infer_db_type_rdbc_mysql_url() {
    assert_eq!(
        RdbcUtils::infer_db_type(Some("rdbc:mysql://host/db"), None),
        Some(DbType::MySql)
    );
}

#[test]
fn infer_db_type_rdbc_postgresql_url() {
    assert_eq!(
        RdbcUtils::infer_db_type(Some("rdbc:postgresql://host/db"), None),
        Some(DbType::PostgreSql)
    );
}

#[test]
fn infer_db_type_rust_sqlite_url() {
    assert_eq!(
        RdbcUtils::infer_db_type(Some("sqlite::memory:"), None),
        Some(DbType::SQLite)
    );
}

#[test]
fn infer_db_type_rdbc_oracle_url() {
    assert_eq!(
        RdbcUtils::infer_db_type(Some("rdbc:oracle:thin:@host:1521:xe"), None),
        Some(DbType::Oracle)
    );
}

#[test]
fn infer_db_type_none_url() {
    assert_eq!(RdbcUtils::infer_db_type(None, None), None);
}

#[test]
fn infer_db_type_rust_postgres_url() {
    assert_eq!(
        RdbcUtils::infer_db_type(Some("postgres://host/db"), None),
        Some(DbType::PostgreSql)
    );
}

#[test]
fn infer_db_type_rust_mysql_url() {
    assert_eq!(
        RdbcUtils::infer_db_type(Some("mysql://host/db"), None),
        Some(DbType::MySql)
    );
}

// ── to_rust_url ────────────────────────────────────────────────

#[test]
fn to_rust_url_rdbc_mysql() {
    let result = RdbcUtils::to_rust_url("rdbc:mysql://localhost:3306/db");
    assert!(result.is_some());
}

#[test]
fn to_rust_url_rdbc_postgresql() {
    let result = RdbcUtils::to_rust_url("rdbc:postgresql://localhost:5432/db");
    assert!(result.is_some());
}

#[test]
fn to_rust_url_rdbc_sqlite() {
    let result = RdbcUtils::to_rust_url("rdbc:sqlite::memory:");
    assert!(result.is_some());
}

#[test]
fn to_rust_url_native_mysql() {
    let result = RdbcUtils::to_rust_url("mysql://localhost:3306/db");
    assert!(result.is_some());
}

#[test]
fn to_rust_url_unsupported() {
    let result = RdbcUtils::to_rust_url("rdbc:oracle:thin:@host:1521:xe");
    assert!(result.is_none());
}

// ── is_*_db_type_name ──────────────────────────────────────────

#[test]
fn is_oracle_db_type_name() {
    assert!(RdbcUtils::is_oracle_db_type_name("oracle"));
    assert!(!RdbcUtils::is_oracle_db_type_name("mysql"));
}

#[test]
fn is_mysql_db_type_name() {
    assert!(RdbcUtils::is_mysql_db_type_name("mysql"));
    assert!(!RdbcUtils::is_mysql_db_type_name("oracle"));
}

#[test]
fn is_pgsql_db_type_name() {
    assert!(RdbcUtils::is_pgsql_db_type_name("postgresql"));
    assert!(!RdbcUtils::is_pgsql_db_type_name("mysql"));
}

#[test]
fn is_sqlserver_db_type_name() {
    assert!(RdbcUtils::is_sqlserver_db_type_name("sqlserver"));
    assert!(!RdbcUtils::is_sqlserver_db_type_name("mysql"));
}

// ── is_my_sql_driver ───────────────────────────────────────────

#[test]
fn is_my_sql_driver() {
    assert!(RdbcUtils::is_my_sql_driver("com.mysql.cj.rdbc.Driver"));
    assert!(!RdbcUtils::is_my_sql_driver("org.postgresql.Driver"));
}
