/// `javax.transaction.xa.XAResource` 的协议标志常量命名空间。
pub struct XaFlags;

impl XaFlags {
    /// 无特殊标志。
    pub const TMNOFLAGS: i32 = 0;
    /// 加入已有事务分支。
    pub const TMJOIN: i32 = 0x0000_0020;
    /// 恢复挂起的事务分支。
    pub const TMRESUME: i32 = 0x0000_0800;
    /// 事务分支操作成功。
    pub const TMSUCCESS: i32 = 0x0000_0004;
    /// 事务分支操作失败。
    pub const TMFAIL: i32 = 0x0000_0008;
    /// 挂起事务分支。
    pub const TMSUSPEND: i32 = 0x0000_0002;
    /// 一阶段提交标志。
    pub const TMONEPHASE: i32 = 0x4000_0000;
    /// 异步操作标志。
    pub const TMASYNC: i32 = 0x0200_0000;
    /// RM 的只读投票结果。
    pub const XA_RDONLY: i32 = 3;
}

/// Java 风格模块常量别名，便于迁移原有 `flags::TMNOFLAGS` 调用。
pub mod flags {
    /// 无特殊标志。
    pub const TMNOFLAGS: i32 = super::XaFlags::TMNOFLAGS;
    /// 加入已有事务分支。
    pub const TMJOIN: i32 = super::XaFlags::TMJOIN;
    /// 恢复挂起的事务分支。
    pub const TMRESUME: i32 = super::XaFlags::TMRESUME;
    /// 事务分支操作成功。
    pub const TMSUCCESS: i32 = super::XaFlags::TMSUCCESS;
    /// 事务分支操作失败。
    pub const TMFAIL: i32 = super::XaFlags::TMFAIL;
    /// 挂起事务分支。
    pub const TMSUSPEND: i32 = super::XaFlags::TMSUSPEND;
    /// 一阶段提交标志。
    pub const TMONEPHASE: i32 = super::XaFlags::TMONEPHASE;
    /// 异步操作标志。
    pub const TMASYNC: i32 = super::XaFlags::TMASYNC;
    /// RM 的只读投票结果。
    pub const XA_RDONLY: i32 = super::XaFlags::XA_RDONLY;
}
