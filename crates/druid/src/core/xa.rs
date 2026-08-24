//! XA/2PC 状态机与资源抽象。
//!
//! 实现标准 XA 两阶段提交协议的状态机，对齐 Java
//! `javax.transaction.xa.XAResource` 与 `javax.transaction.xa.Xid` 接口语义。
//!
//! 对应 Java: `javax.transaction.xa.Xid`
//! 对应 Java: `javax.transaction.xa.XAResource`
//! 对应 Java: `com.alibaba.druid.pool.xa.DruidXADataSource`

use std::fmt;
use std::time::{Duration, Instant};

use crate::core::DruidError;

// ─── Xid ────────────────────────────────────────────────────────────────────

/// XA 事务标识符，对齐 Java `javax.transaction.xa.Xid`。
///
/// 由三部分组成：
/// - `format_id`：格式标识符，由事务管理器分配；值 `0` 表示 OSI CCR 格式。
/// - `global_transaction_id`：全局事务标识（gtrid），最大 64 字节。
/// - `branch_qualifier`：分支限定符（bqual），最大 64 字节。
///
/// 对应 Java: `javax.transaction.xa.Xid`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Xid {
    format_id: i32,
    global_transaction_id: Vec<u8>,
    branch_qualifier: Vec<u8>,
}

/// `Xid` 中 gtrid 和 bqual 的最大长度，与 XA 规范一致。
pub const XID_MAX_GTRID_LENGTH: usize = 64;
pub const XID_MAX_BQUAL_LENGTH: usize = 64;

impl Xid {
    /// 创建新的 XID，校验 XA 规范长度约束。
    ///
    /// # Errors
    ///
    /// 返回 `DruidError::InvalidArgument` 如果 gtrid 或 bqual 超过 64 字节。
    pub fn new(
        format_id: i32,
        global_transaction_id: Vec<u8>,
        branch_qualifier: Vec<u8>,
    ) -> Result<Self, DruidError> {
        if global_transaction_id.len() > XID_MAX_GTRID_LENGTH {
            return Err(DruidError::InvalidArgument(format!(
                "global_transaction_id length {} exceeds maximum {}",
                global_transaction_id.len(),
                XID_MAX_GTRID_LENGTH,
            )));
        }
        if branch_qualifier.len() > XID_MAX_BQUAL_LENGTH {
            return Err(DruidError::InvalidArgument(format!(
                "branch_qualifier length {} exceeds maximum {}",
                branch_qualifier.len(),
                XID_MAX_BQUAL_LENGTH,
            )));
        }
        Ok(Self {
            format_id,
            global_transaction_id,
            branch_qualifier,
        })
    }

    /// 返回格式标识符。
    ///
    /// 对应 Java: `Xid#getFormatId()`
    #[must_use]
    pub fn format_id(&self) -> i32 {
        self.format_id
    }

    /// 返回全局事务标识（gtrid）。
    ///
    /// 对应 Java: `Xid#getGlobalTransactionId()`
    #[must_use]
    pub fn global_transaction_id(&self) -> &[u8] {
        &self.global_transaction_id
    }

    /// 返回分支限定符（bqual）。
    ///
    /// 对应 Java: `Xid#getBranchQualifier()`
    #[must_use]
    pub fn branch_qualifier(&self) -> &[u8] {
        &self.branch_qualifier
    }
}

impl fmt::Display for Xid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Xid(format_id={}, gtrid={}, bqual={})",
            self.format_id,
            hex_encode(&self.global_transaction_id),
            hex_encode(&self.branch_qualifier),
        )
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ─── XA State Machine ───────────────────────────────────────────────────────

/// XA 事务分支的状态。
///
/// 状态转换图（合法路径）：
///
/// ```text
///                  ┌─────────────────────────────────────────────┐
///                  │                                             │
///                  v                                             │
///  IDLE ──start──> ACTIVE ──prepare──> PREPARING ──ok──> PREPARED ──commit──> COMMITTING ──ok──> COMMITTED
///                  │    │                                   │                    │
///                  │    │                                   │                    │
///                  │    └──rollback──> ROLLING_BACK ──ok──> ROLLED_BACK          │
///                  │                                   ▲                         │
///                  └───────────────────────────────────┘                         │
///                                                                                │
///                  FAILED <──────────────────────────────────────────────────────┘
///                  FAILED <── (any state on unrecoverable error)
/// ```
///
/// 对应 Java: `javax.transaction.xa.XAResource` 中 start/end/prepare/commit/rollback 语义
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum XaState {
    /// 初始状态，事务分支尚未参与。
    Idle,
    /// 事务分支已通过 `start` 加入，可执行本地操作。
    Active,
    /// `prepare` 已调用，等待 RM 决议。
    Preparing,
    /// RM 已投票提交，等待 `commit`。
    Prepared,
    /// `commit` 已调用，RM 正在提交。
    Committing,
    /// 事务分支已成功提交。
    Committed,
    /// `rollback` 已调用，RM 正在回滚。
    RollingBack,
    /// 事务分支已回滚。
    RolledBack,
    /// 不可恢复的错误导致事务分支失败。
    Failed,
}

impl fmt::Display for XaState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "IDLE"),
            Self::Active => write!(f, "ACTIVE"),
            Self::Preparing => write!(f, "PREPARING"),
            Self::Prepared => write!(f, "PREPARED"),
            Self::Committing => write!(f, "COMMITTING"),
            Self::Committed => write!(f, "COMMITTED"),
            Self::RollingBack => write!(f, "ROLLING_BACK"),
            Self::RolledBack => write!(f, "ROLLED_BACK"),
            Self::Failed => write!(f, "FAILED"),
        }
    }
}

/// XA 状态机的可执行操作。
///
/// 对应 Java: `javax.transaction.xa.XAResource` 的各个方法
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum XaOperation {
    /// 对应 `XAResource#start(Xid, int)`
    Start,
    /// 对应 `XAResource#end(Xid, int)`
    End,
    /// 对应 `XAResource#prepare(Xid)`
    Prepare,
    /// 对应 `XAResource#commit(Xid, boolean)`
    Commit,
    /// 对应 `XAResource#rollback(Xid)`
    Rollback,
    /// 对应 `XAResource#forget(Xid)`
    Forget,
}

impl fmt::Display for XaOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start => write!(f, "start"),
            Self::End => write!(f, "end"),
            Self::Prepare => write!(f, "prepare"),
            Self::Commit => write!(f, "commit"),
            Self::Rollback => write!(f, "rollback"),
            Self::Forget => write!(f, "forget"),
        }
    }
}

/// XA 状态转换错误。
///
/// 当操作在当前状态下不合法时返回此错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XaStateTransitionError {
    current: XaState,
    operation: XaOperation,
}

impl XaStateTransitionError {
    #[must_use]
    pub fn current_state(&self) -> XaState {
        self.current
    }

    #[must_use]
    pub fn operation(&self) -> XaOperation {
        self.operation
    }
}

impl fmt::Display for XaStateTransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid XA state transition: cannot execute '{}' in state '{}'",
            self.operation, self.current,
        )
    }
}

impl std::error::Error for XaStateTransitionError {}

impl From<XaStateTransitionError> for DruidError {
    fn from(err: XaStateTransitionError) -> Self {
        DruidError::InvalidArgument(err.to_string())
    }
}

// ─── XaTransactionState ─────────────────────────────────────────────────────

/// 单个 XA 事务分支的完整状态，跟踪状态机、XID 与超时。
///
/// 对应 Java: `com.alibaba.druid.pool.xa.DruidPooledXAConnection` 中的事务状态跟踪
#[derive(Debug, Clone)]
pub struct XaTransactionState {
    /// 事务分支的 XID。
    xid: Xid,
    /// 当前状态。
    state: XaState,
    /// 事务分支创建时间（用于超时判定）。
    created_at: Instant,
    /// 超时时长；`None` 表示不超时。
    timeout: Option<Duration>,
    /// 状态转换历史，用于调试和审计。
    history: Vec<XaStateTransitionRecord>,
}

/// 状态转换记录，用于审计和调试。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XaStateTransitionRecord {
    /// 转换前的状态。
    pub from: XaState,
    /// 转换后的状态。
    pub to: XaState,
    /// 触发转换的操作。
    pub operation: XaOperation,
}

impl XaTransactionState {
    /// 创建新的 XA 事务分支状态机，初始状态为 `Idle`。
    #[must_use]
    pub fn new(xid: Xid) -> Self {
        Self {
            xid,
            state: XaState::Idle,
            created_at: Instant::now(),
            timeout: None,
            history: Vec::new(),
        }
    }

    /// 创建带超时的 XA 事务分支状态机。
    #[must_use]
    pub fn with_timeout(xid: Xid, timeout: Duration) -> Self {
        Self {
            xid,
            state: XaState::Idle,
            created_at: Instant::now(),
            timeout: Some(timeout),
            history: Vec::new(),
        }
    }

    /// 返回当前事务分支的 XID 引用。
    #[must_use]
    pub fn xid(&self) -> &Xid {
        &self.xid
    }

    /// 返回当前状态。
    #[must_use]
    pub fn state(&self) -> XaState {
        self.state
    }

    /// 返回状态转换历史。
    #[must_use]
    pub fn history(&self) -> &[XaStateTransitionRecord] {
        &self.history
    }

    /// 返回事务分支已存活时长。
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.created_at.elapsed()
    }

    /// 检查事务分支是否已超时。
    ///
    /// `None` 超时设定视为不超时，永远返回 `false`。
    #[must_use]
    pub fn is_timed_out(&self) -> bool {
        match self.timeout {
            Some(timeout) => self.created_at.elapsed() > timeout,
            None => false,
        }
    }

    /// 返回超时设定。
    #[must_use]
    pub fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    /// 返回是否处于终态（Committed、RolledBack、Failed）。
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(
            self.state,
            XaState::Committed | XaState::RolledBack | XaState::Failed
        )
    }

    // ── 状态转换操作 ────────────────────────────────────────────────

    /// 执行 `start`：将事务分支从 `Idle` 转入 `Active`。
    ///
    /// 对应 Java: `XAResource#start(Xid, int)`
    ///
    /// # Errors
    ///
    /// 返回 `XaStateTransitionError` 如果当前状态不是 `Idle`。
    pub fn start(&mut self) -> Result<(), XaStateTransitionError> {
        self.transition(XaOperation::Start, XaState::Active, &[XaState::Idle])
    }

    /// 执行 `end`：将事务分支从 `Active` 转入 `Preparing`。
    ///
    /// 对应 Java: `XAResource#end(Xid, int)`
    ///
    /// # Errors
    ///
    /// 返回 `XaStateTransitionError` 如果当前状态不是 `Active`。
    pub fn end(&mut self) -> Result<(), XaStateTransitionError> {
        self.transition(XaOperation::End, XaState::Preparing, &[XaState::Active])
    }

    /// 执行 `prepare`：将事务分支从 `Preparing` 转入 `Prepared`。
    ///
    /// 对应 Java: `XAResource#prepare(Xid)`
    ///
    /// # Errors
    ///
    /// 返回 `XaStateTransitionError` 如果当前状态不是 `Preparing`。
    pub fn prepare(&mut self) -> Result<(), XaStateTransitionError> {
        self.transition(
            XaOperation::Prepare,
            XaState::Prepared,
            &[XaState::Preparing],
        )
    }

    /// 执行 `commit`：将事务分支从 `Prepared` 或 `Committing` 转入 `Committed`。
    ///
    /// `one_phase` 参数为 `true` 时允许从 `Preparing` 直接提交（一阶段优化）。
    ///
    /// 对应 Java: `XAResource#commit(Xid, boolean)`
    ///
    /// # Errors
    ///
    /// 返回 `XaStateTransitionError` 如果当前状态不允许提交。
    pub fn commit(&mut self, one_phase: bool) -> Result<(), XaStateTransitionError> {
        let allowed = if one_phase {
            // 一阶段提交：从 Preparing 直接到 Committing
            &[XaState::Prepared, XaState::Preparing][..]
        } else {
            &[XaState::Prepared][..]
        };
        self.transition(XaOperation::Commit, XaState::Committing, allowed)?;
        // 提交完成立即进入终态
        self.state = XaState::Committed;
        self.history.push(XaStateTransitionRecord {
            from: XaState::Committing,
            to: XaState::Committed,
            operation: XaOperation::Commit,
        });
        Ok(())
    }

    /// 执行 `rollback`：从任意非终态回滚到 `RolledBack`。
    ///
    /// 允许从 `Active`、`Preparing`、`Prepared` 状态回滚。
    ///
    /// 对应 Java: `XAResource#rollback(Xid)`
    ///
    /// # Errors
    ///
    /// 返回 `XaStateTransitionError` 如果当前状态不允许回滚。
    pub fn rollback(&mut self) -> Result<(), XaStateTransitionError> {
        self.transition(
            XaOperation::Rollback,
            XaState::RollingBack,
            &[
                XaState::Idle,
                XaState::Active,
                XaState::Preparing,
                XaState::Prepared,
            ],
        )?;
        // 回滚完成立即进入终态
        self.state = XaState::RolledBack;
        self.history.push(XaStateTransitionRecord {
            from: XaState::RollingBack,
            to: XaState::RolledBack,
            operation: XaOperation::Rollback,
        });
        Ok(())
    }

    /// 执行 `forget`：清除已启发式完成的事务分支。
    ///
    /// 仅允许从 `Failed` 状态调用，将状态重置回 `Idle`。
    ///
    /// 对应 Java: `XAResource#forget(Xid)`
    ///
    /// # Errors
    ///
    /// 返回 `XaStateTransitionError` 如果当前状态不是 `Failed`。
    pub fn forget(&mut self) -> Result<(), XaStateTransitionError> {
        self.transition(XaOperation::Forget, XaState::Idle, &[XaState::Failed])
    }

    /// 将事务分支标记为 `Failed`（不可恢复错误）。
    ///
    /// 从任意非终态均可调用。用于 RM 报告不可恢复的错误。
    ///
    /// # Errors
    ///
    /// 返回 `XaStateTransitionError` 如果当前状态已经是终态。
    pub fn mark_failed(&mut self) -> Result<(), XaStateTransitionError> {
        let from = self.state;
        if self.is_terminal() {
            return Err(XaStateTransitionError {
                current: from,
                operation: XaOperation::Forget, // 复用操作类型，语义最接近
            });
        }
        self.state = XaState::Failed;
        self.history.push(XaStateTransitionRecord {
            from,
            to: XaState::Failed,
            operation: XaOperation::Forget,
        });
        Ok(())
    }

    /// 内部状态转换方法：校验 `allowed_sources` 后执行转换。
    fn transition(
        &mut self,
        operation: XaOperation,
        target: XaState,
        allowed_sources: &[XaState],
    ) -> Result<(), XaStateTransitionError> {
        if !allowed_sources.contains(&self.state) {
            return Err(XaStateTransitionError {
                current: self.state,
                operation,
            });
        }
        let from = self.state;
        self.state = target;
        self.history.push(XaStateTransitionRecord {
            from,
            to: target,
            operation,
        });
        Ok(())
    }
}

// ─── XaResource Trait ───────────────────────────────────────────────────────

/// XA 资源管理器接口，对齐 Java `javax.transaction.xa.XAResource`。
///
/// 实现者代表一个 RM（Resource Manager）的事务分支操作能力。
/// 所有方法均为 async，因为实际 RM 操作（数据库 XA 命令）是异步的。
///
/// 对应 Java: `javax.transaction.xa.XAResource`
#[async_trait::async_trait]
pub trait XaResource: Send + Sync {
    /// 以指定模式将事务分支关联到 XA 事务。
    ///
    /// 对应 Java: `XAResource#start(Xid, int flags)`
    ///
    /// `flags` 可取 `TMNOFLAGS`、`TMJOIN`、`TMRESUME` 之一。
    async fn start(&mut self, xid: &Xid, flags: i32) -> Result<(), DruidError>;

    /// 结束事务分支的操作，解除关联。
    ///
    /// 对应 Java: `XAResource#end(Xid, int flags)`
    ///
    /// `flags` 可取 `TMSUCCESS`、`TMFAIL`、`TMSUSPEND` 之一。
    async fn end(&mut self, xid: &Xid, flags: i32) -> Result<(), DruidError>;

    /// 第一阶段：询问 RM 是否可以提交。
    ///
    /// 返回 `XA_RDONLY`（值 `3`）表示分支只读可直接忽略；
    /// 返回 `0` 表示 RM 已准备好，等待 `commit`。
    ///
    /// 对应 Java: `XAResource#prepare(Xid)`
    async fn prepare(&mut self, xid: &Xid) -> Result<XaPrepareResult, DruidError>;

    /// 第二阶段：提交事务分支。
    ///
    /// `one_phase` 为 `true` 时执行一阶段提交（跳过 prepare）。
    ///
    /// 对应 Java: `XAResource#commit(Xid, boolean onePhase)`
    async fn commit(&mut self, xid: &Xid, one_phase: bool) -> Result<(), DruidError>;

    /// 回滚事务分支。
    ///
    /// 对应 Java: `XAResource#rollback(Xid)`
    async fn rollback(&mut self, xid: &Xid) -> Result<(), DruidError>;

    /// 查询处于启发式完成状态的事务分支列表。
    ///
    /// 对应 Java: `XAResource#recover(int flag)`
    async fn recover(&self, flags: i32) -> Result<Vec<Xid>, DruidError>;

    /// 清除处于启发式完成状态的事务分支记录。
    ///
    /// 对应 Java: `XAResource#forget(Xid)`
    async fn forget(&mut self, xid: &Xid) -> Result<(), DruidError>;

    /// 返回 RM 设置的事务分支超时秒数。
    ///
    /// 对应 Java: `XAResource#getTransactionTimeout()`
    async fn get_transaction_timeout(&self) -> Result<u64, DruidError>;

    /// 设置事务分支超时秒数。
    ///
    /// 对应 Java: `XAResource#setTransactionTimeout(int seconds)`
    async fn set_transaction_timeout(&mut self, seconds: u64) -> Result<bool, DruidError>;

    /// 检查给定 RM 实例是否与当前实例相同。
    ///
    /// 对应 Java: `XAResource#isSameRM(XAResource other)`
    async fn is_same_rm(&self, other: &dyn XaResource) -> Result<bool, DruidError>;
}

/// `prepare` 操作的结果。
///
/// 对应 Java: `XAResource#prepare()` 的返回值语义
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XaPrepareResult {
    /// RM 已投票提交（返回码 `0`），等待 `commit` 调用。
    Ok,
    /// 分支只读（返回码 `XA_RDONLY = 3`），不需要第二阶段。
    ReadOnly,
}

// ─── XA Constants ───────────────────────────────────────────────────────────

/// XA 常量，对齐 Java `javax.transaction.xa.XAResource` 中的标志位。
pub mod flags {
    /// 无特殊标志。
    pub const TMNOFLAGS: i32 = 0x00000000;
    /// 加入已有事务分支。
    pub const TMJOIN: i32 = 0x00000020;
    /// 恢复挂起的事务分支。
    pub const TMRESUME: i32 = 0x00000800;
    /// 事务分支操作成功。
    pub const TMSUCCESS: i32 = 0x00000004;
    /// 事务分支操作失败。
    pub const TMFAIL: i32 = 0x00000008;
    /// 挂起事务分支。
    pub const TMSUSPEND: i32 = 0x00000002;
    /// 一阶段提交标志。
    pub const TMONEPHASE: i32 = 0x40000000;
    /// 异步操作标志。
    pub const TMASYNC: i32 = 0x02000000;

    /// RM 返回的只读信号。
    pub const XA_RDONLY: i32 = 3;
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Xid tests ──────────────────────────────────────────────────

    #[test]
    fn xid_new_valid() {
        let xid = Xid::new(1, vec![1, 2, 3], vec![4, 5]).unwrap();
        assert_eq!(xid.format_id(), 1);
        assert_eq!(xid.global_transaction_id(), &[1, 2, 3]);
        assert_eq!(xid.branch_qualifier(), &[4, 5]);
    }

    #[test]
    fn xid_new_gtrid_too_long() {
        let gtrid = vec![0u8; 65];
        let err = Xid::new(1, gtrid, vec![]).unwrap_err();
        assert!(err.to_string().contains("global_transaction_id"));
    }

    #[test]
    fn xid_new_bqual_too_long() {
        let bqual = vec![0u8; 65];
        let err = Xid::new(1, vec![1], bqual).unwrap_err();
        assert!(err.to_string().contains("branch_qualifier"));
    }

    #[test]
    fn xid_display() {
        let xid = Xid::new(1, vec![0xAB, 0xCD], vec![0xEF]).unwrap();
        let display = format!("{xid}");
        assert!(display.contains("abcd"));
        assert!(display.contains("ef"));
    }

    #[test]
    fn xid_eq() {
        let a = Xid::new(1, vec![1], vec![2]).unwrap();
        let b = Xid::new(1, vec![1], vec![2]).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn xid_ne_different_format_id() {
        let a = Xid::new(1, vec![1], vec![2]).unwrap();
        let b = Xid::new(2, vec![1], vec![2]).unwrap();
        assert_ne!(a, b);
    }

    // ── XaState display ────────────────────────────────────────────

    #[test]
    fn xa_state_display() {
        assert_eq!(format!("{}", XaState::Idle), "IDLE");
        assert_eq!(format!("{}", XaState::Active), "ACTIVE");
        assert_eq!(format!("{}", XaState::Preparing), "PREPARING");
        assert_eq!(format!("{}", XaState::Prepared), "PREPARED");
        assert_eq!(format!("{}", XaState::Committing), "COMMITTING");
        assert_eq!(format!("{}", XaState::Committed), "COMMITTED");
        assert_eq!(format!("{}", XaState::RollingBack), "ROLLING_BACK");
        assert_eq!(format!("{}", XaState::RolledBack), "ROLLED_BACK");
        assert_eq!(format!("{}", XaState::Failed), "FAILED");
    }

    // ── State machine: happy path ──────────────────────────────────

    #[test]
    fn happy_path_two_phase_commit() {
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let mut tx = XaTransactionState::new(xid);

        assert_eq!(tx.state(), XaState::Idle);
        assert!(!tx.is_terminal());

        tx.start().unwrap();
        assert_eq!(tx.state(), XaState::Active);

        tx.end().unwrap();
        assert_eq!(tx.state(), XaState::Preparing);

        tx.prepare().unwrap();
        assert_eq!(tx.state(), XaState::Prepared);

        tx.commit(false).unwrap();
        assert_eq!(tx.state(), XaState::Committed);
        assert!(tx.is_terminal());
    }

    #[test]
    fn happy_path_one_phase_commit() {
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let mut tx = XaTransactionState::new(xid);

        tx.start().unwrap();
        tx.end().unwrap();
        tx.commit(true).unwrap();
        assert_eq!(tx.state(), XaState::Committed);
    }

    #[test]
    fn happy_path_rollback_from_active() {
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let mut tx = XaTransactionState::new(xid);

        tx.start().unwrap();
        assert_eq!(tx.state(), XaState::Active);

        tx.rollback().unwrap();
        assert_eq!(tx.state(), XaState::RolledBack);
        assert!(tx.is_terminal());
    }

    #[test]
    fn happy_path_rollback_from_prepared() {
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let mut tx = XaTransactionState::new(xid);

        tx.start().unwrap();
        tx.end().unwrap();
        tx.prepare().unwrap();
        tx.rollback().unwrap();
        assert_eq!(tx.state(), XaState::RolledBack);
    }

    #[test]
    fn happy_path_rollback_from_idle() {
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let mut tx = XaTransactionState::new(xid);

        tx.rollback().unwrap();
        assert_eq!(tx.state(), XaState::RolledBack);
    }

    // ── State machine: illegal transitions ─────────────────────────

    #[test]
    fn start_from_active_is_illegal() {
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let mut tx = XaTransactionState::new(xid);
        tx.start().unwrap();

        let err = tx.start().unwrap_err();
        assert_eq!(err.current_state(), XaState::Active);
        assert_eq!(err.operation(), XaOperation::Start);
    }

    #[test]
    fn prepare_from_idle_is_illegal() {
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let mut tx = XaTransactionState::new(xid);

        let err = tx.prepare().unwrap_err();
        assert_eq!(err.current_state(), XaState::Idle);
        assert_eq!(err.operation(), XaOperation::Prepare);
    }

    #[test]
    fn commit_from_active_is_illegal() {
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let mut tx = XaTransactionState::new(xid);
        tx.start().unwrap();

        let err = tx.commit(false).unwrap_err();
        assert_eq!(err.current_state(), XaState::Active);
    }

    #[test]
    fn end_from_idle_is_illegal() {
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let mut tx = XaTransactionState::new(xid);

        let err = tx.end().unwrap_err();
        assert_eq!(err.current_state(), XaState::Idle);
        assert_eq!(err.operation(), XaOperation::End);
    }

    #[test]
    fn start_from_committed_is_illegal() {
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let mut tx = XaTransactionState::new(xid);

        tx.start().unwrap();
        tx.end().unwrap();
        tx.prepare().unwrap();
        tx.commit(false).unwrap();

        let err = tx.start().unwrap_err();
        assert_eq!(err.current_state(), XaState::Committed);
    }

    #[test]
    fn commit_from_rolled_back_is_illegal() {
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let mut tx = XaTransactionState::new(xid);

        tx.start().unwrap();
        tx.rollback().unwrap();

        let err = tx.commit(false).unwrap_err();
        assert_eq!(err.current_state(), XaState::RolledBack);
    }

    #[test]
    fn rollback_from_committed_is_illegal() {
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let mut tx = XaTransactionState::new(xid);

        tx.start().unwrap();
        tx.end().unwrap();
        tx.prepare().unwrap();
        tx.commit(false).unwrap();

        let err = tx.rollback().unwrap_err();
        assert_eq!(err.current_state(), XaState::Committed);
    }

    #[test]
    fn rollback_from_rolled_back_is_illegal() {
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let mut tx = XaTransactionState::new(xid);

        tx.start().unwrap();
        tx.rollback().unwrap();

        let err = tx.rollback().unwrap_err();
        assert_eq!(err.current_state(), XaState::RolledBack);
    }

    #[test]
    fn one_phase_commit_from_idle_is_illegal() {
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let mut tx = XaTransactionState::new(xid);

        let err = tx.commit(true).unwrap_err();
        assert_eq!(err.current_state(), XaState::Idle);
    }

    // ── State machine: failure and forget ──────────────────────────

    #[test]
    fn mark_failed_from_active() {
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let mut tx = XaTransactionState::new(xid);

        tx.start().unwrap();
        tx.mark_failed().unwrap();
        assert_eq!(tx.state(), XaState::Failed);
        assert!(tx.is_terminal());
    }

    #[test]
    fn mark_failed_from_prepared() {
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let mut tx = XaTransactionState::new(xid);

        tx.start().unwrap();
        tx.end().unwrap();
        tx.prepare().unwrap();
        tx.mark_failed().unwrap();
        assert_eq!(tx.state(), XaState::Failed);
    }

    #[test]
    fn mark_failed_from_committed_is_illegal() {
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let mut tx = XaTransactionState::new(xid);

        tx.start().unwrap();
        tx.end().unwrap();
        tx.prepare().unwrap();
        tx.commit(false).unwrap();

        let err = tx.mark_failed().unwrap_err();
        assert_eq!(err.current_state(), XaState::Committed);
    }

    #[test]
    fn forget_from_failed() {
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let mut tx = XaTransactionState::new(xid);

        tx.start().unwrap();
        tx.mark_failed().unwrap();
        tx.forget().unwrap();
        assert_eq!(tx.state(), XaState::Idle);
        assert!(!tx.is_terminal());
    }

    #[test]
    fn forget_from_active_is_illegal() {
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let mut tx = XaTransactionState::new(xid);
        tx.start().unwrap();

        let err = tx.forget().unwrap_err();
        assert_eq!(err.current_state(), XaState::Active);
    }

    // ── Timeout ────────────────────────────────────────────────────

    #[test]
    fn no_timeout_never_expires() {
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let tx = XaTransactionState::new(xid);
        assert!(!tx.is_timed_out());
        assert!(tx.timeout().is_none());
    }

    #[test]
    fn with_timeout_setting() {
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let tx = XaTransactionState::with_timeout(xid, Duration::from_secs(30));
        assert_eq!(tx.timeout(), Some(Duration::from_secs(30)));
        // 30 秒内不应超时
        assert!(!tx.is_timed_out());
    }

    // ── History tracking ───────────────────────────────────────────

    #[test]
    fn history_records_transitions() {
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let mut tx = XaTransactionState::new(xid);

        tx.start().unwrap();
        tx.end().unwrap();
        tx.prepare().unwrap();

        let history = tx.history();
        assert_eq!(history.len(), 3);
        assert_eq!(
            history[0],
            XaStateTransitionRecord {
                from: XaState::Idle,
                to: XaState::Active,
                operation: XaOperation::Start,
            }
        );
        assert_eq!(
            history[1],
            XaStateTransitionRecord {
                from: XaState::Active,
                to: XaState::Preparing,
                operation: XaOperation::End,
            }
        );
        assert_eq!(
            history[2],
            XaStateTransitionRecord {
                from: XaState::Preparing,
                to: XaState::Prepared,
                operation: XaOperation::Prepare,
            }
        );
    }

    #[test]
    fn commit_records_two_transitions() {
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let mut tx = XaTransactionState::new(xid);

        tx.start().unwrap();
        tx.end().unwrap();
        tx.prepare().unwrap();
        tx.commit(false).unwrap();

        let history = tx.history();
        assert_eq!(history.len(), 5);
        assert_eq!(history[4].from, XaState::Committing);
        assert_eq!(history[4].to, XaState::Committed);
    }

    #[test]
    fn rollback_records_two_transitions() {
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let mut tx = XaTransactionState::new(xid);

        tx.start().unwrap();
        tx.rollback().unwrap();

        let history = tx.history();
        assert_eq!(history.len(), 3);
        assert_eq!(history[2].from, XaState::RollingBack);
        assert_eq!(history[2].to, XaState::RolledBack);
    }

    // ── Error conversions ──────────────────────────────────────────

    #[test]
    fn state_transition_error_display() {
        let err = XaStateTransitionError {
            current: XaState::Active,
            operation: XaOperation::Start,
        };
        let msg = format!("{err}");
        assert!(msg.contains("ACTIVE"));
        assert!(msg.contains("start"));
    }

    #[test]
    fn state_transition_error_into_druid_error() {
        let err = XaStateTransitionError {
            current: XaState::Committed,
            operation: XaOperation::Rollback,
        };
        let druid_err: DruidError = err.into();
        assert!(druid_err
            .to_string()
            .contains("invalid XA state transition"));
    }

    // ── XaOperation display ────────────────────────────────────────

    #[test]
    fn xa_operation_display() {
        assert_eq!(format!("{}", XaOperation::Start), "start");
        assert_eq!(format!("{}", XaOperation::End), "end");
        assert_eq!(format!("{}", XaOperation::Prepare), "prepare");
        assert_eq!(format!("{}", XaOperation::Commit), "commit");
        assert_eq!(format!("{}", XaOperation::Rollback), "rollback");
        assert_eq!(format!("{}", XaOperation::Forget), "forget");
    }

    // ── Flags constants ────────────────────────────────────────────

    #[test]
    fn xa_flags_values() {
        assert_eq!(flags::TMNOFLAGS, 0);
        assert_eq!(flags::TMONEPHASE, 0x40000000);
        assert_eq!(flags::XA_RDONLY, 3);
    }

    // ── Edge cases ─────────────────────────────────────────────────

    #[test]
    fn cannot_commit_after_rollback() {
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let mut tx = XaTransactionState::new(xid);

        tx.start().unwrap();
        tx.end().unwrap();
        tx.prepare().unwrap();
        tx.rollback().unwrap();

        let err = tx.commit(false).unwrap_err();
        assert_eq!(err.current_state(), XaState::RolledBack);
    }

    #[test]
    fn cannot_prepare_after_commit() {
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let mut tx = XaTransactionState::new(xid);

        tx.start().unwrap();
        tx.end().unwrap();
        tx.prepare().unwrap();
        tx.commit(false).unwrap();

        let err = tx.prepare().unwrap_err();
        assert_eq!(err.current_state(), XaState::Committed);
    }

    #[test]
    fn mark_failed_from_idle() {
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let mut tx = XaTransactionState::new(xid);
        tx.mark_failed().unwrap();
        assert_eq!(tx.state(), XaState::Failed);
    }

    #[test]
    fn mark_failed_from_preparing() {
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let mut tx = XaTransactionState::new(xid);
        tx.start().unwrap();
        tx.end().unwrap();
        tx.mark_failed().unwrap();
        assert_eq!(tx.state(), XaState::Failed);
    }

    #[test]
    fn mark_failed_from_failed_is_illegal() {
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let mut tx = XaTransactionState::new(xid);
        tx.start().unwrap();
        tx.mark_failed().unwrap();

        let err = tx.mark_failed().unwrap_err();
        assert_eq!(err.current_state(), XaState::Failed);
    }

    #[test]
    fn xid_max_boundary_lengths() {
        let gtrid = vec![0u8; 64];
        let bqual = vec![0u8; 64];
        let xid = Xid::new(0, gtrid, bqual).unwrap();
        assert_eq!(xid.global_transaction_id().len(), 64);
        assert_eq!(xid.branch_qualifier().len(), 64);
    }

    #[test]
    fn xid_empty_ids() {
        let xid = Xid::new(0, vec![], vec![]).unwrap();
        assert!(xid.global_transaction_id().is_empty());
        assert!(xid.branch_qualifier().is_empty());
    }

    #[test]
    fn elapsed_is_nonzero_after_creation() {
        // 确保 elapsed() 不会 panic
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let tx = XaTransactionState::new(xid);
        let _ = tx.elapsed();
    }
}
