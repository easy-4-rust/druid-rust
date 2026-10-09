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
struct BlockingOwner { entered: Notify, finish: Notify, calls: AtomicUsize, completed: AtomicUsize }
#[async_trait::async_trait]
impl ResourceOwner for BlockingOwner {
 async fn resource_released(&self, _: &ResourceId, _: ResourceKind) -> Result<(),RdbcError> {
  self.calls.fetch_add(1,Ordering::SeqCst);
  self.entered.notify_one();
  self.finish.notified().await;
  self.completed.fetch_add(1,Ordering::SeqCst);
  Ok(())
 }
}
fn main() {
 let mode=std::env::args().nth(1).unwrap();
 tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async move {
  let owner=Arc::new(BlockingOwner { entered:Notify::new(),finish:Notify::new(),calls:AtomicUsize::new(0),completed:AtomicUsize::new(0) });
  let context=Arc::new(ResourceContext::new(ResourceId::new("cancel-contract"),ResourceKind::Blob,ResourceRuntimeCapabilities::FREE,owner.clone()));
  let driver_entered=Arc::new(Notify::new()); let driver_finish=Arc::new(Notify::new()); let driver_calls=Arc::new(AtomicUsize::new(0));
  let c=context.clone();let entered=driver_entered.clone();let finish=driver_finish.clone();let calls=driver_calls.clone();let block_driver=mode=="driver";
  let winner=tokio::spawn(async move {c.release_with(move || async move {
   calls.fetch_add(1,Ordering::SeqCst);entered.notify_one();
   if block_driver {finish.notified().await;} Ok(())
  }).await});
  if block_driver {tokio::time::timeout(Duration::from_secs(2),driver_entered.notified()).await.unwrap();}
  else {tokio::time::timeout(Duration::from_secs(2),owner.entered.notified()).await.unwrap();}
  winner.abort();assert!(winner.await.unwrap_err().is_cancelled());
  driver_finish.notify_one();owner.finish.notify_one();
  let next_calls=driver_calls.clone();
  let result=tokio::time::timeout(Duration::from_millis(250),context.release_with(move || async move {next_calls.fetch_add(1,Ordering::SeqCst);Ok(())})).await;
  println!("window={mode} state={:?} driver_calls={} owner_calls={} owner_completed={} successor_timeout={}",context.state(),driver_calls.load(Ordering::SeqCst),owner.calls.load(Ordering::SeqCst),owner.completed.load(Ordering::SeqCst),result.is_err());
  assert!(result.is_ok(),"cancelled release must complete both driver and owner phases");
  assert!(result.unwrap().is_ok());assert_eq!(context.state(),ResourceState::Freed);
  assert_eq!(driver_calls.load(Ordering::SeqCst),1);assert_eq!(owner.calls.load(Ordering::SeqCst),1);assert_eq!(owner.completed.load(Ordering::SeqCst),1);
 });
}
