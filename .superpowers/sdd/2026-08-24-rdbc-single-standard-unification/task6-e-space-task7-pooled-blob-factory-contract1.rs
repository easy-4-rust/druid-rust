use druid::core::{DruidConnectionHolder,DruidPooledConnection,ConnectionRecycleDisposition};
use druid_rdbc::connection::Connection;
use druid_rdbc::error::{RdbcError,SqlException};
use druid_rdbc::resource::Blob;
use std::sync::{Arc,Mutex,atomic::{AtomicBool,AtomicUsize,Ordering}};
use std::time::Duration;
fn fatal_error() -> RdbcError {RdbcError::SqlException(Box::new(SqlException::driver(77,"blob driver failure").with_sql_state("08006")))}
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
        if self.probe.read_failure.load(Ordering::SeqCst) {return Err(super::fatal_error());}
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
        if self.probe.failures_remaining.fetch_update(Ordering::SeqCst,Ordering::SeqCst,|count|count.checked_sub(1)).is_ok() {return Err(super::fatal_error());}
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
    pub(super) read_failure: Arc<std::sync::atomic::AtomicBool>,
}
impl ReleaseProbe {
    fn new(block: bool) -> Self {
        Self { calls:Arc::new(AtomicUsize::new(0)),started:Arc::new(Notify::new()),finish:Arc::new(Notify::new()),block,failures_remaining:Arc::new(AtomicUsize::new(0)),read_failure:Arc::new(std::sync::atomic::AtomicBool::new(false)) }
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


#[derive(Debug,Default)]
struct Control {
    probes:Mutex<Vec<payload_fixture::ReleaseProbe>>,
    created:AtomicUsize,
    sorter_calls:AtomicUsize,
    discards:AtomicUsize,
    block_free:AtomicBool,
    fail_create:AtomicBool,
    returns:Mutex<Vec<ConnectionRecycleDisposition>>,
}
#[derive(Debug)]
struct DriverConnection {control:Arc<Control>,closed:bool}
druid_rdbc::impl_standard_wrapper!(DriverConnection => Connection);
#[async_trait::async_trait]
impl Connection for DriverConnection {
    async fn create_blob(&mut self) -> Result<Box<dyn Blob>,RdbcError> {
        if self.closed {return Err(RdbcError::ConnectionDiscarded);}
        self.control.created.fetch_add(1,Ordering::SeqCst);
        if self.control.fail_create.load(Ordering::SeqCst) {return Err(fatal_error());}
        let (payload,probe)=payload_fixture::controlled(self.control.block_free.load(Ordering::SeqCst));
        self.control.probes.lock().unwrap().push(probe);
        Ok(payload)
    }
    fn is_closed(&self) -> Result<bool,RdbcError> {Ok(self.closed)}
    async fn close(&mut self) -> Result<(),RdbcError> {self.closed=true;Ok(())}
}
async fn pooled(control:Arc<Control>) -> DruidPooledConnection {
    let holder=DruidConnectionHolder::with_standard_connection(Box::new(DriverConnection {control:control.clone(),closed:false}),42,Duration::ZERO,0).await.expect("capture actual defaults");
    let observed=control.clone();
    DruidPooledConnection::with_standard_holder_and_disposition_observer(holder,"blob-contract".into(),None,false,false,Box::new(move |disposition| {assert!(!disposition.is_reusable());observed.discards.fetch_add(1,Ordering::SeqCst);}),Box::new(move |_,disposition| {
        control.returns.lock().unwrap().push(disposition);true
    }))
}
#[tokio::test]
async fn task7_pooled_blob_factory_parent_close_and_distinct_resources() {
    let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;
    let mut first=connection.create_blob().await.expect("pooled factory delegates and wraps");
    let mut second=connection.create_blob().await.expect("independent resource");
    first.set_bytes(1,&[9]).await.expect("write");first.free().await.expect("explicit free");
    assert_eq!(second.get_bytes(1,4).await.expect("sibling"),vec![1,2,3,4]);
    connection.close().await.expect("parent closes all resources before return");
    assert!(second.length().await.is_err());second.free().await.expect("repeat free");
    let probes=control.probes.lock().unwrap();assert_eq!(probes.len(),2);
    for probe in probes.iter() {assert_eq!(probe.calls.load(Ordering::SeqCst),1);}
    let returns=control.returns.lock().unwrap();assert_eq!(returns.len(),1);assert!(returns[0].is_reusable());
}
#[tokio::test]
async fn task7_pooled_blob_closed_connection_rejects_creation_before_driver() {
    let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;
    connection.close().await.expect("close");
    assert!(connection.create_blob().await.is_err());assert_eq!(control.created.load(Ordering::SeqCst),0);
}
#[tokio::test]
async fn task7_pooled_blob_public_free_cancel_parent_waits_for_real_driver() {
    let control=Arc::new(Control::default());control.block_free.store(true,Ordering::SeqCst);
    let mut connection=pooled(control.clone()).await;let mut blob=connection.create_blob().await.expect("factory");
    let probe=control.probes.lock().unwrap()[0].clone();
    let explicit=tokio::spawn(async move {blob.free().await});
    tokio::time::timeout(Duration::from_secs(2),probe.started.notified()).await.expect("free entered");
    explicit.abort();assert!(explicit.await.expect_err("cancel caller").is_cancelled());
    let mut parent=tokio::spawn(async move {connection.close().await});
    assert!(tokio::time::timeout(Duration::from_millis(20),&mut parent).await.is_err());
    assert!(control.returns.lock().unwrap().is_empty());
    probe.finish.notify_one();
    tokio::time::timeout(Duration::from_secs(2),parent).await.expect("parent timeout").expect("parent join").expect("close");
    assert_eq!(probe.calls.load(Ordering::SeqCst),1);assert_eq!(control.returns.lock().unwrap().len(),1);
}

#[tokio::test]
async fn task7_pooled_blob_read_fatal_error_discards_same_lease_once() {
    let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;
    connection.set_exception_sorter(Arc::new(CountingSorter(control.clone())));
    let mut blob=connection.create_blob().await.expect("factory");
    let probe=control.probes.lock().unwrap()[0].clone();probe.read_failure.store(true,Ordering::SeqCst);
    let error=blob.length().await.expect_err("driver SQL error");
    assert_eq!(error.sql_exception().unwrap().error_code(),77);
    assert!(connection.is_closed().unwrap());
    assert_eq!(control.discards.load(Ordering::SeqCst),1);
    assert_eq!(control.sorter_calls.load(Ordering::SeqCst),1);
    assert!(blob.get_bytes(1,1).await.is_err());
    assert!(connection.create_blob().await.is_err());
    assert_eq!(control.created.load(Ordering::SeqCst),1);
    blob.free().await.expect("free remains allowed on discarded lease");
    connection.close().await.expect("discarded return");connection.close().await.expect("repeat return");
    let returns=control.returns.lock().unwrap();assert_eq!(returns.len(),1);assert!(!returns[0].is_reusable());
    assert_eq!(probe.calls.load(Ordering::SeqCst),1);
}
#[tokio::test]
async fn task7_pooled_blob_release_fatal_error_discards_and_retry_does_not_reuse() {
    let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;
    connection.set_exception_sorter(Arc::new(CountingSorter(control.clone())));
    let mut blob=connection.create_blob().await.expect("factory");let probe=control.probes.lock().unwrap()[0].clone();
    probe.failures_remaining.store(1,Ordering::SeqCst);
    assert_eq!(blob.free().await.expect_err("driver free failure").sql_exception().unwrap().error_code(),77);
    assert!(connection.is_closed().unwrap());
    assert_eq!(control.discards.load(Ordering::SeqCst),1);
    assert_eq!(control.sorter_calls.load(Ordering::SeqCst),1);
    blob.free().await.expect("retry supervised cleanup");
    connection.close().await.expect("discard return");
    assert_eq!(probe.calls.load(Ordering::SeqCst),2);
    let returns=control.returns.lock().unwrap();assert_eq!(returns.len(),1);assert!(!returns[0].is_reusable());
}
#[tokio::test]
async fn task7_pooled_blob_create_fatal_error_preserves_driver_error() {
    let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;
    connection.set_exception_sorter(Arc::new(CountingSorter(control.clone())));
    control.fail_create.store(true,Ordering::SeqCst);
    let error=connection.create_blob().await.expect_err("real driver creation failure");
    assert_eq!(error.sql_exception().unwrap().error_code(),77);assert!(connection.is_closed().unwrap());
    assert_eq!(control.discards.load(Ordering::SeqCst),1);assert_eq!(control.sorter_calls.load(Ordering::SeqCst),1);
    connection.close().await.expect("return discarded holder");
    assert!(control.probes.lock().unwrap().is_empty());
    let returns=control.returns.lock().unwrap();assert_eq!(returns.len(),1);assert!(!returns[0].is_reusable());
}

struct CountingSorter(Arc<Control>);
impl druid::core::ExceptionSorter for CountingSorter {
    fn is_exception_fatal(&self,exception:&SqlException)->bool {
        self.0.sorter_calls.fetch_add(1,Ordering::SeqCst);
        druid::core::ExceptionSorter::is_exception_fatal(&druid::core::PgExceptionSorter,exception)
    }
    fn config_from_properties(&mut self,properties:Option<&druid::core::ExceptionSorterProperties>) {
        druid::core::ExceptionSorter::config_from_properties(&mut druid::core::PgExceptionSorter,properties);
    }
}
#[tokio::test]
async fn task7_pooled_blob_old_resource_cannot_cross_holder_reuse_generation() {
    let control=Arc::new(Control::default());
    let holder=DruidConnectionHolder::with_standard_connection(Box::new(DriverConnection {control:control.clone(),closed:false}),44,Duration::ZERO,0).await.expect("holder");
    let returned=Arc::new(Mutex::new(None));let sink=returned.clone();
    let mut connection=DruidPooledConnection::with_standard_holder(holder,"first".into(),None,false,false,Box::new(move |holder,disposition| {
        assert!(disposition.is_reusable());*sink.lock().unwrap()=Some(holder);true
    }));
    let mut old=connection.create_blob().await.expect("old resource");
    connection.close().await.expect("close first lease");
    let holder=returned.lock().unwrap().take().expect("actual returned holder");
    let returns=control.clone();
    let mut next=DruidPooledConnection::with_standard_holder(holder,"next".into(),None,false,false,Box::new(move |_,disposition| {returns.returns.lock().unwrap().push(disposition);true}));
    let mut current=next.create_blob().await.expect("new generation resource");
    assert!(old.length().await.is_err());old.free().await.expect("old repeat free");
    assert_eq!(current.length().await.expect("current remains open"),4);
    let probes=control.probes.lock().unwrap().clone();
    assert_eq!(probes[0].calls.load(Ordering::SeqCst),1);assert_eq!(probes[1].calls.load(Ordering::SeqCst),0);
    next.close().await.expect("close new lease");assert_eq!(probes[1].calls.load(Ordering::SeqCst),1);
    assert!(control.returns.lock().unwrap()[0].is_reusable());
}
