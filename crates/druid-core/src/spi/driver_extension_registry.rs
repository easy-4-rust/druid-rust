//! Global driver extension registry backed by `inventory`.
//!
//! `druid-wrapper` submits `DriverExtensionDescriptor` items via
//! `inventory::submit!`; Core provides `lookup_driver_extension` to
//! resolve by database type.

use super::driver_extension_descriptor::DriverExtensionDescriptor;
use crate::core::DruidError;
use std::sync::Arc;

inventory::collect!(DriverExtensionDescriptor);

/// Look up a driver extension by database type (e.g., "mysql", "postgresql").
///
/// Returns `Err(DruidError::NoDriverExtension)` if no extension is registered
/// for the given `db_type`.
pub fn lookup_driver_extension(
    db_type: &str,
) -> Result<&'static DriverExtensionDescriptor, DruidError> {
    inventory::iter::<DriverExtensionDescriptor>
        .into_iter()
        .find(|ext| ext.db_type.eq_ignore_ascii_case(db_type))
        .ok_or_else(|| DruidError::NoDriverExtension {
            db_type: db_type.to_owned(),
        })
}
