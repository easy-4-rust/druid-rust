use druid_rdbc::statement::Statement;
use std::sync::Arc;

/// 从共享逻辑 owner 恢复可操作的标准语句句柄。
/// 对应 Java: java.sql.ResultSet#getStatement。
pub(crate) trait LogicalStatementOwner: Send + Sync {
    /// 消费一个共享 owner 引用，返回保留 Prepared/Callable 叶子的标准句柄。
    /// 参数为拥有型 Arc；返回同一逻辑对象的语句。
    fn statement_handle(self: Arc<Self>) -> Box<dyn Statement>;
}
