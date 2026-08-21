use super::RdbcClobAccess;

/// Marker access contract for Java `java.sql.NClob`, which extends all `Clob` operations.
pub trait RdbcNClobAccess: RdbcClobAccess {}
