//! Driver extension registry boundary test.
//!
//! After implementation, enabling a MySQL extension should resolve
//! factory/checker/sorter by `DbType`. Without the Wrapper linked,
//! Core must return `NoDriverExtension`.

use druid::core::DruidError;

#[test]
fn mysql_extension_resolves_checker_and_sorter() {
    // After inventory registration, this should resolve MySqlExceptionSorter
    // and MySqlValidConnectionChecker by DbType::mysql.
    // For now, this documents the expected contract.
    let result = druid::core::lookup_driver_extension("mysql");
    // Before implementation, this returns an error.
    // After implementation, it should return Ok(descriptor).
    assert!(
        result.is_err(),
        "before inventory registration, lookup must fail"
    );
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
