/// Mapping of an SQL `BLOB` binary large object.
///
/// Corresponds to Java: `java.sql.Blob`. Positions are 1-based. The value supports reading,
/// searching, writing, truncating, and freeing its content; a freed value is invalid.
pub use crate::core::RdbcBlob as Blob;
