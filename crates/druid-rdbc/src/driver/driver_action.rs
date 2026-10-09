/// 驱动成功注销后执行资源清理的标准回调。
/// 对应 Java: java.sql.DriverAction。
/// 仅由注册该驱动的管理器在成功注销后通知实现，以便释放驱动资源。
pub trait DriverAction: Send + Sync {
    /// 通知实现其驱动已注销；无参数，无返回值。
    /// 对应 Java: DriverAction#deregister。
    fn deregister(&self);
}
