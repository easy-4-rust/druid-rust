use super::{XaError, XaPrepareResult, Xid};

/// 可由事务管理器调用的 XA 资源管理器协议。
/// 对应 Java: `javax.transaction.xa.XAResource`。
#[async_trait::async_trait]
pub trait XaResource: Send + Sync {
    /// 按 flags 将分支关联到资源管理器。
    ///
    /// 参数为完整 XID 和 XA start flags；成功返回 `Ok(())`，协议或驱动错误保留在 `XaError`。
    async fn start(&mut self, xid: &Xid, flags: i32) -> Result<(), XaError>;
    /// 按 flags 解除分支关联。
    ///
    /// 参数为完整 XID 和 end flags；成功返回 `Ok(())`，失败返回带 XA 错误码的错误。
    async fn end(&mut self, xid: &Xid, flags: i32) -> Result<(), XaError>;
    /// 请求第一阶段投票，保留 prepared 与 read-only 的区别。
    ///
    /// 参数为待准备的 XID；返回 `Ok` 表示等待第二阶段，`ReadOnly` 表示无需第二阶段。
    async fn prepare(&mut self, xid: &Xid) -> Result<XaPrepareResult, XaError>;
    /// 提交分支；`one_phase` 表示一阶段提交。
    ///
    /// 参数为完整 XID 和是否执行一阶段提交；失败返回资源管理器的原始 XA 错误。
    async fn commit(&mut self, xid: &Xid, one_phase: bool) -> Result<(), XaError>;
    /// 回滚指定分支。
    ///
    /// 参数为完整 XID；成功返回 `Ok(())`，失败返回资源管理器的原始 XA 错误。
    async fn rollback(&mut self, xid: &Xid) -> Result<(), XaError>;
    /// 清除资源管理器报告的启发式分支记录。
    ///
    /// 参数为需要清除的 XID；成功返回 `Ok(())`，失败保留 XA 错误码。
    async fn forget(&mut self, xid: &Xid) -> Result<(), XaError>;
    /// 按恢复扫描 flags 返回完整的可恢复分支标识。
    ///
    /// 参数为恢复扫描控制 flags；返回本次扫描得到的 XID 列表或结构化 XA 错误。
    async fn recover(&self, flags: i32) -> Result<Vec<Xid>, XaError>;
    /// 比较真实资源管理器身份，不能只比较诊断名称。
    ///
    /// 参数为待比较的资源；返回身份是否相同或比较过程中的 XA 错误。
    async fn is_same_rm(&self, other: &dyn XaResource) -> Result<bool, XaError>;
    /// 读取事务分支超时秒数。
    ///
    /// 无参数；返回当前有效超时，`0` 表示无超时或由实现按 XA 语义解释。
    async fn get_transaction_timeout(&self) -> Result<u64, XaError>;
    /// 设置事务分支超时；false 表示资源管理器未接受该设置。
    ///
    /// 参数为超时秒数；返回是否接受设置，错误表示资源管理器拒绝或操作失败。
    async fn set_transaction_timeout(&mut self, seconds: u64) -> Result<bool, XaError>;
}
