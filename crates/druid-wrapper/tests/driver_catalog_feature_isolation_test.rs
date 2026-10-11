#![cfg(all(feature = "driver-catalog", not(feature = "sqlx")))]

use druid_wrapper::driver::{DatabaseConnectionConfig, DriverRegistryError, DruidDriverRegistry};

#[test]
fn catalog_without_sqlx_reports_uninstalled_runtime() {
    let registry = DruidDriverRegistry::builtin().expect("catalog must load independently");
    let config = DatabaseConnectionConfig::new("sqlite", "sqlite::memory:")
        .expect("SQLite profile id must be valid");
    let error = registry
        .resolve(&config)
        .err()
        .expect("SQLx must remain unavailable until its feature is selected");
    assert!(matches!(
        error,
        DriverRegistryError::UnsupportedRuntime { .. }
    ));
    assert!(error.to_string().contains("SQLx feature disabled"));
}
