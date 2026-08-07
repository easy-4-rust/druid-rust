//! RDBC 结果集列可空性。
//!
//! 对应 Java：`java.sql.ResultSetMetaData` 的 `columnNoNulls`、
//! `columnNullable` 与 `columnNullableUnknown`。

/// 结果集列可空性，保留 unknown，不能压缩为布尔值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultSetNullability {
    /// 明确不允许 SQL NULL。
    NoNulls,
    /// 明确允许 SQL NULL。
    Nullable,
    /// 驱动未提供可空性。
    Unknown,
}

impl ResultSetNullability {
    /// 返回 Java `ResultSetMetaData` 常量值。
    pub const fn rdbc_code(self) -> i32 {
        match self {
            Self::NoNulls => 0,
            Self::Nullable => 1,
            Self::Unknown => 2,
        }
    }
}
