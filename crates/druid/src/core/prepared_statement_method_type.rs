//! Druid 私有 prepare 重载类型。
//!
//! 对应 Java：`PreparedStatementPool.MethodType`。

/// Druid cache 需要区分的 prepareStatement/prepareCall 重载。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(dead_code)] // All Java overload identities must exist before their call paths cut over.
pub enum PreparedStatementMethodType {
    /// `prepareStatement(String)`。
    M1,
    /// `prepareStatement(String, int, int)`。
    M2,
    /// `prepareStatement(String, int, int, int)`。
    M3,
    /// `prepareStatement(String, int[])`。
    M4,
    /// `prepareStatement(String, String[])`。
    M5,
    /// `prepareStatement(String, int)`。
    M6,
    /// `prepareCall(String)`。
    Precall1,
    /// `prepareCall(String, int, int, int)`。
    Precall2,
    /// `prepareCall(String, int, int)`。
    Precall3,
}
