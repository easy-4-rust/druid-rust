/// XA prepare 阶段的标准投票结果。
/// 对应 Java: `javax.transaction.xa.XAResource#prepare` 返回码。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XaPrepareResult {
    /// 资源管理器已持久化分支，等待第二阶段决议。
    Ok,
    /// 分支只读，不需要第二阶段操作。
    ReadOnly,
}
