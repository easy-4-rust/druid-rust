/// A session with a specific database.
///
/// Corresponds to Java: `java.sql.Connection`. It creates statements and manages transactions,
/// savepoints, isolation, read-only state, catalogs, schemas, type maps, client information,
/// network timeout, and lifecycle. Closing this logical connection returns it to Druid.
pub use crate::core::DruidPooledConnection as Connection;
