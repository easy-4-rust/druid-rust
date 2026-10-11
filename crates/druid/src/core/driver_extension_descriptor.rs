//! Registered driver extension descriptor.

use super::{DriverExtensionFactory, ExceptionSorter, ValidConnectionChecker};
use std::sync::Arc;

/// Bundles a connection factory with optional Druid checker and sorter policy.
#[derive(Debug)]
pub struct DriverExtensionDescriptor {
    pub db_type: &'static str,
    pub factory: DriverExtensionFactory,
    pub checker: Option<fn() -> Arc<dyn ValidConnectionChecker>>,
    pub sorter: Option<fn() -> Arc<dyn ExceptionSorter>>,
}
