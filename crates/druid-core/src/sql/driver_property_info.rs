/// Description of a driver property used to establish a connection.
///
/// Corresponds to Java: `java.sql.DriverPropertyInfo`. It contains the name, current value,
/// description, required flag, and choices. It is configuration metadata, not validation.
pub use crate::core::DriverProperty as DriverPropertyInfo;
