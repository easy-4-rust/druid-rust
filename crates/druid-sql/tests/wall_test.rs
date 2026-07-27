//! druid-sql S3 验收测试：FR-010 ~ FR-015

use druid_sql::{Wall, WallConfig, WallViolation};

// FR-010: SELECT 1 解析通过
#[test]
fn test_select_1_passes() {
    let wall = Wall::new(WallConfig::default());
    assert!(wall.check("SELECT 1").is_ok());
}

#[test]
fn test_select_with_where_passes() {
    let wall = Wall::new(WallConfig::default());
    assert!(wall.check("SELECT id FROM users WHERE id = 1").is_ok());
}

// FR-011: DROP TABLE 拦截
#[test]
fn test_drop_table_blocked() {
    let wall = Wall::new(WallConfig::default());
    let result = wall.check("DROP TABLE users");
    assert!(result.is_err());
    let violations = result.unwrap_err();
    assert!(violations.iter().any(|v| matches!(v, WallViolation::DropTableNotAllowed(_))));
}

#[test]
fn test_drop_table_allowed_when_configured() {
    let config = WallConfig::builder().drop_table_allow(true).build();
    let wall = Wall::new(config);
    assert!(wall.check("DROP TABLE users").is_ok());
}

// FR-012: UPDATE 无 WHERE 拦截
#[test]
fn test_update_without_where_blocked() {
    let wall = Wall::new(WallConfig::default());
    let result = wall.check("UPDATE users SET name = 'a'");
    assert!(result.is_err());
    let violations = result.unwrap_err();
    assert!(violations.iter().any(|v| matches!(v, WallViolation::UpdateWithoutWhere)));
}

#[test]
fn test_update_with_where_passes() {
    let wall = Wall::new(WallConfig::default());
    assert!(wall.check("UPDATE users SET name = 'a' WHERE id = 1").is_ok());
}

// FR-013: DELETE 无 WHERE 拦截
#[test]
fn test_delete_without_where_blocked() {
    let wall = Wall::new(WallConfig::default());
    let result = wall.check("DELETE FROM users");
    assert!(result.is_err());
    let violations = result.unwrap_err();
    assert!(violations.iter().any(|v| matches!(v, WallViolation::DeleteWithoutWhere)));
}

#[test]
fn test_delete_with_where_passes() {
    let wall = Wall::new(WallConfig::default());
    assert!(wall.check("DELETE FROM users WHERE id = 1").is_ok());
}

// FR-014: TRUNCATE 拦截
#[test]
fn test_truncate_blocked() {
    let wall = Wall::new(WallConfig::default());
    let result = wall.check("TRUNCATE users");
    assert!(result.is_err());
    let violations = result.unwrap_err();
    assert!(violations.iter().any(|v| matches!(v, WallViolation::TruncateNotAllowed)));
}

#[test]
fn test_truncate_allowed_when_configured() {
    let config = WallConfig::builder().truncate_allow(true).build();
    let wall = Wall::new(config);
    assert!(wall.check("TRUNCATE users").is_ok());
}

// FR-015: WallConfig builder
#[test]
fn test_wall_config_builder_defaults() {
    let config = WallConfig::default();
    assert!(!config.drop_table_allow);      // 默认拒绝 DROP
    assert!(!config.truncate_allow);        // 默认拒绝 TRUNCATE
    assert!(config.update_must_have_where); // UPDATE 必须有 WHERE
    assert!(config.delete_must_have_where); // DELETE 必须有 WHERE
    assert!(config.select_all_column_allow);       // SELECT * 默认允许
    assert!(config.insert_allow);           // INSERT 默认允许
}

#[test]
fn test_wall_config_builder_chaining() {
    let config = WallConfig::builder()
        .drop_table_allow(true)
        .truncate_allow(true)
        .update_must_have_where(false)
        .deny_table("secret_table")
        .deny_function("pg_sleep")
        .build();
    assert!(config.drop_table_allow);
    assert!(config.truncate_allow);
    assert!(!config.update_must_have_where);
    assert!(config.deny_tables.contains(&"secret_table".to_string()));
    assert!(config.deny_functions.contains(&"pg_sleep".to_string()));
}

// 额外：表黑名单
#[test]
fn test_denied_table_blocked() {
    let config = WallConfig::builder().deny_table("secret_data").build();
    let wall = Wall::new(config);
    let result = wall.check("SELECT * FROM secret_data");
    assert!(result.is_err());
    assert!(result.unwrap_err().iter().any(|v| matches!(v, WallViolation::DeniedTable(_))));
}

// 额外：语法错误
#[test]
fn test_syntax_error_reported() {
    let wall = Wall::new(WallConfig::default());
    let result = wall.check("SELCT * FORM users");
    assert!(result.is_err());
    assert!(result.unwrap_err().iter().any(|v| matches!(v, WallViolation::SyntaxError(_))));
}

// 额外：多语句
#[test]
fn test_multi_statement_parsing() {
    let wall = Wall::new(WallConfig::default());
    // sqlparser 默认逐条解析
    assert!(wall.check("SELECT 1; SELECT 2").is_ok());
}

// 额外：INSERT 默认允许
#[test]
fn test_insert_allowed_by_default() {
    let wall = Wall::new(WallConfig::default());
    assert!(wall.check("INSERT INTO users (name) VALUES ('a')").is_ok());
}
