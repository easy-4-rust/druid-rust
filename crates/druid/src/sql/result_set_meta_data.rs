/// Metadata describing the types and properties of `ResultSet` columns.
///
/// Corresponds to Java: `java.sql.ResultSetMetaData`. Column indexes are 1-based. Metadata
/// includes names, labels, origin, SQL type, mapped class, precision, scale, nullability,
/// signedness, mutability, and auto-increment behavior.
pub use crate::core::ResultSetMetaData;
