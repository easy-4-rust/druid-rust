//! RDBC Statement 生命周期监听协议。
//!
//! 对应 Java 平台接口：`javax.sql.StatementEventListener`。

use super::DruidError;

/// `PreparedStatement` 关闭与错误事件监听器。
///
/// Rust 标准库没有 RDBC `PooledConnection`/`StatementEvent` 标准，因此在
/// `Connection` 平台边界定义最小 SPI。Druid 在 prepared/callable 语句成功关闭、
/// 或执行产生 SQL 异常时发布事件；回调使用创建语句时的连接与逻辑语句身份，
/// 在注册表锁外调用，且单个监听器 panic 不阻断其他回调或资源清理。
pub trait StatementEventListener: Send + Sync {
    /// `PreparedStatement` 已经关闭。
    ///
    /// # 参数
    /// - `connection_id`：创建该语句的物理连接 ID。
    /// - `statement_id`：当前逻辑 Statement 的对象身份。
    fn statement_closed(&self, connection_id: u64, statement_id: usize);

    /// `PreparedStatement` 发生驱动错误。
    ///
    /// # 参数
    /// - `connection_id`：创建该语句的物理连接 ID。
    /// - `statement_id`：当前逻辑 Statement 的对象身份。
    /// - `error`：驱动或 SQL 错误。
    fn statement_error_occurred(&self, connection_id: u64, statement_id: usize, error: &DruidError);
}
