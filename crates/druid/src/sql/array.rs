/// Standard Rust mapping for an SQL `ARRAY` value.
///
/// Corresponds to Java: `java.sql.Array`. It preserves the SQL base type name, type number,
/// and elements, and supports reading the whole value or a range. Access after `free` fails.
pub use crate::core::RdbcArray as Array;
