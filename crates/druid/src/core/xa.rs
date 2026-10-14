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
pub use druid_rdbc::data_source::{
    flags, XaError, XaErrorCode, XaFlags, XaPrepareResult, XaResource, XaResourceManagerIdentity,
    Xid, XID_MAX_BQUAL_LENGTH, XID_MAX_GTRID_LENGTH,
};

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
    /// 事务分支已 suspend，等待 `TMRESUME` 重新关联。
    Suspended,
}

impl fmt::Display for XaState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Idle => write!(f, "IDLE"),
            Self::Active => write!(f, "ACTIVE"),
            Self::Suspended => write!(f, "SUSPENDED"),
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
    /// 对应 `XAResource#start(Xid, TMJOIN)`
    Join,
    /// 对应 `XAResource#start(Xid, TMRESUME)`
    Resume,
    /// 对应 `XAResource#end(Xid, TMSUSPEND)`
    Suspend,
}

impl fmt::Display for XaOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start => write!(f, "start"),
            Self::End => write!(f, "end"),
            Self::Join => write!(f, "join"),
            Self::Resume => write!(f, "resume"),
            Self::Suspend => write!(f, "suspend"),
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

    /// 将已结束的分支重新关联到当前资源。
    ///
    /// 对应 Java: `XAResource#start(Xid, TMJOIN)`；只允许从已结束且尚未 prepare 的状态 join。
    pub fn join(&mut self) -> Result<(), XaStateTransitionError> {
        self.transition(XaOperation::Join, XaState::Active, &[XaState::Preparing])
    }

    /// 恢复此前 suspend 的事务分支。
    ///
    /// 对应 Java: `XAResource#start(Xid, TMRESUME)`。
    pub fn resume(&mut self) -> Result<(), XaStateTransitionError> {
        self.transition(XaOperation::Resume, XaState::Active, &[XaState::Suspended])
    }

    /// 暂停当前事务分支的关联，但保留其未完成状态。
    ///
    /// 对应 Java: `XAResource#end(Xid, TMSUSPEND)`。
    pub fn suspend(&mut self) -> Result<(), XaStateTransitionError> {
        self.transition(XaOperation::Suspend, XaState::Suspended, &[XaState::Active])
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
            // 一阶段提交只能发生在 end 之后、prepare 之前。
            &[XaState::Preparing][..]
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
                XaState::Suspended,
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

// XA protocol contracts are owned by druid-rdbc; this module retains the Druid state machine.

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
    fn one_phase_commit_after_prepare_is_rejected_without_state_change() {
        let xid = Xid::new(1, vec![1], vec![2]).unwrap();
        let mut tx = XaTransactionState::new(xid);
        tx.start().unwrap();
        tx.end().unwrap();
        tx.prepare().unwrap();
        let history_before = tx.history().to_vec();

        let error = tx.commit(true).unwrap_err();

        assert_eq!(error.operation(), XaOperation::Commit);
        assert_eq!(tx.state(), XaState::Prepared);
        assert_eq!(tx.history(), history_before);
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
