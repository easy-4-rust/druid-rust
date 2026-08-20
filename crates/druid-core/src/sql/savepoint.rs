/// A point in the current transaction that can be used for partial rollback.
///
/// Corresponds to Java: `java.sql.Savepoint`. A savepoint has either a driver-generated ID or
/// a user name. Reading the other form fails. Commit or full rollback invalidates the savepoint.
pub use crate::core::Savepoint;
