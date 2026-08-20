//! RDBC `RowIdLifetime` 的 Rust 协议值。
//!
//! 对应 Java 平台对象：`java.sql.RowIdLifetime`。

/// RowId 值保持有效的最长生命周期。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DatabaseMetaDataRowIdLifetime {
    /// 驱动不支持 RowId。
    RowIdUnsupported,
    /// RowId 仅在当前事务内有效。
    RowIdValidOther,
    /// RowId 至少在当前会话内有效。
    RowIdValidSession,
    /// RowId 至少在当前事务内有效。
    RowIdValidTransaction,
    /// RowId 在删除对应行前持续有效。
    RowIdValidForever,
}
