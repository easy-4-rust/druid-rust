//! XA/2PC 使用示例：跨两个资源的分布式事务。
//!
//! 运行：cargo run -p druid --example xa_demo
//!
//! 对应 Java: javax.transaction.xa.{Xid, XAResource} +
//! com.alibaba.druid.pool.xa.DruidXADataSource 的协议层。

use druid::core::xa::{
    flags as xa_flags, XaPrepareResult, XaResource, XaState, XaTransactionState, Xid,
};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

/// 示例资源：内存账本。实现 XaResource 后即可参与 2PC。
/// 真实场景中这里是 MySQL XA / PostgreSQL prepared transaction 等驱动适配器。
struct InMemoryLedger {
    name: &'static str,
    committed: Mutex<HashMap<Vec<u8>, String>>,
    // xid -> (状态机, 预提交数据)
    branches: Mutex<HashMap<Vec<u8>, (XaTransactionState, String)>>,
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
        let (state, staged) = branches
            .get_mut(xid.global_transaction_id())
            .ok_or_else(|| {
                druid::core::DruidError::InvalidArgument(format!(
                    "branch not active: {}",
                    self.name
                ))
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
    async fn start(&mut self, xid: &Xid, _flags: i32) -> Result<(), druid::core::DruidError> {
        let mut branches = self.branches.lock().unwrap();
        branches.insert(
            xid.global_transaction_id().to_vec(),
            (XaTransactionState::new(xid.clone()), String::new()),
        );
        branches
            .get_mut(xid.global_transaction_id())
            .unwrap()
            .0
            .start()
            .map_err(|e| druid::core::DruidError::InvalidArgument(e.to_string()))
    }

    async fn end(&mut self, xid: &Xid, _flags: i32) -> Result<(), druid::core::DruidError> {
        self.branches
            .lock()
            .unwrap()
            .get_mut(xid.global_transaction_id())
            .unwrap()
            .0
            .end()
            .map_err(|e| druid::core::DruidError::InvalidArgument(e.to_string()))
    }

    async fn prepare(&mut self, xid: &Xid) -> Result<XaPrepareResult, druid::core::DruidError> {
        let mut branches = self.branches.lock().unwrap();
        let (state, staged) = branches.get_mut(xid.global_transaction_id()).unwrap();
        state
            .prepare()
            .map_err(|e| druid::core::DruidError::InvalidArgument(e.to_string()))?;
        if staged.is_empty() {
            return Ok(XaPrepareResult::ReadOnly);
        }
        Ok(XaPrepareResult::Ok)
    }

    async fn commit(&mut self, xid: &Xid, _one_phase: bool) -> Result<(), druid::core::DruidError> {
        let mut branches = self.branches.lock().unwrap();
        let (mut state, staged) = branches.remove(xid.global_transaction_id()).unwrap();
        state
            .commit(false)
            .map_err(|e| druid::core::DruidError::InvalidArgument(e.to_string()))?;
        self.committed
            .lock()
            .unwrap()
            .insert(xid.global_transaction_id().to_vec(), staged);
        Ok(())
    }

    async fn rollback(&mut self, xid: &Xid) -> Result<(), druid::core::DruidError> {
        let mut branches = self.branches.lock().unwrap();
        let (mut state, _staged) = branches.remove(xid.global_transaction_id()).unwrap();
        state
            .rollback()
            .map_err(|e| druid::core::DruidError::InvalidArgument(e.to_string()))
    }

    async fn recover(&self, _flags: i32) -> Result<Vec<Xid>, druid::core::DruidError> {
        Ok(self
            .branches
            .lock()
            .unwrap()
            .values()
            .map(|(s, _)| s.xid().clone())
            .collect())
    }

    async fn forget(&mut self, xid: &Xid) -> Result<(), druid::core::DruidError> {
        self.branches
            .lock()
            .unwrap()
            .remove(xid.global_transaction_id());
        Ok(())
    }

    async fn get_transaction_timeout(&self) -> Result<u64, druid::core::DruidError> {
        Ok(0)
    }

    async fn set_transaction_timeout(
        &mut self,
        _seconds: u64,
    ) -> Result<bool, druid::core::DruidError> {
        Ok(false)
    }

    async fn is_same_rm(&self, other: &dyn XaResource) -> Result<bool, druid::core::DruidError> {
        Ok(std::ptr::eq(
            self as *const dyn XaResource as *const u8,
            other as *const dyn XaResource as *const u8,
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
    println!("prepare 结果: A={:?}, B={:?}", pa, pb);

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

    println!("\n=== 场景 2：prepare 阶段失败 → 全体回滚 ===");
    let mut db = InMemoryLedger::new("db-X");
    let bad = new_xid(2, b"gtrid-002", b"b1");
    db.start(&bad, xa_flags::TMNOFLAGS).await?;
    db.end(&bad, xa_flags::TMSUCCESS).await?;
    let pr = db.prepare(&bad).await?; // 空 staged → ReadOnly
    println!("prepare 返回 {:?}，上层决定 rollback", pr);
    db.rollback(&bad).await?;
    println!("回滚完成，分支数 = {}", db.branches.lock().unwrap().len());

    println!("\n=== 场景 3：非法状态转换被拒绝 ===");
    let mut sm = XaTransactionState::with_timeout(tx, Duration::from_secs(30));
    let err = sm.commit(false).unwrap_err(); // Idle 直接 commit
    println!("Idle→commit 被拒绝: {}", err);
    assert!(sm.is_timed_out() == false);

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
