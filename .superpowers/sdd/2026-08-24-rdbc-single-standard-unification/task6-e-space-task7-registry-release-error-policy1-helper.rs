mod core {
pub(crate) use druid_rdbc::error::RdbcError;
#[path = "E:/workspaces/workspace-github-easy-4-rust/druid-rust/crates/druid/src/core/standard_resource_admission.rs"]
pub(crate) mod standard_resource_admission;
#[path = "E:/workspaces/workspace-github-easy-4-rust/druid-rust/crates/druid/src/core/standard_resource_handle.rs"]
pub(crate) mod standard_resource_handle;
#[path = "E:/workspaces/workspace-github-easy-4-rust/druid-rust/crates/druid/src/core/standard_resource_registration.rs"]
pub(crate) mod standard_resource_registration;
#[path = "E:/workspaces/workspace-github-easy-4-rust/druid-rust/crates/druid/src/core/standard_resource_registry_state.rs"]
pub(crate) mod standard_resource_registry_state;
#[path = "E:/workspaces/workspace-github-easy-4-rust/druid-rust/crates/druid/src/core/standard_resource_registry.rs"]
pub(crate) mod standard_resource_registry;

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

use core::standard_resource_handle::StandardResourceHandle;
use core::standard_resource_registration::StandardResourceRegistration;
use core::standard_resource_registry::StandardResourceRegistry;
use std::sync::OnceLock;

/// 两个路径共用的实际释放夹具；context 与 registry 均直接包含生产源码。
struct ReleaseActor {
    context: Arc<ResourceContext>,
    owner: Arc<ControlledOwner>,
    driver_started: Notify,
    driver_finish: Notify,
    block_driver: bool,
    driver_calls: AtomicUsize,
    driver_completed: AtomicUsize,
    failures_remaining: AtomicUsize,
    registration: OnceLock<StandardResourceRegistration>,
}
#[async_trait::async_trait]
impl StandardResourceHandle for ReleaseActor {
    async fn close_resource(&self) -> Result<(), RdbcError> {
        let result = self.context.release_with(|| async {
            self.driver_calls.fetch_add(1, Ordering::SeqCst);
            self.driver_started.notify_one();
            if self.failures_remaining.fetch_update(Ordering::SeqCst,Ordering::SeqCst,|count|count.checked_sub(1)).is_ok() {
                return Err(RdbcError::DriverError("forced driver release failure".into()));
            }
            if self.block_driver { self.driver_finish.notified().await; }
            self.driver_completed.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }).await;
        if self.is_resource_closed() { self.unregister_resource(); }
        result
    }
    fn is_resource_closed(&self) -> bool { self.context.state() == ResourceState::Freed }
    fn registration_slot(&self) -> &OnceLock<StandardResourceRegistration> { &self.registration }
}

fn error_actor(owner_fail:bool,owner_block:bool,driver_failures:usize)->Arc<ReleaseActor> {
    let owner=ControlledOwner::new(owner_block,owner_fail);
    Arc::new(ReleaseActor {context:context(owner.clone()),owner,driver_started:Notify::new(),
        driver_finish:Notify::new(),block_driver:false,driver_calls:AtomicUsize::new(0),
        driver_completed:AtomicUsize::new(0),failures_remaining:AtomicUsize::new(driver_failures),registration:OnceLock::new()})
}
#[tokio::test]
async fn task7_registry_driver_error_retains_resource_then_retries_without_early_owner() {
    let registry=Arc::new(StandardResourceRegistry::default());
    let admission=registry.begin_admission(0).unwrap();
    let actor=error_actor(false,false,1);
    let handle:Arc<dyn StandardResourceHandle>=actor.clone();
    let registrations=registry.register_batch(&admission,vec![handle]).await.unwrap();
    let first=registry.close_registered_after_admission(&registrations,&admission).await;
    assert!(matches!(first,Err(RdbcError::DriverError(ref message)) if message=="forced driver release failure"));
    assert_eq!(actor.context.state(),ResourceState::Open);
    assert_eq!(actor.driver_calls.load(Ordering::SeqCst),1);
    assert_eq!(actor.driver_completed.load(Ordering::SeqCst),0);
    assert_eq!(actor.owner.calls.load(Ordering::SeqCst),0);
    assert_eq!(actor.owner.failures.load(Ordering::SeqCst),1);
    assert!(registry.has_open_resources());
    assert!(registry.open_next_generation().is_err());
    registry.close_registered_after_admission(&registrations,&admission).await.expect("retry retained resource");
    assert_eq!(actor.driver_calls.load(Ordering::SeqCst),2);
    assert_eq!(actor.driver_completed.load(Ordering::SeqCst),1);
    assert_eq!(actor.owner.calls.load(Ordering::SeqCst),1);
    assert_eq!(actor.owner.completed.load(Ordering::SeqCst),1);
    assert_eq!(actor.context.state(),ResourceState::Freed);
    registry.close_all().await.unwrap();
    assert_eq!(registry.open_next_generation().unwrap(),1);
}
#[tokio::test]
async fn task7_registry_owner_error_returns_to_waiter_without_driver_or_owner_replay() {
    let registry=Arc::new(StandardResourceRegistry::default());
    let admission=registry.begin_admission(0).unwrap();
    let actor=error_actor(true,false,0);
    let handle:Arc<dyn StandardResourceHandle>=actor.clone();
    let registrations=registry.register_batch(&admission,vec![handle]).await.unwrap();
    let first=registry.close_registered_after_admission(&registrations,&admission).await;
    assert!(matches!(first,Err(RdbcError::Other(ref message)) if message=="owner notification failed"));
    assert_eq!(actor.context.state(),ResourceState::Freed);
    assert_eq!(actor.driver_calls.load(Ordering::SeqCst),1);
    assert_eq!(actor.driver_completed.load(Ordering::SeqCst),1);
    assert_eq!(actor.owner.calls.load(Ordering::SeqCst),1);
    assert_eq!(actor.owner.completed.load(Ordering::SeqCst),1);
    assert!(registry.open_next_generation().is_err());
    registry.close_all().await.expect("subsequent logical close is idempotent");
    assert_eq!(actor.driver_calls.load(Ordering::SeqCst),1);
    assert_eq!(actor.owner.calls.load(Ordering::SeqCst),1);
    assert_eq!(registry.open_next_generation().unwrap(),1);
}
/// 诊断当前行为，不作为owner错误处置正确性的验收。
#[tokio::test]
async fn task7_diagnostic_cancelled_waiter_owner_error_is_not_retained_for_late_parent() {
    let registry=Arc::new(StandardResourceRegistry::default());
    let admission=registry.begin_admission(0).unwrap();
    let actor=error_actor(true,true,0);
    let handle:Arc<dyn StandardResourceHandle>=actor.clone();
    let registrations=registry.register_batch(&admission,vec![handle]).await.unwrap();
    let closing=registry.clone();
    let waiter=tokio::spawn(async move {closing.close_registered_after_admission(&registrations,&admission).await});
    tokio::time::timeout(Duration::from_secs(2),actor.owner.entered.notified()).await.unwrap();
    waiter.abort();assert!(waiter.await.expect_err("abort").is_cancelled());
    actor.owner.finish.notify_one();
    tokio::time::timeout(Duration::from_secs(2),async {
        while actor.owner.completed.load(Ordering::SeqCst)==0 {tokio::task::yield_now().await;}
    }).await.unwrap();
    assert_eq!(actor.context.state(),ResourceState::Freed);
    registry.close_all().await.expect("observed late parent loses prior owner error");
    assert_eq!(actor.driver_calls.load(Ordering::SeqCst),1);
    assert_eq!(actor.owner.calls.load(Ordering::SeqCst),1);
    println!("DIAGNOSTIC owner_failed=true waiter_cancelled=true late_parent_ok=true driver_calls=1 owner_calls=1; owner disposition NOT_CERTIFIED");
}
