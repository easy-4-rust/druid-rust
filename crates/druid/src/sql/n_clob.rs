/// Mapping of an SQL `NCLOB` national-character large object.
///
/// Corresponds to Java: `java.sql.NClob`. It follows the `Clob` read, write, search,
/// truncate, and free contracts while preserving national-character semantics.
pub use crate::core::RdbcNClob as NClob;
