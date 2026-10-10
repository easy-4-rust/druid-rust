use super::ConnectionEvent;

/// 池化连接生命周期标准监听器，接收完整事件对象而非 Druid 产品错误或数字 ID。
/// 对应 Java: javax.sql.ConnectionEventListener。
pub trait ConnectionEventListener: Send + Sync {
    /// 处理逻辑连接关闭事件；参数保留原来源句柄，返回后不隐式关闭物理连接。
    /// 对应 Java: ConnectionEventListener#connectionClosed。
    fn connection_closed(&self, event: &ConnectionEvent);

    /// 处理连接错误事件；参数包含原 SQL 异常共享引用，方法无返回值。
    /// 对应 Java: ConnectionEventListener#connectionErrorOccurred。
    fn connection_error_occurred(&self, event: &ConnectionEvent);
}
