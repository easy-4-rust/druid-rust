/// Executes a static SQL statement and exposes the results it produces.
///
/// Corresponds to Java: `java.sql.Statement`. It supports query, update, execute, batches,
/// generated keys, fetch and timeout hints, cancellation, multiple results, and large updates.
/// Closing the statement also closes its current result set.
pub use crate::core::DruidPooledStatement as Statement;
