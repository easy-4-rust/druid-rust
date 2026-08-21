//! Driver extension registry boundary test.
//!
//! Verifies that druid-wrapper's inventory registrations are visible through
//! Core's `lookup_driver_extension` by database type.

use druid::core::DruidError;

/// Ensure driver extension registrations are linked before tests run.
fn ensure_extensions() {
    druid_wrapper::init_driver_extensions();
}

#[test]
fn mysql_extension_resolves_checker_and_sorter() {
    ensure_extensions();
    let descriptor = druid::core::lookup_driver_extension("mysql")
        .expect("mysql extension must be registered by druid-wrapper");
    assert_eq!(descriptor.db_type, "mysql");
    assert!(descriptor.checker.is_some(), "mysql must have a checker");
    assert!(descriptor.sorter.is_some(), "mysql must have a sorter");
}

#[test]
fn postgresql_extension_resolves() {
    ensure_extensions();
    let descriptor = druid::core::lookup_driver_extension("postgresql")
        .expect("postgresql extension must be registered");
    assert_eq!(descriptor.db_type, "postgresql");
    assert!(descriptor.checker.is_some());
    assert!(descriptor.sorter.is_some());
}

#[test]
fn sqlite_extension_resolves() {
    let descriptor = druid::core::lookup_driver_extension("sqlite")
        .expect("sqlite extension must be registered");
    assert_eq!(descriptor.db_type, "sqlite");
    assert!(
        descriptor.checker.is_none(),
        "sqlite has no special checker"
    );
    assert!(descriptor.sorter.is_none(), "sqlite has no special sorter");
}

#[test]
fn unknown_extension_returns_no_driver_extension() {
    let result = druid::core::lookup_driver_extension("nonexistent_db_xyz");
    assert!(result.is_err());
    match result {
        Err(DruidError::NoDriverExtension { .. }) => {}
        other => panic!("expected NoDriverExtension, got {other:?}"),
    }
}
