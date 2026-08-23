/// Byte representation of an SQL `ROWID` value.
///
/// Corresponds to Java: `java.sql.RowId`. `RowIdLifetime` metadata defines its validity;
/// callers must not assume it survives a transaction, session, or database restart.
pub use druid_rdbc::types::RdbcRowId as RowId;
