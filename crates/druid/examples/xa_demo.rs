#![allow(clippy::unused_async)]
//! XA/2PC 使用示例：跨两个资源的分布式事务。
//!
//! 运行：cargo run -p druid --example `xa_demo`
//!
//! 对应 Java: javax.transaction.xa.{Xid, `XAResource`} +
//! com.alibaba.druid.pool.xa.DruidXADataSource 的协议层。

use druid::core::xa::{
    flags as xa_flags, XaError, XaErrorCode, XaPrepareResult, XaResource, XaState,
    XaTransactionState, Xid,
};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

/// 示例资源：内存账本。实现 `XaResource` 后即可参与 2PC。
/// 真实场景中这里是 `MySQL` XA / `PostgreSQL` prepared transaction 等驱动适配器。
struct InMemoryLedger {
    name: &'static str,
    committed: Mutex<HashMap<Xid, String>>,
    // xid -> (状态机, 预提交数据)
    branches: Mutex<HashMap<Xid, (XaTransactionState, String, bool)>>,
}

impl InMemoryLedger {
    fn new(name: &'static str) -> Self {
        Self {
            name,
            committed: Mutex::new(HashMap::new()),
            branches: Mutex::new(HashMap::new()),
        }
    }

    /// 在事务分支内登记一条预提交数据（业务操作，非 XA 协议方法）。
    async fn stage(&self, xid: &Xid, value: String) -> Result<(), druid::core::DruidError> {
        let mut branches = self.branches.lock().unwrap();
        let (state, staged, _) = branches.get_mut(xid).ok_or_else(|| {
            druid::core::DruidError::InvalidArgument(format!("branch not active: {}", self.name))
        })?;
        if state.state() != XaState::Active {
            return Err(druid::core::DruidError::InvalidArgument(format!(
                "branch {} not in Active state",
                self.name
            )));
        }
        staged.push_str(&value);
        Ok(())
    }
}

#[async_trait::async_trait]
impl XaResource for InMemoryLedger {
    async fn start(&mut self, xid: &Xid, flags: i32) -> Result<(), XaError> {
        let mut branches = self.branches.lock().unwrap();
        match flags {
            xa_flags::TMNOFLAGS => {
                if branches.contains_key(xid) {
                    return Err(XaError::new(
                        XaErrorCode::XAER_DUPID,
                        "XA branch already exists",
                    ));
                }
                let mut state = XaTransactionState::new(xid.clone());
                state
                    .start()
                    .map_err(|error| XaError::new(XaErrorCode::XAER_PROTO, error.to_string()))?;
                branches.insert(xid.clone(), (state, String::new(), false));
                Ok(())
            }
            xa_flags::TMJOIN | xa_flags::TMRESUME => {
                let (state, _, _) = branches
                    .get_mut(xid)
                    .ok_or_else(|| XaError::new(XaErrorCode::XAER_NOTA, "XA branch not found"))?;
                let result = if flags == xa_flags::TMJOIN {
                    state.join()
                } else {
                    state.resume()
                };
                result.map_err(|error| XaError::new(XaErrorCode::XAER_PROTO, error.to_string()))
            }
            _ => Err(XaError::new(
                XaErrorCode::XAER_INVAL,
                format!("invalid XA start flags: {flags:#x}"),
            )),
        }
    }

    async fn end(&mut self, xid: &Xid, flags: i32) -> Result<(), XaError> {
        if !matches!(
            flags,
            xa_flags::TMSUCCESS | xa_flags::TMFAIL | xa_flags::TMSUSPEND
        ) {
            return Err(XaError::new(
                XaErrorCode::XAER_INVAL,
                format!("unsupported XA end flags: {flags:#x}"),
            ));
        }
        let mut branches = self.branches.lock().unwrap();
        let (state, _, rollback_only) = branches
            .get_mut(xid)
            .ok_or_else(|| XaError::new(XaErrorCode::XAER_NOTA, "XA branch not found"))?;
        match flags {
            xa_flags::TMSUCCESS => state
                .end()
                .map_err(|error| XaError::new(XaErrorCode::XAER_PROTO, error.to_string())),
            xa_flags::TMFAIL => {
                state
                    .end()
                    .map_err(|error| XaError::new(XaErrorCode::XAER_PROTO, error.to_string()))?;
                *rollback_only = true;
                Ok(())
            }
            xa_flags::TMSUSPEND => state
                .suspend()
                .map_err(|error| XaError::new(XaErrorCode::XAER_PROTO, error.to_string())),
            _ => unreachable!("end flags were validated before state access"),
        }
    }

    async fn prepare(&mut self, xid: &Xid) -> Result<XaPrepareResult, XaError> {
        let mut branches = self.branches.lock().unwrap();
        let (is_read_only, rollback_only) = {
            let (state, staged, rollback_only) = branches
                .get_mut(xid)
                .ok_or_else(|| XaError::new(XaErrorCode::XAER_NOTA, "XA branch not found"))?;
            state
                .prepare()
                .map_err(|error| XaError::new(XaErrorCode::XAER_PROTO, error.to_string()))?;
            (staged.is_empty(), *rollback_only)
        };
        if rollback_only {
            branches.remove(xid);
            return Err(XaError::new(
                XaErrorCode::XA_RBROLLBACK,
                "XA branch was ended with TMFAIL",
            ));
        }
        if is_read_only {
            branches.remove(xid);
            return Ok(XaPrepareResult::ReadOnly);
        }
        Ok(XaPrepareResult::Ok)
    }

    async fn commit(&mut self, xid: &Xid, one_phase: bool) -> Result<(), XaError> {
        let mut branches = self.branches.lock().unwrap();
        let staged = {
            let (state, staged, rollback_only) = branches
                .get_mut(xid)
                .ok_or_else(|| XaError::new(XaErrorCode::XAER_NOTA, "XA branch not found"))?;
            if *rollback_only {
                return Err(XaError::new(
                    XaErrorCode::XA_RBROLLBACK,
                    "XA branch was ended with TMFAIL",
                ));
            }
            state
                .commit(one_phase)
                .map_err(|error| XaError::new(XaErrorCode::XAER_PROTO, error.to_string()))?;
            staged.clone()
        };
        branches.remove(xid);
        self.committed.lock().unwrap().insert(xid.clone(), staged);
        Ok(())
    }

    async fn rollback(&mut self, xid: &Xid) -> Result<(), XaError> {
        let mut branches = self.branches.lock().unwrap();
        let (state, _, _) = branches
            .get_mut(xid)
            .ok_or_else(|| XaError::new(XaErrorCode::XAER_NOTA, "XA branch not found"))?;
        state
            .rollback()
            .map_err(|error| XaError::new(XaErrorCode::XAER_PROTO, error.to_string()))?;
        branches.remove(xid);
        Ok(())
    }

    async fn recover(&self, _flags: i32) -> Result<Vec<Xid>, XaError> {
        Ok(self
            .branches
            .lock()
            .unwrap()
            .values()
            .map(|(state, _, _)| state.xid().clone())
            .collect())
    }

    async fn forget(&mut self, xid: &Xid) -> Result<(), XaError> {
        let mut branches = self.branches.lock().unwrap();
        let (state, _, _) = branches
            .get_mut(xid)
            .ok_or_else(|| XaError::new(XaErrorCode::XAER_NOTA, "XA branch not found"))?;
        state
            .forget()
            .map_err(|error| XaError::new(XaErrorCode::XAER_PROTO, error.to_string()))?;
        branches.remove(xid);
        Ok(())
    }

    async fn get_transaction_timeout(&self) -> Result<u64, XaError> {
        Ok(0)
    }

    async fn set_transaction_timeout(&mut self, _seconds: u64) -> Result<bool, XaError> {
        Ok(false)
    }

    async fn is_same_rm(&self, other: &dyn XaResource) -> Result<bool, XaError> {
        Ok(std::ptr::eq(
            std::ptr::from_ref::<dyn XaResource>(self).cast::<u8>(),
            std::ptr::from_ref::<dyn XaResource>(other).cast::<u8>(),
        ))
    }
}

fn new_xid(format_id: i32, gtrid: &[u8], branch: &[u8]) -> Xid {
    Xid::new(format_id, gtrid.to_vec(), branch.to_vec()).expect("valid xid")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== 场景 1：跨两个资源的 2PC 提交 ===");
    let mut account_a = InMemoryLedger::new("account-A");
    let mut account_b = InMemoryLedger::new("account-B");

    let tx = new_xid(1, b"gtrid-001", b"");
    let branch_a = new_xid(1, b"gtrid-001", b"branch-a");
    let branch_b = new_xid(1, b"gtrid-001", b"branch-b");

    // 阶段 0：开启两个分支
    account_a.start(&branch_a, xa_flags::TMNOFLAGS).await?;
    account_b.start(&branch_b, xa_flags::TMNOFLAGS).await?;
    println!("两个分支进入 Active");

    // 业务操作：A 扣款，B 入账
    account_a.stage(&branch_a, "A: -100 ".into()).await?;
    account_b.stage(&branch_b, "B: +100 ".into()).await?;

    // 阶段 1：prepare
    account_a.end(&branch_a, xa_flags::TMSUCCESS).await?;
    account_b.end(&branch_b, xa_flags::TMSUCCESS).await?;
    let pa = account_a.prepare(&branch_a).await?;
    let pb = account_b.prepare(&branch_b).await?;
    println!("prepare 结果: A={pa:?}, B={pb:?}");

    // 阶段 2：全部 Ok 则 commit
    if matches!((pa, pb), (XaPrepareResult::Ok, XaPrepareResult::Ok)) {
        account_a.commit(&branch_a, false).await?;
        account_b.commit(&branch_b, false).await?;
        println!(
            "已提交 → A='{:?}', B='{:?}'",
            account_a
                .committed
                .lock()
                .unwrap()
                .values()
                .collect::<Vec<_>>(),
            account_b
                .committed
                .lock()
                .unwrap()
                .values()
                .collect::<Vec<_>>(),
        );
    }

    println!("\n=== 场景 2：只读 prepare 投票 → 不执行第二阶段 ===");
    let mut db = InMemoryLedger::new("db-X");
    let bad = new_xid(2, b"gtrid-002", b"b1");
    db.start(&bad, xa_flags::TMNOFLAGS).await?;
    db.end(&bad, xa_flags::TMSUCCESS).await?;
    let pr = db.prepare(&bad).await?; // 空 staged → ReadOnly
    println!("prepare 返回 {pr:?}，只读分支不需要 commit 或 rollback");
    println!("分支数 = {}", db.branches.lock().unwrap().len());

    println!("\n=== 场景 3：非法状态转换被拒绝 ===");
    let mut sm = XaTransactionState::with_timeout(tx, Duration::from_secs(30));
    let err = sm.commit(false).unwrap_err(); // Idle 直接 commit
    println!("Idle→commit 被拒绝: {err}");
    assert!(!sm.is_timed_out());

    println!("\n=== 场景 4：超时检测 ===");
    let mut sm2 = XaTransactionState::with_timeout(new_xid(3, b"t3", b"b"), Duration::ZERO);
    sm2.start()?;
    println!(
        "timeout=0 立即超时: is_timed_out() = {}",
        sm2.is_timed_out()
    );

    println!("\n全部场景通过。转换审计轨迹（场景2）:");
    Ok(())
}

#[cfg(test)]
mod protocol_tests {
    use super::*;

    fn xid(branch: &[u8]) -> Xid {
        Xid::new(77, b"global".to_vec(), branch.to_vec()).unwrap()
    }

    #[tokio::test]
    async fn unknown_branch_operations_return_xa_nota_without_panicking() {
        let mut resource = InMemoryLedger::new("ledger");
        let missing = xid(b"missing");

        for result in [
            resource.end(&missing, xa_flags::TMSUCCESS).await,
            resource.commit(&missing, false).await,
            resource.rollback(&missing).await,
            resource.forget(&missing).await,
        ] {
            assert_eq!(result.unwrap_err().code(), XaErrorCode::XAER_NOTA);
        }
        assert_eq!(
            resource.prepare(&missing).await.unwrap_err().code(),
            XaErrorCode::XAER_NOTA
        );
        assert!(resource.branches.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn invalid_start_flags_and_duplicate_xid_have_no_side_effects() {
        let mut resource = InMemoryLedger::new("ledger");
        let first = xid(b"one");

        assert_eq!(
            resource.start(&first, 1).await.unwrap_err().code(),
            XaErrorCode::XAER_INVAL
        );
        assert!(resource.branches.lock().unwrap().is_empty());

        resource.start(&first, xa_flags::TMNOFLAGS).await.unwrap();
        assert_eq!(
            resource
                .start(&first, xa_flags::TMNOFLAGS)
                .await
                .unwrap_err()
                .code(),
            XaErrorCode::XAER_DUPID
        );
        assert_eq!(resource.branches.lock().unwrap().len(), 1);
        assert_eq!(
            resource
                .branches
                .lock()
                .unwrap()
                .get(&first)
                .unwrap()
                .0
                .state(),
            XaState::Active
        );
    }

    #[tokio::test]
    async fn branches_with_same_global_id_keep_distinct_branch_qualifiers() {
        let mut resource = InMemoryLedger::new("ledger");
        let first = xid(b"first");
        let second = xid(b"second");

        resource.start(&first, xa_flags::TMNOFLAGS).await.unwrap();
        resource.start(&second, xa_flags::TMNOFLAGS).await.unwrap();

        assert_eq!(resource.branches.lock().unwrap().len(), 2);
        assert_eq!(
            resource
                .branches
                .lock()
                .unwrap()
                .get(&first)
                .unwrap()
                .0
                .state(),
            XaState::Active
        );
        assert_eq!(
            resource
                .branches
                .lock()
                .unwrap()
                .get(&second)
                .unwrap()
                .0
                .state(),
            XaState::Active
        );
    }

    #[tokio::test]
    async fn join_and_suspend_resume_follow_xa_association_protocol() {
        let mut resource = InMemoryLedger::new("ledger");
        let branch = xid(b"association");
        resource.start(&branch, xa_flags::TMNOFLAGS).await.unwrap();
        resource.end(&branch, xa_flags::TMSUSPEND).await.unwrap();
        assert_eq!(
            resource
                .branches
                .lock()
                .unwrap()
                .get(&branch)
                .unwrap()
                .0
                .state(),
            XaState::Suspended
        );
        resource.start(&branch, xa_flags::TMRESUME).await.unwrap();
        assert_eq!(
            resource
                .branches
                .lock()
                .unwrap()
                .get(&branch)
                .unwrap()
                .0
                .state(),
            XaState::Active
        );
        resource.end(&branch, xa_flags::TMSUCCESS).await.unwrap();
        resource.start(&branch, xa_flags::TMJOIN).await.unwrap();
        assert_eq!(
            resource
                .branches
                .lock()
                .unwrap()
                .get(&branch)
                .unwrap()
                .0
                .state(),
            XaState::Active
        );
    }

    #[tokio::test]
    async fn invalid_end_flags_leave_branch_active() {
        let mut resource = InMemoryLedger::new("ledger");
        let branch = xid(b"invalid-end-flags");
        resource.start(&branch, xa_flags::TMNOFLAGS).await.unwrap();

        assert_eq!(
            resource
                .end(&branch, xa_flags::TMRESUME)
                .await
                .unwrap_err()
                .code(),
            XaErrorCode::XAER_INVAL
        );
        assert_eq!(
            resource
                .branches
                .lock()
                .unwrap()
                .get(&branch)
                .unwrap()
                .0
                .state(),
            XaState::Active
        );
    }

    #[tokio::test]
    async fn tmfail_prepare_returns_rollback_vote_and_does_not_commit() {
        let mut resource = InMemoryLedger::new("ledger");
        let branch = xid(b"rollback-only");
        resource.start(&branch, xa_flags::TMNOFLAGS).await.unwrap();
        resource
            .stage(&branch, "must-not-commit".to_owned())
            .await
            .unwrap();
        resource.end(&branch, xa_flags::TMFAIL).await.unwrap();

        assert_eq!(
            resource.prepare(&branch).await.unwrap_err().code(),
            XaErrorCode::XA_RBROLLBACK
        );
        assert!(resource.branches.lock().unwrap().is_empty());
        assert!(resource.committed.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn one_phase_commit_uses_the_requested_protocol_and_preserves_result() {
        let mut resource = InMemoryLedger::new("ledger");
        let branch = xid(b"one-phase");
        resource.start(&branch, xa_flags::TMNOFLAGS).await.unwrap();
        resource
            .stage(&branch, "committed".to_owned())
            .await
            .unwrap();
        resource.end(&branch, xa_flags::TMSUCCESS).await.unwrap();

        resource.commit(&branch, true).await.unwrap();
        assert_eq!(
            resource.committed.lock().unwrap().get(&branch).unwrap(),
            "committed"
        );
        assert_eq!(
            resource.commit(&branch, false).await.unwrap_err().code(),
            XaErrorCode::XAER_NOTA
        );
    }

    #[tokio::test]
    async fn read_only_vote_removes_branch_and_rejects_second_phase() {
        let mut resource = InMemoryLedger::new("ledger");
        let branch = xid(b"read-only");
        resource.start(&branch, xa_flags::TMNOFLAGS).await.unwrap();
        resource.end(&branch, xa_flags::TMSUCCESS).await.unwrap();

        assert_eq!(
            resource.prepare(&branch).await.unwrap(),
            XaPrepareResult::ReadOnly
        );
        assert_eq!(
            resource.commit(&branch, false).await.unwrap_err().code(),
            XaErrorCode::XAER_NOTA
        );
        assert!(resource.committed.lock().unwrap().is_empty());
    }
}
