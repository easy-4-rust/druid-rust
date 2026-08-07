//! 统计 Filter 上下文监听器。
//!
//! 对应 Java：
//! `com.alibaba.druid.filter.stat.StatFilterContextListener`。

use crate::core::DruidError;

/// 接收 RDBC 执行、连接、ResultSet 与 LOB 统计事件。
///
/// Java 方法可抛出 unchecked exception；Rust 使用 `Result` 保留“首个错误中止
/// 后续 listener 和当前调用”的可观测行为。
pub trait StatFilterContextListener: Send + Sync {
    /// 增加更新行数。
    fn add_update_count(&self, update_count: i32) -> Result<(), DruidError>;

    /// 增加抓取行数。
    fn add_fetch_row_count(&self, fetch_row_count: i32) -> Result<(), DruidError>;

    /// SQL 执行前事件。
    fn execute_before(&self, sql: &str, in_transaction: bool) -> Result<(), DruidError>;

    /// SQL 执行后事件。
    ///
    /// `sql` 与 `error` 分别对应 Java 可空 `String` 和 `Throwable`。普通
    /// Statement 成功 batch 的 `lastExecuteSql` 为 `null`，该三态不能丢失。
    fn execute_after(
        &self,
        sql: Option<&str>,
        nano_span: i64,
        error: Option<&DruidError>,
    ) -> Result<(), DruidError>;

    /// 提交事件。
    fn commit(&self) -> Result<(), DruidError>;

    /// 回滚事件。
    fn rollback(&self) -> Result<(), DruidError>;

    /// 池化连接打开事件。
    fn pool_connect(&self) -> Result<(), DruidError>;

    /// 池化连接关闭事件。
    fn pool_close(&self, nanos: i64) -> Result<(), DruidError>;

    /// 物理连接创建事件。
    fn physical_connection_connect(&self) -> Result<(), DruidError>;

    /// 物理连接关闭事件。
    fn physical_connection_close(&self, nanos: i64) -> Result<(), DruidError>;

    /// ResultSet 打开事件。
    fn result_set_open(&self) -> Result<(), DruidError>;

    /// ResultSet 关闭事件。
    fn result_set_close(&self, nanos: i64) -> Result<(), DruidError>;

    /// Clob 打开事件。
    fn clob_open(&self) -> Result<(), DruidError>;

    /// Blob 打开事件。
    fn blob_open(&self) -> Result<(), DruidError>;
}
