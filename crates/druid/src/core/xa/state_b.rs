use super::*;

impl XaTransactionState {
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
    pub(crate) fn transition(
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
