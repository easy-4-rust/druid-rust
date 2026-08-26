//! Canonical aliases for scalar values used by JDBC-like descriptors.

/// Arbitrary-precision decimal scalar。
pub type RdbcBigDecimal = ::bigdecimal::BigDecimal;
/// Calendar-free SQL date scalar。
pub type RdbcNaiveDate = ::chrono::NaiveDate;
/// Calendar-free SQL timestamp scalar。
pub type RdbcNaiveDateTime = ::chrono::NaiveDateTime;
/// Calendar-free SQL time scalar。
pub type RdbcNaiveTime = ::chrono::NaiveTime;
