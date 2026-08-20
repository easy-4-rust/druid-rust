extern crate druid_core as druid;
use druid_core::core::{
    MsSqlValidConnectionChecker, OceanBaseValidConnectionChecker, ValidConnectionChecker,
};
use std::time::Duration;

// ── OceanBaseValidConnectionChecker ────────────────────────────

#[test]
fn ocean_base_checker_new() {
    let c = OceanBaseValidConnectionChecker::new();
    assert_eq!(
        OceanBaseValidConnectionChecker::COMMON_VALIDATE_QUERY,
        "SELECT 'x' FROM DUAL"
    );
    let _ = c;
}

#[test]
fn ocean_base_checker_mysql_mode() {
    let c = OceanBaseValidConnectionChecker::mysql_mode();
    assert_eq!(
        OceanBaseValidConnectionChecker::MYSQL_VALIDATE_QUERY,
        "/* ping */ SELECT 1"
    );
    let _ = c;
}

#[test]
fn ocean_base_checker_default() {
    let c = OceanBaseValidConnectionChecker::default();
    let _ = c;
}

#[test]
fn ocean_base_checker_clone_copy_debug() {
    let c = OceanBaseValidConnectionChecker::new();
    let c2 = c;
    let _ = format!("{:?}", c2);
}

// ── MsSqlValidConnectionChecker ────────────────────────────────

#[test]
fn mssql_checker_default() {
    let c = MsSqlValidConnectionChecker::default();
    assert_eq!(
        MsSqlValidConnectionChecker::DEFAULT_VALIDATION_QUERY,
        "SELECT 1"
    );
    let _ = c;
}

#[test]
fn mssql_checker_clone_copy_debug() {
    let c = MsSqlValidConnectionChecker::default();
    let c2 = c;
    let _ = format!("{:?}", c2);
}
