/// Mapping of an SQL `CLOB` character large object.
///
/// Corresponds to Java: `java.sql.Clob`. Positions are 1-based. It supports character and
/// ASCII streams, substrings, searching, writing, truncating, and freeing the value.
pub use crate::core::RdbcClob as Clob;
