//! Driver extension descriptor: factory + checker + sorter for a database type.

use crate::core::{DruidError, ExceptionSorter, PhysicalConnectionFactory, ValidConnectionChecker};
use std::sync::Arc;

/// Describes a concrete database driver extension registered by `druid-wrapper`.
///
/// Each descriptor bundles the `PhysicalConnectionFactory`, optional
/// `ValidConnectionChecker`, and optional `ExceptionSorter` for a given
/// database type (e.g., "mysql", "postgresql", "oracle").
#[derive(Debug)]
pub struct DriverExtensionDescriptor {
    /// Database type identifier (lowercase, e.g. "mysql", "postgresql").
    pub db_type: &'static str,
    /// Factory that creates unpoised physical connections for this database.
    pub factory: fn(&str) -> Result<Arc<dyn PhysicalConnectionFactory>, DruidError>,
    /// Optional connection checker (e.g., `MySqlValidConnectionChecker`).
    pub checker: Option<fn() -> Arc<dyn ValidConnectionChecker>>,
    /// Optional exception sorter (e.g., `MySqlExceptionSorter`).
    pub sorter: Option<fn() -> Arc<dyn ExceptionSorter>>,
}
