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

mod payload_fixture {
#[path = "F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008/task7-canonical-blob-candidate1.rs"]
mod canonical_blob;
use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{RdbcInputStream, RdbcOutputStream, ResourceOperations};
use canonical_blob::Blob;
use std::cell::Cell;
use std::future::Future;

/// 真实外部可变资源合同夹具；Cell 显式保留 Send、排除 Sync。
#[derive(Debug)]
struct SendOnlyBlob {
    bytes: Vec<u8>,
    freed: bool,
    releases: Cell<usize>,
    probe: ReleaseProbe,
}
impl SendOnlyBlob {
    fn ensure_open(&self) -> Result<(), RdbcError> {
        if self.freed { Err(RdbcError::rdbc_resource_closed("Blob")) } else { Ok(()) }
    }
    fn offset(position: i64) -> Result<usize, RdbcError> {
        position.checked_sub(1).filter(|value| *value >= 0)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| RdbcError::InvalidArgument("invalid 1-based position".into()))
    }
}
#[async_trait::async_trait]
impl Blob for SendOnlyBlob {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::READ | ResourceOperations::WRITE | ResourceOperations::RANGE |
        ResourceOperations::SEARCH | ResourceOperations::TRUNCATE | ResourceOperations::FREE
    }
    async fn length(&mut self) -> Result<i64, RdbcError> {
        self.ensure_open()?;
        i64::try_from(self.bytes.len()).map_err(|_| RdbcError::InvalidArgument("length overflow".into()))
    }
    async fn get_bytes(&mut self, position: i64, length: i32) -> Result<Vec<u8>, RdbcError> {
        self.ensure_open()?;
        let start=Self::offset(position)?;
        let count=usize::try_from(length).map_err(|_| RdbcError::InvalidArgument("negative length".into()))?;
        let tail=self.bytes.get(start..).ok_or_else(|| RdbcError::InvalidArgument("position outside blob".into()))?;
        Ok(tail[..count.min(tail.len())].to_vec())
    }
    async fn set_bytes(&mut self, position: i64, bytes: &[u8]) -> Result<i32,RdbcError> {
        self.ensure_open()?;
        let start=Self::offset(position)?;
        if start > self.bytes.len() {return Err(RdbcError::InvalidArgument("write position outside blob".into()));}
        let length=i32::try_from(bytes.len()).map_err(|_| RdbcError::InvalidArgument("write length overflow".into()))?;
        let end=start.checked_add(bytes.len()).ok_or_else(|| RdbcError::InvalidArgument("write range overflow".into()))?;
        self.bytes.resize(self.bytes.len().max(end),0);
        self.bytes[start..end].copy_from_slice(bytes);
        Ok(length)
    }
    async fn set_bytes_range(&mut self, position:i64, bytes:&[u8], offset:i32, length:i32) -> Result<i32,RdbcError> {
        self.ensure_open()?;
        let start=usize::try_from(offset).map_err(|_| RdbcError::InvalidArgument("negative byte offset".into()))?;
        let count=usize::try_from(length).map_err(|_| RdbcError::InvalidArgument("negative byte length".into()))?;
        let end=start.checked_add(count).ok_or_else(|| RdbcError::InvalidArgument("byte range overflow".into()))?;
        let range=bytes.get(start..end).ok_or_else(|| RdbcError::InvalidArgument("byte range outside input".into()))?;
        self.set_bytes(position,range).await
    }
    async fn position_bytes(&mut self, pattern:&[u8], start:i64) -> Result<Option<i64>,RdbcError> {
        self.ensure_open()?;
        let offset=Self::offset(start)?;
        let tail=self.bytes.get(offset..).ok_or_else(|| RdbcError::InvalidArgument("search start outside blob".into()))?;
        if pattern.is_empty() {return Ok(Some(start));}
        Ok(tail.windows(pattern.len()).position(|window| window==pattern)
            .map(|found| i64::try_from(offset+found+1).expect("offset bounded by blob length")))
    }
    async fn position_blob(&mut self, pattern:&mut dyn Blob, start:i64) -> Result<Option<i64>,RdbcError> {
        self.ensure_open()?;
        let length=i32::try_from(pattern.length().await?).map_err(|_| RdbcError::InvalidArgument("pattern exceeds i32".into()))?;
        let bytes=pattern.get_bytes(1,length).await?;
        self.position_bytes(&bytes,start).await
    }
    async fn truncate(&mut self, length:i64) -> Result<(),RdbcError> {
        self.ensure_open()?;
        let length=usize::try_from(length).map_err(|_| RdbcError::InvalidArgument("negative truncate length".into()))?;
        if length>self.bytes.len() {return Err(RdbcError::InvalidArgument("truncate expands blob".into()));}
        self.bytes.truncate(length); Ok(())
    }
    async fn free(&mut self) -> Result<(),RdbcError> {
        self.probe.calls.fetch_add(1, Ordering::SeqCst);
        self.probe.started.notify_one();
        if self.probe.block { self.probe.finish.notified().await; }
        if !self.freed { self.releases.set(self.releases.get()+1); self.freed=true; }
        Ok(())
    }
    async fn get_binary_stream(&mut self) -> Result<RdbcInputStream,RdbcError> {
        self.ensure_open()?;Err(RdbcError::feature_not_supported("Blob#getBinaryStream"))
    }
    async fn set_binary_stream(&mut self,_position:i64) -> Result<RdbcOutputStream,RdbcError> {
        self.ensure_open()?;Err(RdbcError::feature_not_supported("Blob#setBinaryStream"))
    }
    async fn get_binary_stream_range(&mut self,_position:i64,_length:i64) -> Result<RdbcInputStream,RdbcError> {
        self.ensure_open()?;Err(RdbcError::feature_not_supported("Blob#getBinaryStream(long,long)"))
    }
}
fn send_future<F: Future + Send>(future:F) -> F {future}
fn send_static<T:Send+'static>() {}
fn blob() -> SendOnlyBlob {SendOnlyBlob {bytes:vec![1,2,3,4],freed:false,releases:Cell::new(0),probe:ReleaseProbe::new(false)}}

use std::sync::{Arc,atomic::{AtomicUsize,Ordering}};
use tokio::sync::Notify;
#[derive(Clone, Debug)]
pub(super) struct ReleaseProbe {
    pub(super) calls: Arc<AtomicUsize>,
    pub(super) started: Arc<Notify>,
    pub(super) finish: Arc<Notify>,
    block: bool,
}
impl ReleaseProbe {
    fn new(block: bool) -> Self {
        Self { calls:Arc::new(AtomicUsize::new(0)),started:Arc::new(Notify::new()),finish:Arc::new(Notify::new()),block }
    }
}
pub(super) fn controlled(block: bool) -> (Box<dyn Blob>,ReleaseProbe) {
    let probe=ReleaseProbe::new(block);
    (Box::new(SendOnlyBlob { bytes:vec![1,2,3,4],freed:false,releases:Cell::new(0),probe:probe.clone() }),probe)
}
pub(super) use canonical_blob::Blob as StandardBlob;
#[cfg(task7_require_driver_sync)]
fn negative_sync_control() {
    fn requires_sync<T: Sync>() {}
    requires_sync::<SendOnlyBlob>();
}

}

/// 仅存在于证据harness：内部同步actor独占一个Send非Sync的标准候选驱动资源。
struct BlobActor {
    payload: tokio::sync::Mutex<Box<dyn payload_fixture::StandardBlob>>,
    context: Arc<ResourceContext>,
    registration: OnceLock<StandardResourceRegistration>,
}
#[async_trait::async_trait]
impl StandardResourceHandle for BlobActor {
    async fn close_resource(&self) -> Result<(),RdbcError> {
        let result=self.context.release_with(|| async { self.payload.lock().await.free().await }).await;
        if self.is_resource_closed() { self.unregister_resource(); }
        result
    }
    fn is_resource_closed(&self)->bool {self.context.state()==ResourceState::Freed}
    fn registration_slot(&self)->&OnceLock<StandardResourceRegistration>{&self.registration}
}
async fn parent_close_race(owner_window:bool) {
    fn requires_send_sync<T:Send+Sync+'static>(){}
    requires_send_sync::<BlobActor>();
    let (payload,probe)=payload_fixture::controlled(!owner_window);
    let owner=ControlledOwner::new(owner_window,false);
    let actor=Arc::new(BlobActor {payload:tokio::sync::Mutex::new(payload),context:context(owner.clone()),registration:OnceLock::new()});
    assert_eq!(actor.payload.lock().await.length().await.expect("real candidate read"),4);
    let registry=Arc::new(StandardResourceRegistry::default());
    let admission=registry.begin_admission(0).expect("admit");
    let handle:Arc<dyn StandardResourceHandle>=actor.clone();
    let registrations=registry.register_batch(&admission,vec![handle]).await.expect("register");
    let explicit_registry=registry.clone();
    let explicit=tokio::spawn(async move {explicit_registry.close_registered_after_admission(&registrations,&admission).await});
    tokio::time::timeout(Duration::from_secs(2),async {
        if owner_window {owner.entered.notified().await;} else {probe.started.notified().await;}
    }).await.expect("exact driver/owner window");
    let parent_registry=registry.clone();
    let mut parent=tokio::spawn(async move {parent_registry.close_all().await});
    // JoinHandle等待超时不取消实际父级关闭任务。
    assert!(tokio::time::timeout(Duration::from_millis(20),&mut parent).await.is_err());
    assert!(registry.has_open_resources());
    assert!(registry.open_next_generation().is_err());
    explicit.abort();
    assert!(explicit.await.expect_err("explicit caller cancelled").is_cancelled());
    if owner_window {owner.finish.notify_one();} else {probe.finish.notify_one();}
    tokio::time::timeout(Duration::from_millis(500),&mut parent).await.expect("parent drain completes").expect("parent task").expect("parent close");
    assert_eq!(probe.calls.load(Ordering::SeqCst),1);
    assert_eq!(owner.calls.load(Ordering::SeqCst),1);
    assert_eq!(owner.completed.load(Ordering::SeqCst),1);
    assert_eq!(actor.context.state(),ResourceState::Freed);
    assert!(actor.payload.lock().await.length().await.is_err());
    actor.close_resource().await.expect("repeat logical close");
    assert_eq!(probe.calls.load(Ordering::SeqCst),1);
    assert_eq!(owner.calls.load(Ordering::SeqCst),1);
    assert!(!registry.has_open_resources());
    registry.close_all().await.expect("repeat parent close");
    assert_eq!(registry.open_next_generation().expect("clean reopen"),1);
}
#[tokio::test]
async fn task7_send_only_blob_parent_close_survives_explicit_driver_cancel(){parent_close_race(false).await;}
#[tokio::test]
async fn task7_send_only_blob_parent_close_survives_explicit_owner_cancel(){parent_close_race(true).await;}
