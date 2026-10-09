mod core {
pub(crate) use druid_rdbc::error::RdbcError;
#[path = "E:/workspaces/workspace-github-easy-4-rust/druid-rust/crates/druid/src/core/standard_resource_admission.rs"]
pub(crate) mod standard_resource_admission;
#[path = "E:/workspaces/workspace-github-easy-4-rust/druid-rust/crates/druid/src/core/standard_resource_handle.rs"]
pub(crate) mod standard_resource_handle;
#[path = "F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008/task7-full-druid-source-candidate1/src/core/standard_resource_registration.rs"]
pub(crate) mod standard_resource_registration;
#[path = "E:/workspaces/workspace-github-easy-4-rust/druid-rust/crates/druid/src/core/standard_resource_registry_state.rs"]
pub(crate) mod standard_resource_registry_state;
#[path = "F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008/task7-full-druid-source-candidate1/src/core/standard_resource_registry.rs"]
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
#[path="F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008/task7-full-druid-source-candidate1/src/core/druid_blob_state.rs"]
pub(crate) mod druid_blob_state;
#[path="F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008/task7-full-druid-source-candidate1/src/core/druid_blob.rs"]
pub(crate) mod druid_blob;
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
        ResourceRuntimeCapabilities::large_object(), owner))
}

use core::standard_resource_registry::StandardResourceRegistry;
use core::druid_blob::DruidBlob;
use druid_rdbc::resource::Blob;

mod payload_fixture {
use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{RdbcInputStream, RdbcOutputStream, ResourceOperations};
use druid_rdbc::resource::Blob;
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
        if self.probe.failures_remaining.fetch_update(Ordering::SeqCst,Ordering::SeqCst,|count|count.checked_sub(1)).is_ok() {return Err(RdbcError::Other("driver release failed".into()));}
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
    pub(super) failures_remaining: Arc<AtomicUsize>,
}
impl ReleaseProbe {
    fn new(block: bool) -> Self {
        Self { calls:Arc::new(AtomicUsize::new(0)),started:Arc::new(Notify::new()),finish:Arc::new(Notify::new()),block,failures_remaining:Arc::new(AtomicUsize::new(0)) }
    }
}
pub(super) fn controlled(block: bool) -> (Box<dyn Blob>,ReleaseProbe) {
    let probe=ReleaseProbe::new(block);
    (Box::new(SendOnlyBlob { bytes:vec![1,2,3,4],freed:false,releases:Cell::new(0),probe:probe.clone() }),probe)
}
#[cfg(task7_require_driver_sync)]
fn negative_sync_control() {
    fn requires_sync<T: Sync>() {}
    requires_sync::<SendOnlyBlob>();
}

}


async fn proxy(owner:Arc<ControlledOwner>,block:bool,registry:&Arc<StandardResourceRegistry>) -> (DruidBlob,payload_fixture::ReleaseProbe,Arc<ResourceContext>) {
    let (payload,probe)=payload_fixture::controlled(block);
    let ctx=context(owner);
    let admission=registry.begin_admission(0).expect("admission");
    let blob=DruidBlob::register(payload,ctx.clone(),registry.clone(),&admission).await.expect("register proxy");
    (blob,probe,ctx)
}
async fn public_cancel(owner_window:bool) {
    let registry=Arc::new(StandardResourceRegistry::default());
    let owner=ControlledOwner::new(owner_window,false);
    let (mut blob,probe,ctx)=proxy(owner.clone(),!owner_window,&registry).await;
    assert_eq!(blob.length().await.expect("read"),4);
    let explicit=tokio::spawn(async move {blob.free().await});
    tokio::time::timeout(Duration::from_secs(2),async {
        if owner_window {owner.entered.notified().await;} else {probe.started.notified().await;}
    }).await.expect("release window");
    let parent_registry=registry.clone();
    let mut parent=tokio::spawn(async move {parent_registry.close_all().await});
    assert!(tokio::time::timeout(Duration::from_millis(20),&mut parent).await.is_err());
    assert!(registry.begin_admission(0).is_err());
    assert!(registry.open_next_generation().is_err());
    explicit.abort();
    assert!(explicit.await.expect_err("cancel public free caller").is_cancelled());
    if owner_window {owner.finish.notify_one();} else {probe.finish.notify_one();}
    tokio::time::timeout(Duration::from_millis(500),parent).await.expect("drain timeout").expect("join").expect("parent drain");
    assert_eq!(probe.calls.load(Ordering::SeqCst),1);
    assert_eq!(owner.calls.load(Ordering::SeqCst),1);
    assert_eq!(owner.completed.load(Ordering::SeqCst),1);
    assert_eq!(ctx.state(),ResourceState::Freed);
    assert_eq!(registry.open_next_generation().expect("reopen"),1);
}
#[tokio::test]
async fn task7_public_blob_driver_cancel(){public_cancel(false).await;}
#[tokio::test]
async fn task7_public_blob_owner_cancel(){public_cancel(true).await;}
#[tokio::test]
async fn task7_public_blob_read_write_capability_and_repeat_free() {
    let registry=Arc::new(StandardResourceRegistry::default());
    let owner=ControlledOwner::new(false,false);
    let (mut blob,probe,_)=proxy(owner.clone(),false,&registry).await;
    assert!(!blob.capabilities().contains(druid_rdbc::resource::ResourceOperations::STREAM));
    assert!(blob.get_binary_stream().await.is_err());
    assert!(blob.set_binary_stream(1).await.is_err());
    assert!(blob.get_binary_stream_range(1,1).await.is_err());
    assert_eq!(blob.get_bytes(2,20).await.expect("read tail"),vec![2,3,4]);
    assert_eq!(blob.position_bytes(&[3],1).await.expect("search"),Some(3));
    assert_eq!(blob.set_bytes(2,&[7,8]).await.expect("write"),2);
    assert_eq!(blob.set_bytes_range(4,&[9,10,11],1,2).await.expect("write range"),2);
    assert_eq!(blob.get_bytes(1,20).await.expect("written bytes"),vec![1,7,8,10,11]);
    let (mut pattern,_,_)=proxy(ControlledOwner::new(false,false),false,&registry).await;
    pattern.truncate(1).await.expect("pattern truncate");
    assert_eq!(blob.position_blob(&mut pattern,1).await.expect("blob search"),Some(1));
    blob.truncate(3).await.expect("truncate");
    assert_eq!(blob.length().await.expect("length"),3);
    blob.free().await.expect("free");blob.free().await.expect("repeat free");
    assert!(blob.length().await.is_err());
    assert!(blob.get_bytes(0,-1).await.is_err());
    assert_eq!(probe.calls.load(Ordering::SeqCst),1);
    assert_eq!(owner.calls.load(Ordering::SeqCst),1);
    assert!(pattern.length().await.is_ok());
    drop(pattern);
    registry.close_all().await.expect("parent closes dropped sibling");
}
#[tokio::test]
async fn task7_public_blob_parent_close_invalidates_operations() {
    let registry=Arc::new(StandardResourceRegistry::default());
    let (mut blob,probe,_)=proxy(ControlledOwner::new(false,false),false,&registry).await;
    registry.close_all().await.expect("close parent");
    assert!(blob.length().await.is_err());
    blob.free().await.expect("already freed");
    assert_eq!(probe.calls.load(Ordering::SeqCst),1);
}
#[tokio::test]
async fn task7_public_blob_late_parent_retains_owner_error_without_replaying_driver() {
    let registry=Arc::new(StandardResourceRegistry::default());
    let owner=ControlledOwner::new(true,true);
    let (mut blob,probe,ctx)=proxy(owner.clone(),false,&registry).await;
    let explicit=tokio::spawn(async move {blob.free().await});
    tokio::time::timeout(Duration::from_secs(2),owner.entered.notified()).await.expect("owner window");
    explicit.abort();assert!(explicit.await.expect_err("cancel waiter").is_cancelled());
    owner.finish.notify_one();
    tokio::time::timeout(Duration::from_secs(2),async {
        while ctx.state()!=ResourceState::Freed {tokio::task::yield_now().await;}
    }).await.expect("driver and owner completed");
    let error=registry.close_all().await.expect_err("retain terminal owner error for late parent");
    assert!(error.to_string().contains("owner notification failed"));
    assert_eq!(probe.calls.load(Ordering::SeqCst),1);
    assert_eq!(owner.calls.load(Ordering::SeqCst),1);
    assert!(registry.open_next_generation().is_err());
}

#[tokio::test]
async fn task7_public_blob_driver_failure_is_retryable_and_owner_is_not_notified_early() {
    let registry=Arc::new(StandardResourceRegistry::default());
    let owner=ControlledOwner::new(false,false);
    let (mut blob,probe,ctx)=proxy(owner.clone(),false,&registry).await;
    probe.failures_remaining.store(1,Ordering::SeqCst);
    assert!(blob.free().await.expect_err("driver failure").to_string().contains("driver release failed"));
    assert_eq!(ctx.state(),ResourceState::Open);
    assert_eq!(owner.calls.load(Ordering::SeqCst),0);
    assert_eq!(owner.failures.load(Ordering::SeqCst),1);
    blob.free().await.expect("retry original resource release");
    assert_eq!(probe.calls.load(Ordering::SeqCst),2);
    assert_eq!(owner.calls.load(Ordering::SeqCst),1);
    registry.close_all().await.expect("clean parent");
}
#[tokio::test]
async fn task7_public_blob_parent_closing_rejects_read_before_driver_finishes() {
    let registry=Arc::new(StandardResourceRegistry::default());
    let (mut blob,probe,_)=proxy(ControlledOwner::new(false,false),true,&registry).await;
    let parent_registry=registry.clone();
    let parent=tokio::spawn(async move {parent_registry.close_all().await});
    tokio::time::timeout(Duration::from_secs(2),probe.started.notified()).await.expect("driver close started");
    assert!(tokio::time::timeout(Duration::from_millis(500),blob.length()).await.expect("closed check must not wait on driver lock").is_err());
    let explicit=tokio::spawn(async move {blob.free().await});
    probe.finish.notify_one();
    parent.await.expect("parent join").expect("parent close");
    explicit.await.expect("public free join").expect("join existing cleanup");
    assert_eq!(probe.calls.load(Ordering::SeqCst),1);
}
