//! Registered driver extension descriptor.

use super::{ConnectionFactory, ExceptionSorter, RdbcError, ValidConnectionChecker};
use std::sync::Arc;

/// Bundles a connection factory with optional Druid checker and sorter policy.
#[derive(Debug)]
pub struct DriverExtensionDescriptor {
    pub db_type: &'static str,
    pub factory: fn(&str) -> Result<Arc<dyn ConnectionFactory>, RdbcError>,
    pub checker: Option<fn() -> Arc<dyn ValidConnectionChecker>>,
    pub sorter: Option<fn() -> Arc<dyn ExceptionSorter>>,
}
