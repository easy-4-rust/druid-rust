//! RDBC Statement 生命周期监听协议。
//!
//! 对应 Java 平台接口：`javax.sql.StatementEventListener`。

use super::DruidError;

/// PreparedStatement 关闭与错误事件监听器。
///
/// Rust 标准库没有 RDBC `PooledConnection`/`StatementEvent` 标准，因此在
/// `PhysicalConnection` 平台边界定义最小 SPI。Java Druid core 当前只保存、
/// 移除并在 holder reset 时清空此监听器，没有主动发布两个回调；Rust 保留同样
/// 的生产行为，方法用于外部驱动或后续 XA Adapter 按平台合同发布事件。
pub trait StatementEventListener: Send + Sync {
    /// PreparedStatement 已经关闭。
    ///
    /// # 参数
    /// - `connection_id`：创建该语句的物理连接 ID。
    /// - `statement_id`：当前逻辑 Statement 的对象身份。
    fn statement_closed(&self, connection_id: u64, statement_id: usize);

    /// PreparedStatement 发生驱动错误。
    ///
    /// # 参数
    /// - `connection_id`：创建该语句的物理连接 ID。
    /// - `statement_id`：当前逻辑 Statement 的对象身份。
    /// - `error`：驱动或 SQL 错误。
    fn statement_error_occurred(&self, connection_id: u64, statement_id: usize, error: &DruidError);
}
