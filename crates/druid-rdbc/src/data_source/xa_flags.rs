/// `javax.transaction.xa.XAResource` 的协议标志常量命名空间。
pub struct XaFlags;

impl XaFlags {
    /// 无特殊标志。
    pub const TMNOFLAGS: i32 = 0;
    /// 加入已有事务分支。
    pub const TMJOIN: i32 = 0x0020_0000;
    /// 恢复挂起的事务分支。
    pub const TMRESUME: i32 = 0x0800_0000;
    /// 事务分支操作成功。
    pub const TMSUCCESS: i32 = 0x0400_0000;
    /// 事务分支操作失败。
    pub const TMFAIL: i32 = 0x2000_0000;
    /// 挂起事务分支。
    pub const TMSUSPEND: i32 = 0x0200_0000;
    /// 一阶段提交标志。
    pub const TMONEPHASE: i32 = 0x4000_0000;
    /// 开始 XA 恢复扫描。
    pub const TMSTARTRSCAN: i32 = 0x0100_0000;
    /// 结束 XA 恢复扫描。
    pub const TMENDRSCAN: i32 = 0x0080_0000;
    /// 正常 prepare 投票结果。
    pub const XA_OK: i32 = 0;
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
    /// 开始 XA 恢复扫描。
    pub const TMSTARTRSCAN: i32 = super::XaFlags::TMSTARTRSCAN;
    /// 结束 XA 恢复扫描。
    pub const TMENDRSCAN: i32 = super::XaFlags::TMENDRSCAN;
    /// 正常 prepare 投票结果。
    pub const XA_OK: i32 = super::XaFlags::XA_OK;
    /// RM 的只读投票结果。
    pub const XA_RDONLY: i32 = super::XaFlags::XA_RDONLY;
}
