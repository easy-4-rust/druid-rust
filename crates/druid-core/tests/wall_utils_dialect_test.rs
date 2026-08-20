extern crate druid_core as druid;
use druid::sql::{WallConfig, WallUtils};

#[test]
fn wall_utils_is_validate_my_sql_valid() {
    assert!(WallUtils::is_validate_my_sql("SELECT 1").unwrap());
}

#[test]
fn wall_utils_is_validate_my_sql_drop() {
    let result = WallUtils::is_validate_my_sql("DROP TABLE t");
    assert!(result.is_ok());
}

#[test]
fn wall_utils_is_validate_my_sql_with_config() {
    let config = WallConfig::builder().select_allow(false).build();
    assert!(!WallUtils::is_validate_my_sql_with_config("SELECT 1", config).unwrap());
}

#[test]
fn wall_utils_is_validate_postgres_valid() {
    assert!(WallUtils::is_validate_postgres("SELECT 1").unwrap());
}

#[test]
fn wall_utils_is_validate_postgres_with_config() {
    let config = WallConfig::builder().select_allow(false).build();
    assert!(!WallUtils::is_validate_postgres_with_config("SELECT 1", config).unwrap());
}

#[test]
fn wall_utils_is_validate_oracle_valid() {
    assert!(WallUtils::is_validate_oracle("SELECT 1 FROM DUAL").unwrap());
}

#[test]
fn wall_utils_is_validate_oracle_with_config() {
    let config = WallConfig::builder().select_allow(false).build();
    assert!(!WallUtils::is_validate_oracle_with_config("SELECT 1 FROM DUAL", config).unwrap());
}

#[test]
fn wall_utils_is_validate_db2_valid() {
    assert!(WallUtils::is_validate_db2("SELECT 1").unwrap());
}

#[test]
fn wall_utils_is_validate_db2_with_config() {
    let config = WallConfig::builder().select_allow(false).build();
    assert!(!WallUtils::is_validate_db2_with_config("SELECT 1", config).unwrap());
}

#[test]
fn wall_utils_is_validate_sql_server_valid() {
    assert!(WallUtils::is_validate_sql_server("SELECT 1").unwrap());
}

#[test]
fn wall_utils_is_validate_sql_server_with_config() {
    let config = WallConfig::builder().select_allow(false).build();
    assert!(!WallUtils::is_validate_sql_server_with_config("SELECT 1", config).unwrap());
}
