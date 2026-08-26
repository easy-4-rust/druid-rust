//! Canonical value-level names for Task 6 JDBC LOB/stream descriptors.
//!
//! Task 7 still owns resource runtime hardening; these aliases expose only the already-existing
//! handle identity so canonical method descriptors do not depend on the transitional resource
//! module path.

/// Array value handle。
pub type RdbcArray = crate::resource::RdbcArray;
/// Blob value handle。
pub type RdbcBlob = crate::resource::RdbcBlob;
/// Character-length overload identity。
pub type RdbcCharacterLength = crate::resource::RdbcCharacterLength;
/// Clob value handle。
pub type RdbcClob = crate::resource::RdbcClob;
/// Input-stream handle。
pub type RdbcInputStream = crate::resource::RdbcInputStream;
/// NClob value handle。
pub type RdbcNClob = crate::resource::RdbcNClob;
/// Reader handle。
pub type RdbcReader = crate::resource::RdbcReader;
/// Ref value handle。
pub type RdbcRef = crate::resource::RdbcRef;
/// SQLXML value handle。
pub type RdbcSqlXml = crate::resource::RdbcSqlXml;
/// Stream-length overload identity。
pub type RdbcStreamLength = crate::resource::RdbcStreamLength;
