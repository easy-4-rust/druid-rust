/// Tabular query data together with its cursor.
///
/// Corresponds to Java: `java.sql.ResultSet`. The cursor begins before the first row and values
/// are readable after a successful `next`. Column indexes are 1-based. SQL NULL uses the target
/// type's empty/default mapping and is distinguished by `was_null`.
pub use crate::core::DruidPooledResultSet as ResultSet;
