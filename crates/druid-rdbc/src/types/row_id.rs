//! SQL 行标识的平台字节值，来源为 Java `java.sql.RowId`。

/// 数据库行标识。对应 Java: `java.sql.RowId`。
///
/// 保存驱动返回的字节，并保留按字节比较与计算哈希的现有值策略。
/// 调用方还须考虑来源表、数据源及 `RowIdLifetime` 有效期；相同字节不保证
/// 跨来源代表同一行，也不保证跨事务、会话或数据库重启仍然有效。
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct RowId {
    bytes: Vec<u8>,
}

impl std::fmt::Debug for RowId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 标准类型归属迁移不改变已有资源转发日志中的对象名称及字节展示。
        formatter
            .debug_struct("RdbcRowId")
            .field("bytes", &self.bytes)
            .finish()
    }
}

impl RowId {
    /// 根据驱动字节创建行标识。
    ///
    /// 参数 `bytes` 为驱动提供的行标识字节；返回拥有这些字节的值。
    pub fn new(bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            bytes: bytes.into(),
        }
    }

    /// 借用行标识字节；返回与本对象具有相同借用有效期的只读切片。
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// 返回行标识字节副本。对应 Java: `RowId#getBytes`。
    ///
    /// 修改返回数组不改变本对象；该复制策略保留当前 Rust 行为。
    #[must_use]
    pub fn get_bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }
}
