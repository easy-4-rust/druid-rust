use super::StatementEvent;

/// PreparedStatement 生命周期标准监听器；独立于 Druid Filter 的执行事件。
/// 对应 Java: javax.sql.StatementEventListener。
pub trait StatementEventListener: Send + Sync {
    /// 处理语句关闭事件；参数保留原来源与语句引用，方法无返回值。
    /// 对应 Java: StatementEventListener#statementClosed。
    fn statement_closed(&self, event: &StatementEvent);

    /// 处理语句错误事件；参数保留原 SQLException 引用，方法无返回值。
    /// 对应 Java: StatementEventListener#statementErrorOccurred。
    fn statement_error_occurred(&self, event: &StatementEvent);
}
