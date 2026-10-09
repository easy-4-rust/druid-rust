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
    registration: OnceLock<StandardResourceRegistration>,
}
#[async_trait::async_trait]
impl StandardResourceHandle for ReleaseActor {
    async fn close_resource(&self) -> Result<(), RdbcError> {
        let result = self.context.release_with(|| async {
            self.driver_calls.fetch_add(1, Ordering::SeqCst);
            self.driver_started.notify_one();
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
async fn cancellation_contract(registry_owned: bool, owner_window: bool) {
    let owner = ControlledOwner::new(owner_window, false);
    let actor = Arc::new(ReleaseActor {
        context: context(Arc::clone(&owner)), owner,
        driver_started: Notify::new(), driver_finish: Notify::new(),
        block_driver: !owner_window, driver_calls: AtomicUsize::new(0),
        driver_completed: AtomicUsize::new(0), registration: OnceLock::new(),
    });
    let registry = Arc::new(StandardResourceRegistry::default());
    let caller = if registry_owned {
        let admission = registry.begin_admission(0).expect("admit original generation");
        let handle: Arc<dyn StandardResourceHandle> = actor.clone();
        let registrations = registry.register_batch(&admission, vec![handle]).await.expect("register actor");
        let closing = Arc::clone(&registry);
        tokio::spawn(async move {
            closing.close_registered_after_admission(&registrations, &admission).await
        })
    } else {
        let closing = Arc::clone(&actor);
        tokio::spawn(async move { closing.close_resource().await })
    };
    tokio::time::timeout(Duration::from_secs(2), async {
        if owner_window { actor.owner.entered.notified().await; }
        else { actor.driver_started.notified().await; }
    }).await.expect("observed exact release window");
    assert_eq!(actor.context.state(), ResourceState::Releasing);
    if registry_owned {
        assert!(registry.has_open_resources());
        assert!(registry.open_next_generation().is_err(), "cannot recycle while resource release is pending");
    }
    caller.abort();
    assert!(caller.await.expect_err("caller abort").is_cancelled());
    if owner_window { actor.owner.finish.notify_one(); }
    else { actor.driver_finish.notify_one(); }
    let successor = tokio::time::timeout(Duration::from_millis(250), async {
        if registry_owned { registry.close_all().await }
        else { actor.close_resource().await }
    }).await;
    assert!(matches!(successor, Ok(Ok(()))), "release must complete after caller cancellation: {successor:?}");
    assert_eq!(actor.driver_calls.load(Ordering::SeqCst), 1);
    assert_eq!(actor.driver_completed.load(Ordering::SeqCst), 1);
    assert_eq!(actor.owner.calls.load(Ordering::SeqCst), 1);
    assert_eq!(actor.owner.completed.load(Ordering::SeqCst), 1);
    assert_eq!(actor.context.state(), ResourceState::Freed);
    assert!(actor.close_resource().await.is_ok());
    assert_eq!(actor.driver_calls.load(Ordering::SeqCst), 1);
    assert_eq!(actor.owner.calls.load(Ordering::SeqCst), 1);
    if registry_owned {
        assert!(!registry.has_open_resources());
        assert_eq!(registry.open_next_generation().expect("clean next generation"), 1);
    }
}
#[tokio::test]
async fn task7_direct_driver_window_cancellation_completes() { cancellation_contract(false, false).await; }
#[tokio::test]
async fn task7_direct_owner_window_cancellation_completes() { cancellation_contract(false, true).await; }
#[tokio::test]
async fn task7_registry_driver_window_cancellation_completes() { cancellation_contract(true, false).await; }
#[tokio::test]
async fn task7_registry_owner_window_cancellation_completes() { cancellation_contract(true, true).await; }
