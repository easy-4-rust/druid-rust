/// Comprehensive metadata about the database and its structure.
///
/// Corresponds to Java: `java.sql.DatabaseMetaData`. It reports database and driver identity,
/// identifier rules, SQL and transaction capabilities, and catalogs, schemas, tables, columns,
/// keys, indexes, routines, UDTs, and pseudo-columns. Driver claims are not certification.
pub type DatabaseMetaData<'connection> = crate::core::DatabaseMetaDataProxyImpl<'connection>;
