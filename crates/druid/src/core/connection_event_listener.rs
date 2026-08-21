use super::DruidError;

/// 池化连接关闭与 SQL 错误监听器。
///
/// 对应 Java 平台接口: `javax.sql.ConnectionEventListener`。Rust 标准库没有
/// RDBC/XA 监听标准，因此在 `PhysicalConnection` 边界定义最小 SPI。
pub trait ConnectionEventListener: Send + Sync {
    /// 逻辑池化连接已经关闭或归还。
    fn connection_closed(&self, connection_id: u64);

    /// 连接操作产生 SQL/驱动错误。
    fn connection_error_occurred(&self, connection_id: u64, error: &DruidError);
}
