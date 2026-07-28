//! JDBC `RowId` 平台值。
//!
//! 对应 Java 平台对象：`java.sql.RowId`。Java 合同以 `getBytes()` 内容定义
//! equality/hashCode，Rust 直接保存原始字节并派生相同的值语义。

/// 数据库行标识。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JdbcRowId {
    bytes: Vec<u8>,
}

impl JdbcRowId {
    /// 从驱动返回的 RowId 字节创建值。
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            bytes: bytes.into(),
        }
    }

    /// 对应 Java `RowId#getBytes()`。
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}
