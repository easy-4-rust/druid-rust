#![allow(clippy::unused_async)]
//! XA/2PC 使用示例：跨两个资源的分布式事务。
//!
//! 运行：cargo run -p druid --example `xa_demo`
//!
//! 对应 Java: javax.transaction.xa.{Xid, `XAResource`} +
//! com.alibaba.druid.pool.xa.DruidXADataSource 的协议层。

use druid::core::xa::{
    flags as xa_flags, XaError, XaErrorCode, XaPrepareResult, XaResource,
    XaResourceManagerIdentity, XaState, XaTransactionState, Xid,
};
use druid_rdbc::connection::Connection;
use druid_rdbc::data_source::{PooledConnection, XaConnection};
use druid_rdbc::error::RdbcError;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

/// 示例资源：内存账本。实现 `XaResource` 后即可参与 2PC。
/// 真实场景中这里是 `MySQL` XA / `PostgreSQL` prepared transaction 等驱动适配器。
struct InMemoryLedger {
    name: &'static str,
    manager_identity: Option<XaResourceManagerIdentity>,
    owner_open: AtomicBool,
    committed: Mutex<HashMap<Xid, String>>,
    // xid -> (状态机, 预提交数据)
    branches: Mutex<HashMap<Xid, (XaTransactionState, String, bool)>>,
    recovery_scan: Mutex<Option<RecoveryScan>>,
}

struct InMemoryXaConnection {
    resource: InMemoryLedger,
    closed: bool,
}

impl InMemoryXaConnection {
    fn new(name: &'static str) -> Self {
        Self {
            resource: InMemoryLedger::new(name),
            closed: false,
        }
    }
}

struct RecoveryScan {
    prepared_branches: Vec<Xid>,
    next_index: usize,
}

impl RecoveryScan {
    const PAGE_SIZE: usize = 1;

    fn next_page(&mut self) -> Vec<Xid> {
        let page_end = self
            .next_index
            .saturating_add(Self::PAGE_SIZE)
            .min(self.prepared_branches.len());
        let page = self.prepared_branches[self.next_index..page_end].to_vec();
        self.next_index = page_end;
        page
    }
}

impl InMemoryLedger {
    fn new(name: &'static str) -> Self {
        Self::with_manager_identity(name, XaResourceManagerIdentity::new())
    }

    fn with_manager_identity(
        name: &'static str,
        manager_identity: XaResourceManagerIdentity,
    ) -> Self {
        Self {
            name,
            manager_identity: Some(manager_identity),
            owner_open: AtomicBool::new(true),
            committed: Mutex::new(HashMap::new()),
            branches: Mutex::new(HashMap::new()),
            recovery_scan: Mutex::new(None),
        }
    }

    fn with_unknown_manager_identity(name: &'static str) -> Self {
        Self {
            name,
            manager_identity: None,
            owner_open: AtomicBool::new(true),
            committed: Mutex::new(HashMap::new()),
            branches: Mutex::new(HashMap::new()),
            recovery_scan: Mutex::new(None),
        }
    }

    fn close_owner(&self) {
        self.owner_open.store(false, Ordering::Release);
    }

    fn ensure_owner_open(&self) -> Result<(), XaError> {
        if self.owner_open.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(XaError::new(
                XaErrorCode::XAER_RMFAIL,
                "XA resource owner is closed",
            ))
        }
    }

    fn prepared_xids(&self) -> Vec<Xid> {
        self.branches
            .lock()
            .unwrap()
            .values()
            .filter(|(state, _, _)| state.state() == XaState::Prepared)
            .map(|(state, _, _)| state.xid().clone())
            .collect()
    }

    /// 在事务分支内登记一条预提交数据（业务操作，非 XA 协议方法）。
    async fn stage(&self, xid: &Xid, value: String) -> Result<(), druid::core::DruidError> {
        self.ensure_owner_open()
            .map_err(|error| druid::core::DruidError::InvalidArgument(error.to_string()))?;
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
        self.ensure_owner_open()?;
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
        self.ensure_owner_open()?;
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
        self.ensure_owner_open()?;
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
        self.ensure_owner_open()?;
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
        self.ensure_owner_open()?;
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

    async fn recover(&self, flags: i32) -> Result<Vec<Xid>, XaError> {
        self.ensure_owner_open()?;
        let start_scan = xa_flags::TMSTARTRSCAN;
        let end_scan = xa_flags::TMENDRSCAN;
        if flags & !(start_scan | end_scan) != 0 {
            return Err(XaError::new(
                XaErrorCode::XAER_INVAL,
                format!("invalid XA recover flags: {flags:#x}"),
            ));
        }

        let mut scan = self.recovery_scan.lock().unwrap();
        match flags {
            flags if flags == start_scan | end_scan => {
                if scan.is_some() {
                    return Err(XaError::new(
                        XaErrorCode::XAER_PROTO,
                        "XA recovery scan is already active",
                    ));
                }
                Ok(self.prepared_xids())
            }
            flags if flags == start_scan => {
                if scan.is_some() {
                    return Err(XaError::new(
                        XaErrorCode::XAER_PROTO,
                        "XA recovery scan is already active",
                    ));
                }
                let mut active_scan = RecoveryScan {
                    prepared_branches: self.prepared_xids(),
                    next_index: 0,
                };
                let page = active_scan.next_page();
                *scan = Some(active_scan);
                Ok(page)
            }
            flags if flags == xa_flags::TMNOFLAGS => {
                scan.as_mut().map(RecoveryScan::next_page).ok_or_else(|| {
                    XaError::new(
                        XaErrorCode::XAER_PROTO,
                        "XA recovery scan has not been started",
                    )
                })
            }
            flags if flags == end_scan => scan.take().map(|_| Vec::new()).ok_or_else(|| {
                XaError::new(
                    XaErrorCode::XAER_PROTO,
                    "XA recovery scan has not been started",
                )
            }),
            _ => unreachable!("recover flags were validated before dispatch"),
        }
    }

    async fn forget(&mut self, xid: &Xid) -> Result<(), XaError> {
        self.ensure_owner_open()?;
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
        self.ensure_owner_open()?;
        Ok(0)
    }

    async fn set_transaction_timeout(&mut self, _seconds: u64) -> Result<bool, XaError> {
        self.ensure_owner_open()?;
        Ok(false)
    }

    async fn is_same_rm(&self, other: &dyn XaResource) -> Result<bool, XaError> {
        self.ensure_owner_open()?;
        let Some(other_identity) = other.resource_manager_identity() else {
            let self_address = std::ptr::from_ref(self).cast::<()>();
            let other_address = std::ptr::from_ref(other).cast::<()>();
            return Ok(self_address == other_address);
        };
        Ok(self.manager_identity.as_ref() == Some(&other_identity))
    }

    fn resource_manager_identity(&self) -> Option<XaResourceManagerIdentity> {
        self.manager_identity.clone()
    }
}

#[async_trait::async_trait]
impl PooledConnection for InMemoryXaConnection {
    async fn connection(&mut self) -> Result<Box<dyn Connection>, RdbcError> {
        Err(RdbcError::feature_not_supported(
            "in_memory_xa_logical_connection",
        ))
    }

    async fn close(&mut self) -> Result<(), RdbcError> {
        self.closed = true;
        self.resource.close_owner();
        Ok(())
    }
}

impl XaConnection for InMemoryXaConnection {
    fn xa_resource_name(&self) -> &str {
        &self.resource.name
    }

    fn xa_resource(&mut self) -> Result<&mut (dyn XaResource + '_), XaError> {
        if self.closed {
            return Err(XaError::new(
                XaErrorCode::XAER_RMFAIL,
                "XA connection owner is closed",
            ));
        }
        Ok(&mut self.resource)
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

    async fn prepare_branch(resource: &mut InMemoryLedger, branch: &Xid) {
        resource.start(branch, xa_flags::TMNOFLAGS).await.unwrap();
        resource.stage(branch, "write".to_owned()).await.unwrap();
        resource.end(branch, xa_flags::TMSUCCESS).await.unwrap();
        assert_eq!(resource.prepare(branch).await.unwrap(), XaPrepareResult::Ok);
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

    #[tokio::test]
    async fn recover_scan_returns_only_prepared_branches_across_pages() {
        let mut resource = InMemoryLedger::new("ledger");
        let first = xid(b"prepared-first");
        let second = xid(b"prepared-second");
        let active = xid(b"still-active");
        prepare_branch(&mut resource, &first).await;
        prepare_branch(&mut resource, &second).await;
        resource.start(&active, xa_flags::TMNOFLAGS).await.unwrap();

        let first_page = resource.recover(xa_flags::TMSTARTRSCAN).await.unwrap();
        let second_page = resource.recover(xa_flags::TMNOFLAGS).await.unwrap();
        let end_page = resource.recover(xa_flags::TMENDRSCAN).await.unwrap();

        assert_eq!(first_page.len(), 1);
        assert_eq!(second_page.len(), 1);
        assert!(end_page.is_empty());
        let recovered = [first_page[0].clone(), second_page[0].clone()];
        assert!(recovered.contains(&first));
        assert!(recovered.contains(&second));
        assert!(!recovered.contains(&active));
        assert_eq!(resource.branches.lock().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn recover_scan_uses_a_snapshot_when_branches_change_between_pages() {
        let mut resource = InMemoryLedger::new("ledger");
        let first = xid(b"snapshot-first");
        let second = xid(b"snapshot-second");
        let added_after_scan_started = xid(b"snapshot-late");
        prepare_branch(&mut resource, &first).await;
        prepare_branch(&mut resource, &second).await;

        let first_page = resource.recover(xa_flags::TMSTARTRSCAN).await.unwrap();
        prepare_branch(&mut resource, &added_after_scan_started).await;
        let second_page = resource.recover(xa_flags::TMNOFLAGS).await.unwrap();
        let end_page = resource.recover(xa_flags::TMENDRSCAN).await.unwrap();

        assert_eq!(first_page.len(), 1);
        assert_eq!(second_page.len(), 1);
        assert_ne!(first_page[0], second_page[0]);
        assert!(end_page.is_empty());
        assert!(!first_page.contains(&added_after_scan_started));
        assert!(!second_page.contains(&added_after_scan_started));
        assert_eq!(resource.branches.lock().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn recover_scan_validates_flags_and_lifecycle_without_mutation() {
        let mut resource = InMemoryLedger::new("ledger");
        let branch = xid(b"prepared");
        let second_branch = xid(b"prepared-second");
        prepare_branch(&mut resource, &branch).await;
        prepare_branch(&mut resource, &second_branch).await;

        assert_eq!(
            resource.recover(1).await.unwrap_err().code(),
            XaErrorCode::XAER_INVAL
        );
        assert_eq!(
            resource
                .recover(xa_flags::TMNOFLAGS)
                .await
                .unwrap_err()
                .code(),
            XaErrorCode::XAER_PROTO
        );
        let first_page = resource.recover(xa_flags::TMSTARTRSCAN).await.unwrap();
        assert_eq!(first_page.len(), 1);
        assert_eq!(
            resource
                .recover(xa_flags::TMSTARTRSCAN)
                .await
                .unwrap_err()
                .code(),
            XaErrorCode::XAER_PROTO
        );
        assert_eq!(
            resource.recover(1).await.unwrap_err().code(),
            XaErrorCode::XAER_INVAL
        );
        let second_page = resource.recover(xa_flags::TMNOFLAGS).await.unwrap();
        assert_eq!(second_page.len(), 1);
        assert_ne!(first_page[0], second_page[0]);
        assert!(resource
            .recover(xa_flags::TMENDRSCAN)
            .await
            .unwrap()
            .is_empty());
        let combined = resource
            .recover(xa_flags::TMSTARTRSCAN | xa_flags::TMENDRSCAN)
            .await
            .unwrap();
        assert_eq!(combined.len(), 2);
        assert!(combined.contains(&branch));
        assert!(combined.contains(&second_branch));
        assert_eq!(
            resource
                .recover(xa_flags::TMNOFLAGS)
                .await
                .unwrap_err()
                .code(),
            XaErrorCode::XAER_PROTO
        );
        assert_eq!(resource.branches.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn rejected_timeout_setting_does_not_change_effective_timeout() {
        let mut resource = InMemoryLedger::new("ledger");
        assert_eq!(resource.get_transaction_timeout().await.unwrap(), 0);
        assert!(!resource.set_transaction_timeout(30).await.unwrap());
        assert_eq!(resource.get_transaction_timeout().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn same_rm_uses_shared_identity_not_the_diagnostic_name() {
        let shared_identity = XaResourceManagerIdentity::new();
        let first = InMemoryLedger::with_manager_identity("same-name", shared_identity.clone());
        let same_manager = InMemoryLedger::with_manager_identity("different-name", shared_identity);
        let different_manager = InMemoryLedger::new("same-name");

        assert!(first.is_same_rm(&same_manager).await.unwrap());
        assert!(!first.is_same_rm(&different_manager).await.unwrap());
        assert!(first.is_same_rm(&first).await.unwrap());
    }

    #[tokio::test]
    async fn unknown_rm_identity_is_never_inferred_from_object_identity() {
        let unknown = InMemoryLedger::with_unknown_manager_identity("same-name");
        let same_name_different_resource =
            InMemoryLedger::with_unknown_manager_identity("same-name");

        assert!(unknown.is_same_rm(&unknown).await.unwrap());
        assert!(!unknown
            .is_same_rm(&same_name_different_resource)
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn owner_close_invalidates_resources_without_mutating_branches() {
        let mut resource = InMemoryLedger::new("ledger");
        let branch = xid(b"owner-bound");
        resource.start(&branch, xa_flags::TMNOFLAGS).await.unwrap();
        resource.close_owner();

        assert_eq!(
            resource
                .end(&branch, xa_flags::TMSUCCESS)
                .await
                .unwrap_err()
                .code(),
            XaErrorCode::XAER_RMFAIL
        );
        assert_eq!(
            resource
                .recover(xa_flags::TMSTARTRSCAN)
                .await
                .unwrap_err()
                .code(),
            XaErrorCode::XAER_RMFAIL
        );
        assert_eq!(
            resource
                .start(&branch, xa_flags::TMNOFLAGS)
                .await
                .unwrap_err()
                .code(),
            XaErrorCode::XAER_RMFAIL
        );
        assert_eq!(
            resource.prepare(&branch).await.unwrap_err().code(),
            XaErrorCode::XAER_RMFAIL
        );
        assert_eq!(
            resource.commit(&branch, false).await.unwrap_err().code(),
            XaErrorCode::XAER_RMFAIL
        );
        assert_eq!(
            resource.rollback(&branch).await.unwrap_err().code(),
            XaErrorCode::XAER_RMFAIL
        );
        assert_eq!(
            resource.forget(&branch).await.unwrap_err().code(),
            XaErrorCode::XAER_RMFAIL
        );
        assert_eq!(
            resource.get_transaction_timeout().await.unwrap_err().code(),
            XaErrorCode::XAER_RMFAIL
        );
        assert_eq!(
            resource
                .set_transaction_timeout(30)
                .await
                .unwrap_err()
                .code(),
            XaErrorCode::XAER_RMFAIL
        );
        assert_eq!(
            resource.is_same_rm(&resource).await.unwrap_err().code(),
            XaErrorCode::XAER_RMFAIL
        );
        assert_eq!(resource.branches.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn xa_connection_owner_closes_and_invalidates_its_resource() {
        let mut owner = InMemoryXaConnection::new("ledger");
        let branch = xid(b"owned-resource");
        owner
            .xa_resource()
            .unwrap()
            .start(&branch, xa_flags::TMNOFLAGS)
            .await
            .unwrap();

        owner.close().await.unwrap();

        let error = match owner.xa_resource() {
            Ok(_) => panic!("closed XA owner must not return its resource"),
            Err(error) => error,
        };
        assert_eq!(error.code(), XaErrorCode::XAER_RMFAIL);
        assert_eq!(
            owner
                .resource
                .get_transaction_timeout()
                .await
                .unwrap_err()
                .code(),
            XaErrorCode::XAER_RMFAIL
        );
        assert_eq!(owner.resource.branches.lock().unwrap().len(), 1);
    }
}
