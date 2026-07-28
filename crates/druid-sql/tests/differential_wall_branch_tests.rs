//! Differential tests for Wall deny_schema, deny_function, multi-statement branches.

use druid_sql::{Wall, WallConfig, WallViolation};

// ── deny_schema tests ──

#[test]
fn test_wall_deny_schema_in_select() {
    let c = WallConfig::builder().deny_schema("pg_catalog").build();
    let wall = Wall::new(c);
    // SELECT with schema-qualified table
    let result = wall.check("SELECT * FROM pg_catalog.pg_user");
    // Should pass since deny_schema is not yet wired in Wall (architecture doc says TBD)
    // For now just verify no panic
    let _ = result;
}

// ── deny_function tests ──

#[test]
fn test_wall_deny_function_not_wired() {
    // deny_functions is stored in WallConfig but not yet checked in Wall::check
    // This is a known gap documented in architecture doc §15
    let c = WallConfig::builder().deny_function("sleep").build();
    let wall = Wall::new(c);
    // Verify no panic even with deny_function set
    let _ = wall.check("SELECT sleep(1)");
}

// ── multi-statement branches ──

#[test]
fn test_wall_multi_statement_single() {
    let wall = Wall::new(WallConfig::default());
    assert!(wall.check("SELECT 1").is_ok());
}

#[test]
fn test_wall_multi_statement_two_valid() {
    let wall = Wall::new(WallConfig::default());
    // Two valid statements separated by semicolon
    assert!(wall.check("SELECT 1; SELECT 2").is_ok());
}

#[test]
fn test_wall_multi_statement_one_drop() {
    let wall = Wall::new(WallConfig::default());
    let result = wall.check("SELECT 1; DROP TABLE users");
    assert!(result.is_err());
}

#[test]
fn test_wall_multi_statement_all_valid() {
    let wall = Wall::new(WallConfig::default());
    let result =
        wall.check("SELECT 1; INSERT INTO t (c) VALUES (1); UPDATE t SET c = 1 WHERE id = 1");
    assert!(result.is_ok());
}

// ── Additional Wall branch tests ──

#[test]
fn test_wall_update_allowed_when_disabled() {
    let c = WallConfig::builder().update_allow(false).build();
    let wall = Wall::new(c);
    let result = wall.check("UPDATE users SET x = 1 WHERE id = 1");
    assert!(result.is_err());
}

#[test]
fn test_wall_insert_allowed_when_disabled() {
    let c = WallConfig::builder().insert_allow(false).build();
    let wall = Wall::new(c);
    let result = wall.check("INSERT INTO t (c) VALUES (1)");
    assert!(result.is_err());
}

#[test]
fn test_wall_delete_allowed_when_disabled() {
    let c = WallConfig::builder().delete_allow(false).build();
    let wall = Wall::new(c);
    let result = wall.check("DELETE FROM users WHERE id = 1");
    assert!(result.is_err());
}

#[test]
fn test_wall_drop_allowed_when_enabled() {
    let c = WallConfig::builder().drop_table_allow(true).build();
    let wall = Wall::new(c);
    assert!(wall.check("DROP TABLE users").is_ok());
}

#[test]
fn test_wall_truncate_allowed_when_enabled() {
    let c = WallConfig::builder().truncate_allow(true).build();
    let wall = Wall::new(c);
    assert!(wall.check("TRUNCATE users").is_ok());
}

// ── Table deny tests ──

#[test]
fn test_wall_deny_table_in_update() {
    let c = WallConfig::builder().deny_table("admin").build();
    let wall = Wall::new(c);
    assert!(wall.check("UPDATE admin SET x = 1 WHERE id = 1").is_err());
}

#[test]
fn test_wall_deny_table_in_delete() {
    let c = WallConfig::builder().deny_table("admin").build();
    let wall = Wall::new(c);
    assert!(wall.check("DELETE FROM admin WHERE id = 1").is_err());
}

#[test]
fn test_wall_deny_table_in_select() {
    let c = WallConfig::builder().deny_table("admin").build();
    let wall = Wall::new(c);
    assert!(wall.check("SELECT * FROM admin").is_err());
}

#[test]
fn test_wall_deny_table_in_insert() {
    let c = WallConfig::builder().deny_table("admin").build();
    let wall = Wall::new(c);
    assert!(wall.check("INSERT INTO admin (c) VALUES (1)").is_err());
}

// ── Query subquery branch ──

#[test]
fn test_wall_query_with_subquery() {
    let wall = Wall::new(WallConfig::default());
    assert!(wall
        .check("SELECT * FROM t WHERE id IN (SELECT id FROM s)")
        .is_ok());
}

// ── Complex multi-violation ──

#[test]
fn test_wall_complex_multi_violation() {
    let c = WallConfig::builder().deny_table("secret").build();
    let wall = Wall::new(c);
    let result = wall.check("SELECT * FROM secret WHERE id = 1");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .iter()
        .any(|v| matches!(v, WallViolation::DeniedTable(_))));
}

// ── Empty/whitespace SQL ──

#[test]
fn test_wall_whitespace_only() {
    let wall = Wall::new(WallConfig::default());
    // Empty or whitespace may parse as no-op
    let _ = wall.check("   ");
}
