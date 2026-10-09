use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{RdbcBlobAccess, RdbcResourceAccess, ResourceHandleFactory, ResourceOperations};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;
use tokio::sync::Notify;
#[derive(Debug)]
struct ControlledBlob { entered: Notify, finish: Notify, calls: AtomicUsize, finished: AtomicBool }
#[async_trait::async_trait]
impl RdbcResourceAccess for ControlledBlob {
    fn capabilities(&self) -> ResourceOperations { ResourceOperations::READ | ResourceOperations::FREE }
    async fn free(&self) -> Result<(), RdbcError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.entered.notify_one();
        self.finish.notified().await;
        self.finished.store(true, Ordering::SeqCst);
        Ok(())
    }
}
#[async_trait::async_trait]
impl RdbcBlobAccess for ControlledBlob {
    async fn length(&self) -> Result<i64, RdbcError> { Ok(3) }
    async fn get_bytes(&self, position: i64, length: i32) -> Result<Vec<u8>, RdbcError> {
        if position < 1 || length < 0 { return Err(RdbcError::InvalidArgument("invalid range".into())); }
        let start = usize::try_from(position - 1).map_err(|_| RdbcError::InvalidArgument("position overflow".into()))?;
        let end = start.checked_add(length as usize).ok_or_else(|| RdbcError::InvalidArgument("range overflow".into()))?;
        [7u8, 8, 9].get(start..end).map(<[u8]>::to_vec).ok_or_else(|| RdbcError::InvalidArgument("out of range".into()))
    }
}
fn main() {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    runtime.block_on(async {
        let access = Arc::new(ControlledBlob { entered: Notify::new(), finish: Notify::new(), calls: AtomicUsize::new(0), finished: AtomicBool::new(false) });
        let blob = ResourceHandleFactory::blob(access.clone());
        let alias = blob.clone();
        let winner = tokio::spawn(async move { alias.free().await });
        tokio::time::timeout(Duration::from_secs(2), access.entered.notified()).await.expect("driver free must start");
        winner.abort();
        let aborted = winner.await.expect_err("winning caller is cancelled").is_cancelled();
        access.finish.notify_one();
        let successor = tokio::time::timeout(Duration::from_millis(250), blob.free()).await;
        println!("aborted={aborted} driver_calls={} driver_finished={} alias_freed={} successor_timeout={} read_rejected={}", access.calls.load(Ordering::SeqCst), access.finished.load(Ordering::SeqCst), blob.is_freed(), successor.is_err(), blob.length().await.is_err());
        assert!(successor.is_ok(), "cancelled free must not strand the shared resource in Releasing");
        assert!(successor.unwrap().is_ok());
        assert_eq!(access.calls.load(Ordering::SeqCst), 1);
        assert!(access.finished.load(Ordering::SeqCst));
        assert!(blob.is_freed());
    });
}
