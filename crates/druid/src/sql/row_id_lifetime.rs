/// Lifetime that a data source declares for its `ROWID` values.
///
/// Corresponds to Java: `java.sql.RowIdLifetime`. It distinguishes unsupported, transaction,
/// session, and effectively unlimited lifetimes. Druid does not extend the driver's lifetime.
pub use druid_rdbc::metadata::RowIdLifetime;
