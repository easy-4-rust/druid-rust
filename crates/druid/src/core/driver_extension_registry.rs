//! Inventory-backed driver extension registry.

use super::{DriverExtensionDescriptor, RdbcError};

inventory::collect!(DriverExtensionDescriptor);

pub fn lookup_driver_extension(
    db_type: &str,
) -> Result<&'static DriverExtensionDescriptor, RdbcError> {
    inventory::iter::<DriverExtensionDescriptor>
        .into_iter()
        .find(|extension| extension.db_type.eq_ignore_ascii_case(db_type))
        .ok_or_else(|| RdbcError::NoDriverExtension {
            db_type: db_type.to_owned(),
        })
}
