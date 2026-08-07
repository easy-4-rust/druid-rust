/// Mapping of an SQL `REF` that refers to a structured value in the database.
///
/// Corresponds to Java: `java.sql.Ref`. It exposes the referenced SQL type name and value and,
/// when supported, can replace the target. The connection type map controls UDT conversion.
pub use crate::core::RdbcRef as Ref;
