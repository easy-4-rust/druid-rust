/// Executes SQL stored procedures.
///
/// Corresponds to Java: `java.sql.CallableStatement`. It extends `PreparedStatement` with
/// indexed or named IN parameters, OUT and INOUT registration, typed output retrieval, and
/// the RDBC 4.2 REF CURSOR type.
pub use crate::core::DruidPooledCallableStatement as CallableStatement;
