/// XA 规范错误码，保留事务管理器或驱动给出的原始数值。
/// 对应 Java: `javax.transaction.xa.XAException#errorCode`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct XaErrorCode(pub i32);

impl XaErrorCode {
    /// 异步 XA 操作结果尚未确定。
    pub const XAER_ASYNC: Self = Self(-2);
    /// 资源管理器发生未分类错误。
    pub const XAER_RMERR: Self = Self(-3);
    /// 资源管理器不可用。
    pub const XAER_RMFAIL: Self = Self(-7);
    /// 事务分支回滚。
    pub const XA_RBROLLBACK: Self = Self(100);
    /// 分支因通信错误回滚。
    pub const XA_RBCOMMFAIL: Self = Self(101);
    /// 分支因死锁回滚。
    pub const XA_RBDEADLOCK: Self = Self(102);
    /// 分支因完整性约束回滚。
    pub const XA_RBINTEGRITY: Self = Self(103);
    /// 分支因其他原因回滚。
    pub const XA_RBOTHER: Self = Self(104);
    /// 分支因协议错误回滚。
    pub const XA_RBPROTO: Self = Self(105);
    /// 输入参数无效。
    pub const XAER_INVAL: Self = Self(-5);
    /// 当前资源管理器不支持该操作。
    pub const XAER_NOTA: Self = Self(-4);
    /// XA 协议调用顺序非法。
    pub const XAER_PROTO: Self = Self(-6);
    /// XID 已存在。
    pub const XAER_DUPID: Self = Self(-8);
    /// 当前操作发生在资源管理器外部。
    pub const XAER_OUTSIDE: Self = Self(-9);
    /// 资源管理器发生启发式混合结果。
    pub const XA_HEURMIX: Self = Self(5);
    /// 资源管理器报告启发式提交。
    pub const XA_HEURCOM: Self = Self(7);
    /// 资源管理器报告启发式回滚。
    pub const XA_HEURRB: Self = Self(6);
    /// 资源管理器报告启发式危险状态。
    pub const XA_HEURHAZ: Self = Self(8);
    /// 分支因超时而回滚。
    pub const XA_RBTIMEOUT: Self = Self(106);
}
