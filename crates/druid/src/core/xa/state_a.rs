use super::*;

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
}
