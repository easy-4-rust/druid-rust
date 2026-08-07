/// Represents a precompiled, parameterized SQL statement.
///
/// Corresponds to Java: `java.sql.PreparedStatement`. Parameter indexes are 1-based and all
/// required values must be bound before execution. Bindings cover SQL NULL, LOBs, temporal
/// values, streams, and general objects; invalid conversions produce a data exception.
pub use crate::core::DruidPooledPreparedStatement as PreparedStatement;
