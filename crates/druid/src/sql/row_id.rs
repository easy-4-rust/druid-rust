/// Byte representation of an SQL `ROWID` value.
///
/// Corresponds to Java: `java.sql.RowId`. `RowIdLifetime` metadata defines its validity;
/// callers must not assume it survives a transaction, session, or database restart.
pub use crate::core::RdbcRowId as RowId;
