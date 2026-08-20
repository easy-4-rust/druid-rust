//! 数据源级致命连接错误处理协议。

use super::DruidError;

/// 接收池化连接识别出的致命数据库错误。
///
/// 对应 Java:
/// `com.alibaba.druid.pool.DruidDataSource#handleFatalError` 与
/// `DruidAbstractDataSource` 的 fatal-error 状态。协议放在核心连接边界，
/// 使 `DruidPooledConnection` 不反向依赖具体连接池实现。
pub(crate) trait FatalErrorHandler: Send + Sync {
    /// 记录致命错误并返回数据源是否进入 `onFatalError` 状态。
    ///
    /// # 参数
    ///
    /// - `error`：已经由 `ExceptionSorter` 判定为 fatal 的 SQL 错误。
    /// - `sql`：触发错误的 SQL；连接级操作没有 SQL 时为 `None`。
    fn handle_fatal_error(&self, error: &DruidError, sql: Option<&str>) -> bool;

    /// 在 fatal 处置未触发常规 `minIdle` 补池时唤醒一次创建路径。
    fn request_fatal_error_refill(&self);

    /// 成功验证连接后解除 `onFatalError` 门禁。
    fn clear_on_fatal_error(&self);
}
