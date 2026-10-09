//! Canonical aliases for scalar values used by JDBC-like descriptors.

/// Arbitrary-precision decimal scalar。
pub type RdbcBigDecimal = ::bigdecimal::BigDecimal;
/// Calendar-free SQL date scalar。
pub type RdbcNaiveDate = ::chrono::NaiveDate;
/// Calendar-free SQL timestamp scalar。
pub type RdbcNaiveDateTime = ::chrono::NaiveDateTime;
/// Calendar-free SQL time scalar。
pub type RdbcNaiveTime = ::chrono::NaiveTime;

/// 将二进制浮点精确转换为平台 Decimal；参数为浮点值，非有限值返回 None。
/// 对应 Java: java.sql.ResultSet#getBigDecimal 的平台数值转换。
pub(crate) fn decimal_from_f64(value: f64) -> Option<RdbcBigDecimal> {
    ::bigdecimal::FromPrimitive::from_f64(value)
}

/// 将平台 Decimal 截断为 i64；参数为 Decimal，超范围返回 None。
/// 对应 Java: java.sql.ResultSet#getObject(int, Long.class)。
pub(crate) fn decimal_to_i64(value: &RdbcBigDecimal) -> Option<i64> {
    ::bigdecimal::ToPrimitive::to_i64(value)
}

/// 将平台 Decimal 转换为浮点；参数为 Decimal，不能表示时返回 None。
/// 对应 Java: java.sql.ResultSet#getObject(int, Double.class)。
pub(crate) fn decimal_to_f64(value: &RdbcBigDecimal) -> Option<f64> {
    ::bigdecimal::ToPrimitive::to_f64(value)
}
