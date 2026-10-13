use crate::error::RdbcError;

/// 与 Statement 执行独立的取消入口，可在 Statement 被另一个任务独占借用时持有。
///
/// 对应 Java: `java.sql.Statement#cancel`。实现必须验证 owner、语句及活动执行代次；
/// 不支持时返回 SQLState `0A000`，不得将丢弃执行 Future 当作服务器取消。
#[async_trait::async_trait]
pub trait StatementCancellation: Send + Sync {
    /// 请求取消获取该控制对象时对应的当前执行；取消确认/失败以 Result 返回。
    async fn cancel(&self) -> Result<(), RdbcError>;
}
