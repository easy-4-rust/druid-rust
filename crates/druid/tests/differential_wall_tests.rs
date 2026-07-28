//! Differential tests: druid-sql Wall vs Druid Java 1.2.28 WallConfig + WallProvider.
//!
//! References:
//!   - core/src/main/java/com/alibaba/druid/wall/WallConfig.java
//!   - core/src/main/java/com/alibaba/druid/wall/WallProvider.java
//!   - core/src/test/java/com/alibaba/druid/bvt/wall/WallBVTTest.java
//!   - core/src/test/java/com/alibaba/druid/bvt/wall/MySqlWallTest.java

use druid::sql::{Wall, WallConfig, WallViolation};

// ── WallConfig 46-field defaults match DruidJava WallConfig constructor ──

/// All WallConfig boolean defaults verified against DruidJava source.
#[test]
fn test_wall_config_all_46_defaults() {
    let c = WallConfig::default();
    // SQL type control (11 fields)
    assert!(c.select_allow);
    assert!(c.select_all_column_allow);
    assert!(c.select_into_allow);
    assert!(c.insert_allow);
    assert!(c.update_allow);
    assert!(c.delete_allow);
    assert!(!c.drop_table_allow); // DruidJava: false
    assert!(!c.truncate_allow); // DruidJava: false
    assert!(c.alter_table_allow);
    assert!(c.create_table_allow);
    assert!(c.commit_allow);
    // Transaction/session (5 fields)
    assert!(c.rollback_allow);
    assert!(c.use_allow);
    assert!(c.show_allow);
    assert!(c.describe_allow);
    assert!(c.start_transaction_allow);
    // WHERE enforcement (6 fields)
    assert!(c.set_allow);
    assert!(c.update_must_have_where); // DruidJava: true
    assert!(c.delete_must_have_where); // DruidJava: true
    assert!(c.select_where_alway_true_check);
    assert!(c.select_having_alway_true_check);
    assert!(c.update_where_alway_true_check);
    // Condition checks (5 fields)
    assert!(c.delete_where_alway_true_check);
    assert!(!c.condition_and_alway_true_allow);
    assert!(!c.condition_and_alway_false_allow);
    assert!(!c.condition_double_const_allow);
    assert!(c.condition_like_true_allow);
    // Syntax control (9 fields)
    assert!(c.case_condition_const_allow);
    assert!(!c.multi_statement_allow); // DruidJava: false
    assert!(c.hint_allow);
    assert!(c.none_base_statement_allow);
    assert!(!c.limit_zero_allow); // DruidJava: false
    assert!(c.comment_allow);
    assert!(c.variant_check);
    assert!(!c.must_parameterized);
    assert!(c.metadata_allow);
    // Lists + schema (5 fields)
    assert!(c.wrap_allow);
    assert!(c.deny_tables.is_empty());
    assert!(c.deny_functions.is_empty());
    assert!(c.deny_schemas.is_empty());
    assert!(c.deny_variants.is_empty());
    // White list (3 fields)
    assert!(!c.select_white_list);
    assert!(!c.function_white_list);
    assert!(!c.schema_white_list);
    // Multi-tenant (2 fields)
    assert!(c.tenant_column.is_empty());
    assert!(c.tenant_table_pattern.is_empty());
}

// ── DruidJava WallBVTTest.java behavioral tests ──

/// WallBVTTest#test_delete_0: DELETE without WHERE → Denied.
#[test]
fn test_wall_delete_without_where() {
    let wall = Wall::new(WallConfig::default());
    let result = wall.check("DELETE FROM users");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .iter()
        .any(|v| matches!(v, WallViolation::DeleteWithoutWhere)));
}

/// WallBVTTest#test_delete_1: DELETE with WHERE → Allowed.
#[test]
fn test_wall_delete_with_where() {
    let wall = Wall::new(WallConfig::default());
    assert!(wall.check("DELETE FROM users WHERE id = 1").is_ok());
}

/// WallBVTTest#test_update_0: UPDATE without WHERE → Denied.
#[test]
fn test_wall_update_without_where() {
    let wall = Wall::new(WallConfig::default());
    let result = wall.check("UPDATE users SET name = 'x'");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .iter()
        .any(|v| matches!(v, WallViolation::UpdateWithoutWhere)));
}

/// WallBVTTest#test_update_1: UPDATE with WHERE → Allowed.
#[test]
fn test_wall_update_with_where() {
    let wall = Wall::new(WallConfig::default());
    assert!(wall
        .check("UPDATE users SET name = 'x' WHERE id = 1")
        .is_ok());
}

/// WallBVTTest#test_drop: DROP TABLE → Denied.
#[test]
fn test_wall_drop_table_denied() {
    let wall = Wall::new(WallConfig::default());
    let result = wall.check("DROP TABLE users");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .iter()
        .any(|v| matches!(v, WallViolation::DropTableNotAllowed(_))));
}

/// WallBVTTest#test_truncate: TRUNCATE → Denied.
#[test]
fn test_wall_truncate_denied() {
    let wall = Wall::new(WallConfig::default());
    let result = wall.check("TRUNCATE users");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .iter()
        .any(|v| matches!(v, WallViolation::TruncateNotAllowed)));
}

/// WallBVTTest#test_select: SELECT → Allowed.
#[test]
fn test_wall_select_allowed() {
    let wall = Wall::new(WallConfig::default());
    assert!(wall
        .check("SELECT id, name FROM users WHERE id = 1")
        .is_ok());
}

/// WallBVTTest#test_insert: INSERT → Allowed.
#[test]
fn test_wall_insert_allowed() {
    let wall = Wall::new(WallConfig::default());
    assert!(wall
        .check("INSERT INTO users (name) VALUES ('test')")
        .is_ok());
}

/// WallBVTTest#test_deny_table: denied table → Blocked.
#[test]
fn test_wall_deny_table() {
    let c = WallConfig::builder().deny_table("secret_data").build();
    let wall = Wall::new(c);
    let result = wall.check("SELECT * FROM secret_data");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .iter()
        .any(|v| matches!(v, WallViolation::DeniedTable(_))));
}

/// WallBVTTest#test_drop_table_allowed: DROP TABLE allowed when configured.
#[test]
fn test_wall_drop_allowed_when_configured() {
    let c = WallConfig::builder().drop_table_allow(true).build();
    assert!(Wall::new(c).check("DROP TABLE users").is_ok());
}

/// WallBVTTest#test_truncate_allowed: TRUNCATE allowed when configured.
#[test]
fn test_wall_truncate_allowed_when_configured() {
    let c = WallConfig::builder().truncate_allow(true).build();
    assert!(Wall::new(c).check("TRUNCATE users").is_ok());
}

/// WallBVTTest#test_delete_denied: DELETE denied when configured.
#[test]
fn test_wall_delete_denied_when_configured() {
    let c = WallConfig::builder().delete_allow(false).build();
    let result = Wall::new(c).check("DELETE FROM users WHERE id = 1");
    assert!(result.is_err());
}

/// WallBVTTest#test_update_denied: UPDATE denied when configured.
#[test]
fn test_wall_update_denied_when_configured() {
    let c = WallConfig::builder().update_allow(false).build();
    let result = Wall::new(c).check("UPDATE users SET name = 'x' WHERE id = 1");
    assert!(result.is_err());
}

/// WallBVTTest#test_syntax_error: malformed SQL → SyntaxError.
#[test]
fn test_wall_syntax_error() {
    let wall = Wall::new(WallConfig::default());
    let result = wall.check("SELCT * FORM users");
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .iter()
        .any(|v| matches!(v, WallViolation::SyntaxError(_))));
}

/// Multiple violations in one statement.
#[test]
fn test_wall_multiple_violations() {
    let wall = Wall::new(WallConfig::default());
    // DROP TABLE + TRUNCATE in same parse (multi)
    let result = wall.check("DROP TABLE users; TRUNCATE orders");
    assert!(result.is_err());
    let v = result.unwrap_err();
    assert!(
        v.len() >= 2,
        "should report at least 2 violations, got {}",
        v.len()
    );
}

/// Table name check across SELECT/UPDATE/DELETE.
#[test]
fn test_wall_deny_table_in_select() {
    let c = WallConfig::builder().deny_table("admin").build();
    assert!(Wall::new(c).check("SELECT * FROM admin").is_err());
}

#[test]
fn test_wall_deny_table_in_update() {
    let c = WallConfig::builder().deny_table("admin").build();
    assert!(Wall::new(c)
        .check("UPDATE admin SET x = 1 WHERE id = 1")
        .is_err());
}

#[test]
fn test_wall_deny_table_in_delete() {
    let c = WallConfig::builder().deny_table("admin").build();
    assert!(Wall::new(c)
        .check("DELETE FROM admin WHERE id = 1")
        .is_err());
}

/// WallConfig builder chaining.
#[test]
fn test_wall_config_builder_chain() {
    let c = WallConfig::builder()
        .drop_table_allow(true)
        .truncate_allow(true)
        .update_must_have_where(false)
        .multi_statement_allow(true)
        .comment_allow(false)
        .limit_zero_allow(true)
        .deny_table("t1")
        .deny_function("sleep")
        .deny_schema("pg_catalog")
        .tenant_column("tenant_id")
        .build();
    assert!(c.drop_table_allow);
    assert!(c.truncate_allow);
    assert!(!c.update_must_have_where);
    assert!(c.multi_statement_allow);
    assert!(!c.comment_allow);
    assert!(c.limit_zero_allow);
    assert_eq!(c.deny_tables, vec!["t1"]);
    assert_eq!(c.deny_functions, vec!["sleep"]);
    assert_eq!(c.deny_schemas, vec!["pg_catalog"]);
    assert_eq!(c.tenant_column, "tenant_id");
}

/// Empty SQL → no violations.
#[test]
fn test_wall_empty_sql() {
    assert!(Wall::new(WallConfig::default()).check("").is_ok());
}

// ── WallViolation Display formatting ──

#[test]
fn test_wall_violation_display_drop_table() {
    let v = WallViolation::DropTableNotAllowed("users".to_string());
    assert!(format!("{v}").contains("DROP TABLE"));
    assert!(format!("{v}").contains("users"));
}

#[test]
fn test_wall_violation_display_truncate() {
    let v = WallViolation::TruncateNotAllowed;
    assert!(format!("{v}").contains("TRUNCATE"));
}

#[test]
fn test_wall_violation_display_delete_without_where() {
    let v = WallViolation::DeleteWithoutWhere;
    assert!(format!("{v}").contains("DELETE"));
    assert!(format!("{v}").contains("WHERE"));
}

#[test]
fn test_wall_violation_display_update_without_where() {
    let v = WallViolation::UpdateWithoutWhere;
    assert!(format!("{v}").contains("UPDATE"));
}

#[test]
fn test_wall_violation_display_denied_table() {
    let v = WallViolation::DeniedTable("secret".to_string());
    assert!(format!("{v}").contains("secret"));
}

#[test]
fn test_wall_violation_display_denied_function() {
    let v = WallViolation::DeniedFunction("sleep".to_string());
    assert!(format!("{v}").contains("sleep"));
}

#[test]
fn test_wall_violation_display_syntax_error() {
    let v = WallViolation::SyntaxError("unexpected token".to_string());
    assert!(format!("{v}").contains("unexpected token"));
}

#[test]
fn test_wall_violation_is_error() {
    use std::error::Error;
    let v = WallViolation::DeleteWithoutWhere;
    assert!(v.source().is_none()); // Error trait impl
}
