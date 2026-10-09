mod core {
#[path = "E:/workspaces/workspace-github-easy-4-rust/druid-rust/crates/druid/src/core/resource_id.rs"]
pub(crate) mod resource_id;
#[path = "E:/workspaces/workspace-github-easy-4-rust/druid-rust/crates/druid/src/core/resource_kind.rs"]
pub(crate) mod resource_kind;
#[path = "E:/workspaces/workspace-github-easy-4-rust/druid-rust/crates/druid/src/core/resource_state.rs"]
pub(crate) mod resource_state;
#[path = "E:/workspaces/workspace-github-easy-4-rust/druid-rust/crates/druid/src/core/resource_runtime_capabilities.rs"]
pub(crate) mod resource_runtime_capabilities;
#[path = "E:/workspaces/workspace-github-easy-4-rust/druid-rust/crates/druid/src/core/resource_owner.rs"]
pub(crate) mod resource_owner;
#[path = "E:/workspaces/workspace-github-easy-4-rust/druid-rust/crates/druid/src/core/resource_context.rs"]
pub(crate) mod resource_context;
}
use core::resource_context::ResourceContext;
use core::resource_id::ResourceId;
use core::resource_kind::ResourceKind;
use core::resource_owner::ResourceOwner;
use core::resource_runtime_capabilities::ResourceRuntimeCapabilities;
use core::resource_state::ResourceState;
use druid_rdbc::error::RdbcError;
use std::sync::{Arc, atomic::{AtomicUsize,Ordering}};
use std::time::Duration;
use tokio::sync::Notify;

struct ControlledOwner {
    entered: Notify,
    finish: Notify,
    block: bool,
    fail: bool,
    calls: AtomicUsize,
    completed: AtomicUsize,
    failures: AtomicUsize,
}
impl ControlledOwner {
    fn new(block: bool, fail: bool) -> Arc<Self> {
        Arc::new(Self { entered: Notify::new(), finish: Notify::new(), block, fail,
            calls: AtomicUsize::new(0), completed: AtomicUsize::new(0), failures: AtomicUsize::new(0) })
    }
}
#[async_trait::async_trait]
impl ResourceOwner for ControlledOwner {
    async fn resource_released(&self, _: &ResourceId, _: ResourceKind) -> Result<(), RdbcError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.entered.notify_one();
        if self.block { self.finish.notified().await; }
        self.completed.fetch_add(1, Ordering::SeqCst);
        if self.fail { Err(RdbcError::Other("owner notification failed".into())) } else { Ok(()) }
    }
    fn resource_failed(&self, _: &ResourceId, _: ResourceKind, _: &RdbcError) {
        self.failures.fetch_add(1, Ordering::SeqCst);
    }
}
fn context(owner: Arc<ControlledOwner>) -> Arc<ResourceContext> {
    Arc::new(ResourceContext::new(ResourceId::new("release-contract"), ResourceKind::Blob,
        ResourceRuntimeCapabilities::FREE, owner))
}
#[tokio::test]
async fn concurrent_success_releases_driver_and_notifies_owner_once() {
    let owner = ControlledOwner::new(false, false);
    let context = context(owner.clone());
    let calls = AtomicUsize::new(0);
    let (first, second) = tokio::join!(context.release_with(|| async {
        calls.fetch_add(1, Ordering::SeqCst); tokio::task::yield_now().await; Ok(())
    }), context.release_with(|| async { calls.fetch_add(1, Ordering::SeqCst); Ok(()) }));
    assert!(first.is_ok()); assert!(second.is_ok());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(owner.calls.load(Ordering::SeqCst), 1);
    assert_eq!(owner.completed.load(Ordering::SeqCst), 1);
    assert_eq!(context.state(), ResourceState::Freed);
}
#[tokio::test]
async fn failed_driver_release_can_retry_without_early_owner_notification() {
    let owner = ControlledOwner::new(false, false); let context = context(owner.clone());
    let calls = AtomicUsize::new(0);
    let failure = context.release_with(|| async { calls.fetch_add(1,Ordering::SeqCst);
        Err(RdbcError::Other("driver release failed".into())) }).await;
    assert!(failure.is_err()); assert_eq!(context.state(), ResourceState::Open);
    assert_eq!(owner.calls.load(Ordering::SeqCst), 0);
    assert_eq!(owner.failures.load(Ordering::SeqCst), 1);
    assert!(context.release_with(|| async { calls.fetch_add(1,Ordering::SeqCst); Ok(()) }).await.is_ok());
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    assert_eq!(owner.completed.load(Ordering::SeqCst), 1);
    assert_eq!(context.state(), ResourceState::Freed);
}
#[tokio::test]
async fn owner_error_does_not_replay_successful_driver_release() {
    let owner = ControlledOwner::new(false, true); let context = context(owner.clone());
    let calls = AtomicUsize::new(0);
    assert!(context.release_with(|| async { calls.fetch_add(1,Ordering::SeqCst); Ok(()) }).await.is_err());
    assert_eq!(context.state(), ResourceState::Freed);
    assert!(context.release_with(|| async { calls.fetch_add(1,Ordering::SeqCst); Ok(()) }).await.is_ok());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(owner.calls.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn cancelling_waiter_does_not_cancel_winning_release() {
    let owner = ControlledOwner::new(true, false); let context = context(owner.clone());
    let calls = Arc::new(AtomicUsize::new(0));
    let winning_context = context.clone(); let winning_calls = calls.clone();
    let winner = tokio::spawn(async move { winning_context.release_with(|| async {
        winning_calls.fetch_add(1,Ordering::SeqCst); Ok(()) }).await });
    tokio::time::timeout(Duration::from_secs(2), owner.entered.notified()).await.unwrap();
    let waiting_context = context.clone(); let waiting_calls = calls.clone();
    let waiter = tokio::spawn(async move { waiting_context.release_with(|| async {
        waiting_calls.fetch_add(1,Ordering::SeqCst); Ok(()) }).await });
    tokio::task::yield_now().await; waiter.abort(); assert!(waiter.await.unwrap_err().is_cancelled());
    owner.finish.notify_one();
    assert!(tokio::time::timeout(Duration::from_secs(2),winner).await.unwrap().unwrap().is_ok());
    assert_eq!(calls.load(Ordering::SeqCst),1); assert_eq!(owner.completed.load(Ordering::SeqCst),1);
    assert_eq!(context.state(),ResourceState::Freed);
}
async fn cancellation_contract(block_driver: bool) {
    let owner=ControlledOwner::new(!block_driver,false); let context=context(owner.clone());
    let driver_entered=Arc::new(Notify::new()); let driver_finish=Arc::new(Notify::new());
    let calls=Arc::new(AtomicUsize::new(0));
    let winning_context=context.clone(); let entered=driver_entered.clone();
    let finish=driver_finish.clone(); let winning_calls=calls.clone();
    let winner=tokio::spawn(async move { winning_context.release_with(|| async {
        winning_calls.fetch_add(1,Ordering::SeqCst); entered.notify_one();
        if block_driver {finish.notified().await;} Ok(()) }).await });
    if block_driver {tokio::time::timeout(Duration::from_secs(2),driver_entered.notified()).await.unwrap();}
    else {tokio::time::timeout(Duration::from_secs(2),owner.entered.notified()).await.unwrap();}
    winner.abort();assert!(winner.await.unwrap_err().is_cancelled());
    driver_finish.notify_one();owner.finish.notify_one();
    let following=context.release_with(|| async {calls.fetch_add(1,Ordering::SeqCst);Ok(())});
    assert!(tokio::time::timeout(Duration::from_millis(250),following).await
        .expect("cancelled caller must not strand release").is_ok());
    assert_eq!(calls.load(Ordering::SeqCst),1);
    assert_eq!(owner.calls.load(Ordering::SeqCst),1);
    assert_eq!(owner.completed.load(Ordering::SeqCst),1);
    assert_eq!(context.state(),ResourceState::Freed);
}
#[tokio::test]
async fn winning_caller_cancelled_during_driver_release_still_completes() { cancellation_contract(true).await; }
#[tokio::test]
async fn winning_caller_cancelled_during_owner_notification_still_completes() { cancellation_contract(false).await; }
