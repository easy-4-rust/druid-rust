//! 资源所有者、取消与唯一标准句柄的端到端合同。
//! 对应 Java: DruidPooledConnection、DruidPooledResultSet 和 java.sql 资源。
//! 保留已验证的104项候选合同；正式Cargo运行另行记录，不能由源复制推定通过。

use druid_rdbc::resource::{SqlXml,RdbcXmlRepresentationType,RdbcXmlSource,RdbcXmlResult,RdbcInputStream,RdbcOutputStream,RdbcReader,RdbcWriter,ResourceOperations};
use druid::core::{DruidConnectionHolder,DruidPooledConnection,ConnectionRecycleDisposition};
use druid_rdbc::connection::Connection;
use druid_rdbc::error::{RdbcError,SqlException};
use druid_rdbc::resource::{Blob,Clob,NClob};
use druid_rdbc::types::RdbcString;
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
    async fn position_blob(&mut self, pattern:druid_rdbc::resource::BlobPattern<'_>, start:i64) -> Result<Option<i64>,RdbcError> {
        self.ensure_open()?;
        let bytes=match pattern {
            druid_rdbc::resource::BlobPattern::SameResource => self.bytes.clone(),
            druid_rdbc::resource::BlobPattern::Other(pattern) => {
                let length=i32::try_from(pattern.length().await?).map_err(|_| RdbcError::InvalidArgument("pattern exceeds i32".into()))?;
                pattern.get_bytes(1,length).await?
            }
        };
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
    pub(super) block: bool,
    pub(super) failures_remaining: Arc<AtomicUsize>,
    pub(super) read_failure: Arc<std::sync::atomic::AtomicBool>,
}
impl ReleaseProbe {
    pub(super) fn new(block: bool) -> Self {
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
    struct_args:Mutex<Vec<(Option<String>,Option<Vec<RdbcObject>>)>>,
    structs:Mutex<Vec<Arc<StructProbe>>>,
    array_disabled:AtomicUsize,
    arrays:Mutex<Vec<Arc<ArrayProbe>>>,
    probes:Mutex<Vec<payload_fixture::ReleaseProbe>>,
    created:AtomicUsize,
    factory_calls:Mutex<Vec<&'static str>>,
    sorter_calls:AtomicUsize,
    discards:AtomicUsize,
    block_free:AtomicBool,
    fail_create:AtomicBool,
    returns:Mutex<Vec<ConnectionRecycleDisposition>>,
    xml:Mutex<Vec<Arc<XmlProbe>>>,
    xml_no_stream:AtomicBool,
}
#[derive(Debug)]
struct DriverConnection {control:Arc<Control>,closed:bool}
druid_rdbc::impl_standard_wrapper!(DriverConnection => Connection);
#[async_trait::async_trait]
impl Connection for DriverConnection {
    async fn create_struct(&mut self,name:Option<&str>,attributes:Option<Vec<RdbcObject>>)->Result<Box<dyn druid_rdbc::resource::Struct>,RdbcError> {
        if self.closed {return Err(RdbcError::ConnectionDiscarded);}
        self.control.created.fetch_add(1,Ordering::SeqCst);self.control.factory_calls.lock().unwrap().push("create_struct");
        self.control.struct_args.lock().unwrap().push((name.map(str::to_owned),attributes.clone()));
        let name=name.ok_or_else(||RdbcError::SqlException(Box::new(SqlException::driver(94,"null struct name").with_sql_state("HY009"))))?;
        if self.control.fail_create.load(Ordering::SeqCst) {return Err(fatal_error());}
        let probe=Arc::new(StructProbe {name:name.into(),attributes:attributes.unwrap_or_default(),maps:Mutex::new(vec![]),fail:AtomicBool::new(false)});
        self.control.structs.lock().unwrap().push(probe.clone());
        Ok(Box::new(StructDriver {probe,calls:std::cell::Cell::new(0)}))
    }

    async fn create_array_of(&mut self, type_name:Option<&str>, elements:Option<Vec<RdbcObject>>) -> Result<Box<dyn Array>,RdbcError> {
        if self.closed {return Err(RdbcError::ConnectionDiscarded);}
        self.control.created.fetch_add(1,Ordering::SeqCst);
        self.control.factory_calls.lock().unwrap().push("create_array_of");
        if self.control.fail_create.load(Ordering::SeqCst) {return Err(fatal_error());}
        let probe=Arc::new(ArrayProbe { supported:ResourceOperations::array() & !ResourceOperations::from_bits_retain(self.control.array_disabled.load(Ordering::SeqCst) as _), free_failures:AtomicUsize::new(0), free_fatal:AtomicBool::new(false), nonfatal_read:AtomicBool::new(false), args:(type_name.map(str::to_owned),elements),
            maps:Mutex::new(vec![]), calls:Mutex::new(vec![]), frees:AtomicUsize::new(0),
            started:tokio::sync::Notify::new(), finish:tokio::sync::Notify::new(),
            block:self.control.block_free.load(Ordering::SeqCst), read_failure:AtomicBool::new(false),
            bad_metadata:AtomicBool::new(false), block_cursor:AtomicBool::new(false), cursor_started:tokio::sync::Notify::new(), cursor_finish:tokio::sync::Notify::new(), cursor_next_failure:AtomicBool::new(false), cursors:Mutex::new(vec![]) });
        self.control.arrays.lock().unwrap().push(probe.clone());
        Ok(Box::new(ArrayDriver {probe,freed:false,not_sync:std::cell::Cell::new(0)}))
    }

    async fn create_blob(&mut self) -> Result<Box<dyn Blob>,RdbcError> {
        if self.closed {return Err(RdbcError::ConnectionDiscarded);}
        self.control.created.fetch_add(1,Ordering::SeqCst);
        if self.control.fail_create.load(Ordering::SeqCst) {return Err(fatal_error());}
        let (payload,probe)=payload_fixture::controlled(self.control.block_free.load(Ordering::SeqCst));
        self.control.probes.lock().unwrap().push(probe);
        Ok(payload)
    }
    async fn create_clob(&mut self) -> Result<Box<dyn Clob>,RdbcError> {
        if self.closed {return Err(RdbcError::ConnectionDiscarded);}
        self.control.created.fetch_add(1,Ordering::SeqCst);
        self.control.factory_calls.lock().unwrap().push("create_clob");
        if self.control.fail_create.load(Ordering::SeqCst) {return Err(fatal_error());}
        let (payload,probe)=character_fixture::controlled(self.control.block_free.load(Ordering::SeqCst));
        self.control.probes.lock().unwrap().push(probe);Ok(Box::new(payload))
    }
    async fn create_n_clob(&mut self) -> Result<Box<dyn NClob>,RdbcError> {
        if self.closed {return Err(RdbcError::ConnectionDiscarded);}
        self.control.created.fetch_add(1,Ordering::SeqCst);
        self.control.factory_calls.lock().unwrap().push("create_n_clob");
        if self.control.fail_create.load(Ordering::SeqCst) {return Err(fatal_error());}
        let (payload,probe)=character_fixture::controlled(self.control.block_free.load(Ordering::SeqCst));
        self.control.probes.lock().unwrap().push(probe);Ok(Box::new(payload))
    }
    fn is_closed(&self) -> Result<bool,RdbcError> {Ok(self.closed)}
    async fn create_sql_xml(&mut self)->Result<Box<dyn SqlXml>,RdbcError> {
        if self.closed {return Err(RdbcError::ConnectionDiscarded);}
        self.control.created.fetch_add(1,Ordering::SeqCst);self.control.factory_calls.lock().unwrap().push("create_sql_xml");
        if self.control.fail_create.load(Ordering::SeqCst) {return Err(fatal_error());}
        let probe=Arc::new(XmlProbe::new(self.control.block_free.load(Ordering::SeqCst)));
        self.control.xml.lock().unwrap().push(probe.clone());
        Ok(Box::new(XmlDriver {probe,freed:false,not_sync:std::cell::Cell::new(0),stream:!self.control.xml_no_stream.load(Ordering::SeqCst)}))
    }
    async fn close(&mut self) -> Result<(),RdbcError> {self.closed=true;Ok(())}
}
async fn pooled(control:Arc<Control>) -> DruidPooledConnection {
    let holder=DruidConnectionHolder::with_standard_connection(Box::new(DriverConnection {control:control.clone(),closed:false}),42,Duration::ZERO,0).await.expect("capture actual defaults");
    let observed=control.clone();
    DruidPooledConnection::with_standard_holder_and_disposition_observer(holder,"blob-contract".into(),None,false,false,Box::new(move |disposition| {assert!(!disposition.is_reusable());observed.discards.fetch_add(1,Ordering::SeqCst);}),Box::new(move |_,disposition| {
        control.returns.lock().unwrap().push(disposition);true
    }))
}
async fn pooled_with_array_filter(control:Arc<Control>) -> DruidPooledConnection {
    let holder=DruidConnectionHolder::with_standard_connection(Box::new(DriverConnection {control:control.clone(),closed:false}),42,Duration::ZERO,0).await.expect("capture actual defaults");
    let observed=control.clone();
    DruidPooledConnection::with_standard_holder_and_disposition_observer(holder,"blob-contract".into(),Some(Arc::new(druid::core::FilterChain::new())),false,false,Box::new(move |disposition| {assert!(!disposition.is_reusable());observed.discards.fetch_add(1,Ordering::SeqCst);}),Box::new(move |_,disposition| {
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

mod character_fixture {
use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{RdbcInputStream,RdbcOutputStream,RdbcReader,RdbcWriter,ResourceOperations};
use druid_rdbc::resource::Clob;
use druid_rdbc::resource::NClob;
use druid_rdbc::types::RdbcString;
use std::cell::Cell;
use super::payload_fixture::ReleaseProbe;
use std::sync::atomic::Ordering;
#[derive(Debug)]
pub(super) struct SendOnlyClob { units: Vec<u16>, freed: bool, releases: Cell<usize>,probe:ReleaseProbe }
impl SendOnlyClob {
    fn ensure_open(&self) -> Result<(),RdbcError> {
        if self.freed {Err(RdbcError::rdbc_resource_closed("Clob"))} else {Ok(())}
    }
    fn offset(position:i64) -> Result<usize,RdbcError> {
        position.checked_sub(1).filter(|value| *value>=0).and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| RdbcError::InvalidArgument("invalid 1-based position".into()))
    }
    fn write_units(&mut self, position:i64, units:&[u16]) -> Result<i32,RdbcError> {
        self.ensure_open()?;let start=Self::offset(position)?;
        if start>self.units.len() {return Err(RdbcError::InvalidArgument("write position outside clob".into()));}
        let count=i32::try_from(units.len()).map_err(|_|RdbcError::InvalidArgument("write length overflow".into()))?;
        let end=start.checked_add(units.len()).ok_or_else(||RdbcError::InvalidArgument("write range overflow".into()))?;
        self.units.resize(self.units.len().max(end),0);self.units[start..end].copy_from_slice(units);Ok(count)
    }
}
#[async_trait::async_trait]
impl Clob for SendOnlyClob {
    fn capabilities(&self) -> ResourceOperations {
        ResourceOperations::READ|ResourceOperations::WRITE|ResourceOperations::RANGE|
        ResourceOperations::SEARCH|ResourceOperations::TRUNCATE|ResourceOperations::FREE
    }
    async fn length(&mut self) -> Result<i64,RdbcError> {
        self.ensure_open()?;if self.probe.read_failure.load(Ordering::SeqCst) {return Err(super::fatal_error());}
        i64::try_from(self.units.len()).map_err(|_|RdbcError::InvalidArgument("length overflow".into()))
    }
    async fn get_sub_string(&mut self, position:i64, length:i32) -> Result<RdbcString,RdbcError> {
        self.ensure_open()?;let start=Self::offset(position)?;
        let count=usize::try_from(length).map_err(|_|RdbcError::InvalidArgument("negative length".into()))?;
        let tail=self.units.get(start..).ok_or_else(||RdbcError::InvalidArgument("position outside clob".into()))?;
        Ok(RdbcString::from_utf16(tail[..count.min(tail.len())].to_vec()))
    }
    async fn position_string(&mut self, pattern:&RdbcString, start:i64) -> Result<Option<i64>,RdbcError> {
        self.ensure_open()?;let offset=Self::offset(start)?;
        let tail=self.units.get(offset..).ok_or_else(||RdbcError::InvalidArgument("search outside clob".into()))?;
        if pattern.is_empty() {return Ok(Some(start));}
        Ok(tail.windows(pattern.len()).position(|part|part==pattern.as_utf16())
            .map(|found|i64::try_from(offset+found+1).expect("bounded clob length")))
    }
    async fn position_clob(&mut self, pattern:druid_rdbc::resource::ClobPattern<'_>, start:i64) -> Result<Option<i64>,RdbcError> {
        self.ensure_open()?;
        let value=match pattern {
            druid_rdbc::resource::ClobPattern::SameResource => RdbcString::from_utf16(self.units.clone()),
            druid_rdbc::resource::ClobPattern::Other(pattern) => {
                let count=i32::try_from(pattern.length().await?).map_err(|_|RdbcError::InvalidArgument("pattern too long".into()))?;
                pattern.get_sub_string(1,count).await?
            }
        };
        self.position_string(&value,start).await
    }
    async fn set_string(&mut self, position:i64, value:&RdbcString) -> Result<i32,RdbcError> {
        self.write_units(position,value.as_utf16())
    }
    async fn set_string_range(&mut self, position:i64, value:&RdbcString, offset:i32, length:i32) -> Result<i32,RdbcError> {
        self.ensure_open()?;
        let start=usize::try_from(offset).map_err(|_|RdbcError::InvalidArgument("negative input offset".into()))?;
        let count=usize::try_from(length).map_err(|_|RdbcError::InvalidArgument("negative input length".into()))?;
        let end=start.checked_add(count).ok_or_else(||RdbcError::InvalidArgument("input range overflow".into()))?;
        let units=value.as_utf16().get(start..end).ok_or_else(||RdbcError::InvalidArgument("input range outside string".into()))?;
        self.write_units(position,units)
    }
    async fn truncate(&mut self, length:i64) -> Result<(),RdbcError> {
        self.ensure_open()?;let count=usize::try_from(length).map_err(|_|RdbcError::InvalidArgument("negative truncate length".into()))?;
        if count>self.units.len() {return Err(RdbcError::InvalidArgument("truncate expands clob".into()));}
        self.units.truncate(count);Ok(())
    }
    async fn free(&mut self) -> Result<(),RdbcError> {
        self.probe.calls.fetch_add(1,Ordering::SeqCst);self.probe.started.notify_one();
        if self.probe.block {self.probe.finish.notified().await;}
        if self.probe.failures_remaining.fetch_update(Ordering::SeqCst,Ordering::SeqCst,|count|count.checked_sub(1)).is_ok() {return Err(super::fatal_error());}
        if !self.freed {self.releases.set(self.releases.get()+1);self.freed=true;}Ok(())
    }
    async fn get_character_stream(&mut self) -> Result<RdbcReader,RdbcError> {
        self.ensure_open()?;Err(RdbcError::feature_not_supported("Clob#getCharacterStream"))
    }
    async fn get_ascii_stream(&mut self) -> Result<RdbcInputStream,RdbcError> {
        self.ensure_open()?;Err(RdbcError::feature_not_supported("Clob#getAsciiStream"))
    }
    async fn set_ascii_stream(&mut self,_position:i64) -> Result<RdbcOutputStream,RdbcError> {
        self.ensure_open()?;Err(RdbcError::feature_not_supported("Clob#setAsciiStream"))
    }
    async fn set_character_stream(&mut self,_position:i64) -> Result<RdbcWriter,RdbcError> {
        self.ensure_open()?;Err(RdbcError::feature_not_supported("Clob#setCharacterStream"))
    }
    async fn get_character_stream_range(&mut self,_position:i64,_length:i64) -> Result<RdbcReader,RdbcError> {
        self.ensure_open()?;Err(RdbcError::feature_not_supported("Clob#getCharacterStream(long,long)"))
    }
}
impl NClob for SendOnlyClob {}
pub(super) fn controlled(block:bool)->(SendOnlyClob,ReleaseProbe) {
    let probe=ReleaseProbe::new(block);
    (SendOnlyClob {units:RdbcString::from("A😀Z").as_utf16().to_vec(),freed:false,releases:Cell::new(0),probe:probe.clone()},probe)
}
#[cfg(task7_require_character_sync)]
fn negative_character_sync() {fn requires_sync<T:Sync>(){} requires_sync::<SendOnlyClob>();}

}

async fn character(connection:&mut DruidPooledConnection,national:bool)->Box<dyn Clob> {
    if national {connection.create_n_clob().await.expect("national factory")} else {connection.create_clob().await.expect("clob factory")}
}
#[tokio::test]
async fn task7_pooled_character_utf16_units_and_pattern_resource() {
    for national in [false,true] {
        let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;
        let mut resource=character(&mut connection,national).await;
        assert_eq!(resource.length().await.unwrap(),4);
        assert_eq!(resource.get_sub_string(2,1).await.unwrap().as_utf16(),&[0xD83D]);
        assert_eq!(resource.position_string(&RdbcString::from("😀"),1).await.unwrap(),Some(2));
        assert_eq!(resource.set_string_range(1,&RdbcString::from("😀"),1,1).await.unwrap(),1);
        assert_eq!(resource.get_sub_string(1,4).await.unwrap().as_utf16(),&[0xDE00,0xD83D,0xDE00,90]);
        resource.set_string(1,&RdbcString::from("A")).await.unwrap();
        let mut pattern=connection.create_clob().await.unwrap();pattern.truncate(1).await.unwrap();
        assert_eq!(resource.position_clob(druid_rdbc::resource::ClobPattern::Other(&mut *pattern),1).await.unwrap(),Some(1));
        resource.truncate(2).await.unwrap();assert_eq!(resource.length().await.unwrap(),2);
        assert!(resource.set_string_range(1,&RdbcString::from("😀"),2,1).await.is_err());
        assert_eq!(control.factory_calls.lock().unwrap()[0],if national {"create_n_clob"}else{"create_clob"});
        connection.close().await.unwrap();
    }
}
#[tokio::test]
async fn task7_pooled_character_parent_cleanup_and_distinct_kinds() {
    let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;
    let mut clob=connection.create_clob().await.unwrap();let mut national=connection.create_n_clob().await.unwrap();
    clob.set_string(1,&RdbcString::from("X")).await.unwrap();clob.free().await.unwrap();clob.free().await.unwrap();
    assert_eq!(national.get_sub_string(1,1).await.unwrap().as_utf16(),&[65]);
    connection.close().await.unwrap();
    assert!(national.length().await.unwrap_err().to_string().contains("NClob"));national.free().await.unwrap();
    let probes=control.probes.lock().unwrap();for probe in probes.iter() {assert_eq!(probe.calls.load(Ordering::SeqCst),1);}
    assert_eq!(*control.factory_calls.lock().unwrap(),vec!["create_clob","create_n_clob"]);
    let returns=control.returns.lock().unwrap();assert_eq!(returns.len(),1);assert!(returns[0].is_reusable());
}
#[tokio::test]
async fn task7_pooled_character_cancellation_does_not_cancel_parent_cleanup() {
    for national in [false,true] {
        let control=Arc::new(Control::default());control.block_free.store(true,Ordering::SeqCst);
        let mut connection=pooled(control.clone()).await;let mut resource=character(&mut connection,national).await;
        let probe=control.probes.lock().unwrap()[0].clone();
        let explicit=tokio::spawn(async move {resource.free().await});
        tokio::time::timeout(Duration::from_secs(2),probe.started.notified()).await.unwrap();
        explicit.abort();assert!(explicit.await.unwrap_err().is_cancelled());
        let mut parent=tokio::spawn(async move {connection.close().await});
        assert!(tokio::time::timeout(Duration::from_millis(20),&mut parent).await.is_err());
        assert!(control.returns.lock().unwrap().is_empty());probe.finish.notify_one();
        tokio::time::timeout(Duration::from_secs(2),parent).await.unwrap().unwrap().unwrap();
        assert_eq!(probe.calls.load(Ordering::SeqCst),1);assert_eq!(control.returns.lock().unwrap().len(),1);
    }
}
#[tokio::test]
async fn task7_pooled_character_fatal_read_and_free_classified_once() {
    for national in [false,true] {for during_free in [false,true] {
        let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;
        connection.set_exception_sorter(Arc::new(CountingSorter(control.clone())));
        let mut resource=character(&mut connection,national).await;let probe=control.probes.lock().unwrap()[0].clone();
        let error=if during_free {probe.failures_remaining.store(1,Ordering::SeqCst);resource.free().await.unwrap_err()}
            else {probe.read_failure.store(true,Ordering::SeqCst);resource.length().await.unwrap_err()};
        assert_eq!(error.sql_exception().unwrap().error_code(),77);
        assert_eq!(control.sorter_calls.load(Ordering::SeqCst),1);assert_eq!(control.discards.load(Ordering::SeqCst),1);
        assert!(connection.is_closed().unwrap());resource.free().await.unwrap();connection.close().await.unwrap();
        assert_eq!(probe.calls.load(Ordering::SeqCst),if during_free {2}else{1});
        assert!(!control.returns.lock().unwrap()[0].is_reusable());
    }}
}
#[tokio::test]
async fn task7_pooled_character_closed_error_precedes_all_unsupported_operations() {
    for national in [false,true] {
        let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;
        let mut resource=character(&mut connection,national).await;
        assert!(!resource.capabilities().contains(druid_rdbc::resource::ResourceOperations::STREAM));
        let errors=[resource.get_character_stream().await.err().unwrap(),resource.get_ascii_stream().await.err().unwrap(),
            resource.set_ascii_stream(1).await.err().unwrap(),resource.set_character_stream(1).await.err().unwrap(),resource.get_character_stream_range(1,1).await.err().unwrap()];
        for error in errors {assert_eq!(error.class_name(),"java.sql.SQLFeatureNotSupportedException");}
        let mut pattern=connection.create_clob().await.unwrap();resource.free().await.unwrap();let text=RdbcString::from("x");
        let errors=[resource.length().await.err().unwrap(),resource.get_sub_string(1,1).await.err().unwrap(),resource.get_character_stream().await.err().unwrap(),resource.get_ascii_stream().await.err().unwrap(),
            resource.position_string(&text,1).await.err().unwrap(),resource.position_clob(druid_rdbc::resource::ClobPattern::Other(&mut *pattern),1).await.err().unwrap(),resource.set_string(1,&text).await.err().unwrap(),resource.set_string_range(1,&text,0,1).await.err().unwrap(),
            resource.set_ascii_stream(1).await.err().unwrap(),resource.set_character_stream(1).await.err().unwrap(),resource.truncate(0).await.err().unwrap(),resource.get_character_stream_range(1,1).await.err().unwrap()];
        for error in errors {assert_eq!(error.class_name(),"java.sql.SQLException");}
        connection.close().await.unwrap();
    }
}
#[tokio::test]
async fn task7_pooled_character_closed_factories_do_not_touch_driver() {
    let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;connection.close().await.unwrap();
    assert!(connection.create_clob().await.is_err());assert!(connection.create_n_clob().await.is_err());assert_eq!(control.created.load(Ordering::SeqCst),0);
}

#[tokio::test]
async fn task7_pooled_character_create_fatal_preserves_error_and_discards_once() {
    for national in [false,true] {
        let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;
        connection.set_exception_sorter(Arc::new(CountingSorter(control.clone())));
        control.fail_create.store(true,Ordering::SeqCst);
        let error=if national {connection.create_n_clob().await.unwrap_err()} else {connection.create_clob().await.unwrap_err()};
        assert_eq!(error.sql_exception().unwrap().error_code(),77);assert!(connection.is_closed().unwrap());
        assert_eq!(control.discards.load(Ordering::SeqCst),1);assert_eq!(control.sorter_calls.load(Ordering::SeqCst),1);
        assert_eq!(control.created.load(Ordering::SeqCst),1);
        assert_eq!(*control.factory_calls.lock().unwrap(),vec![if national {"create_n_clob"}else{"create_clob"}]);
        assert!(control.probes.lock().unwrap().is_empty());connection.close().await.unwrap();
        let returns=control.returns.lock().unwrap();assert_eq!(returns.len(),1);assert!(!returns[0].is_reusable());
    }
}
#[tokio::test]
async fn task7_pooled_character_old_handle_isolated_across_holder_reuse() {
    for national in [false,true] {
        let control=Arc::new(Control::default());
        let holder=DruidConnectionHolder::with_standard_connection(Box::new(DriverConnection {control:control.clone(),closed:false}),44,Duration::ZERO,0).await.unwrap();
        let returned=Arc::new(Mutex::new(None));let sink=returned.clone();
        let mut connection=DruidPooledConnection::with_standard_holder(holder,"first".into(),None,false,false,Box::new(move |holder,disposition| {
            assert!(disposition.is_reusable());*sink.lock().unwrap()=Some(holder);true
        }));
        let mut old=character(&mut connection,national).await;connection.close().await.unwrap();
        let holder=returned.lock().unwrap().take().unwrap();let returns=control.clone();
        let mut next=DruidPooledConnection::with_standard_holder(holder,"next".into(),None,false,false,Box::new(move |_,disposition| {returns.returns.lock().unwrap().push(disposition);true}));
        let mut current=character(&mut next,national).await;
        assert!(old.length().await.is_err());old.free().await.unwrap();
        assert_eq!(current.get_sub_string(1,4).await.unwrap().as_utf16(),RdbcString::from("A😀Z").as_utf16());
        let probes=control.probes.lock().unwrap().clone();assert_eq!(probes[0].calls.load(Ordering::SeqCst),1);assert_eq!(probes[1].calls.load(Ordering::SeqCst),0);
        next.close().await.unwrap();assert_eq!(probes[1].calls.load(Ordering::SeqCst),1);assert!(control.returns.lock().unwrap()[0].is_reusable());
    }
}

#[derive(Debug)]
struct XmlProbe {
    release:payload_fixture::ReleaseProbe,
    units:Arc<Mutex<Vec<u16>>>,
    binary:Arc<Mutex<Vec<u8>>>,
    reads:AtomicUsize,writes:AtomicUsize,
    write_block:AtomicBool,write_started:tokio::sync::Notify,write_finish:tokio::sync::Notify,
    read_nonfatal:AtomicBool,
    sources:Mutex<Vec<RdbcXmlSource>>,results:Mutex<Vec<RdbcXmlResult>>,
    read_block:AtomicBool,read_started:tokio::sync::Notify,read_finish:tokio::sync::Notify,
    representations:Mutex<Vec<Option<RdbcXmlRepresentationType>>>,
}
impl XmlProbe {
    fn new(block:bool)->Self {Self {release:payload_fixture::ReleaseProbe::new(block),units:Arc::new(Mutex::new(RdbcString::from("<v>😀</v>").as_utf16().to_vec())),binary:Arc::new(Mutex::new(Vec::new())),reads:AtomicUsize::new(0),writes:AtomicUsize::new(0),write_block:AtomicBool::new(false),write_started:tokio::sync::Notify::new(),write_finish:tokio::sync::Notify::new(),read_nonfatal:AtomicBool::new(false),sources:Mutex::new(Vec::new()),results:Mutex::new(Vec::new()),read_block:AtomicBool::new(false),read_started:tokio::sync::Notify::new(),read_finish:tokio::sync::Notify::new(),representations:Mutex::new(Vec::new())}}
}
#[derive(Debug)]
struct XmlDriver {probe:Arc<XmlProbe>,freed:bool,not_sync:std::cell::Cell<usize>,stream:bool}
impl XmlDriver {
    fn open(&self)->Result<(),RdbcError> {if self.freed {Err(RdbcError::rdbc_resource_closed("SQLXML"))} else {Ok(())}}
    async fn read(&mut self)->Result<(),RdbcError> {self.open()?;self.not_sync.set(self.not_sync.get()+1);self.probe.reads.fetch_add(1,Ordering::SeqCst);self.probe.read_started.notify_one();if self.probe.read_block.load(Ordering::SeqCst) {self.probe.read_finish.notified().await;}if self.probe.release.read_failure.load(Ordering::SeqCst) {Err(fatal_error())} else if self.probe.read_nonfatal.load(Ordering::SeqCst) {Err(RdbcError::SqlException(Box::new(SqlException::new(88,Some("22000".into()),Some("invalid XML".into())))))} else {Ok(())}}
    async fn write(&mut self)->Result<(),RdbcError> {self.open()?;self.not_sync.set(self.not_sync.get()+1);self.probe.writes.fetch_add(1,Ordering::SeqCst);self.probe.write_started.notify_one();if self.probe.write_block.load(Ordering::SeqCst){self.probe.write_finish.notified().await;}Ok(())}
}
#[derive(Debug)]
struct XmlSourceValue {representation:Option<RdbcXmlRepresentationType>,units:Vec<u16>}
impl druid_rdbc::extensions::XmlSource for XmlSourceValue {}
#[derive(Debug)]
struct XmlResultValue {representation:Option<RdbcXmlRepresentationType>,units:Arc<Mutex<Vec<u16>>>}
impl druid_rdbc::extensions::XmlResult for XmlResultValue {}
#[derive(Debug)]
struct XmlReader {units:Vec<u16>,cursor:usize,closed:bool}
impl druid_rdbc::extensions::CharacterReader for XmlReader {
    fn read_utf16(&mut self,buffer:&mut[u16])->Result<usize,RdbcError> {if self.closed {return Err(RdbcError::DriverError("reader closed".into()));}let n=buffer.len().min(self.units.len()-self.cursor);buffer[..n].copy_from_slice(&self.units[self.cursor..self.cursor+n]);self.cursor+=n;Ok(n)}
    fn close(&mut self)->Result<(),RdbcError> {self.closed=true;Ok(())}
}
#[derive(Debug)]
struct XmlWriter {units:Arc<Mutex<Vec<u16>>>,closed:bool}
impl druid_rdbc::extensions::CharacterWriter for XmlWriter {
    fn write_utf16(&mut self,units:&[u16])->Result<usize,RdbcError> {if self.closed {return Err(RdbcError::DriverError("writer closed".into()));}self.units.lock().unwrap().extend_from_slice(units);Ok(units.len())}
    fn flush(&mut self)->Result<(),RdbcError> {if self.closed {Err(RdbcError::DriverError("writer closed".into()))}else{Ok(())}}
    fn close(&mut self)->Result<(),RdbcError> {self.closed=true;Ok(())}
}
#[derive(Debug)]
struct XmlBinaryWriter(Arc<Mutex<Vec<u8>>>);
impl std::io::Write for XmlBinaryWriter {
    fn write(&mut self,bytes:&[u8])->std::io::Result<usize> {self.0.lock().unwrap().extend_from_slice(bytes);Ok(bytes.len())}
    fn flush(&mut self)->std::io::Result<()> {Ok(())}
}
#[async_trait::async_trait]
impl SqlXml for XmlDriver {
    fn capabilities(&self)->ResourceOperations {let base=ResourceOperations::READ|ResourceOperations::WRITE|ResourceOperations::FREE;if self.stream {base|ResourceOperations::STREAM}else{base}}
    async fn free(&mut self)->Result<(),RdbcError> {
        if self.freed{return Ok(());}self.probe.release.calls.fetch_add(1,Ordering::SeqCst);self.probe.release.started.notify_one();
        if self.probe.release.block {self.probe.release.finish.notified().await;}
        if self.probe.release.failures_remaining.fetch_update(Ordering::SeqCst,Ordering::SeqCst,|n|n.checked_sub(1)).is_ok() {return Err(fatal_error());}self.freed=true;Ok(())
    }
    async fn binary_stream(&mut self)->Result<RdbcInputStream,RdbcError> {self.read().await?;let text=String::from_utf16_lossy(&self.probe.units.lock().unwrap());Ok(RdbcInputStream::from_bytes(text.into_bytes()))}
    async fn set_binary_stream(&mut self)->Result<RdbcOutputStream,RdbcError> {self.write().await?;self.probe.binary.lock().unwrap().clear();Ok(RdbcOutputStream::new(XmlBinaryWriter(self.probe.binary.clone())))}
    async fn character_stream(&mut self)->Result<RdbcReader,RdbcError> {self.read().await?;Ok(RdbcReader::new(XmlReader {units:self.probe.units.lock().unwrap().clone(),cursor:0,closed:false}))}
    async fn set_character_stream(&mut self)->Result<RdbcWriter,RdbcError> {self.write().await?;self.probe.units.lock().unwrap().clear();Ok(RdbcWriter::new(XmlWriter {units:self.probe.units.clone(),closed:false}))}
    async fn string(&mut self)->Result<RdbcString,RdbcError> {self.read().await?;Ok(RdbcString::from_utf16(self.probe.units.lock().unwrap().clone()))}
    async fn set_string(&mut self,value:&RdbcString)->Result<(),RdbcError> {self.write().await?;*self.probe.units.lock().unwrap()=value.as_utf16().to_vec();Ok(())}
    async fn source(&mut self,representation:Option<&RdbcXmlRepresentationType>)->Result<RdbcXmlSource,RdbcError> {self.read().await?;self.probe.representations.lock().unwrap().push(representation.cloned());{let value=RdbcXmlSource::new(Arc::new(XmlSourceValue {representation:representation.cloned(),units:self.probe.units.lock().unwrap().clone()}));self.probe.sources.lock().unwrap().push(value.clone());Ok(value)}}
    async fn result(&mut self,representation:Option<&RdbcXmlRepresentationType>)->Result<RdbcXmlResult,RdbcError> {self.write().await?;self.probe.representations.lock().unwrap().push(representation.cloned());{let value=RdbcXmlResult::new(Arc::new(XmlResultValue {representation:representation.cloned(),units:self.probe.units.clone()}));self.probe.results.lock().unwrap().push(value.clone());Ok(value)}}
}
async fn xml_read(resource:&mut dyn SqlXml,kind:usize)->Result<(),RdbcError> {
    match kind {0=>resource.string().await.map(|_|()),1=>resource.binary_stream().await.map(|_|()),2=>resource.character_stream().await.map(|_|()),3=>resource.source(None).await.map(|_|()),_=>panic!("test read kind")}
}
async fn xml_write(resource:&mut dyn SqlXml,kind:usize)->Result<(),RdbcError> {
    match kind {0=>resource.set_string(&RdbcString::from("<x/>" )).await,1=>resource.set_binary_stream().await.map(|_|()),2=>resource.set_character_stream().await.map(|_|()),3=>resource.result(None).await.map(|_|()),_=>panic!("test write kind")}
}
#[tokio::test]
async fn task7_xml_reads_share_one_consumption_gate() {
    for first in 0..4 {for second in 0..4 {
        let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;let mut xml=connection.create_sql_xml().await.unwrap();
        xml_read(&mut *xml,first).await.unwrap();let error=xml_read(&mut *xml,second).await.unwrap_err();assert_eq!(error.class_name(),"java.sql.SQLException");
        assert_eq!(control.xml.lock().unwrap()[0].reads.load(Ordering::SeqCst),1);connection.close().await.unwrap();
    }}
}
#[tokio::test]
async fn task7_xml_writes_share_one_consumption_gate() {
    for first in 0..4 {for second in 0..4 {
        let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;let mut xml=connection.create_sql_xml().await.unwrap();
        xml_write(&mut *xml,first).await.unwrap();let error=xml_write(&mut *xml,second).await.unwrap_err();assert_eq!(error.class_name(),"java.sql.SQLException");
        assert_eq!(control.xml.lock().unwrap()[0].writes.load(Ordering::SeqCst),1);connection.close().await.unwrap();
    }}
}
#[tokio::test]
async fn task7_xml_preserves_utf16_streams_nullable_and_vendor_representations() {
    let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;
    let mut text=connection.create_sql_xml().await.unwrap();let input=RdbcString::from_utf16(vec![60,0xD800,62]);text.set_string(&input).await.unwrap();assert_eq!(text.string().await.unwrap(),input);
    let mut chars=connection.create_sql_xml().await.unwrap();let writer=chars.set_character_stream().await.unwrap();writer.write_utf16(&[60,0xDFFF,62]).unwrap();writer.close().unwrap();assert!(writer.is_closed());let reader=chars.character_stream().await.unwrap();let mut buffer=[0u16;3];assert_eq!(reader.read_utf16(&mut buffer).unwrap(),3);assert_eq!(buffer,[60,0xDFFF,62]);reader.close().unwrap();
    let mut binary=connection.create_sql_xml().await.unwrap();let stream=binary.binary_stream().await.unwrap();assert_eq!(stream.read_to_end().unwrap(),"<v>😀</v>".as_bytes());stream.close().unwrap();
    let mut binary_write=connection.create_sql_xml().await.unwrap();let stream=binary_write.set_binary_stream().await.unwrap();stream.write(b"<b/>").unwrap();stream.close().unwrap();assert_eq!(*control.xml.lock().unwrap()[3].binary.lock().unwrap(),b"<b/>");
    for representation in [None,Some(RdbcXmlRepresentationType::Dom),Some(RdbcXmlRepresentationType::Custom("vendor.Xml".into()))] {
        let mut xml=connection.create_sql_xml().await.unwrap();xml.source(representation.as_ref()).await.unwrap();xml.result(representation.as_ref()).await.unwrap();
        let probe=control.xml.lock().unwrap().last().unwrap().clone();assert_eq!(*probe.representations.lock().unwrap(),vec![representation.clone(),representation]);
    }
    connection.close().await.unwrap();
}
#[tokio::test]
async fn task7_xml_cancelled_read_cannot_reenter_driver() {
    let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;let mut xml=connection.create_sql_xml().await.unwrap();let probe=control.xml.lock().unwrap()[0].clone();probe.read_block.store(true,Ordering::SeqCst);
    assert!(tokio::time::timeout(Duration::from_millis(20),xml.string()).await.is_err());assert_eq!(probe.reads.load(Ordering::SeqCst),1);
    let error=tokio::time::timeout(Duration::from_millis(20),xml.string()).await.expect("second invocation must reject").unwrap_err();assert_eq!(error.class_name(),"java.sql.SQLException");assert_eq!(probe.reads.load(Ordering::SeqCst),1);
    xml.free().await.unwrap();connection.close().await.unwrap();assert_eq!(probe.release.calls.load(Ordering::SeqCst),1);
}
#[tokio::test]
async fn task7_xml_free_cancel_parent_waits_and_closes_once() {
    let control=Arc::new(Control::default());control.block_free.store(true,Ordering::SeqCst);let mut connection=pooled(control.clone()).await;let mut xml=connection.create_sql_xml().await.unwrap();let probe=control.xml.lock().unwrap()[0].clone();
    let caller=tokio::spawn(async move {xml.free().await});tokio::time::timeout(Duration::from_secs(2),probe.release.started.notified()).await.unwrap();caller.abort();assert!(caller.await.unwrap_err().is_cancelled());
    let mut parent=tokio::spawn(async move {connection.close().await});assert!(tokio::time::timeout(Duration::from_millis(20),&mut parent).await.is_err());assert!(control.returns.lock().unwrap().is_empty());probe.release.finish.notify_one();tokio::time::timeout(Duration::from_secs(2),parent).await.unwrap().unwrap().unwrap();assert_eq!(probe.release.calls.load(Ordering::SeqCst),1);
}
#[tokio::test]
async fn task7_xml_free_rejects_all_reads_and_writes_before_driver() {
    let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;let mut xml=connection.create_sql_xml().await.unwrap();let probe=control.xml.lock().unwrap()[0].clone();xml.free().await.unwrap();xml.free().await.unwrap();
    for kind in 0..4 {assert_eq!(xml_read(&mut *xml,kind).await.unwrap_err().class_name(),"java.sql.SQLException");assert_eq!(xml_write(&mut *xml,kind).await.unwrap_err().class_name(),"java.sql.SQLException");}
    assert_eq!(probe.reads.load(Ordering::SeqCst),0);assert_eq!(probe.writes.load(Ordering::SeqCst),0);assert_eq!(probe.release.calls.load(Ordering::SeqCst),1);connection.close().await.unwrap();
}
#[tokio::test]
async fn task7_xml_unsupported_stream_does_not_consume_supported_string() {
    let control=Arc::new(Control::default());control.xml_no_stream.store(true,Ordering::SeqCst);let mut connection=pooled(control.clone()).await;let mut xml=connection.create_sql_xml().await.unwrap();assert!(!xml.capabilities().contains(ResourceOperations::STREAM));
    assert_eq!(xml.binary_stream().await.unwrap_err().class_name(),"java.sql.SQLFeatureNotSupportedException");assert_eq!(xml.set_binary_stream().await.unwrap_err().class_name(),"java.sql.SQLFeatureNotSupportedException");xml.set_string(&RdbcString::from("<s/>" )).await.unwrap();assert_eq!(xml.string().await.unwrap(),RdbcString::from("<s/>"));let probe=control.xml.lock().unwrap()[0].clone();assert_eq!(probe.reads.load(Ordering::SeqCst),1);assert_eq!(probe.writes.load(Ordering::SeqCst),1);connection.close().await.unwrap();
}
#[tokio::test]
async fn task7_xml_create_read_and_free_fatal_error_classification() {
    for kind in 0..3 {
        let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;connection.set_exception_sorter(Arc::new(CountingSorter(control.clone())));
        if kind==0 {control.fail_create.store(true,Ordering::SeqCst);assert_eq!(connection.create_sql_xml().await.unwrap_err().sql_exception().unwrap().error_code(),77);assert!(control.xml.lock().unwrap().is_empty());}
        else {let mut xml=connection.create_sql_xml().await.unwrap();let probe=control.xml.lock().unwrap()[0].clone();let error=if kind==1 {probe.release.read_failure.store(true,Ordering::SeqCst);xml.string().await.unwrap_err()}else{probe.release.failures_remaining.store(1,Ordering::SeqCst);xml.free().await.unwrap_err()};assert_eq!(error.sql_exception().unwrap().error_code(),77);xml.free().await.unwrap();assert_eq!(probe.release.calls.load(Ordering::SeqCst),if kind==2{2}else{1});}
        assert!(connection.is_closed().unwrap());assert_eq!(control.discards.load(Ordering::SeqCst),1);assert_eq!(control.sorter_calls.load(Ordering::SeqCst),1);connection.close().await.unwrap();assert!(!control.returns.lock().unwrap()[0].is_reusable());
    }
}
#[tokio::test]
async fn task7_xml_closed_factory_does_not_touch_driver() {
    let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;connection.close().await.unwrap();assert!(connection.create_sql_xml().await.is_err());assert_eq!(control.created.load(Ordering::SeqCst),0);
}
#[cfg(task7_require_xml_sync)]
fn negative_xml_sync() {fn require_sync<T:Sync>(){}require_sync::<XmlDriver>();}

#[tokio::test]
async fn task7_xml_source_and_result_preserve_exact_driver_identity() {
    for representation in [None,Some(RdbcXmlRepresentationType::Stream),Some(RdbcXmlRepresentationType::Custom("vendor.Identity".into()))] {
        let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;let mut xml=connection.create_sql_xml().await.unwrap();let source=xml.source(representation.as_ref()).await.unwrap();let result=xml.result(representation.as_ref()).await.unwrap();let probe=control.xml.lock().unwrap()[0].clone();
        assert_eq!(source,probe.sources.lock().unwrap()[0]);assert_eq!(result,probe.results.lock().unwrap()[0]);assert_eq!(*probe.representations.lock().unwrap(),vec![representation.clone(),representation]);connection.close().await.unwrap();
    }
}
#[tokio::test]
async fn task7_xml_cancelled_write_cannot_reenter_driver() {
    let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;let mut xml=connection.create_sql_xml().await.unwrap();let probe=control.xml.lock().unwrap()[0].clone();probe.write_block.store(true,Ordering::SeqCst);let text=RdbcString::from("<cancelled/>" );
    assert!(tokio::time::timeout(Duration::from_millis(20),xml.set_string(&text)).await.is_err());assert_eq!(probe.writes.load(Ordering::SeqCst),1);
    let error=tokio::time::timeout(Duration::from_millis(20),xml.set_string(&text)).await.expect("second write rejects without awaiting driver").unwrap_err();assert_eq!(error.class_name(),"java.sql.SQLException");assert_eq!(probe.writes.load(Ordering::SeqCst),1);xml.free().await.unwrap();connection.close().await.unwrap();assert_eq!(probe.release.calls.load(Ordering::SeqCst),1);
}
#[tokio::test]
async fn task7_xml_nonfatal_driver_error_consumes_read_without_discard() {
    let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;connection.set_exception_sorter(Arc::new(CountingSorter(control.clone())));let mut xml=connection.create_sql_xml().await.unwrap();let probe=control.xml.lock().unwrap()[0].clone();probe.read_nonfatal.store(true,Ordering::SeqCst);
    assert_eq!(xml.string().await.unwrap_err().sql_exception().unwrap().error_code(),88);assert!(!connection.is_closed().unwrap());let error=xml.string().await.unwrap_err();assert_eq!(error.class_name(),"java.sql.SQLException");assert!(error.to_string().contains("readable"));assert_eq!(probe.reads.load(Ordering::SeqCst),1);assert_eq!(control.sorter_calls.load(Ordering::SeqCst),1);assert_eq!(control.discards.load(Ordering::SeqCst),0);
    xml.set_string(&RdbcString::from("<valid/>" )).await.unwrap();connection.close().await.unwrap();assert!(control.returns.lock().unwrap()[0].is_reusable());
}
#[tokio::test]
async fn task7_xml_old_resource_cannot_affect_reused_holder() {
    let control=Arc::new(Control::default());let holder=DruidConnectionHolder::with_standard_connection(Box::new(DriverConnection {control:control.clone(),closed:false}),51,Duration::ZERO,0).await.unwrap();let returned=Arc::new(Mutex::new(None));let sink=returned.clone();
    let mut connection=DruidPooledConnection::with_standard_holder(holder,"first".into(),None,false,false,Box::new(move |holder,disposition|{assert!(disposition.is_reusable());*sink.lock().unwrap()=Some(holder);true}));let mut old=connection.create_sql_xml().await.unwrap();connection.close().await.unwrap();let holder=returned.lock().unwrap().take().unwrap();let returns=control.clone();
    let mut next=DruidPooledConnection::with_standard_holder(holder,"next".into(),None,false,false,Box::new(move |_,disposition|{returns.returns.lock().unwrap().push(disposition);true}));let mut current=next.create_sql_xml().await.unwrap();assert!(old.string().await.is_err());old.free().await.unwrap();let text=RdbcString::from("<next/>" );current.set_string(&text).await.unwrap();assert_eq!(current.string().await.unwrap(),text);let probes=control.xml.lock().unwrap().clone();assert_eq!(probes[0].release.calls.load(Ordering::SeqCst),1);assert_eq!(probes[1].release.calls.load(Ordering::SeqCst),0);next.close().await.unwrap();assert_eq!(probes[1].release.calls.load(Ordering::SeqCst),1);assert!(control.returns.lock().unwrap()[0].is_reusable());
}

use druid_rdbc::resource::Array;
use druid_rdbc::result_set::{ColumnIndex, ResultSet};
use druid_rdbc::metadata::{ColumnDescriptor, ColumnNullability, ResultSetMetaData, ResultSetMetaDataSnapshot};
use druid_rdbc::types::{RdbcObject, RdbcTypeMap, RdbcTargetType, Value};

#[derive(Debug)]
struct ArrayProbe {
    supported:ResourceOperations,
    free_failures:AtomicUsize,
    free_fatal:AtomicBool,
    nonfatal_read:AtomicBool,
    args: (Option<String>, Option<Vec<RdbcObject>>),
    maps: Mutex<Vec<RdbcTypeMap>>,
    calls: Mutex<Vec<&'static str>>,
    frees: AtomicUsize,
    started: tokio::sync::Notify,
    finish: tokio::sync::Notify,
    block: bool,
    read_failure: AtomicBool,
    bad_metadata: AtomicBool,
    block_cursor:AtomicBool,
    cursor_started:tokio::sync::Notify,
    cursor_finish:tokio::sync::Notify,
    cursor_next_failure: AtomicBool,
    cursors: Mutex<Vec<Arc<ArrayCursorProbe>>>,
}
#[derive(Debug, Default)]
struct ArrayCursorProbe { closes: AtomicUsize, closed: AtomicBool }
#[derive(Debug)]
struct ArrayDriver {
    probe: Arc<ArrayProbe>,
    freed: bool,
    not_sync: std::cell::Cell<usize>,
}
impl ArrayDriver {
    fn check(&self, name: &'static str) -> Result<(), RdbcError> {
        if self.freed { return Err(RdbcError::rdbc_resource_closed("Array")); }
        self.not_sync.set(self.not_sync.get() + 1);
        self.probe.calls.lock().unwrap().push(name);
        let mut required=if name.starts_with("result_set") {ResourceOperations::RESULT_SET} else {ResourceOperations::READ};
        if name.contains("range") { required|=ResourceOperations::RANGE; }
        if name.contains("type_map") { required|=ResourceOperations::TYPE_MAP; }
        if !self.probe.supported.contains(required) {return Err(RdbcError::feature_not_supported(name));}
        if self.probe.nonfatal_read.load(Ordering::SeqCst) {return Err(array_nonfatal_error());}
        if self.probe.read_failure.load(Ordering::SeqCst) { return Err(fatal_error()); }
        Ok(())
    }
    fn range(&self, index: i64, count: i32) -> Result<Vec<(i64,RdbcObject)>, RdbcError> {
        let start = index.checked_sub(1).and_then(|i| usize::try_from(i).ok())
            .ok_or_else(|| RdbcError::InvalidArgument("Array position is 1-based".into()))?;
        let count = usize::try_from(count).map_err(|_| RdbcError::InvalidArgument("negative count".into()))?;
        let values = self.probe.args.1.as_deref().unwrap_or(&[]);
        let tail = values.get(start..).ok_or_else(|| RdbcError::InvalidArgument("Array range outside value".into()))?;
        Ok(tail.iter().take(count).cloned().enumerate().map(|(i,value)| (index+i as i64,value)).collect())
    }
    fn all(&self) -> Vec<(i64,RdbcObject)> {
        self.probe.args.1.as_deref().unwrap_or(&[]).iter().cloned().enumerate()
            .map(|(i,value)| (i as i64+1,value)).collect()
    }
    fn cursor(&self, rows: Vec<(i64,RdbcObject)>) -> Box<dyn ResultSet> {
        let probe = Arc::new(ArrayCursorProbe::default());
        self.probe.cursors.lock().unwrap().push(probe.clone());
        Box::new(ArrayCursor { rows, offset: 0, on_row: false, probe, parent: self.probe.clone() })
    }
    fn map(&self, map: &RdbcTypeMap) { self.probe.maps.lock().unwrap().push(map.clone()); }
}
#[async_trait::async_trait]
impl Array for ArrayDriver {
    fn capabilities(&self) -> ResourceOperations { self.probe.supported }
    async fn base_type_name(&mut self) -> Result<String,RdbcError> {
        self.check("base_type_name")?; Ok(self.probe.args.0.clone().unwrap_or_else(|| "vendor.default".into()))
    }
    async fn base_type(&mut self) -> Result<i32,RdbcError> { self.check("base_type")?; Ok(4) }
    async fn values(&mut self) -> Result<Vec<RdbcObject>,RdbcError> {
        self.check("values")?; Ok(self.all().into_iter().map(|(_,value)| value).collect())
    }
    async fn values_with_type_map(&mut self, map: &RdbcTypeMap) -> Result<Vec<RdbcObject>,RdbcError> {
        self.check("values_with_type_map")?; self.map(map); Ok(self.all().into_iter().map(|(_,v)| v).collect())
    }
    async fn values_range(&mut self, index:i64, count:i32) -> Result<Vec<RdbcObject>,RdbcError> {
        self.check("values_range")?; Ok(self.range(index,count)?.into_iter().map(|(_,v)| v).collect())
    }
    async fn values_range_with_type_map(&mut self,index:i64,count:i32,map:&RdbcTypeMap) -> Result<Vec<RdbcObject>,RdbcError> {
        self.check("values_range_with_type_map")?; self.map(map); Ok(self.range(index,count)?.into_iter().map(|(_,v)| v).collect())
    }
    async fn result_set(&mut self) -> Result<Box<dyn ResultSet>,RdbcError> {
        self.check("result_set")?;
        self.probe.cursor_started.notify_one();
        if self.probe.block_cursor.load(Ordering::SeqCst) { self.probe.cursor_finish.notified().await; }
        Ok(self.cursor(self.all()))
    }
    async fn result_set_with_type_map(&mut self,map:&RdbcTypeMap) -> Result<Box<dyn ResultSet>,RdbcError> {
        self.check("result_set_with_type_map")?; self.map(map); Ok(self.cursor(self.all()))
    }
    async fn result_set_range(&mut self,index:i64,count:i32) -> Result<Box<dyn ResultSet>,RdbcError> {
        self.check("result_set_range")?; Ok(self.cursor(self.range(index,count)?))
    }
    async fn result_set_range_with_type_map(&mut self,index:i64,count:i32,map:&RdbcTypeMap) -> Result<Box<dyn ResultSet>,RdbcError> {
        self.check("result_set_range_with_type_map")?; self.map(map); Ok(self.cursor(self.range(index,count)?))
    }
    async fn free(&mut self) -> Result<(),RdbcError> {
        if self.freed { return Ok(()); }
        self.probe.frees.fetch_add(1,Ordering::SeqCst); self.probe.started.notify_one();
        if self.probe.block { self.probe.finish.notified().await; }
        if self.probe.free_failures.fetch_update(Ordering::SeqCst,Ordering::SeqCst,|n|n.checked_sub(1)).is_ok() {
            return Err(if self.probe.free_fatal.load(Ordering::SeqCst) {fatal_error()} else {array_nonfatal_error()});
        }
        self.freed = true; Ok(())
    }
}
#[derive(Debug)]
struct ArrayCursor { rows:Vec<(i64,RdbcObject)>, offset:usize, on_row:bool, probe:Arc<ArrayCursorProbe>, parent:Arc<ArrayProbe> }
druid_rdbc::impl_standard_wrapper!(ArrayCursor => ResultSet);
#[async_trait::async_trait]
impl ResultSet for ArrayCursor {
    fn get_statement(&self) -> Result<Option<Box<dyn druid_rdbc::statement::Statement>>,RdbcError> { Ok(None) }
    async fn next(&mut self) -> Result<bool,RdbcError> {
        if self.probe.closed.load(Ordering::SeqCst) { return Err(RdbcError::ConnectionDiscarded); }
        if self.parent.cursor_next_failure.load(Ordering::SeqCst) { return Err(fatal_error()); }
        self.on_row = self.offset < self.rows.len();
        if self.on_row { self.offset += 1; } Ok(self.on_row)
    }
    fn value(&self,column:ColumnIndex) -> Result<Value,RdbcError> {
        if self.probe.closed.load(Ordering::SeqCst) || !self.on_row { return Err(RdbcError::ConnectionDiscarded); }
        let (index,value) = &self.rows[self.offset-1];
        match column.get() {
            1 => Ok(Value::Int(*index)),
            2 => match value { RdbcObject::Scalar(v) => Ok(v.clone()), _ => Err(RdbcError::feature_not_supported("ArrayCursor#value")) },
            _ => Err(RdbcError::InvalidArgument("two columns".into())),
        }
    }
    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>,RdbcError> {
        if self.parent.bad_metadata.load(Ordering::SeqCst) { return Err(RdbcError::Other("array cursor metadata rejected".into())); }
        Ok(Arc::new(ResultSetMetaDataSnapshot::new(vec![
            ColumnDescriptor::new("INDEX","BIGINT",ColumnNullability::NoNulls),
            ColumnDescriptor::new("VALUE","INTEGER",ColumnNullability::Nullable),
        ])))
    }
    async fn close(&mut self) -> Result<(),RdbcError> {
        if !self.probe.closed.swap(true,Ordering::SeqCst) { self.probe.closes.fetch_add(1,Ordering::SeqCst); }
        Ok(())
    }
    fn is_closed(&self) -> Result<bool,RdbcError> { Ok(self.probe.closed.load(Ordering::SeqCst)) }
}
fn array_values() -> Vec<RdbcObject> { vec![Value::Int(7).into(),Value::Null.into(),Value::Int(9).into()] }
fn array_probe(control:&Control) -> Arc<ArrayProbe> { control.arrays.lock().unwrap().last().unwrap().clone() }
fn array_map() -> RdbcTypeMap { let mut map=RdbcTypeMap::new(); map.insert("vendor.Integer",RdbcTargetType::Integer); map }

#[tokio::test]
async fn task7_array_factory_preserves_nullable_input_and_all_value_overloads() {
    let control=Arc::new(Control::default()); let mut connection=pooled(control.clone()).await;
    let mut array=connection.create_array_of(Some("vendor.Integer"),Some(array_values())).await.expect("pooled Array factory");
    assert_eq!(array.base_type_name().await.unwrap(),"vendor.Integer"); assert_eq!(array.base_type().await.unwrap(),4);
    assert_eq!(array.values().await.unwrap(),array_values()); let map=array_map();
    assert_eq!(array.values_with_type_map(&map).await.unwrap(),array_values());
    assert_eq!(array.values_range(2,99).await.unwrap(),array_values()[1..]);
    assert_eq!(array.values_range_with_type_map(2,1,&map).await.unwrap(),vec![Value::Null.into()]);
    assert!(array.values_range(0,1).await.is_err()); assert!(array.values_range(1,-1).await.is_err());
    assert_eq!(array_probe(&control).maps.lock().unwrap().as_slice(),&[map.clone(),map]);
    let mut null=connection.create_array_of(None,None).await.unwrap(); assert!(null.values().await.unwrap().is_empty());
    let mut empty=connection.create_array_of(Some(""),Some(vec![])).await.unwrap(); assert!(empty.values().await.unwrap().is_empty());
    let probes=control.arrays.lock().unwrap(); assert_eq!(probes[1].args,(None,None)); assert_eq!(probes[2].args,(Some("".into()),Some(vec![]))); drop(probes);
    array.free().await.unwrap(); assert!(array.base_type().await.is_err()); null.free().await.unwrap(); empty.free().await.unwrap(); connection.close().await.unwrap();
}
#[tokio::test]
async fn task7_array_four_cursor_overloads_have_no_creating_statement_and_parent_cleanup() {
    let control=Arc::new(Control::default()); let mut connection=pooled(control.clone()).await;
    let mut array=connection.create_array_of(Some("vendor.Integer"),Some(array_values())).await.unwrap(); let map=array_map();
    let mut cursors=vec![array.result_set().await.unwrap(),array.result_set_with_type_map(&map).await.unwrap(),
        array.result_set_range(2,1).await.unwrap(),array.result_set_range_with_type_map(3,1,&map).await.unwrap()];
    for (i,cursor) in cursors.iter_mut().enumerate() {
        assert!(cursor.get_statement().unwrap().is_none()); assert_eq!(cursor.meta_data().unwrap().column_count(),2);
        assert!(cursor.next().await.unwrap()); assert_eq!(cursor.value(ColumnIndex::new(1).unwrap()).unwrap(),Value::Int(if i<2 {1} else {i as i64}));
        assert_eq!(cursor.value(ColumnIndex::new(2).unwrap()).unwrap(),if i==2 {Value::Null} else if i==3 {Value::Int(9)} else {Value::Int(7)});
    }
    let probe=array_probe(&control); connection.close().await.unwrap();
    assert_eq!(control.returns.lock().unwrap().len(),1); assert_eq!(probe.frees.load(Ordering::SeqCst),1);
    for raw in probe.cursors.lock().unwrap().iter() { assert_eq!(raw.closes.load(Ordering::SeqCst),1); }
    for cursor in &mut cursors { assert!(cursor.next().await.is_err()); cursor.close().await.unwrap(); }
    array.free().await.unwrap(); assert_eq!(probe.frees.load(Ordering::SeqCst),1);
}
#[tokio::test]
async fn task7_array_free_cancellation_remains_owned_until_parent_returns() {
    let control=Arc::new(Control::default()); control.block_free.store(true,Ordering::SeqCst);
    let mut connection=pooled(control.clone()).await; let mut array=connection.create_array_of(None,Some(array_values())).await.unwrap();
    let probe=array_probe(&control); let mut release=Box::pin(array.free());
    tokio::select! { _=probe.started.notified()=>{}, result=&mut release=>panic!("free finished before release: {result:?}") }
    drop(release); let mut parent=Box::pin(connection.close());
    assert!(tokio::time::timeout(Duration::from_millis(30),&mut parent).await.is_err());
    assert!(control.returns.lock().unwrap().is_empty()); probe.finish.notify_one();
    tokio::time::timeout(Duration::from_secs(2),&mut parent).await.unwrap().unwrap(); drop(parent);
    array.free().await.unwrap(); assert_eq!(probe.frees.load(Ordering::SeqCst),1); assert_eq!(control.returns.lock().unwrap().len(),1);
}
#[tokio::test]
async fn task7_array_cursor_open_failure_does_not_deadlock_parent_cleanup() {
    let control=Arc::new(Control::default()); let mut connection=pooled_with_array_filter(control.clone()).await;
    let mut array=connection.create_array_of(None,Some(array_values())).await.unwrap(); let probe=array_probe(&control);
    probe.bad_metadata.store(true,Ordering::SeqCst);
    let result=tokio::time::timeout(Duration::from_secs(2),array.result_set()).await.expect("handoff cleanup must finish");
    assert!(result.is_err()); tokio::time::timeout(Duration::from_secs(2),connection.close()).await.unwrap().unwrap();
    assert_eq!(probe.frees.load(Ordering::SeqCst),1); assert_eq!(probe.cursors.lock().unwrap()[0].closes.load(Ordering::SeqCst),1);
}
#[tokio::test]
async fn task7_array_cursor_driver_fatal_error_discards_actual_owner_once() {
    let control=Arc::new(Control::default()); let mut connection=pooled(control.clone()).await;
    connection.set_exception_sorter(Arc::new(CountingSorter(control.clone())));
    let mut array=connection.create_array_of(None,Some(array_values())).await.unwrap(); let probe=array_probe(&control);
    let mut cursor=array.result_set().await.unwrap(); probe.cursor_next_failure.store(true,Ordering::SeqCst);
    let error=cursor.next().await.unwrap_err(); assert_eq!(error.sql_exception().unwrap().sql_state(),Some("08006"));
    assert_eq!(control.discards.load(Ordering::SeqCst),1); assert!(array.values().await.is_err());
    array.free().await.unwrap(); connection.close().await.unwrap(); assert_eq!(control.discards.load(Ordering::SeqCst),1);
}

#[tokio::test]
async fn task7_array_explicit_free_closes_child_before_connection_return() {
    let control=Arc::new(Control::default()); let mut connection=pooled(control.clone()).await;
    let mut array=connection.create_array_of(None,Some(array_values())).await.unwrap(); let probe=array_probe(&control);
    let mut cursor=array.result_set().await.unwrap(); assert!(cursor.next().await.unwrap());
    array.free().await.unwrap(); assert!(control.returns.lock().unwrap().is_empty());
    assert_eq!(probe.cursors.lock().unwrap()[0].closes.load(Ordering::SeqCst),1);
    assert!(cursor.next().await.is_err()); cursor.close().await.unwrap();
    assert!(array.values().await.is_err()); array.free().await.unwrap(); connection.close().await.unwrap();
    assert_eq!(probe.frees.load(Ordering::SeqCst),1); assert_eq!(probe.cursors.lock().unwrap()[0].closes.load(Ordering::SeqCst),1);
}
#[tokio::test]
async fn task7_array_parent_close_during_cursor_creation_drains_rejected_admission() {
    let control=Arc::new(Control::default()); let mut connection=pooled(control.clone()).await;
    let mut array=connection.create_array_of(None,Some(array_values())).await.unwrap(); let probe=array_probe(&control);
    probe.block_cursor.store(true,Ordering::SeqCst); let mut open=Box::pin(array.result_set());
    tokio::select! { _=probe.cursor_started.notified()=>{}, result=&mut open=>panic!("cursor should block, completed_ok={}",result.is_ok()) }
    let mut parent=Box::pin(connection.close());
    assert!(tokio::time::timeout(Duration::from_millis(30),&mut parent).await.is_err());
    assert!(control.returns.lock().unwrap().is_empty()); probe.cursor_finish.notify_one();
    let result=tokio::time::timeout(Duration::from_secs(2),&mut open).await.expect("rejected admission must drain");
    assert!(result.is_err()); drop(open);
    tokio::time::timeout(Duration::from_secs(2),&mut parent).await.expect("parent must finish").unwrap(); drop(parent);
    assert_eq!(probe.frees.load(Ordering::SeqCst),1); assert_eq!(probe.cursors.lock().unwrap()[0].closes.load(Ordering::SeqCst),1);
    assert_eq!(control.returns.lock().unwrap().len(),1);
}

#[tokio::test]
async fn task7_array_without_filter_defers_metadata_error_until_requested() {
    let control=Arc::new(Control::default()); let mut connection=pooled(control.clone()).await;
    let mut array=connection.create_array_of(None,Some(array_values())).await.unwrap(); let probe=array_probe(&control);
    probe.bad_metadata.store(true,Ordering::SeqCst);
    let mut cursor=array.result_set().await.expect("without Filter chain metadata is lazy");
    assert!(cursor.meta_data().is_err()); assert!(cursor.next().await.unwrap());
    array.free().await.unwrap(); assert!(cursor.next().await.is_err());
    connection.close().await.unwrap();
    assert_eq!(probe.frees.load(Ordering::SeqCst),1);
    assert_eq!(probe.cursors.lock().unwrap()[0].closes.load(Ordering::SeqCst),1);
    assert_eq!(control.returns.lock().unwrap().len(),1);
}

fn array_nonfatal_error() -> RdbcError {
    RdbcError::SqlException(Box::new(SqlException::driver(88,"array nonfatal failure").with_sql_state("22000")))
}
#[tokio::test]
async fn task7_array_sparse_capability_matrix_and_closed_precedence() {
    let flags=[ResourceOperations::READ,ResourceOperations::RANGE,ResourceOperations::TYPE_MAP,ResourceOperations::RESULT_SET];
    for mask in 0..16 {
        let control=Arc::new(Control::default()); let mut disabled=ResourceOperations::empty();
        for (bit,flag) in flags.iter().enumerate() {if mask & (1<<bit)!=0 {disabled|=*flag;}}
        control.array_disabled.store(disabled.bits() as usize,Ordering::SeqCst);
        let mut connection=pooled(control.clone()).await;
        let mut array=connection.create_array_of(None,Some(array_values())).await.unwrap(); let probe=array_probe(&control);
        let supported=ResourceOperations::array() & !disabled; assert_eq!(array.capabilities(),supported);
        let map=array_map();
        macro_rules! check {
            ($operation:expr,$required:expr,$closed:expr) => {{
                let before=probe.calls.lock().unwrap().len(); let result=$operation.await.map(|_|());
                let expected=!$closed && supported.contains($required);
                if expected {assert!(result.is_ok(),"mask={mask}, error={result:?}");}
                else { let error=result.unwrap_err(); let unsupported=error.sql_exception().and_then(|e|e.sql_state())==Some("0A000");
                    assert_eq!(unsupported,!$closed,"mask={mask}, closed={}, error={error:?}",$closed);
                }
                assert_eq!(probe.calls.lock().unwrap().len(),before+usize::from(expected),"unsupported/closed must not touch driver");
            }};
        }
        macro_rules! all {
            ($closed:expr) => {{
                check!(array.base_type_name(),ResourceOperations::READ,$closed);
                check!(array.base_type(),ResourceOperations::READ,$closed);
                check!(array.values(),ResourceOperations::READ,$closed);
                check!(array.values_with_type_map(&map),ResourceOperations::READ|ResourceOperations::TYPE_MAP,$closed);
                check!(array.values_range(1,2),ResourceOperations::READ|ResourceOperations::RANGE,$closed);
                check!(array.values_range_with_type_map(1,2,&map),ResourceOperations::READ|ResourceOperations::RANGE|ResourceOperations::TYPE_MAP,$closed);
                check!(array.result_set(),ResourceOperations::RESULT_SET,$closed);
                check!(array.result_set_with_type_map(&map),ResourceOperations::RESULT_SET|ResourceOperations::TYPE_MAP,$closed);
                check!(array.result_set_range(1,2),ResourceOperations::RESULT_SET|ResourceOperations::RANGE,$closed);
                check!(array.result_set_range_with_type_map(1,2,&map),ResourceOperations::RESULT_SET|ResourceOperations::RANGE|ResourceOperations::TYPE_MAP,$closed);
            }};
        }
        all!(false); array.free().await.unwrap(); all!(true); array.free().await.unwrap();
        connection.close().await.unwrap(); assert_eq!(probe.frees.load(Ordering::SeqCst),1);
        for cursor in probe.cursors.lock().unwrap().iter() {assert_eq!(cursor.closes.load(Ordering::SeqCst),1);}
        assert_eq!(control.returns.lock().unwrap().len(),1);
    }
}
#[tokio::test]
async fn task7_array_free_error_preserves_payload_and_retries_without_replaying_children() {
    for fatal in [false,true] {
        let control=Arc::new(Control::default()); let mut connection=pooled(control.clone()).await;
        connection.set_exception_sorter(Arc::new(CountingSorter(control.clone())));
        let mut array=connection.create_array_of(None,Some(array_values())).await.unwrap(); let probe=array_probe(&control);
        let mut cursor=array.result_set().await.unwrap(); probe.free_failures.store(1,Ordering::SeqCst); probe.free_fatal.store(fatal,Ordering::SeqCst);
        let error=array.free().await.unwrap_err(); assert_eq!(error.sql_exception().unwrap().error_code(),if fatal {77} else {88});
        assert_eq!(error.sql_exception().unwrap().sql_state(),Some(if fatal {"08006"} else {"22000"}));
        assert_eq!(probe.frees.load(Ordering::SeqCst),1); assert_eq!(probe.cursors.lock().unwrap()[0].closes.load(Ordering::SeqCst),1);
        assert!(cursor.next().await.is_err()); assert!(array.values().await.is_err());
        assert_eq!(control.discards.load(Ordering::SeqCst),usize::from(fatal));
        array.free().await.unwrap(); array.free().await.unwrap(); connection.close().await.unwrap();
        assert_eq!(probe.frees.load(Ordering::SeqCst),2); assert_eq!(probe.cursors.lock().unwrap()[0].closes.load(Ordering::SeqCst),1);
        assert_eq!(control.sorter_calls.load(Ordering::SeqCst),1); assert_eq!(control.discards.load(Ordering::SeqCst),usize::from(fatal));
        assert_eq!(control.returns.lock().unwrap().len(),1); assert_eq!(control.returns.lock().unwrap()[0].is_reusable(),!fatal);
    }
}
#[tokio::test]
async fn task7_array_read_error_classification_preserves_driver_error() {
    for fatal in [false,true] {
        let control=Arc::new(Control::default()); let mut connection=pooled(control.clone()).await;
        connection.set_exception_sorter(Arc::new(CountingSorter(control.clone())));
        let mut array=connection.create_array_of(None,Some(array_values())).await.unwrap(); let probe=array_probe(&control);
        probe.read_failure.store(fatal,Ordering::SeqCst); probe.nonfatal_read.store(!fatal,Ordering::SeqCst);
        let error=array.values().await.unwrap_err(); assert_eq!(error.sql_exception().unwrap().error_code(),if fatal {77} else {88});
        assert_eq!(error.sql_exception().unwrap().sql_state(),Some(if fatal {"08006"} else {"22000"}));
        assert_eq!(control.sorter_calls.load(Ordering::SeqCst),1); assert_eq!(control.discards.load(Ordering::SeqCst),usize::from(fatal));
        probe.read_failure.store(false,Ordering::SeqCst); probe.nonfatal_read.store(false,Ordering::SeqCst);
        if fatal {assert!(array.values().await.is_err()); assert_eq!(probe.calls.lock().unwrap().len(),1);}
        else {assert_eq!(array.values().await.unwrap(),array_values());}
        array.free().await.unwrap(); connection.close().await.unwrap(); assert_eq!(probe.frees.load(Ordering::SeqCst),1);
        assert_eq!(control.returns.lock().unwrap()[0].is_reusable(),!fatal);
    }
}
#[tokio::test]
async fn task7_array_closed_factory_does_not_touch_driver() {
    let control=Arc::new(Control::default()); let mut connection=pooled(control.clone()).await;
    connection.close().await.unwrap(); assert!(connection.create_array_of(None,None).await.is_err());
    assert_eq!(control.created.load(Ordering::SeqCst),0); assert!(control.arrays.lock().unwrap().is_empty());
}
#[tokio::test]
async fn task7_array_factory_fatal_error_discards_once_and_preserves_error() {
    let control=Arc::new(Control::default()); let mut connection=pooled(control.clone()).await;
    connection.set_exception_sorter(Arc::new(CountingSorter(control.clone()))); control.fail_create.store(true,Ordering::SeqCst);
    let error=connection.create_array_of(None,None).await.unwrap_err(); assert_eq!(error.sql_exception().unwrap().error_code(),77);
    assert_eq!(error.sql_exception().unwrap().sql_state(),Some("08006")); assert!(connection.is_closed().unwrap());
    assert!(connection.create_array_of(None,None).await.is_err()); assert_eq!(control.created.load(Ordering::SeqCst),1);
    assert_eq!(control.sorter_calls.load(Ordering::SeqCst),1); assert_eq!(control.discards.load(Ordering::SeqCst),1);
    assert!(control.arrays.lock().unwrap().is_empty()); connection.close().await.unwrap();
    assert_eq!(control.returns.lock().unwrap().len(),1); assert!(!control.returns.lock().unwrap()[0].is_reusable());
}
#[tokio::test]
async fn task7_array_old_resource_and_cursor_cannot_affect_reused_holder() {
    let control=Arc::new(Control::default());
    let holder=DruidConnectionHolder::with_standard_connection(Box::new(DriverConnection {control:control.clone(),closed:false}),61,Duration::ZERO,0).await.unwrap();
    let returned=Arc::new(Mutex::new(None)); let sink=returned.clone();
    let mut connection=DruidPooledConnection::with_standard_holder(holder,"first".into(),None,false,false,Box::new(move |holder,disposition| {assert!(disposition.is_reusable());*sink.lock().unwrap()=Some(holder);true}));
    let mut old=connection.create_array_of(None,Some(array_values())).await.unwrap(); let mut old_cursor=old.result_set().await.unwrap();
    connection.close().await.unwrap(); let holder=returned.lock().unwrap().take().expect("actual returned holder"); let returns=control.clone();
    let mut next=DruidPooledConnection::with_standard_holder(holder,"next".into(),None,false,false,Box::new(move |_,disposition| {returns.returns.lock().unwrap().push(disposition);true}));
    let mut current=next.create_array_of(None,Some(vec![Value::Int(99).into()])).await.unwrap(); let mut current_cursor=current.result_set().await.unwrap();
    assert!(old.values().await.is_err()); assert!(old_cursor.next().await.is_err()); old.free().await.unwrap(); old_cursor.close().await.unwrap();
    assert_eq!(current.values().await.unwrap(),vec![Value::Int(99).into()]); assert!(current_cursor.next().await.unwrap());
    assert_eq!(current_cursor.value(ColumnIndex::new(2).unwrap()).unwrap(),Value::Int(99));
    let probes=control.arrays.lock().unwrap().clone(); assert_eq!(probes[0].frees.load(Ordering::SeqCst),1); assert_eq!(probes[1].frees.load(Ordering::SeqCst),0);
    assert_eq!(probes[0].cursors.lock().unwrap()[0].closes.load(Ordering::SeqCst),1); assert_eq!(probes[1].cursors.lock().unwrap()[0].closes.load(Ordering::SeqCst),0);
    next.close().await.unwrap(); assert_eq!(probes[1].frees.load(Ordering::SeqCst),1); assert_eq!(probes[1].cursors.lock().unwrap()[0].closes.load(Ordering::SeqCst),1);
    assert!(control.returns.lock().unwrap()[0].is_reusable());
}

#[derive(Debug)]
struct StructProbe {name:String,attributes:Vec<RdbcObject>,maps:Mutex<Vec<RdbcTypeMap>>,fail:AtomicBool}
#[derive(Debug)]
struct StructDriver {probe:Arc<StructProbe>,calls:std::cell::Cell<usize>}
#[async_trait::async_trait]
impl druid_rdbc::resource::Struct for StructDriver {
    fn capabilities(&self)->ResourceOperations {ResourceOperations::READ|ResourceOperations::TYPE_MAP}
    async fn sql_type_name(&mut self)->Result<String,RdbcError> {
        self.calls.set(self.calls.get()+1);if self.probe.fail.load(Ordering::SeqCst) {return Err(array_nonfatal_error());}Ok(self.probe.name.clone())
    }
    async fn attributes(&mut self)->Result<Vec<RdbcObject>,RdbcError> {
        self.calls.set(self.calls.get()+1);if self.probe.fail.load(Ordering::SeqCst) {return Err(array_nonfatal_error());}Ok(self.probe.attributes.clone())
    }
    async fn attributes_with_type_map(&mut self,map:&RdbcTypeMap)->Result<Vec<RdbcObject>,RdbcError> {
        self.calls.set(self.calls.get()+1);self.probe.maps.lock().unwrap().push(map.clone());
        if self.probe.fail.load(Ordering::SeqCst) {return Err(array_nonfatal_error());}
        let mut result=self.probe.attributes.clone();if map.get("vendor.Integer").is_some() && !result.is_empty() {result[0]=RdbcObject::String("explicit".into());}Ok(result)
    }
}
#[tokio::test]
async fn task7_pooled_struct_owned_methods_and_lifetime_after_connection_close() {
    let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;
    let mut value=connection.create_struct(Some("schema.vendor.Struct"),Some(array_values())).await.unwrap();
    assert_eq!(value.sql_type_name().await.unwrap(),"schema.vendor.Struct");assert_eq!(value.attributes().await.unwrap(),array_values());
    let map=array_map();let mapped=value.attributes_with_type_map(&map).await.unwrap();assert_eq!(mapped[0],RdbcObject::String("explicit".into()));assert_eq!(mapped[1..],array_values()[1..]);
    assert_eq!(control.structs.lock().unwrap()[0].maps.lock().unwrap().as_slice(),&[map]);assert_eq!(value.attributes().await.unwrap(),array_values());
    connection.close().await.unwrap();assert_eq!(control.returns.lock().unwrap().len(),1);assert!(control.returns.lock().unwrap()[0].is_reusable());
    assert_eq!(value.sql_type_name().await.unwrap(),"schema.vendor.Struct");assert_eq!(value.attributes().await.unwrap(),array_values());
}
#[tokio::test]
async fn task7_pooled_struct_nullable_input_and_errors_are_preserved() {
    let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;
    let mut absent=connection.create_struct(Some("vendor.Empty"),None).await.unwrap();let mut empty=connection.create_struct(Some("vendor.Empty"),Some(vec![])).await.unwrap();
    assert!(absent.attributes().await.unwrap().is_empty());assert!(empty.attributes().await.unwrap().is_empty());
    let error=connection.create_struct(None,None).await.unwrap_err();assert_eq!(error.sql_exception().unwrap().error_code(),94);assert_eq!(error.sql_exception().unwrap().sql_state(),Some("HY009"));
    let args=control.struct_args.lock().unwrap();assert_eq!(args[0],(Some("vendor.Empty".into()),None));assert_eq!(args[1],(Some("vendor.Empty".into()),Some(vec![])));assert_eq!(args[2],(None,None));drop(args);
    let probe=control.structs.lock().unwrap()[0].clone();probe.fail.store(true,Ordering::SeqCst);
    for error in [absent.sql_type_name().await.unwrap_err(),absent.attributes().await.unwrap_err(),absent.attributes_with_type_map(&array_map()).await.unwrap_err()] {assert_eq!(error,array_nonfatal_error());}
    assert!(!connection.is_closed().unwrap());assert_eq!(control.discards.load(Ordering::SeqCst),0);connection.close().await.unwrap();assert_eq!(absent.attributes().await.unwrap_err(),array_nonfatal_error());
}
#[tokio::test]
async fn task7_pooled_struct_closed_factory_does_not_touch_driver() {
    let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;connection.close().await.unwrap();assert!(connection.create_struct(None,None).await.is_err());assert_eq!(control.created.load(Ordering::SeqCst),0);
}
#[tokio::test]
async fn task7_pooled_struct_factory_fatal_classifies_current_lease_once() {
    let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;
    connection.set_exception_sorter(Arc::new(CountingSorter(control.clone())));control.fail_create.store(true,Ordering::SeqCst);
    let error=connection.create_struct(Some("vendor.Type"),None).await.unwrap_err();assert_eq!(error,fatal_error());assert!(connection.is_closed().unwrap());
    assert!(connection.create_struct(Some("vendor.Type"),None).await.is_err());assert_eq!(control.created.load(Ordering::SeqCst),1);
    assert_eq!(control.sorter_calls.load(Ordering::SeqCst),1);assert_eq!(control.discards.load(Ordering::SeqCst),1);connection.close().await.unwrap();assert!(!control.returns.lock().unwrap()[0].is_reusable());
}
#[tokio::test]
async fn task7_external_lease_delegates_owned_struct_and_preserves_arguments() {
    let control=Arc::new(Control::default());let raw:Box<dyn Connection>=Box::new(DriverConnection {control:control.clone(),closed:false});
    let mut lease=druid::core::ExternalConnectionLease::new(Box::new(raw));
    let mut value=lease.create_struct(Some("vendor.External"),Some(array_values())).await.unwrap();assert_eq!(value.sql_type_name().await.unwrap(),"vendor.External");
    assert_eq!(control.struct_args.lock().unwrap().as_slice(),&[(Some("vendor.External".into()),Some(array_values()))]);
    lease.close().await.unwrap();assert_eq!(value.attributes().await.unwrap(),array_values());assert!(lease.create_struct(Some("vendor.External"),None).await.is_err());assert_eq!(control.created.load(Ordering::SeqCst),1);
}

mod ref_contract {
use druid_rdbc::error::{RdbcError, SqlException};
use druid_rdbc::resource::{Ref, Struct, RdbcRef, RdbcStruct, ResourceHandleFactory, ResourceOperations};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetExt, ResultSetRead};
use druid_rdbc::types::{RdbcObject, RdbcTypeMap, RdbcTargetType, Value};
use std::cell::Cell;
use std::sync::{Arc, Mutex, atomic::{AtomicBool, AtomicUsize, Ordering}};

#[derive(Debug, Default)]
struct Probe {
    calls: AtomicUsize,
    attributes: AtomicUsize,
    drops: AtomicUsize,
    maps: Mutex<Vec<RdbcTypeMap>>,
    fail: AtomicBool,
    fatal: AtomicBool,
    block: AtomicBool,
    started: tokio::sync::Notify,
}
fn failure() -> RdbcError { RdbcError::SqlException(Box::new(SqlException::driver(95,"ref driver failure").with_sql_state("22000"))) }
fn map() -> RdbcTypeMap { let mut result=RdbcTypeMap::new(); result.insert("schema.Target",RdbcTargetType::String); result }

#[derive(Debug)]
struct DriverStruct { probe:Arc<Probe>, calls:Cell<usize> }
#[async_trait::async_trait]
impl Struct for DriverStruct {
    fn capabilities(&self)->ResourceOperations {ResourceOperations::READ | ResourceOperations::TYPE_MAP}
    async fn sql_type_name(&mut self)->Result<String,RdbcError> {self.calls.set(self.calls.get()+1);Ok("schema.Target".into())}
    async fn attributes(&mut self)->Result<Vec<RdbcObject>,RdbcError> {
        self.calls.set(self.calls.get()+1);self.probe.attributes.fetch_add(1,Ordering::SeqCst);
        Ok(vec![Value::Int(7).into(),Value::Null.into()])
    }
    async fn attributes_with_type_map(&mut self,map:&RdbcTypeMap)->Result<Vec<RdbcObject>,RdbcError> {
        self.probe.maps.lock().unwrap().push(map.clone());
        let mut result=self.attributes().await?;
        if map.get("schema.Target").is_some() {result[0]=RdbcObject::String("explicit attribute".into());}
        Ok(result)
    }
}
#[derive(Debug)]
struct DriverRef { probe:Arc<Probe>, value:RdbcObject, capabilities:ResourceOperations, calls:Cell<usize> }
impl DriverRef {
    async fn record(&mut self)->Result<(),RdbcError> {
        self.calls.set(self.calls.get()+1);self.probe.calls.fetch_add(1,Ordering::SeqCst);
        if self.probe.block.load(Ordering::SeqCst) {self.probe.started.notify_one();std::future::pending::<()>().await;}
        if self.probe.fail.load(Ordering::SeqCst) {Err(if self.probe.fatal.load(Ordering::SeqCst) {RdbcError::SqlException(Box::new(SqlException::driver(96,"fatal ref driver failure").with_sql_state("08006")))} else {failure()})} else {Ok(())}
    }
}
impl Drop for DriverRef {fn drop(&mut self) {self.probe.drops.fetch_add(1,Ordering::SeqCst);}}
#[async_trait::async_trait]
impl Ref for DriverRef {
    fn capabilities(&self)->ResourceOperations {self.capabilities}
    async fn base_type_name(&mut self)->Result<String,RdbcError> {self.record().await?;Ok("schema.Target".into())}
    async fn object(&mut self)->Result<RdbcObject,RdbcError> {self.record().await?;Ok(self.value.clone())}
    async fn object_with_type_map(&mut self,map:&RdbcTypeMap)->Result<RdbcObject,RdbcError> {
        self.record().await?;self.probe.maps.lock().unwrap().push(map.clone());
        if map.get("schema.Target").is_some() {Ok(RdbcObject::String("explicit ref".into()))} else {Ok(self.value.clone())}
    }
    async fn set_object(&mut self,value:RdbcObject)->Result<(),RdbcError> {self.record().await?;self.value=value;Ok(())}
}
fn reference(probe:Arc<Probe>,caps:ResourceOperations)->RdbcRef {
    let structure=ResourceHandleFactory::structure(Box::new(DriverStruct {probe:probe.clone(),calls:Cell::new(0)}));
    ResourceHandleFactory::reference(Box::new(DriverRef {probe,value:RdbcObject::Struct(structure),capabilities:caps,calls:Cell::new(0)}))
}
fn send_static<T:Send+'static>() {}
fn shared_send<T:Send+Sync+'static>() {}
fn send_future<T:std::future::Future+Send>(future:T)->T {future}

#[tokio::test]
async fn task7_ref_standard_handle_is_lazy_and_preserves_clone_identity() {
    send_static::<DriverRef>();send_static::<DriverStruct>();shared_send::<RdbcRef>();shared_send::<RdbcStruct>();
    let probe=Arc::new(Probe::default());let value=reference(probe.clone(),ResourceOperations::reference());let alias=value.clone();
    assert_eq!(value,alias);assert_eq!(value.resource_id(),alias.resource_id());
    assert_eq!(probe.calls.load(Ordering::SeqCst),0);assert_eq!(probe.attributes.load(Ordering::SeqCst),0);
    assert_eq!(send_future(value.base_type_name()).await.unwrap(),"schema.Target");
    let RdbcObject::Struct(structure)=alias.object().await.unwrap() else {panic!("default Ref mapping must preserve Struct")};
    assert_eq!(probe.attributes.load(Ordering::SeqCst),0);assert_eq!(structure.sql_type_name().await.unwrap(),"schema.Target");
    assert_eq!(structure.attributes().await.unwrap(),vec![Value::Int(7).into(),Value::Null.into()]);
    assert_eq!(probe.attributes.load(Ordering::SeqCst),1);
    let other=reference(probe,ResourceOperations::reference());assert_ne!(value,other);assert_ne!(value.resource_id(),other.resource_id());
}
#[tokio::test]
async fn task7_ref_aliases_map_and_write_preserve_same_target() {
    let probe=Arc::new(Probe::default());let value=reference(probe.clone(),ResourceOperations::reference());let alias=value.clone();
    assert_eq!(value.get_base_type_name().await.unwrap(),"schema.Target");
    let original=value.get_object().await.unwrap();let explicit=map();
    assert_eq!(value.get_object_with_type_map(&explicit).await.unwrap(),RdbcObject::String("explicit ref".into()));
    assert_eq!(value.object().await.unwrap(),original);assert_eq!(probe.maps.lock().unwrap().as_slice(),&[explicit]);
    alias.set_object(Value::Null.into()).await.unwrap();assert!(value.object().await.unwrap().is_null());
    value.set_object(original.clone()).await.unwrap();assert_eq!(alias.object().await.unwrap(),original);
}
#[tokio::test]
async fn task7_ref_all_eight_capability_combinations_reject_before_driver() {
    let flags=[ResourceOperations::READ,ResourceOperations::WRITE,ResourceOperations::TYPE_MAP];
    for mask in 0..8 {
        let caps=flags.iter().enumerate().filter(|(i,_)|mask&(1<<i)!=0).fold(ResourceOperations::empty(),|caps,(_,flag)|caps|*flag);
        let probe=Arc::new(Probe::default());let value=reference(probe.clone(),caps|ResourceOperations::FREE|ResourceOperations::STREAM);
        assert_eq!(value.capabilities(),caps);
        let outcomes=[(value.base_type_name().await.map(|_|()),ResourceOperations::READ),(value.object().await.map(|_|()),ResourceOperations::READ),
            (value.object_with_type_map(&map()).await.map(|_|()),ResourceOperations::READ|ResourceOperations::TYPE_MAP),(value.set_object(Value::Null.into()).await,ResourceOperations::WRITE)];
        let mut expected=0;
        for (result,required) in outcomes {if caps.contains(required) {result.unwrap();expected+=1;} else {assert_eq!(result.unwrap_err().sql_exception().unwrap().sql_state(),Some("0A000"));}}
        assert_eq!(probe.calls.load(Ordering::SeqCst),expected);
    }
}
#[tokio::test]
async fn task7_ref_driver_errors_are_preserved_and_recoverable() {
    let probe=Arc::new(Probe::default());let value=reference(probe.clone(),ResourceOperations::reference());probe.fail.store(true,Ordering::SeqCst);
    for error in [value.base_type_name().await.unwrap_err(),value.object().await.unwrap_err(),value.object_with_type_map(&map()).await.unwrap_err(),value.set_object(Value::Null.into()).await.unwrap_err()] {assert_eq!(error,failure());}
    assert_eq!(probe.calls.load(Ordering::SeqCst),4);probe.fail.store(false,Ordering::SeqCst);assert!(matches!(value.object().await.unwrap(),RdbcObject::Struct(_)));
}
#[tokio::test]
async fn task7_ref_cancelled_call_unlocks_shared_standard_object() {
    let probe=Arc::new(Probe::default());let value=reference(probe.clone(),ResourceOperations::reference());probe.block.store(true,Ordering::SeqCst);
    let alias=value.clone();let task=tokio::spawn(async move {alias.object().await});probe.started.notified().await;
    task.abort();assert!(task.await.unwrap_err().is_cancelled());probe.block.store(false,Ordering::SeqCst);
    assert_eq!(value.base_type_name().await.unwrap(),"schema.Target");assert_eq!(probe.calls.load(Ordering::SeqCst),2);
    drop(value);assert_eq!(probe.drops.load(Ordering::SeqCst),1);
}
#[derive(Debug)]
struct RefCursor {value:RdbcRef,closed:bool,reads:Mutex<Vec<String>>,null:AtomicBool,fail:AtomicBool}
druid_rdbc::impl_standard_wrapper!(RefCursor => ResultSet);
impl RefCursor {
    fn read(&self,key:String,request:&ResultSetRead)->Result<RdbcObject,RdbcError> {
        if self.closed {return Err(RdbcError::ConnectionDiscarded);}
        assert!(matches!(request,ResultSetRead::Ref));self.reads.lock().unwrap().push(key);
        if self.fail.load(Ordering::SeqCst) {return Err(failure());}
        Ok(if self.null.load(Ordering::SeqCst) {Value::Null.into()} else {RdbcObject::Ref(self.value.clone())})
    }
}
#[async_trait::async_trait]
impl ResultSet for RefCursor {
    async fn next(&mut self)->Result<bool,RdbcError> {Ok(!self.closed)}
    fn read_object(&self,column:ColumnIndex,request:&ResultSetRead)->Result<RdbcObject,RdbcError> {self.read(column.get().to_string(),request)}
    fn read_object_by_label(&self,label:&str,request:&ResultSetRead)->Result<RdbcObject,RdbcError> {self.read(label.into(),request)}
    async fn close(&mut self)->Result<(),RdbcError> {self.closed=true;Ok(())}
    fn is_closed(&self)->Result<bool,RdbcError> {Ok(self.closed)}
}
#[tokio::test]
async fn task7_result_set_ref_by_index_and_label_preserve_identity_null_and_errors() {
    let probe=Arc::new(Probe::default());let value=reference(probe.clone(),ResourceOperations::reference());
    let mut cursor=RefCursor {value:value.clone(),closed:false,reads:Mutex::new(vec![]),null:AtomicBool::new(false),fail:AtomicBool::new(false)};
    let first=cursor.reference(ColumnIndex::new(1).unwrap()).unwrap().unwrap();let second=cursor.reference_by_label("OriginalLabel").unwrap().unwrap();
    assert_eq!(first,second);assert_eq!(first,value);assert_eq!(*cursor.reads.lock().unwrap(),vec!["1","OriginalLabel"]);
    cursor.null.store(true,Ordering::SeqCst);assert!(cursor.reference_by_label("Nullable").unwrap().is_none());
    cursor.fail.store(true,Ordering::SeqCst);assert_eq!(cursor.reference(ColumnIndex::new(1).unwrap()).unwrap_err(),failure());
    cursor.close().await.unwrap();assert_eq!(cursor.reference_by_label("closed").unwrap_err(),RdbcError::ConnectionDiscarded);
    drop(cursor);drop(value);drop(second);assert_eq!(probe.drops.load(Ordering::SeqCst),0);
    assert_eq!(first.base_type_name().await.unwrap(),"schema.Target");drop(first);assert_eq!(probe.drops.load(Ordering::SeqCst),1);
}
#[tokio::test]
async fn task7_struct_value_handle_maps_without_changing_default_attributes() {
    let probe=Arc::new(Probe::default());let value=reference(probe.clone(),ResourceOperations::reference());
    let RdbcObject::Struct(structure)=value.object().await.unwrap() else {panic!("Struct")};let alias=structure.clone();assert_eq!(structure,alias);
    let explicit=map();let mapped=structure.attributes_with_type_map(&explicit).await.unwrap();assert_eq!(mapped[0],RdbcObject::String("explicit attribute".into()));assert!(mapped[1].is_null());
    assert_eq!(alias.attributes().await.unwrap()[0],RdbcObject::Scalar(Value::Int(7)));assert_eq!(probe.maps.lock().unwrap().as_slice(),&[explicit]);
}

mod pooled_flow {
    use super::*;
    use druid::core::{DruidConnectionHolder,DruidPooledConnection};
    use druid_rdbc::connection::Connection;
    use druid_rdbc::statement::{Statement,StatementOptions};
    use std::time::Duration;

    #[derive(Debug)]
    struct RefConnection {value:RdbcRef,closed:bool,closed_statements:Arc<AtomicUsize>}
    druid_rdbc::impl_standard_wrapper!(RefConnection => Connection);
    #[async_trait::async_trait]
    impl Connection for RefConnection {
        async fn create_statement(&mut self,_:StatementOptions)->Result<Box<dyn Statement>,RdbcError> {
            if self.closed {return Err(RdbcError::ConnectionDiscarded);}
            Ok(Box::new(RefStatement {value:self.value.clone(),closed:false,closed_statements:self.closed_statements.clone()}))
        }
        async fn close(&mut self)->Result<(),RdbcError> {self.closed=true;Ok(())}
        fn is_closed(&self)->Result<bool,RdbcError> {Ok(self.closed)}
    }
    #[derive(Debug)]
    struct RefStatement {value:RdbcRef,closed:bool,closed_statements:Arc<AtomicUsize>}
    druid_rdbc::impl_standard_wrapper!(RefStatement => Statement);
    #[async_trait::async_trait]
    impl Statement for RefStatement {
        async fn execute_query(&mut self,_:&str)->Result<Box<dyn ResultSet>,RdbcError> {
            if self.closed {return Err(RdbcError::ConnectionDiscarded);}
            Ok(Box::new(RefCursor {value:self.value.clone(),closed:false,reads:Mutex::new(vec![]),null:AtomicBool::new(false),fail:AtomicBool::new(false)}))
        }
        async fn close(&mut self)->Result<(),RdbcError> {
            if !self.closed {self.closed=true;self.closed_statements.fetch_add(1,Ordering::SeqCst);}Ok(())
        }
        fn is_closed(&self)->Result<bool,RdbcError> {Ok(self.closed)}
    }
    async fn pooled(value:RdbcRef,closed_statements:Arc<AtomicUsize>,discards:Arc<AtomicUsize>)->DruidPooledConnection {
        let holder=DruidConnectionHolder::with_standard_connection(Box::new(RefConnection {value,closed:false,closed_statements}),77,Duration::ZERO,0).await.unwrap();
        DruidPooledConnection::with_standard_holder_and_disposition_observer(holder,"ref-contract".into(),None,false,false,
            Box::new(move |_| {discards.fetch_add(1,Ordering::SeqCst);}),Box::new(move |_,_|true))
    }
    #[tokio::test]
    async fn task7_pooled_result_set_ref_keeps_standard_driver_identity_after_parent_close() {
        let probe=Arc::new(Probe::default());let value=reference(probe.clone(),ResourceOperations::reference());
        let closed=Arc::new(AtomicUsize::new(0));let discards=Arc::new(AtomicUsize::new(0));let mut connection=pooled(value.clone(),closed.clone(),discards.clone()).await;
        let mut statement=connection.create_statement(StatementOptions::default()).await.unwrap();let mut cursor=statement.execute_query("select ref_value").await.unwrap();assert!(cursor.next().await.unwrap());
        let first=cursor.reference(ColumnIndex::new(1).unwrap()).unwrap().unwrap();let alias=cursor.reference_by_label("OriginalLabel").unwrap().unwrap();assert_eq!(first,value);assert_eq!(first,alias);
        assert_eq!(probe.calls.load(Ordering::SeqCst),0);assert_eq!(probe.attributes.load(Ordering::SeqCst),0);
        cursor.close().await.unwrap();assert!(cursor.reference_by_label("closed").is_err());statement.close().await.unwrap();connection.close().await.unwrap();
        assert_eq!(closed.load(Ordering::SeqCst),1);assert_eq!(first.base_type_name().await.unwrap(),"schema.Target");
        let RdbcObject::Struct(structure)=alias.object().await.unwrap() else {panic!("Struct")};assert_eq!(structure.attributes().await.unwrap()[0],Value::Int(7).into());
        assert_eq!(discards.load(Ordering::SeqCst),0);
    }
    #[tokio::test]
    async fn task7_pooled_returned_ref_preserves_raw_method_errors_without_extra_owner_policy() {
        let probe=Arc::new(Probe::default());let value=reference(probe.clone(),ResourceOperations::reference());let discards=Arc::new(AtomicUsize::new(0));
        let mut connection=pooled(value,Arc::new(AtomicUsize::new(0)),discards.clone()).await;let mut statement=connection.create_statement(StatementOptions::default()).await.unwrap();
        let cursor=statement.execute_query("select ref_value").await.unwrap();let result=cursor.reference(ColumnIndex::new(1).unwrap()).unwrap().unwrap();
        probe.fatal.store(true,Ordering::SeqCst);probe.fail.store(true,Ordering::SeqCst);let error=result.object().await.unwrap_err();assert_eq!(error.sql_exception().unwrap().sql_state(),Some("08006"));assert_eq!(error.sql_exception().unwrap().error_code(),96);assert_eq!(discards.load(Ordering::SeqCst),0);assert!(!connection.is_closed().unwrap());
        probe.fail.store(false,Ordering::SeqCst);assert_eq!(result.base_type_name().await.unwrap(),"schema.Target");connection.close().await.unwrap();
    }
}


#[tokio::test]
async fn task7_ref_and_struct_parameters_preserve_identity_without_implicit_materialization() {
    use druid_rdbc::statement::PreparedInputParameter;
    use druid_rdbc::types::{RdbcParameter,RdbcParameterValue};
    let probe=Arc::new(Probe::default());let value=reference(probe.clone(),ResourceOperations::reference());
    let parameter=PreparedInputParameter::Ref(Some(value.clone()));
    let Some(RdbcParameterValue::Object(RdbcObject::Ref(returned)))=parameter.value() else {panic!("Ref parameter")};assert_eq!(returned,value);
    assert!(parameter.scalar_value().is_err());assert_eq!(probe.calls.load(Ordering::SeqCst),0);
    let RdbcObject::Struct(structure)=value.object().await.unwrap() else {panic!("Struct")};
    let parameter=PreparedInputParameter::object_with_sql_type(Some(RdbcObject::Struct(structure.clone())),2002);
    let Some(RdbcParameterValue::Object(RdbcObject::Struct(returned)))=parameter.value() else {panic!("Struct parameter")};assert_eq!(returned,structure);
    assert!(parameter.scalar_value().is_err());assert_eq!(parameter.sql_type(),2002);assert_eq!(probe.attributes.load(Ordering::SeqCst),0);
}
#[derive(Debug)]
struct NestedStruct {value:RdbcRef,calls:Cell<usize>}
#[async_trait::async_trait]
impl Struct for NestedStruct {
    fn capabilities(&self)->ResourceOperations {ResourceOperations::READ|ResourceOperations::TYPE_MAP}
    async fn sql_type_name(&mut self)->Result<String,RdbcError> {self.calls.set(self.calls.get()+1);Ok("schema.Parent".into())}
    async fn attributes(&mut self)->Result<Vec<RdbcObject>,RdbcError> {self.calls.set(self.calls.get()+1);Ok(vec![RdbcObject::Ref(self.value.clone()),Value::Null.into()])}
    async fn attributes_with_type_map(&mut self,_:&RdbcTypeMap)->Result<Vec<RdbcObject>,RdbcError> {self.attributes().await}
}
#[tokio::test]
async fn task7_struct_nested_ref_remains_same_lazy_standard_object() {
    let probe=Arc::new(Probe::default());let value=reference(probe.clone(),ResourceOperations::reference());
    let parent=ResourceHandleFactory::structure(Box::new(NestedStruct {value:value.clone(),calls:Cell::new(0)}));
    let attributes=parent.attributes().await.unwrap();let RdbcObject::Ref(nested)=&attributes[0] else {panic!("Ref")};assert_eq!(nested,&value);assert!(attributes[1].is_null());
    assert_eq!(probe.calls.load(Ordering::SeqCst),0);assert_eq!(probe.attributes.load(Ordering::SeqCst),0);
    nested.set_object(Value::Null.into()).await.unwrap();assert!(value.object().await.unwrap().is_null());
}

}

// 新合同追加到原58项消费者之后，不替换既有测试体。
fn value_array_probe(supported:ResourceOperations, block:bool)->Arc<ArrayProbe> {
    Arc::new(ArrayProbe {supported,free_failures:AtomicUsize::new(0),free_fatal:AtomicBool::new(false),nonfatal_read:AtomicBool::new(false),args:(Some("vendor.Integer".into()),Some(array_values())),maps:Mutex::new(Vec::new()),calls:Mutex::new(Vec::new()),frees:AtomicUsize::new(0),started:tokio::sync::Notify::new(),finish:tokio::sync::Notify::new(),block,read_failure:AtomicBool::new(false),bad_metadata:AtomicBool::new(false),block_cursor:AtomicBool::new(false),cursor_started:tokio::sync::Notify::new(),cursor_finish:tokio::sync::Notify::new(),cursor_next_failure:AtomicBool::new(false),cursors:Mutex::new(Vec::new())})
}
fn value_array(probe:Arc<ArrayProbe>)->druid_rdbc::resource::RdbcArray {
    druid_rdbc::resource::ResourceHandleFactory::array(Box::new(ArrayDriver {probe,freed:false,not_sync:std::cell::Cell::new(0)}))
}
async fn value_array_operation(value:&druid_rdbc::resource::RdbcArray,op:usize)->Result<(),RdbcError> {
    let map=array_map();
    match op {
        0=>value.base_type_name().await.map(|_|()),1=>value.base_type().await.map(|_|()),
        2=>value.values().await.map(|_|()),3=>value.values_with_type_map(&map).await.map(|_|()),
        4=>value.values_range(1,2).await.map(|_|()),5=>value.values_range_with_type_map(1,2,&map).await.map(|_|()),
        6=>{let mut cursor=value.result_set().await?;cursor.close().await},
        7=>{let mut cursor=value.result_set_with_type_map(&map).await?;cursor.close().await},
        8=>{let mut cursor=value.result_set_range(1,2).await?;cursor.close().await},
        9=>{let mut cursor=value.result_set_range_with_type_map(1,2,&map).await?;cursor.close().await},
        _=>panic!("unknown operation"),
    }
}
#[tokio::test]
async fn task7_array_value_all_capabilities_gate_before_driver_and_after_free() {
    let flags=[ResourceOperations::READ,ResourceOperations::RANGE,ResourceOperations::TYPE_MAP,ResourceOperations::RESULT_SET,ResourceOperations::FREE];
    let required=[flags[0],flags[0],flags[0],flags[0]|flags[2],flags[0]|flags[1],flags[0]|flags[1]|flags[2],flags[0]|flags[3],flags[0]|flags[3]|flags[2],flags[0]|flags[1]|flags[3],flags[0]|flags[1]|flags[3]|flags[2]];
    for mask in 0..32 {
        let mut supported=ResourceOperations::empty();for (bit,flag) in flags.iter().enumerate(){if mask&(1<<bit)!=0 {supported|=*flag;}}
        let probe=value_array_probe(supported|ResourceOperations::WRITE|ResourceOperations::STREAM,false);
        let value=value_array(probe.clone());assert_eq!(value.capabilities(),supported);
        for (op,required) in required.iter().enumerate(){
            let before=probe.calls.lock().unwrap().len();let result=value_array_operation(&value,op).await;
            if supported.contains(*required) {assert!(result.is_ok(),"mask {mask} op {op}");assert_eq!(probe.calls.lock().unwrap().len(),before+1);}
            else {let error=result.unwrap_err();assert_eq!(error.class_name(),"java.sql.SQLFeatureNotSupportedException");assert_eq!(probe.calls.lock().unwrap().len(),before);}
        }
        if supported.contains(ResourceOperations::FREE) {
            value.free().await.unwrap();value.clone().free().await.unwrap();assert!(value.is_freed());assert_eq!(probe.frees.load(Ordering::SeqCst),1);
            for op in 0..10 {let before=probe.calls.lock().unwrap().len();assert!(value_array_operation(&value,op).await.is_err());assert_eq!(probe.calls.lock().unwrap().len(),before);}
        } else {let error=value.free().await.unwrap_err();assert_eq!(error.class_name(),"java.sql.SQLFeatureNotSupportedException");assert_eq!(probe.frees.load(Ordering::SeqCst),0);assert!(!value.is_freed());}
    }
}
#[tokio::test]
async fn task7_array_value_identity_maps_ranges_and_raw_cursor_ownership() {
    let probe=value_array_probe(ResourceOperations::array(),false);let value=value_array(probe.clone());let cloned=value.clone();
    assert_eq!(value,cloned);assert_eq!(value.resource_id(),cloned.resource_id());assert_ne!(value,value_array(value_array_probe(ResourceOperations::array(),false)));
    assert_eq!(value.get_base_type_name().await.unwrap(),"vendor.Integer");assert_eq!(value.get_base_type().await.unwrap(),4);assert_eq!(value.get_array().await.unwrap(),array_values());
    assert_eq!(value.values_range(2,2).await.unwrap(),array_values()[1..]);assert!(value.values_range(0,1).await.is_err());assert!(value.values_range(1,-1).await.is_err());
    let map=array_map();assert_eq!(value.values_with_type_map(&map).await.unwrap(),array_values());assert_eq!(value.values_range_with_type_map(2,1,&map).await.unwrap(),vec![Value::Null.into()]);
    let mut cursor=value.result_set_range_with_type_map(2,1,&map).await.unwrap();assert!(cursor.next().await.unwrap());assert_eq!(cursor.value(ColumnIndex::new(1).unwrap()).unwrap(),Value::Int(2));assert_eq!(cursor.value(ColumnIndex::new(2).unwrap()).unwrap(),Value::Null);
    assert_eq!(*probe.maps.lock().unwrap(),vec![map.clone(),map.clone(),map]);
    value.free().await.unwrap();assert_eq!(probe.cursors.lock().unwrap()[0].closes.load(Ordering::SeqCst),0);cursor.close().await.unwrap();assert_eq!(probe.cursors.lock().unwrap()[0].closes.load(Ordering::SeqCst),1);
}
#[tokio::test]
async fn task7_array_value_original_error_and_failed_free_retry() {
    let probe=value_array_probe(ResourceOperations::array(),false);let value=value_array(probe.clone());
    probe.nonfatal_read.store(true,Ordering::SeqCst);assert_eq!(value.values().await.unwrap_err().to_string(),array_nonfatal_error().to_string());probe.nonfatal_read.store(false,Ordering::SeqCst);
    probe.free_failures.store(1,Ordering::SeqCst);assert_eq!(value.free().await.unwrap_err().to_string(),array_nonfatal_error().to_string());assert!(!value.is_freed());assert_eq!(value.values().await.unwrap(),array_values());
    value.free().await.unwrap();assert!(value.is_freed());assert_eq!(probe.frees.load(Ordering::SeqCst),2);
}
#[tokio::test]
async fn task7_array_value_cancelled_raw_free_requires_retry_confirmation() {
    let probe=value_array_probe(ResourceOperations::array(),true);let value=value_array(probe.clone());let task_value=value.clone();
    let task=tokio::spawn(async move{task_value.free().await});probe.started.notified().await;task.abort();assert!(task.await.unwrap_err().is_cancelled());
    assert!(!value.is_freed());let calls=probe.calls.lock().unwrap().len();assert!(value.values().await.is_err());assert_eq!(probe.calls.lock().unwrap().len(),calls);
    let task_value=value.clone();let retry=tokio::spawn(async move{task_value.free().await});probe.started.notified().await;probe.finish.notify_one();retry.await.unwrap().unwrap();
    assert!(value.is_freed());value.free().await.unwrap();assert_eq!(probe.frees.load(Ordering::SeqCst),2);
}
#[tokio::test]
async fn task7_array_value_concurrent_clone_free_calls_driver_once() {
    let probe=value_array_probe(ResourceOperations::array(),true);let value=value_array(probe.clone());let first=value.clone();let second=value.clone();
    let a=tokio::spawn(async move{first.free().await});probe.started.notified().await;let b=tokio::spawn(async move{second.free().await});assert!(value.values().await.is_err());probe.finish.notify_one();
    a.await.unwrap().unwrap();b.await.unwrap().unwrap();assert_eq!(probe.frees.load(Ordering::SeqCst),1);assert!(value.is_freed());
}
#[tokio::test]
async fn task7_array_value_cancelled_read_unlocks_without_consuming_resource() {
    let probe=value_array_probe(ResourceOperations::array(),false);probe.block_cursor.store(true,Ordering::SeqCst);let value=value_array(probe.clone());let child=value.clone();
    let task=tokio::spawn(async move{child.result_set().await.map(|_|())});probe.cursor_started.notified().await;task.abort();assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(value.values().await.unwrap(),array_values());assert!(!value.is_freed());value.free().await.unwrap();assert_eq!(probe.frees.load(Ordering::SeqCst),1);
}
#[tokio::test]
async fn task7_array_value_pooled_free_cancel_and_parent_close_share_driver_release() {
    let control=Arc::new(Control::default());control.block_free.store(true,Ordering::SeqCst);let mut connection=pooled(control.clone()).await;
    let raw=connection.create_array_of(Some("vendor.Integer"),Some(array_values())).await.unwrap();let probe=array_probe(&control);let value=druid_rdbc::resource::ResourceHandleFactory::array(raw);
    let child=value.clone();let release=tokio::spawn(async move{child.free().await});probe.started.notified().await;release.abort();assert!(release.await.unwrap_err().is_cancelled());assert!(!value.is_freed());assert!(value.values().await.is_err());
    let close=tokio::spawn(async move{connection.close().await});probe.finish.notify_one();close.await.unwrap().unwrap();value.free().await.unwrap();assert!(value.is_freed());assert_eq!(probe.frees.load(Ordering::SeqCst),1);
}

/// 在驱动入口计数，避免驱动自身closed检查掩盖共享句柄漏检查。
#[derive(Debug)]
struct BoundaryArray {inner:ArrayDriver,values_calls:Arc<AtomicUsize>,panic_once:bool}
#[async_trait::async_trait]
impl Array for BoundaryArray {
    fn capabilities(&self)->ResourceOperations {self.inner.capabilities()}
    async fn base_type_name(&mut self)->Result<String,RdbcError>{self.inner.base_type_name().await}
    async fn base_type(&mut self)->Result<i32,RdbcError>{self.inner.base_type().await}
    async fn values(&mut self)->Result<Vec<RdbcObject>,RdbcError>{self.values_calls.fetch_add(1,Ordering::SeqCst);self.inner.values().await}
    async fn values_with_type_map(&mut self,map:&RdbcTypeMap)->Result<Vec<RdbcObject>,RdbcError>{self.inner.values_with_type_map(map).await}
    async fn values_range(&mut self,index:i64,count:i32)->Result<Vec<RdbcObject>,RdbcError>{self.inner.values_range(index,count).await}
    async fn values_range_with_type_map(&mut self,index:i64,count:i32,map:&RdbcTypeMap)->Result<Vec<RdbcObject>,RdbcError>{self.inner.values_range_with_type_map(index,count,map).await}
    async fn result_set(&mut self)->Result<Box<dyn ResultSet>,RdbcError>{self.inner.result_set().await}
    async fn result_set_with_type_map(&mut self,map:&RdbcTypeMap)->Result<Box<dyn ResultSet>,RdbcError>{self.inner.result_set_with_type_map(map).await}
    async fn result_set_range(&mut self,index:i64,count:i32)->Result<Box<dyn ResultSet>,RdbcError>{self.inner.result_set_range(index,count).await}
    async fn result_set_range_with_type_map(&mut self,index:i64,count:i32,map:&RdbcTypeMap)->Result<Box<dyn ResultSet>,RdbcError>{self.inner.result_set_range_with_type_map(index,count,map).await}
    async fn free(&mut self)->Result<(),RdbcError>{if self.panic_once {self.panic_once=false;panic!("controlled driver free panic");}self.inner.free().await}
}
#[tokio::test]
async fn task7_array_value_panicked_free_keeps_unknown_state_until_retry() {
    let probe=value_array_probe(ResourceOperations::array(),false);let values_calls=Arc::new(AtomicUsize::new(0));
    let value=druid_rdbc::resource::ResourceHandleFactory::array(Box::new(BoundaryArray {inner:ArrayDriver {probe:probe.clone(),freed:false,not_sync:std::cell::Cell::new(0)},values_calls:values_calls.clone(),panic_once:true}));
    let child=value.clone();let task=tokio::spawn(async move{child.free().await});assert!(task.await.unwrap_err().is_panic());assert!(!value.is_freed());assert!(value.values().await.is_err());assert_eq!(values_calls.load(Ordering::SeqCst),0);
    value.free().await.unwrap();assert!(value.is_freed());assert_eq!(probe.frees.load(Ordering::SeqCst),1);
}

#[tokio::test]
async fn task7_array_value_waiting_read_rechecks_release_after_lock_handoff() {
    let probe=value_array_probe(ResourceOperations::array(),false);
    probe.block_cursor.store(true,Ordering::SeqCst);
    let values_calls=Arc::new(AtomicUsize::new(0));
    let value=druid_rdbc::resource::ResourceHandleFactory::array(Box::new(BoundaryArray {inner:ArrayDriver {probe:probe.clone(),freed:false,not_sync:std::cell::Cell::new(0)},values_calls:values_calls.clone(),panic_once:false}));let child=value.clone();
    let cursor_task=tokio::spawn(async move{child.result_set().await});
    probe.cursor_started.notified().await;
    let mut release=Box::pin(value.free());
    std::future::poll_fn(|cx|{assert!(std::future::Future::poll(release.as_mut(),cx).is_pending());std::task::Poll::Ready(())}).await;
    let mut read=Box::pin(value.values());
    std::future::poll_fn(|cx|{assert!(std::future::Future::poll(read.as_mut(),cx).is_pending());std::task::Poll::Ready(())}).await;
    let calls=probe.calls.lock().unwrap().len();
    probe.cursor_finish.notify_one();let mut cursor=cursor_task.await.unwrap().unwrap();
    release.await.unwrap();assert!(read.await.is_err());
    assert_eq!(probe.calls.lock().unwrap().len(),calls);assert_eq!(probe.frees.load(Ordering::SeqCst),1);assert_eq!(values_calls.load(Ordering::SeqCst),0);
    cursor.close().await.unwrap();
}

use druid_rdbc::resource::{BlobPattern, RdbcBlob, ResourceHandleFactory};
#[derive(Debug)]
struct BlobValueProbe {
    supported:ResourceOperations, data:Arc<Mutex<Vec<u8>>>, calls:Mutex<Vec<&'static str>>,
    free_calls:AtomicUsize, failures:AtomicUsize, panic_free:AtomicBool,
    block_free:AtomicBool, free_started:tokio::sync::Notify, free_finish:tokio::sync::Notify,
    block_read:AtomicBool, read_started:tokio::sync::Notify, read_finish:tokio::sync::Notify,
    input:Mutex<Option<RdbcInputStream>>,output:Mutex<Option<RdbcOutputStream>>,same:AtomicUsize,
}
fn blob_value_probe(supported:ResourceOperations)->Arc<BlobValueProbe> {
    Arc::new(BlobValueProbe {supported,data:Arc::new(Mutex::new(vec![1,2,3,4])),calls:Mutex::new(Vec::new()),free_calls:AtomicUsize::new(0),failures:AtomicUsize::new(0),panic_free:AtomicBool::new(false),block_free:AtomicBool::new(false),free_started:tokio::sync::Notify::new(),free_finish:tokio::sync::Notify::new(),block_read:AtomicBool::new(false),read_started:tokio::sync::Notify::new(),read_finish:tokio::sync::Notify::new(),input:Mutex::new(None),output:Mutex::new(None),same:AtomicUsize::new(0)})
}
#[derive(Debug)]
struct BlobValueDriver {probe:Arc<BlobValueProbe>,freed:bool,not_sync:std::cell::Cell<usize>}
impl BlobValueDriver {
    fn touch(&self,name:&'static str)->Result<(),RdbcError>{
        self.probe.calls.lock().unwrap().push(name);self.not_sync.set(self.not_sync.get()+1);
        if self.freed {Err(RdbcError::rdbc_resource_closed("Blob"))}else{Ok(())}
    }
    fn range(&self,position:i64,length:i64)->Result<Vec<u8>,RdbcError>{
        let offset=position.checked_sub(1).and_then(|n|usize::try_from(n).ok()).ok_or_else(||RdbcError::InvalidArgument("position is 1-based".into()))?;
        let count=usize::try_from(length).map_err(|_|RdbcError::InvalidArgument("negative length".into()))?;
        let bytes=self.probe.data.lock().unwrap();let tail=bytes.get(offset..).ok_or_else(||RdbcError::InvalidArgument("position outside Blob".into()))?;Ok(tail[..count.min(tail.len())].to_vec())
    }
    fn search(&self,pattern:&[u8],start:i64)->Result<Option<i64>,RdbcError>{
        let offset=start.checked_sub(1).and_then(|n|usize::try_from(n).ok()).ok_or_else(||RdbcError::InvalidArgument("search is 1-based".into()))?;
        let bytes=self.probe.data.lock().unwrap();let tail=bytes.get(offset..).ok_or_else(||RdbcError::InvalidArgument("search outside Blob".into()))?;
        if pattern.is_empty(){return Ok(Some(start));}Ok(tail.windows(pattern.len()).position(|w|w==pattern).map(|n|(offset+n+1) as i64))
    }
    fn write_bytes(&self,position:i64,bytes:&[u8])->Result<i32,RdbcError>{
        let offset=position.checked_sub(1).and_then(|n|usize::try_from(n).ok()).ok_or_else(||RdbcError::InvalidArgument("write is 1-based".into()))?;
        let mut data=self.probe.data.lock().unwrap();if offset>data.len(){return Err(RdbcError::InvalidArgument("write outside Blob".into()));}
        let new_len=data.len().max(offset+bytes.len());data.resize(new_len,0);data[offset..offset+bytes.len()].copy_from_slice(bytes);Ok(bytes.len() as i32)
    }
}
#[derive(Debug)]
struct BlobValueWriter {data:Arc<Mutex<Vec<u8>>>,position:usize}
impl std::io::Write for BlobValueWriter {
    fn write(&mut self,bytes:&[u8])->std::io::Result<usize>{let mut data=self.data.lock().unwrap();let end=self.position+bytes.len();let new_len=data.len().max(end);data.resize(new_len,0);data[self.position..end].copy_from_slice(bytes);self.position=end;Ok(bytes.len())}
    fn flush(&mut self)->std::io::Result<()>{Ok(())}
}
#[async_trait::async_trait]
impl Blob for BlobValueDriver {
    fn capabilities(&self)->ResourceOperations{self.probe.supported}
    async fn length(&mut self)->Result<i64,RdbcError>{self.touch("length")?;self.probe.read_started.notify_one();if self.probe.block_read.load(Ordering::SeqCst){self.probe.read_finish.notified().await;}Ok(self.probe.data.lock().unwrap().len() as i64)}
    async fn get_bytes(&mut self,position:i64,length:i32)->Result<Vec<u8>,RdbcError>{self.touch("get_bytes")?;self.range(position,i64::from(length))}
    async fn get_binary_stream(&mut self)->Result<RdbcInputStream,RdbcError>{self.touch("get_binary_stream")?;let stream=RdbcInputStream::from_bytes(self.probe.data.lock().unwrap().clone());*self.probe.input.lock().unwrap()=Some(stream.clone());Ok(stream)}
    async fn position_bytes(&mut self,pattern:&[u8],start:i64)->Result<Option<i64>,RdbcError>{self.touch("position_bytes")?;self.search(pattern,start)}
    async fn position_blob(&mut self,pattern:BlobPattern<'_>,start:i64)->Result<Option<i64>,RdbcError>{
        self.touch("position_blob")?;tokio::task::yield_now().await;
        let bytes=match pattern {BlobPattern::SameResource=>{self.probe.same.fetch_add(1,Ordering::SeqCst);self.probe.data.lock().unwrap().clone()},BlobPattern::Other(pattern)=>{let length=i32::try_from(pattern.length().await?).map_err(|_|RdbcError::InvalidArgument("large pattern".into()))?;pattern.get_bytes(1,length).await?}};
        self.search(&bytes,start)
    }
    async fn set_bytes(&mut self,position:i64,bytes:&[u8])->Result<i32,RdbcError>{self.touch("set_bytes")?;self.write_bytes(position,bytes)}
    async fn set_bytes_range(&mut self,position:i64,bytes:&[u8],offset:i32,length:i32)->Result<i32,RdbcError>{
        self.touch("set_bytes_range")?;let offset=usize::try_from(offset).map_err(|_|RdbcError::InvalidArgument("negative offset".into()))?;let length=usize::try_from(length).map_err(|_|RdbcError::InvalidArgument("negative length".into()))?;let end=offset.checked_add(length).ok_or_else(||RdbcError::InvalidArgument("range overflow".into()))?;self.write_bytes(position,bytes.get(offset..end).ok_or_else(||RdbcError::InvalidArgument("input range".into()))?)
    }
    async fn set_binary_stream(&mut self,position:i64)->Result<RdbcOutputStream,RdbcError>{self.touch("set_binary_stream")?;let position=position.checked_sub(1).and_then(|n|usize::try_from(n).ok()).ok_or_else(||RdbcError::InvalidArgument("stream position".into()))?;if position>self.probe.data.lock().unwrap().len(){return Err(RdbcError::InvalidArgument("stream outside Blob".into()));}let stream=RdbcOutputStream::new(BlobValueWriter{data:self.probe.data.clone(),position});*self.probe.output.lock().unwrap()=Some(stream.clone());Ok(stream)}
    async fn truncate(&mut self,length:i64)->Result<(),RdbcError>{self.touch("truncate")?;let length=usize::try_from(length).map_err(|_|RdbcError::InvalidArgument("negative length".into()))?;let mut data=self.probe.data.lock().unwrap();if length>data.len(){return Err(RdbcError::InvalidArgument("truncate expands Blob".into()));}data.truncate(length);Ok(())}
    async fn free(&mut self)->Result<(),RdbcError>{
        self.probe.free_calls.fetch_add(1,Ordering::SeqCst);if self.probe.panic_free.swap(false,Ordering::SeqCst){panic!("controlled Blob free panic");}
        self.probe.free_started.notify_one();if self.probe.block_free.load(Ordering::SeqCst){self.probe.free_finish.notified().await;}
        if self.probe.failures.fetch_update(Ordering::SeqCst,Ordering::SeqCst,|n|n.checked_sub(1)).is_ok(){return Err(fatal_error());}self.freed=true;Ok(())
    }
    async fn get_binary_stream_range(&mut self,position:i64,length:i64)->Result<RdbcInputStream,RdbcError>{self.touch("get_binary_stream_range")?;let offset=position.checked_sub(1).and_then(|n|usize::try_from(n).ok()).ok_or_else(||RdbcError::InvalidArgument("stream position".into()))?;let count=usize::try_from(length).map_err(|_|RdbcError::InvalidArgument("negative length".into()))?;if offset.checked_add(count).is_none_or(|end|end>self.probe.data.lock().unwrap().len()){return Err(RdbcError::InvalidArgument("stream range".into()));}let stream=RdbcInputStream::from_bytes(self.range(position,length)?);*self.probe.input.lock().unwrap()=Some(stream.clone());Ok(stream)}
}
fn blob_value(probe:Arc<BlobValueProbe>)->RdbcBlob {ResourceHandleFactory::blob(Box::new(BlobValueDriver{probe,freed:false,not_sync:std::cell::Cell::new(0)}))}
async fn blob_value_operation(value:&RdbcBlob,pattern:&RdbcBlob,op:usize)->Result<(),RdbcError>{
    match op {0=>value.length().await.map(|_|()),1=>value.get_bytes(1,2).await.map(|_|()),2=>value.get_binary_stream().await.map(|_|()),3=>value.position_bytes(&[9],1).await.map(|_|()),4=>value.position_blob(pattern,1).await.map(|_|()),5=>value.set_bytes(1,&[9]).await.map(|_|()),6=>value.set_bytes_range(1,&[7,8],1,1).await.map(|_|()),7=>value.set_binary_stream(1).await.map(|_|()),8=>value.truncate(3).await,9=>value.get_binary_stream_range(1,2).await.map(|_|()),_=>panic!("unknown Blob operation")}
}
#[tokio::test]
async fn task7_blob_value_all_128_capabilities_and_freed_precedence(){
    let flags=[ResourceOperations::READ,ResourceOperations::WRITE,ResourceOperations::SEARCH,ResourceOperations::TRUNCATE,ResourceOperations::STREAM,ResourceOperations::RANGE,ResourceOperations::FREE];
    let required=[flags[0],flags[0]|flags[5],flags[0]|flags[4],flags[0]|flags[2],flags[0]|flags[2],flags[1],flags[1]|flags[5],flags[1]|flags[4],flags[1]|flags[3],flags[0]|flags[4]|flags[5]];
    for mask in 0..128 {let mut supported=ResourceOperations::empty();for(bit,flag)in flags.iter().enumerate(){if mask&(1<<bit)!=0{supported|=*flag;}}
        let probe=blob_value_probe(supported|ResourceOperations::TYPE_MAP|ResourceOperations::RESULT_SET);let value=blob_value(probe.clone());let pattern=blob_value(blob_value_probe(ResourceOperations::blob()));assert_eq!(value.capabilities(),supported);
        for(op,required)in required.iter().enumerate(){let before=probe.calls.lock().unwrap().len();let result=blob_value_operation(&value,&pattern,op).await;if supported.contains(*required){assert!(result.is_ok(),"mask {mask} operation {op}");assert_eq!(probe.calls.lock().unwrap().len(),before+1);}else{assert_eq!(result.unwrap_err().class_name(),"java.sql.SQLFeatureNotSupportedException");assert_eq!(probe.calls.lock().unwrap().len(),before);}}
        if supported.contains(ResourceOperations::FREE){value.free().await.unwrap();value.clone().free().await.unwrap();assert!(value.is_freed());assert_eq!(probe.free_calls.load(Ordering::SeqCst),1);for op in 0..10{let before=probe.calls.lock().unwrap().len();assert!(blob_value_operation(&value,&pattern,op).await.is_err());assert_eq!(probe.calls.lock().unwrap().len(),before);}}
        else{assert_eq!(value.free().await.unwrap_err().class_name(),"java.sql.SQLFeatureNotSupportedException");assert_eq!(probe.free_calls.load(Ordering::SeqCst),0);assert!(!value.is_freed());}
    }
}
#[tokio::test]
async fn task7_blob_value_identity_offsets_search_and_exact_streams(){
    let probe=blob_value_probe(ResourceOperations::blob());let value=blob_value(probe.clone());assert_eq!(value,value.clone());assert_eq!(value.resource_id(),value.clone().resource_id());assert_ne!(value,blob_value(blob_value_probe(ResourceOperations::blob())));
    assert_eq!(value.get_bytes(2,2).await.unwrap(),vec![2,3]);assert!(value.get_bytes(0,1).await.is_err());assert!(value.get_bytes(1,-1).await.is_err());assert_eq!(value.set_bytes_range(2,&[7,8,9],1,2).await.unwrap(),2);assert_eq!(value.get_bytes(1,4).await.unwrap(),vec![1,8,9,4]);assert!(value.set_bytes_range(1,&[1],-1,1).await.is_err());assert_eq!(value.position_bytes(&[8,9],1).await.unwrap(),Some(2));
    let input=value.get_binary_stream_range(2,2).await.unwrap();assert_eq!(Some(input.clone()),*probe.input.lock().unwrap());assert_eq!(input.read_to_end().unwrap(),vec![8,9]);assert!(value.get_binary_stream_range(4,2).await.is_err());
    let output=value.set_binary_stream(3).await.unwrap();assert_eq!(Some(output.clone()),*probe.output.lock().unwrap());assert_eq!(output.write(&[5,6,7]).unwrap(),3);output.close().unwrap();assert_eq!(value.get_bytes(1,5).await.unwrap(),vec![1,8,5,6,7]);value.truncate(2).await.unwrap();assert_eq!(value.length().await.unwrap(),2);value.free().await.unwrap();
}
#[tokio::test]
async fn task7_blob_value_self_pattern_is_delegated_without_snapshot(){
    let probe=blob_value_probe(ResourceOperations::blob());let value=blob_value(probe.clone());
    assert_eq!(value.position_blob(&value,1).await.unwrap(),Some(1));assert_eq!(value.position_blob(&value.clone(),2).await.unwrap(),None);assert!(value.position_blob(&value,0).await.is_err());
    assert_eq!(probe.same.load(Ordering::SeqCst),3);assert_eq!(*probe.calls.lock().unwrap(),vec!["position_blob";3]);
}
#[tokio::test]
async fn task7_blob_value_opposite_searches_use_consistent_lock_order(){
    let a_probe=blob_value_probe(ResourceOperations::blob());let b_probe=blob_value_probe(ResourceOperations::blob());let a=blob_value(a_probe.clone());let b=blob_value(b_probe.clone());let aa=a.clone();let bb=b.clone();
    let first=tokio::spawn(async move{a.position_blob(&b,1).await});let second=tokio::spawn(async move{bb.position_blob(&aa,1).await});
    tokio::time::timeout(Duration::from_secs(2),async{assert_eq!(first.await.unwrap().unwrap(),Some(1));assert_eq!(second.await.unwrap().unwrap(),Some(1));}).await.expect("opposite searches must not deadlock");
    for probe in [a_probe,b_probe]{let calls=probe.calls.lock().unwrap();assert_eq!(calls.iter().filter(|&&c|c=="length").count(),1);assert_eq!(calls.iter().filter(|&&c|c=="get_bytes").count(),1);assert_eq!(calls.iter().filter(|&&c|c=="position_blob").count(),1);}
}
#[tokio::test]
async fn task7_blob_value_closed_pattern_never_enters_target_driver(){
    let probe=blob_value_probe(ResourceOperations::blob());let value=blob_value(probe.clone());let pattern=blob_value(blob_value_probe(ResourceOperations::blob()));pattern.free().await.unwrap();assert!(value.position_blob(&pattern,1).await.is_err());assert!(probe.calls.lock().unwrap().is_empty());
}
#[tokio::test]
async fn task7_blob_value_free_error_cancel_panic_and_retry_confirmation(){
    let probe=blob_value_probe(ResourceOperations::blob());let value=blob_value(probe.clone());probe.failures.store(1,Ordering::SeqCst);assert_eq!(value.free().await.unwrap_err().to_string(),fatal_error().to_string());probe.free_started.notified().await;assert!(!value.is_freed());assert_eq!(value.length().await.unwrap(),4);
    probe.panic_free.store(true,Ordering::SeqCst);let child=value.clone();assert!(tokio::spawn(async move{child.free().await}).await.unwrap_err().is_panic());assert!(!value.is_freed());assert!(value.length().await.is_err());
    probe.block_free.store(true,Ordering::SeqCst);let child=value.clone();let task=tokio::spawn(async move{child.free().await});probe.free_started.notified().await;task.abort();assert!(task.await.unwrap_err().is_cancelled());assert!(value.length().await.is_err());probe.block_free.store(false,Ordering::SeqCst);value.free().await.unwrap();assert!(value.is_freed());value.free().await.unwrap();assert_eq!(probe.free_calls.load(Ordering::SeqCst),4);
}
#[tokio::test]
async fn task7_blob_value_concurrent_free_is_once(){
    let probe=blob_value_probe(ResourceOperations::blob());probe.block_free.store(true,Ordering::SeqCst);let value=blob_value(probe.clone());let a=value.clone();let b=value.clone();let first=tokio::spawn(async move{a.free().await});probe.free_started.notified().await;let second=tokio::spawn(async move{b.free().await});assert!(value.length().await.is_err());probe.free_finish.notify_one();first.await.unwrap().unwrap();second.await.unwrap().unwrap();assert_eq!(probe.free_calls.load(Ordering::SeqCst),1);
}
#[tokio::test]
async fn task7_blob_value_waiting_read_rechecks_state_after_free(){
    let probe=blob_value_probe(ResourceOperations::blob());probe.block_read.store(true,Ordering::SeqCst);let value=blob_value(probe.clone());let child=value.clone();let first=tokio::spawn(async move{child.length().await});probe.read_started.notified().await;
    let mut release=Box::pin(value.free());std::future::poll_fn(|cx|{assert!(std::future::Future::poll(release.as_mut(),cx).is_pending());std::task::Poll::Ready(())}).await;
    let mut read=Box::pin(value.get_bytes(1,1));std::future::poll_fn(|cx|{assert!(std::future::Future::poll(read.as_mut(),cx).is_pending());std::task::Poll::Ready(())}).await;probe.read_finish.notify_one();first.await.unwrap().unwrap();release.await.unwrap();assert!(read.await.is_err());assert_eq!(*probe.calls.lock().unwrap(),vec!["length"]);
}
#[tokio::test]
async fn task7_blob_value_pooled_cancellation_parent_close_is_once(){
    let control=Arc::new(Control::default());control.block_free.store(true,Ordering::SeqCst);let mut connection=pooled(control.clone()).await;let raw=connection.create_blob().await.unwrap();let probe=control.probes.lock().unwrap()[0].clone();let value=ResourceHandleFactory::blob(raw);let child=value.clone();
    let task=tokio::spawn(async move{child.free().await});probe.started.notified().await;task.abort();assert!(task.await.unwrap_err().is_cancelled());assert!(value.length().await.is_err());let close=tokio::spawn(async move{connection.close().await});probe.finish.notify_one();close.await.unwrap().unwrap();value.free().await.unwrap();assert_eq!(probe.calls.load(Ordering::SeqCst),1);assert!(value.is_freed());
}

#[tokio::test]
async fn task7_blob_value_blocked_first_locks_preserve_global_order(){
    let ap=blob_value_probe(ResourceOperations::blob());let bp=blob_value_probe(ResourceOperations::blob());
    ap.block_read.store(true,Ordering::SeqCst);bp.block_read.store(true,Ordering::SeqCst);
    let a=blob_value(ap.clone());let b=blob_value(bp.clone());let aa=a.clone();let bb=b.clone();
    let hold_a=tokio::spawn(async move{aa.length().await});ap.read_started.notified().await;
    let hold_b=tokio::spawn(async move{bb.length().await});bp.read_started.notified().await;
    // 两个接收者都被真实读取占用，先让搜索在首把锁排队，避免单线程调度掩盖反向取锁。
    let mut first=Box::pin(a.position_blob(&b,1));let mut second=Box::pin(b.position_blob(&a,1));
    std::future::poll_fn(|cx|{assert!(std::future::Future::poll(first.as_mut(),cx).is_pending());assert!(std::future::Future::poll(second.as_mut(),cx).is_pending());std::task::Poll::Ready(())}).await;
    ap.block_read.store(false,Ordering::SeqCst);bp.block_read.store(false,Ordering::SeqCst);
    ap.read_finish.notify_one();bp.read_finish.notify_one();hold_a.await.unwrap().unwrap();hold_b.await.unwrap().unwrap();
    let (left,right)=tokio::time::timeout(Duration::from_secs(2),async{tokio::join!(first,second)}).await.expect("both searches must complete after queued reads release their locks");
    assert_eq!(left.unwrap(),Some(1));assert_eq!(right.unwrap(),Some(1));
    for probe in [ap,bp]{let calls=probe.calls.lock().unwrap();assert_eq!(calls.iter().filter(|&&name|name=="position_blob").count(),1);assert_eq!(calls.iter().filter(|&&name|name=="length").count(),2);}
}

#[tokio::test]
async fn task7_blob_value_pooled_self_and_other_search_preserve_owner_lifecycle(){
    let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;
    let first=ResourceHandleFactory::blob(connection.create_blob().await.unwrap());let second=ResourceHandleFactory::blob(connection.create_blob().await.unwrap());
    assert_eq!(first.position_blob(&first.clone(),1).await.unwrap(),Some(1));assert_eq!(first.position_blob(&first,2).await.unwrap(),None);assert_eq!(first.position_blob(&second,1).await.unwrap(),Some(1));
    connection.close().await.unwrap();assert!(first.length().await.is_err());assert!(second.length().await.is_err());
    first.free().await.unwrap();second.free().await.unwrap();assert!(first.is_freed());assert!(second.is_freed());
    for probe in control.probes.lock().unwrap().iter(){assert_eq!(probe.calls.load(Ordering::SeqCst),1);}
    assert_eq!(control.returns.lock().unwrap().len(),1);
}
#[tokio::test]
async fn task7_blob_value_cancelled_pattern_read_releases_both_locks(){
    let ap=blob_value_probe(ResourceOperations::blob());let bp=blob_value_probe(ResourceOperations::blob());bp.block_read.store(true,Ordering::SeqCst);
    let a=blob_value(ap);let b=blob_value(bp.clone());let aa=a.clone();let bb=b.clone();let search=tokio::spawn(async move{aa.position_blob(&bb,1).await});bp.read_started.notified().await;
    search.abort();assert!(search.await.unwrap_err().is_cancelled());bp.block_read.store(false,Ordering::SeqCst);
    tokio::time::timeout(Duration::from_secs(2),async{assert_eq!(a.length().await.unwrap(),4);assert_eq!(b.length().await.unwrap(),4);a.free().await.unwrap();b.free().await.unwrap();}).await.expect("cancelled search must release both resource locks");
}

#[tokio::test]
async fn task7_blob_value_target_capability_precedes_closed_pattern_for_both_orders(){
    // 构造两种身份排序，避免只覆盖恰好先锁目标的分支。
    for target_first in [true,false]{
        let target_probe=blob_value_probe(ResourceOperations::READ|ResourceOperations::FREE);
        let (target,pattern)=if target_first {
            (blob_value(target_probe.clone()),blob_value(blob_value_probe(ResourceOperations::blob())))
        }else{
            let pattern=blob_value(blob_value_probe(ResourceOperations::blob()));(blob_value(target_probe.clone()),pattern)
        };
        pattern.free().await.unwrap();
        let error=target.position_blob(&pattern,1).await.unwrap_err();
        assert_eq!(error.class_name(),"java.sql.SQLFeatureNotSupportedException");assert!(target_probe.calls.lock().unwrap().is_empty());
        target.free().await.unwrap();let error=target.position_blob(&pattern,1).await.unwrap_err();assert_eq!(error.class_name(),RdbcError::rdbc_resource_closed("Blob").class_name());
    }
}

mod clob_value_contract {
use super::*;
use druid_rdbc::resource::{ClobPattern,RdbcClob,RdbcNClob,ResourceHandleFactory};
use std::cell::Cell;
#[derive(Debug)]
struct Probe {
    supported:ResourceOperations, units:Arc<Mutex<Vec<u16>>>,calls:Mutex<Vec<&'static str>>,
    frees:AtomicUsize,failures:AtomicUsize,panic_free:AtomicBool,block_free:AtomicBool,
    free_started:tokio::sync::Notify,free_finish:tokio::sync::Notify,
    block_read:AtomicBool,read_started:tokio::sync::Notify,read_finish:tokio::sync::Notify,
    same:AtomicUsize,reader:Mutex<Option<RdbcReader>>,writer:Mutex<Option<RdbcWriter>>,
    input:Mutex<Option<RdbcInputStream>>,output:Mutex<Option<RdbcOutputStream>>,
}
fn probe(supported:ResourceOperations)->Arc<Probe>{Arc::new(Probe{
    supported,units:Arc::new(Mutex::new(vec![65,0xD83D,0xDE00,90])),calls:Mutex::new(Vec::new()),
    frees:AtomicUsize::new(0),failures:AtomicUsize::new(0),panic_free:AtomicBool::new(false),block_free:AtomicBool::new(false),
    free_started:tokio::sync::Notify::new(),free_finish:tokio::sync::Notify::new(),block_read:AtomicBool::new(false),
    read_started:tokio::sync::Notify::new(),read_finish:tokio::sync::Notify::new(),same:AtomicUsize::new(0),
    reader:Mutex::new(None),writer:Mutex::new(None),input:Mutex::new(None),output:Mutex::new(None),
})}
#[derive(Debug)] struct Driver {probe:Arc<Probe>,freed:bool,not_sync:Cell<usize>}
fn offset(position:i64)->Result<usize,RdbcError>{position.checked_sub(1).filter(|v|*v>=0).and_then(|v|usize::try_from(v).ok()).ok_or_else(||RdbcError::InvalidArgument("invalid 1-based position".into()))}
fn count(value:i64)->Result<usize,RdbcError>{usize::try_from(value).map_err(|_|RdbcError::InvalidArgument("negative length".into()))}
fn write(units:&Arc<Mutex<Vec<u16>>>,position:usize,value:&[u16])->Result<usize,RdbcError>{
    let mut data=units.lock().unwrap();if position>data.len(){return Err(RdbcError::InvalidArgument("write outside clob".into()));}
    let end=position.checked_add(value.len()).ok_or_else(||RdbcError::InvalidArgument("overflow".into()))?;let length=data.len().max(end);data.resize(length,0);data[position..end].copy_from_slice(value);Ok(value.len())
}
#[derive(Debug)] struct CharacterSink {units:Arc<Mutex<Vec<u16>>>,position:usize}
impl druid_rdbc::extensions::CharacterWriter for CharacterSink {
    fn write_utf16(&mut self,value:&[u16])->Result<usize,RdbcError>{let n=write(&self.units,self.position,value)?;self.position+=n;Ok(n)}
    fn flush(&mut self)->Result<(),RdbcError>{Ok(())}
    fn close(&mut self)->Result<(),RdbcError>{Ok(())}
}
#[derive(Debug)] struct AsciiSink {units:Arc<Mutex<Vec<u16>>>,position:usize}
impl std::io::Write for AsciiSink {
    fn write(&mut self,bytes:&[u8])->std::io::Result<usize>{let value=bytes.iter().map(|v|u16::from(*v)).collect::<Vec<_>>();let n=write(&self.units,self.position,&value).map_err(std::io::Error::other)?;self.position+=n;Ok(n)}
    fn flush(&mut self)->std::io::Result<()>{Ok(())}
}
impl Driver {
    fn enter(&self,name:&'static str)->Result<(),RdbcError>{if self.freed{return Err(RdbcError::rdbc_resource_closed("Clob"));}self.probe.calls.lock().unwrap().push(name);self.not_sync.set(self.not_sync.get()+1);Ok(())}
    fn range(&self,position:i64,length:i64,strict:bool)->Result<Vec<u16>,RdbcError>{let offset=offset(position)?;let n=count(length)?;let data=self.probe.units.lock().unwrap();let tail=data.get(offset..).ok_or_else(||RdbcError::InvalidArgument("range position".into()))?;if strict && n>tail.len(){return Err(RdbcError::InvalidArgument("range length".into()));}Ok(tail[..n.min(tail.len())].to_vec())}
    fn search(&self,pattern:&[u16],start:i64)->Result<Option<i64>,RdbcError>{let i=offset(start)?;let data=self.probe.units.lock().unwrap();let tail=data.get(i..).ok_or_else(||RdbcError::InvalidArgument("search outside clob".into()))?;if pattern.is_empty(){return Ok(Some(start));}Ok(tail.windows(pattern.len()).position(|w|w==pattern).map(|p|(i+p+1)as i64))}
}
#[async_trait::async_trait]
impl Clob for Driver {
    fn capabilities(&self)->ResourceOperations{self.probe.supported}
    async fn length(&mut self)->Result<i64,RdbcError>{self.enter("length")?;self.probe.read_started.notify_one();if self.probe.block_read.load(Ordering::SeqCst){self.probe.read_finish.notified().await;}Ok(self.probe.units.lock().unwrap().len()as i64)}
    async fn get_sub_string(&mut self,position:i64,length:i32)->Result<RdbcString,RdbcError>{self.enter("get_sub_string")?;Ok(RdbcString::from_utf16(self.range(position,i64::from(length),false)?))}
    async fn get_character_stream(&mut self)->Result<RdbcReader,RdbcError>{self.enter("get_character_stream")?;let reader=RdbcReader::from_utf16(self.probe.units.lock().unwrap().clone());*self.probe.reader.lock().unwrap()=Some(reader.clone());Ok(reader)}
    async fn get_ascii_stream(&mut self)->Result<RdbcInputStream,RdbcError>{self.enter("get_ascii_stream")?;let stream=RdbcInputStream::from_bytes(self.probe.units.lock().unwrap().iter().map(|v|u8::try_from(*v).unwrap_or(b'?')).collect::<Vec<_>>());*self.probe.input.lock().unwrap()=Some(stream.clone());Ok(stream)}
    async fn position_string(&mut self,pattern:&RdbcString,start:i64)->Result<Option<i64>,RdbcError>{self.enter("position_string")?;self.search(pattern.as_utf16(),start)}
    async fn position_clob(&mut self,pattern:ClobPattern<'_>,start:i64)->Result<Option<i64>,RdbcError>{
        self.enter("position_clob")?;let units=match pattern {ClobPattern::SameResource=>{self.probe.same.fetch_add(1,Ordering::SeqCst);self.probe.units.lock().unwrap().clone()},ClobPattern::Other(pattern)=>{let n=i32::try_from(pattern.length().await?).map_err(|_|RdbcError::InvalidArgument("large pattern".into()))?;pattern.get_sub_string(1,n).await?.as_utf16().to_vec()}};self.search(&units,start)
    }
    async fn set_string(&mut self,position:i64,value:&RdbcString)->Result<i32,RdbcError>{self.enter("set_string")?;Ok(write(&self.probe.units,offset(position)?,value.as_utf16())?as i32)}
    async fn set_string_range(&mut self,position:i64,value:&RdbcString,input_offset:i32,length:i32)->Result<i32,RdbcError>{self.enter("set_string_range")?;let start=count(i64::from(input_offset))?;let end=start.checked_add(count(i64::from(length))?).ok_or_else(||RdbcError::InvalidArgument("input overflow".into()))?;let units=value.as_utf16().get(start..end).ok_or_else(||RdbcError::InvalidArgument("input range".into()))?;Ok(write(&self.probe.units,offset(position)?,units)?as i32)}
    async fn set_ascii_stream(&mut self,position:i64)->Result<RdbcOutputStream,RdbcError>{self.enter("set_ascii_stream")?;let stream=RdbcOutputStream::new(AsciiSink{units:self.probe.units.clone(),position:offset(position)?});*self.probe.output.lock().unwrap()=Some(stream.clone());Ok(stream)}
    async fn set_character_stream(&mut self,position:i64)->Result<RdbcWriter,RdbcError>{self.enter("set_character_stream")?;let writer=RdbcWriter::new(CharacterSink{units:self.probe.units.clone(),position:offset(position)?});*self.probe.writer.lock().unwrap()=Some(writer.clone());Ok(writer)}
    async fn truncate(&mut self,length:i64)->Result<(),RdbcError>{self.enter("truncate")?;let n=count(length)?;let mut units=self.probe.units.lock().unwrap();if n>units.len(){return Err(RdbcError::InvalidArgument("cannot expand".into()));}units.truncate(n);Ok(())}
    async fn free(&mut self)->Result<(),RdbcError>{self.probe.frees.fetch_add(1,Ordering::SeqCst);self.probe.free_started.notify_one();if self.probe.panic_free.swap(false,Ordering::SeqCst){panic!("controlled Clob free panic");}if self.probe.block_free.load(Ordering::SeqCst){self.probe.free_finish.notified().await;}if self.probe.failures.fetch_update(Ordering::SeqCst,Ordering::SeqCst,|n|n.checked_sub(1)).is_ok(){return Err(fatal_error());}self.freed=true;Ok(())}
    async fn get_character_stream_range(&mut self,position:i64,length:i64)->Result<RdbcReader,RdbcError>{self.enter("get_character_stream_range")?;let reader=RdbcReader::from_utf16(self.range(position,length,true)?);*self.probe.reader.lock().unwrap()=Some(reader.clone());Ok(reader)}
}
impl NClob for Driver {}
fn driver(probe:Arc<Probe>)->Driver{Driver{probe,freed:false,not_sync:Cell::new(0)}}
fn clob(probe:Arc<Probe>)->RdbcClob{ResourceHandleFactory::clob(Box::new(driver(probe)))}
fn n_clob(probe:Arc<Probe>)->RdbcNClob{ResourceHandleFactory::n_clob(Box::new(driver(probe)))}
async fn operation(value:&RdbcClob,pattern:&RdbcClob,op:usize)->Result<(),RdbcError>{let text=RdbcString::from("Q");match op{
    0=>value.length().await.map(|_|()),1=>value.get_sub_string(1,2).await.map(|_|()),2=>value.get_character_stream().await.map(|_|()),3=>value.get_ascii_stream().await.map(|_|()),4=>value.position_string(&text,1).await.map(|_|()),5=>value.position_clob(pattern,1).await.map(|_|()),6=>value.set_string(1,&text).await.map(|_|()),7=>value.set_string_range(1,&text,0,1).await.map(|_|()),8=>value.set_ascii_stream(1).await.map(|_|()),9=>value.set_character_stream(1).await.map(|_|()),10=>value.truncate(3).await,11=>value.get_character_stream_range(1,2).await.map(|_|()),_=>panic!("unknown Clob operation")}}
#[tokio::test]
async fn all_capability_combinations_and_freed_precedence(){
    let flags=[ResourceOperations::READ,ResourceOperations::WRITE,ResourceOperations::SEARCH,ResourceOperations::TRUNCATE,ResourceOperations::STREAM,ResourceOperations::RANGE,ResourceOperations::FREE];
    let required=[flags[0],flags[0]|flags[5],flags[0]|flags[4],flags[0]|flags[4],flags[0]|flags[2],flags[0]|flags[2],flags[1],flags[1]|flags[5],flags[1]|flags[4],flags[1]|flags[4],flags[1]|flags[3],flags[0]|flags[4]|flags[5]];
    for national in [false,true]{for mask in 0..128{let mut supported=ResourceOperations::empty();for(bit,flag)in flags.iter().enumerate(){if mask&(1<<bit)!=0{supported|=*flag;}}
        let p=probe(supported|ResourceOperations::TYPE_MAP|ResourceOperations::RESULT_SET);let national_value=national.then(||n_clob(p.clone()));let value=if let Some(n)=&national_value{n.as_clob().clone()}else{clob(p.clone())};let pattern=clob(probe(ResourceOperations::clob()));assert_eq!(value.capabilities(),supported);
        for(op,required)in required.iter().enumerate(){let before=p.calls.lock().unwrap().len();let result=operation(&value,&pattern,op).await;if supported.contains(*required){assert!(result.is_ok(),"national {national} mask {mask} op {op}");assert_eq!(p.calls.lock().unwrap().len(),before+1);}else{assert_eq!(result.unwrap_err().class_name(),"java.sql.SQLFeatureNotSupportedException");assert_eq!(p.calls.lock().unwrap().len(),before);}}
        if supported.contains(ResourceOperations::FREE){value.free().await.unwrap();value.clone().free().await.unwrap();assert_eq!(p.frees.load(Ordering::SeqCst),1);for op in 0..12{let before=p.calls.lock().unwrap().len();assert_eq!(operation(&value,&pattern,op).await.unwrap_err().class_name(),"java.sql.SQLException");assert_eq!(p.calls.lock().unwrap().len(),before);}if let Some(n)=national_value{assert!(n.is_freed());n.free().await.unwrap();}}
        else{assert_eq!(value.free().await.unwrap_err().class_name(),"java.sql.SQLFeatureNotSupportedException");assert_eq!(p.frees.load(Ordering::SeqCst),0);assert!(!value.is_freed());}
    }}
}
#[tokio::test]
async fn utf16_offsets_and_original_stream_identity(){
    let p=probe(ResourceOperations::clob());let value=clob(p.clone());assert_eq!(value.length().await.unwrap(),4);assert_eq!(value,value.clone());assert_ne!(value,clob(probe(ResourceOperations::clob())));
    assert_eq!(value.get_sub_string(2,1).await.unwrap().as_utf16(),&[0xD83D]);assert_eq!(value.position_string(&RdbcString::from("😀"),1).await.unwrap(),Some(2));assert!(value.get_sub_string(0,1).await.is_err());
    assert_eq!(value.set_string_range(1,&RdbcString::from("😀"),1,1).await.unwrap(),1);assert_eq!(value.get_sub_string(1,1).await.unwrap().as_utf16(),&[0xDE00]);assert!(value.set_string_range(1,&RdbcString::from("😀"),2,1).await.is_err());
    let reader=value.get_character_stream_range(1,2).await.unwrap();assert_eq!(Some(reader.clone()),*p.reader.lock().unwrap());assert_eq!(reader.read_to_end_utf16().unwrap(),vec![0xDE00,0xD83D]);assert!(value.get_character_stream_range(4,2).await.is_err());
    let writer=value.set_character_stream(2).await.unwrap();assert_eq!(Some(writer.clone()),*p.writer.lock().unwrap());assert_eq!(writer.write_utf16(&[0xD800,0xDC00,0xDFFF]).unwrap(),3);writer.close().unwrap();assert_eq!(value.get_sub_string(2,3).await.unwrap().as_utf16(),&[0xD800,0xDC00,0xDFFF]);
    let output=value.set_ascii_stream(1).await.unwrap();assert_eq!(Some(output.clone()),*p.output.lock().unwrap());output.write(b"AB").unwrap();output.close().unwrap();let input=value.get_ascii_stream().await.unwrap();assert_eq!(Some(input.clone()),*p.input.lock().unwrap());assert_eq!(input.read_to_end().unwrap(),b"AB??");
    let full=value.get_character_stream().await.unwrap();assert_eq!(Some(full.clone()),*p.reader.lock().unwrap());assert_eq!(full.read_to_end_utf16().unwrap(),vec![65,66,0xDC00,0xDFFF]);value.truncate(2).await.unwrap();assert_eq!(value.length().await.unwrap(),2);
}
#[tokio::test]
async fn national_view_shares_identity_content_alias_and_free(){
    let p=probe(ResourceOperations::clob());let n=n_clob(p.clone());let view=n.as_clob().clone();assert_eq!(n.resource_id(),view.resource_id());assert_eq!(n,n.clone());
    n.set_string(1,&RdbcString::from("国")).await.unwrap();assert_eq!(view.get_sub_string(1,1).await.unwrap(),RdbcString::from("国"));assert_eq!(n.position_clob(&view,1).await.unwrap(),Some(1));assert_eq!(view.position_clob(n.as_clob(),2).await.unwrap(),None);assert_eq!(p.same.load(Ordering::SeqCst),2);
    view.free().await.unwrap();assert!(n.is_freed());assert!(n.length().await.unwrap_err().to_string().contains("NClob"));n.free().await.unwrap();assert_eq!(p.frees.load(Ordering::SeqCst),1);
}
#[tokio::test]
async fn self_pattern_preserves_start_and_no_snapshot(){
    let p=probe(ResourceOperations::clob());let value=clob(p.clone());assert_eq!(value.position_clob(&value,1).await.unwrap(),Some(1));assert_eq!(value.position_clob(&value.clone(),2).await.unwrap(),None);assert!(value.position_clob(&value,0).await.is_err());assert_eq!(p.same.load(Ordering::SeqCst),3);assert_eq!(*p.calls.lock().unwrap(),vec!["position_clob";3]);
}
#[tokio::test]
async fn target_admission_precedes_closed_pattern_in_both_orders(){
    for reverse in [false,true]{let target=probe(ResourceOperations::READ|ResourceOperations::FREE);let other=probe(ResourceOperations::clob());let (value,pattern)=if reverse{let pattern=clob(other);(clob(target.clone()),pattern)}else{(clob(target.clone()),clob(other))};pattern.free().await.unwrap();assert_eq!(value.position_clob(&pattern,1).await.unwrap_err().class_name(),"java.sql.SQLFeatureNotSupportedException");assert!(target.calls.lock().unwrap().is_empty());value.free().await.unwrap();assert_eq!(value.position_clob(&pattern,1).await.unwrap_err().class_name(),"java.sql.SQLException");}
}
#[tokio::test]
async fn failure_panic_cancellation_retry_confirmation(){
    for national in [false,true]{let p=probe(ResourceOperations::clob());let n=national.then(||n_clob(p.clone()));let value=if let Some(n)=&n{n.as_clob().clone()}else{clob(p.clone())};p.failures.store(1,Ordering::SeqCst);assert_eq!(value.free().await.unwrap_err().to_string(),fatal_error().to_string());p.free_started.notified().await;assert!(!value.is_freed());assert_eq!(value.length().await.unwrap(),4);
        p.panic_free.store(true,Ordering::SeqCst);let v=value.clone();assert!(tokio::spawn(async move{v.free().await}).await.unwrap_err().is_panic());p.free_started.notified().await;assert!(value.length().await.is_err());assert!(!value.is_freed());
        p.block_free.store(true,Ordering::SeqCst);let v=value.clone();let task=tokio::spawn(async move{v.free().await});p.free_started.notified().await;task.abort();assert!(task.await.unwrap_err().is_cancelled());assert!(!value.is_freed());assert!(value.length().await.is_err());p.block_free.store(false,Ordering::SeqCst);value.free().await.unwrap();value.free().await.unwrap();assert_eq!(p.frees.load(Ordering::SeqCst),4);if let Some(n)=n{assert!(n.is_freed());}
    }
}
#[tokio::test]
async fn concurrent_view_free_releases_driver_once(){
    let p=probe(ResourceOperations::clob());p.block_free.store(true,Ordering::SeqCst);let n=n_clob(p.clone());let view=n.as_clob().clone();let task=tokio::spawn(async move{view.free().await});p.free_started.notified().await;let second=tokio::spawn(async move{n.free().await});p.free_finish.notify_one();task.await.unwrap().unwrap();second.await.unwrap().unwrap();assert_eq!(p.frees.load(Ordering::SeqCst),1);
}
#[tokio::test]
async fn cancelled_search_releases_both_resource_locks(){
    let a=clob(probe(ResourceOperations::clob()));let p=probe(ResourceOperations::clob());p.block_read.store(true,Ordering::SeqCst);let b=clob(p.clone());let aa=a.clone();let bb=b.clone();let task=tokio::spawn(async move{aa.position_clob(&bb,1).await});p.read_started.notified().await;task.abort();assert!(task.await.unwrap_err().is_cancelled());p.block_read.store(false,Ordering::SeqCst);tokio::time::timeout(Duration::from_secs(2),async{assert_eq!(a.length().await.unwrap(),4);assert_eq!(b.length().await.unwrap(),4);a.free().await.unwrap();b.free().await.unwrap();}).await.unwrap();
}
#[tokio::test]
async fn actual_pooled_values_preserve_views_and_cancel_safe_parent_cleanup(){
    for national in [false,true]{let control=Arc::new(Control::default());control.block_free.store(true,Ordering::SeqCst);let mut connection=pooled(control.clone()).await;
        let n=if national{Some(ResourceHandleFactory::n_clob(connection.create_n_clob().await.unwrap()))}else{None};let value=if let Some(n)=&n{n.as_clob().clone()}else{ResourceHandleFactory::clob(connection.create_clob().await.unwrap())};
        assert_eq!(value.position_clob(&value.clone(),1).await.unwrap(),Some(1));let probe=control.probes.lock().unwrap()[0].clone();let v=value.clone();let release=tokio::spawn(async move{v.free().await});probe.started.notified().await;release.abort();assert!(release.await.unwrap_err().is_cancelled());assert!(!value.is_freed());assert!(value.length().await.is_err());let mut parent=tokio::spawn(async move{connection.close().await});assert!(tokio::time::timeout(Duration::from_millis(20),&mut parent).await.is_err());assert!(control.returns.lock().unwrap().is_empty());probe.finish.notify_one();parent.await.unwrap().unwrap();value.free().await.unwrap();assert!(value.is_freed());if let Some(n)=n{assert!(n.is_freed());n.free().await.unwrap();}assert_eq!(probe.calls.load(Ordering::SeqCst),1);assert_eq!(control.returns.lock().unwrap().len(),1);
    }
}
#[cfg(task7_require_clob_value_sync)] fn negative_sync(){fn requires_sync<T:Sync>(){}requires_sync::<Driver>();}
#[cfg(task7_require_old_clob_access)] use druid_rdbc::resource::{RdbcClobAccess,RdbcNClobAccess};
#[tokio::test]
async fn blocked_first_locks_preserve_global_order(){
    let ap=probe(ResourceOperations::clob());let bp=probe(ResourceOperations::clob());
    ap.block_read.store(true,Ordering::SeqCst);bp.block_read.store(true,Ordering::SeqCst);
    let a=clob(ap.clone());let b=clob(bp.clone());let aa=a.clone();let bb=b.clone();
    let hold_a=tokio::spawn(async move{aa.length().await});ap.read_started.notified().await;
    let hold_b=tokio::spawn(async move{bb.length().await});bp.read_started.notified().await;
    // 两个接收者都被真实读取占用，先让搜索在首把锁排队，避免单线程调度掩盖反向取锁。
    let mut first=Box::pin(a.position_clob(&b,1));let mut second=Box::pin(b.position_clob(&a,1));
    std::future::poll_fn(|cx|{assert!(std::future::Future::poll(first.as_mut(),cx).is_pending());assert!(std::future::Future::poll(second.as_mut(),cx).is_pending());std::task::Poll::Ready(())}).await;
    ap.block_read.store(false,Ordering::SeqCst);bp.block_read.store(false,Ordering::SeqCst);
    ap.read_finish.notify_one();bp.read_finish.notify_one();hold_a.await.unwrap().unwrap();hold_b.await.unwrap().unwrap();
    let (left,right)=tokio::time::timeout(Duration::from_secs(2),async{tokio::join!(first,second)}).await.expect("both searches must complete after queued reads release their locks");
    assert_eq!(left.unwrap(),Some(1));assert_eq!(right.unwrap(),Some(1));
    for probe in [ap,bp]{let calls=probe.calls.lock().unwrap();assert_eq!(calls.iter().filter(|&&name|name=="position_clob").count(),1);assert_eq!(calls.iter().filter(|&&name|name=="length").count(),2);}
}

#[tokio::test]
async fn target_admission_precedence_covers_both_verified_identity_orders(){
    // UUID身份随机；排序三个同能力句柄，明确让关闭模式处于两个目标之间。
    let mut values=(0..3).map(|_|{let p=probe(ResourceOperations::READ|ResourceOperations::FREE);(clob(p.clone()),p)}).collect::<Vec<_>>();
    values.sort_by(|a,b|a.0.resource_id().cmp(b.0.resource_id()));let (high,high_probe)=values.pop().unwrap();let (pattern,_)=values.pop().unwrap();let (low,low_probe)=values.pop().unwrap();
    assert!(low.resource_id()<pattern.resource_id());assert!(high.resource_id()>pattern.resource_id());pattern.free().await.unwrap();
    for(value,p)in [(low,low_probe),(high,high_probe)]{assert_eq!(value.position_clob(&pattern,1).await.unwrap_err().class_name(),"java.sql.SQLFeatureNotSupportedException");assert!(p.calls.lock().unwrap().is_empty());value.free().await.unwrap();assert_eq!(value.position_clob(&pattern,1).await.unwrap_err().class_name(),"java.sql.SQLException");}
}
#[tokio::test]
async fn national_public_methods_delegate_all_inherited_operations(){
    let p=probe(ResourceOperations::clob());let n=n_clob(p.clone());let view=n.as_clob().clone();
    assert_eq!(n.length().await.unwrap(),4);assert_eq!(n.get_sub_string(2,1).await.unwrap().as_utf16(),&[0xD83D]);
    assert_eq!(n.position_string(&RdbcString::from("😀"),1).await.unwrap(),Some(2));assert_eq!(n.position_clob(&view,1).await.unwrap(),Some(1));
    let full=n.get_character_stream().await.unwrap();assert_eq!(Some(full.clone()),*p.reader.lock().unwrap());assert_eq!(full.read_to_end_utf16().unwrap(),vec![65,0xD83D,0xDE00,90]);
    let ranged=n.get_character_stream_range(3,1).await.unwrap();assert_eq!(Some(ranged.clone()),*p.reader.lock().unwrap());assert_eq!(ranged.read_to_end_utf16().unwrap(),vec![0xDE00]);
    let ascii=n.get_ascii_stream().await.unwrap();assert_eq!(Some(ascii.clone()),*p.input.lock().unwrap());assert_eq!(ascii.read_to_end().unwrap(),b"A??Z");
    assert_eq!(n.set_string(1,&RdbcString::from("文")).await.unwrap(),1);assert_eq!(n.set_string_range(2,&RdbcString::from("😀"),1,1).await.unwrap(),1);
    let writer=n.set_character_stream(3).await.unwrap();assert_eq!(Some(writer.clone()),*p.writer.lock().unwrap());writer.write_utf16(&[0xDFFF]).unwrap();writer.close().unwrap();
    let output=n.set_ascii_stream(4).await.unwrap();assert_eq!(Some(output.clone()),*p.output.lock().unwrap());output.write(b"Q").unwrap();output.close().unwrap();
    assert_eq!(view.get_sub_string(1,4).await.unwrap().as_utf16(),&[0x6587,0xDE00,0xDFFF,81]);n.truncate(3).await.unwrap();assert_eq!(view.length().await.unwrap(),3);
    assert_eq!(&p.calls.lock().unwrap()[..12],&["length","get_sub_string","position_string","position_clob","get_character_stream","get_character_stream_range","get_ascii_stream","set_string","set_string_range","set_character_stream","set_ascii_stream","get_sub_string"]);
    assert_eq!(n.capabilities(),view.capabilities());assert_eq!(n.resource_id(),view.resource_id());n.free().await.unwrap();view.free().await.unwrap();assert!(n.is_freed());assert_eq!(p.frees.load(Ordering::SeqCst),1);
    let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;let pooled_n=ResourceHandleFactory::n_clob(connection.create_n_clob().await.unwrap());let pooled_c=ResourceHandleFactory::clob(connection.create_clob().await.unwrap());
    assert_eq!(pooled_n.position_clob(&pooled_c,1).await.unwrap(),Some(1));assert_eq!(pooled_c.position_clob(pooled_n.as_clob(),1).await.unwrap(),Some(1));connection.close().await.unwrap();pooled_n.free().await.unwrap();pooled_c.free().await.unwrap();
    assert!(pooled_n.is_freed());assert!(pooled_c.is_freed());for probe in control.probes.lock().unwrap().iter(){assert_eq!(probe.calls.load(Ordering::SeqCst),1);}assert_eq!(control.returns.lock().unwrap().len(),1);
}

}

mod xml_value_contract {
use super::*;
use druid_rdbc::resource::{RdbcSqlXml,ResourceHandleFactory};
#[derive(Debug)]struct Probe {
    xml:Arc<XmlProbe>,supported:ResourceOperations,panic_read:AtomicBool,panic_write:AtomicBool,fail_write:AtomicBool,
    panic_free:AtomicBool,free_attempts:AtomicUsize,free_started:tokio::sync::Notify,
    input:Mutex<Option<RdbcInputStream>>,output:Mutex<Option<RdbcOutputStream>>,reader:Mutex<Option<RdbcReader>>,writer:Mutex<Option<RdbcWriter>>,
}
fn probe(supported:ResourceOperations,block_free:bool)->Arc<Probe>{Arc::new(Probe{xml:Arc::new(XmlProbe::new(block_free)),supported,panic_read:AtomicBool::new(false),panic_write:AtomicBool::new(false),fail_write:AtomicBool::new(false),panic_free:AtomicBool::new(false),free_attempts:AtomicUsize::new(0),free_started:tokio::sync::Notify::new(),input:Mutex::new(None),output:Mutex::new(None),reader:Mutex::new(None),writer:Mutex::new(None)})}
#[derive(Debug)]struct Driver{inner:XmlDriver,probe:Arc<Probe>}
fn driver(p:Arc<Probe>)->Driver{Driver{inner:XmlDriver{probe:p.xml.clone(),freed:false,not_sync:std::cell::Cell::new(0),stream:true},probe:p}}
fn value(p:Arc<Probe>)->RdbcSqlXml{ResourceHandleFactory::sql_xml(Box::new(driver(p)))}
#[async_trait::async_trait]
impl SqlXml for Driver {
    fn capabilities(&self)->ResourceOperations{self.probe.supported}
    async fn free(&mut self)->Result<(),RdbcError>{self.probe.free_attempts.fetch_add(1,Ordering::SeqCst);self.probe.free_started.notify_one();if self.probe.panic_free.swap(false,Ordering::SeqCst){panic!("controlled XML free panic");}self.inner.free().await}
    async fn binary_stream(&mut self)->Result<RdbcInputStream,RdbcError>{let stream=self.inner.binary_stream().await?;*self.probe.input.lock().unwrap()=Some(stream.clone());Ok(stream)}
    async fn set_binary_stream(&mut self)->Result<RdbcOutputStream,RdbcError>{let stream=self.inner.set_binary_stream().await?;*self.probe.output.lock().unwrap()=Some(stream.clone());Ok(stream)}
    async fn character_stream(&mut self)->Result<RdbcReader,RdbcError>{let reader=self.inner.character_stream().await?;*self.probe.reader.lock().unwrap()=Some(reader.clone());Ok(reader)}
    async fn set_character_stream(&mut self)->Result<RdbcWriter,RdbcError>{let writer=self.inner.set_character_stream().await?;*self.probe.writer.lock().unwrap()=Some(writer.clone());Ok(writer)}
    async fn string(&mut self)->Result<RdbcString,RdbcError>{let text=self.inner.string().await?;if self.probe.panic_read.swap(false,Ordering::SeqCst){panic!("controlled XML read panic");}Ok(text)}
    async fn set_string(&mut self,text:&RdbcString)->Result<(),RdbcError>{self.inner.set_string(text).await?;if self.probe.panic_write.swap(false,Ordering::SeqCst){panic!("controlled XML write panic");}if self.probe.fail_write.load(Ordering::SeqCst){return Err(RdbcError::SqlException(Box::new(SqlException::driver(89,"XML write failed").with_sql_state("22000"))));}Ok(())}
    async fn source(&mut self,representation:Option<&RdbcXmlRepresentationType>)->Result<RdbcXmlSource,RdbcError>{self.inner.source(representation).await}
    async fn result(&mut self,representation:Option<&RdbcXmlRepresentationType>)->Result<RdbcXmlResult,RdbcError>{self.inner.result(representation).await}
}
async fn read(v:&RdbcSqlXml,kind:usize)->Result<(),RdbcError>{match kind{0=>v.string().await.map(|_|()),1=>v.binary_stream().await.map(|_|()),2=>v.character_stream().await.map(|_|()),3=>v.source(None).await.map(|_|()),4=>v.get_string().await.map(|_|()),5=>v.get_binary_stream().await.map(|_|()),6=>v.get_character_stream().await.map(|_|()),_=>panic!("read kind")}}
async fn write(v:&RdbcSqlXml,kind:usize)->Result<(),RdbcError>{match kind{0=>v.set_string(&RdbcString::from("<v/>" )).await,1=>v.set_binary_stream().await.map(|_|()),2=>v.set_character_stream().await.map(|_|()),3=>v.result(None).await.map(|_|()),_=>panic!("write kind")}}
async fn operation(v:&RdbcSqlXml,op:usize)->Result<(),RdbcError>{if op<4{read(v,op).await}else{write(v,op-4).await}}
fn sequence_error(error:RdbcError){assert_eq!(error.class_name(),"java.sql.SQLException");assert_eq!(error.sql_exception().unwrap().sql_state(),Some("HY010"));}
#[tokio::test]
async fn every_capability_combination_and_freed_precedence(){
    let flags=[ResourceOperations::READ,ResourceOperations::WRITE,ResourceOperations::STREAM,ResourceOperations::FREE];let required=[flags[0],flags[0]|flags[2],flags[0]|flags[2],flags[0],flags[1],flags[1]|flags[2],flags[1]|flags[2],flags[1]];
    for mask in 0..16{let mut supported=ResourceOperations::empty();for(bit,flag)in flags.iter().enumerate(){if mask&(1<<bit)!=0{supported|=*flag;}}
        for(op,required)in required.iter().enumerate(){let p=probe(supported|ResourceOperations::TYPE_MAP|ResourceOperations::RESULT_SET|ResourceOperations::RANGE,false);let v=value(p.clone());assert_eq!(v.capabilities(),supported);let result=operation(&v,op).await;
            if supported.contains(*required){assert!(result.is_ok(),"mask {mask} op {op}");assert_eq!(p.xml.reads.load(Ordering::SeqCst)+p.xml.writes.load(Ordering::SeqCst),1);}else{assert_eq!(result.unwrap_err().class_name(),"java.sql.SQLFeatureNotSupportedException");assert_eq!(p.xml.reads.load(Ordering::SeqCst)+p.xml.writes.load(Ordering::SeqCst),0);}
            if supported.contains(ResourceOperations::FREE){v.free().await.unwrap();v.clone().free().await.unwrap();assert!(v.is_freed());assert_eq!(p.free_attempts.load(Ordering::SeqCst),1);let before=p.xml.reads.load(Ordering::SeqCst)+p.xml.writes.load(Ordering::SeqCst);for other in 0..8{assert_eq!(operation(&v,other).await.unwrap_err().class_name(),"java.sql.SQLException");}assert_eq!(p.xml.reads.load(Ordering::SeqCst)+p.xml.writes.load(Ordering::SeqCst),before);}
            else{assert_eq!(v.free().await.unwrap_err().class_name(),"java.sql.SQLFeatureNotSupportedException");assert_eq!(p.free_attempts.load(Ordering::SeqCst),0);assert!(!v.is_freed());}
        }
    }
}
#[tokio::test]
async fn aliases_share_all_read_and_write_consumption_gates(){
    for first in 0..7{for second in 0..7{let p=probe(ResourceOperations::sql_xml(),false);let v=value(p.clone());let alias=v.clone();assert_eq!(v,alias);assert_eq!(v.resource_id(),alias.resource_id());read(&v,first).await.unwrap();sequence_error(read(&alias,second).await.unwrap_err());assert_eq!(p.xml.reads.load(Ordering::SeqCst),1);write(&v,0).await.unwrap();assert_eq!(p.xml.writes.load(Ordering::SeqCst),1);v.free().await.unwrap();}}
    for first in 0..4{for second in 0..4{let p=probe(ResourceOperations::sql_xml(),false);let v=value(p.clone());write(&v,first).await.unwrap();sequence_error(write(&v.clone(),second).await.unwrap_err());assert_eq!(p.xml.writes.load(Ordering::SeqCst),1);read(&v,0).await.unwrap();assert_eq!(p.xml.reads.load(Ordering::SeqCst),1);v.free().await.unwrap();}}
}
#[tokio::test]
async fn unsupported_stream_does_not_consume_supported_text_direction(){
    let p=probe(ResourceOperations::READ|ResourceOperations::WRITE|ResourceOperations::FREE,false);let v=value(p.clone());assert!(v.get_binary_stream().await.is_err());assert!(v.get_character_stream().await.is_err());assert!(v.set_binary_stream().await.is_err());assert!(v.set_character_stream().await.is_err());let text=RdbcString::from_utf16(vec![60,0xD800,62]);v.set_string(&text).await.unwrap();assert_eq!(v.get_string().await.unwrap(),text);assert_eq!(p.xml.reads.load(Ordering::SeqCst),1);assert_eq!(p.xml.writes.load(Ordering::SeqCst),1);v.free().await.unwrap();
}
#[tokio::test]
async fn original_streams_utf16_and_nullable_framework_identity(){
    let p=probe(ResourceOperations::sql_xml(),false);let v=value(p.clone());let writer=v.set_character_stream().await.unwrap();assert_eq!(Some(writer.clone()),*p.writer.lock().unwrap());writer.write_utf16(&[60,0xDFFF,62]).unwrap();writer.close().unwrap();let reader=v.get_character_stream().await.unwrap();assert_eq!(Some(reader.clone()),*p.reader.lock().unwrap());assert_eq!(reader.read_to_end_utf16().unwrap(),vec![60,0xDFFF,62]);reader.close().unwrap();v.free().await.unwrap();
    let p=probe(ResourceOperations::sql_xml(),false);let v=value(p.clone());let output=v.set_binary_stream().await.unwrap();assert_eq!(Some(output.clone()),*p.output.lock().unwrap());output.write(b"<bytes/>").unwrap();output.close().unwrap();assert_eq!(*p.xml.binary.lock().unwrap(),b"<bytes/>");let input=v.get_binary_stream().await.unwrap();assert_eq!(Some(input.clone()),*p.input.lock().unwrap());assert_eq!(input.read_to_end().unwrap(),"<v>😀</v>".as_bytes());input.close().unwrap();v.free().await.unwrap();
    for representation in [None,Some(RdbcXmlRepresentationType::Dom),Some(RdbcXmlRepresentationType::Stream),Some(RdbcXmlRepresentationType::Sax),Some(RdbcXmlRepresentationType::Stax),Some(RdbcXmlRepresentationType::Custom("vendor.ExactXml".into()))]{
        let p=probe(ResourceOperations::sql_xml(),false);let v=value(p.clone());let source=v.source(representation.as_ref()).await.unwrap();let result=v.result(representation.as_ref()).await.unwrap();assert_eq!(source,p.xml.sources.lock().unwrap()[0]);assert_eq!(result,p.xml.results.lock().unwrap()[0]);assert_eq!(*p.xml.representations.lock().unwrap(),vec![representation.clone(),representation]);sequence_error(v.source(None).await.unwrap_err());sequence_error(v.result(None).await.unwrap_err());v.free().await.unwrap();
    }
}
#[tokio::test]
async fn errors_and_panics_do_not_rearm_direction(){
    let p=probe(ResourceOperations::sql_xml(),false);p.xml.read_nonfatal.store(true,Ordering::SeqCst);let v=value(p.clone());assert_eq!(v.get_string().await.unwrap_err().sql_exception().unwrap().error_code(),88);p.xml.read_nonfatal.store(false,Ordering::SeqCst);sequence_error(v.source(None).await.unwrap_err());assert_eq!(p.xml.reads.load(Ordering::SeqCst),1);v.set_string(&RdbcString::from("<valid/>" )).await.unwrap();v.free().await.unwrap();
    let p=probe(ResourceOperations::sql_xml(),false);p.fail_write.store(true,Ordering::SeqCst);let v=value(p.clone());assert_eq!(v.set_string(&RdbcString::from("<bad/>" )).await.unwrap_err().sql_exception().unwrap().error_code(),89);p.fail_write.store(false,Ordering::SeqCst);sequence_error(v.result(None).await.unwrap_err());assert_eq!(p.xml.writes.load(Ordering::SeqCst),1);v.get_string().await.unwrap();v.free().await.unwrap();
    for during_read in [false,true]{let p=probe(ResourceOperations::sql_xml(),false);let v=value(p.clone());if during_read{p.panic_read.store(true,Ordering::SeqCst);}else{p.panic_write.store(true,Ordering::SeqCst);}let alias=v.clone();let task=tokio::spawn(async move{if during_read{alias.string().await.map(|_|())}else{alias.set_string(&RdbcString::from("<panic/>" )).await}});assert!(task.await.unwrap_err().is_panic());if during_read{sequence_error(v.string().await.unwrap_err());assert_eq!(p.xml.reads.load(Ordering::SeqCst),1);v.set_string(&RdbcString::from("<other/>" )).await.unwrap();}else{sequence_error(v.set_string(&RdbcString::from("<again/>" )).await.unwrap_err());assert_eq!(p.xml.writes.load(Ordering::SeqCst),1);v.string().await.unwrap();}v.free().await.unwrap();}
}
#[tokio::test]
async fn cancelled_entered_read_or_write_consumes_only_its_direction(){
    for during_read in [false,true]{let p=probe(ResourceOperations::sql_xml(),false);let v=value(p.clone());if during_read{p.xml.read_block.store(true,Ordering::SeqCst);}else{p.xml.write_block.store(true,Ordering::SeqCst);}let alias=v.clone();let task=tokio::spawn(async move{if during_read{alias.string().await.map(|_|())}else{alias.set_string(&RdbcString::from("<cancel/>" )).await}});if during_read{p.xml.read_started.notified().await;}else{p.xml.write_started.notified().await;}task.abort();assert!(task.await.unwrap_err().is_cancelled());p.xml.read_block.store(false,Ordering::SeqCst);p.xml.write_block.store(false,Ordering::SeqCst);if during_read{sequence_error(v.get_binary_stream().await.unwrap_err());v.set_string(&RdbcString::from("<other/>" )).await.unwrap();}else{sequence_error(v.set_character_stream().await.unwrap_err());v.get_string().await.unwrap();}assert_eq!(p.xml.reads.load(Ordering::SeqCst),1);assert_eq!(p.xml.writes.load(Ordering::SeqCst),1);v.free().await.unwrap();}
}
#[tokio::test]
async fn cancelled_waiter_does_not_consume_direction_before_driver_entry(){
    let p=probe(ResourceOperations::sql_xml(),false);p.xml.write_block.store(true,Ordering::SeqCst);let v=value(p.clone());let alias=v.clone();let writing=tokio::spawn(async move{alias.set_string(&RdbcString::from("<held/>" )).await});p.xml.write_started.notified().await;
    let mut waiting=Box::pin(v.get_string());std::future::poll_fn(|cx|{assert!(std::future::Future::poll(waiting.as_mut(),cx).is_pending());std::task::Poll::Ready(())}).await;drop(waiting);assert_eq!(p.xml.reads.load(Ordering::SeqCst),0);p.xml.write_finish.notify_one();writing.await.unwrap().unwrap();assert_eq!(v.get_string().await.unwrap(),RdbcString::from("<held/>"));assert_eq!(p.xml.reads.load(Ordering::SeqCst),1);v.free().await.unwrap();
}
#[tokio::test]
async fn concurrent_clones_enter_read_driver_once(){
    let p=probe(ResourceOperations::sql_xml(),false);p.xml.read_block.store(true,Ordering::SeqCst);let v=value(p.clone());let alias=v.clone();let reading=tokio::spawn(async move{alias.string().await});p.xml.read_started.notified().await;let mut waiting=Box::pin(v.source(None));std::future::poll_fn(|cx|{assert!(std::future::Future::poll(waiting.as_mut(),cx).is_pending());std::task::Poll::Ready(())}).await;p.xml.read_finish.notify_one();reading.await.unwrap().unwrap();sequence_error(waiting.await.unwrap_err());assert_eq!(p.xml.reads.load(Ordering::SeqCst),1);v.free().await.unwrap();
}
#[tokio::test]
async fn free_failure_panic_cancel_and_concurrent_retry_confirm_real_release(){
    let p=probe(ResourceOperations::sql_xml(),false);let v=value(p.clone());p.xml.release.failures_remaining.store(1,Ordering::SeqCst);assert_eq!(v.free().await.unwrap_err().to_string(),fatal_error().to_string());p.free_started.notified().await;assert!(!v.is_freed());v.string().await.unwrap();p.panic_free.store(true,Ordering::SeqCst);let alias=v.clone();assert!(tokio::spawn(async move{alias.free().await}).await.unwrap_err().is_panic());assert!(!v.is_freed());assert!(v.set_string(&RdbcString::from("<x/>" )).await.is_err());v.free().await.unwrap();v.free().await.unwrap();assert_eq!(p.free_attempts.load(Ordering::SeqCst),3);assert!(v.is_freed());
    let p=probe(ResourceOperations::sql_xml(),true);let v=value(p.clone());let alias=v.clone();let task=tokio::spawn(async move{alias.free().await});p.xml.release.started.notified().await;task.abort();assert!(task.await.unwrap_err().is_cancelled());assert!(!v.is_freed());assert!(v.string().await.is_err());let alias=v.clone();let retry=tokio::spawn(async move{alias.free().await});p.xml.release.started.notified().await;let alias=v.clone();let concurrent=tokio::spawn(async move{alias.free().await});p.xml.release.finish.notify_one();retry.await.unwrap().unwrap();concurrent.await.unwrap().unwrap();assert!(v.is_freed());assert_eq!(p.free_attempts.load(Ordering::SeqCst),2);assert_eq!(p.xml.release.calls.load(Ordering::SeqCst),2);
}
#[tokio::test]
async fn actual_pooled_value_cancellation_parent_cleanup_and_fatal_error(){
    let control=Arc::new(Control::default());control.block_free.store(true,Ordering::SeqCst);let mut connection=pooled(control.clone()).await;let v=ResourceHandleFactory::sql_xml(connection.create_sql_xml().await.unwrap());let probe=control.xml.lock().unwrap()[0].clone();v.set_string(&RdbcString::from("<pool/>" )).await.unwrap();assert_eq!(v.get_string().await.unwrap(),RdbcString::from("<pool/>"));let alias=v.clone();let task=tokio::spawn(async move{alias.free().await});probe.release.started.notified().await;task.abort();assert!(task.await.unwrap_err().is_cancelled());assert!(!v.is_freed());let mut parent=tokio::spawn(async move{connection.close().await});assert!(tokio::time::timeout(Duration::from_millis(20),&mut parent).await.is_err());assert!(control.returns.lock().unwrap().is_empty());probe.release.finish.notify_one();parent.await.unwrap().unwrap();v.free().await.unwrap();assert!(v.is_freed());assert_eq!(probe.release.calls.load(Ordering::SeqCst),1);assert_eq!(control.returns.lock().unwrap().len(),1);
    let control=Arc::new(Control::default());let mut connection=pooled(control.clone()).await;connection.set_exception_sorter(Arc::new(CountingSorter(control.clone())));let v=ResourceHandleFactory::sql_xml(connection.create_sql_xml().await.unwrap());control.xml.lock().unwrap()[0].release.read_failure.store(true,Ordering::SeqCst);assert_eq!(v.get_string().await.unwrap_err().sql_exception().unwrap().error_code(),77);assert!(connection.is_closed().unwrap());assert_eq!(control.sorter_calls.load(Ordering::SeqCst),1);assert_eq!(control.discards.load(Ordering::SeqCst),1);v.free().await.unwrap();connection.close().await.unwrap();assert!(!control.returns.lock().unwrap()[0].is_reusable());
}
#[cfg(task7_require_xml_value_sync)]fn negative_sync(){fn require_sync<T:Sync>(){}require_sync::<Driver>();}
#[cfg(task7_require_old_xml_access)]use druid_rdbc::resource::{RdbcSqlXmlAccess,RdbcResourceAccess};

#[tokio::test]
async fn queued_read_rechecks_free_before_driver_entry(){
    let p=probe(ResourceOperations::sql_xml(),false);p.xml.write_block.store(true,Ordering::SeqCst);let v=value(p.clone());let alias=v.clone();let writing=tokio::spawn(async move{alias.set_string(&RdbcString::from("<held/>" )).await});p.xml.write_started.notified().await;
    let mut freeing=Box::pin(v.free());std::future::poll_fn(|cx|{assert!(std::future::Future::poll(freeing.as_mut(),cx).is_pending());std::task::Poll::Ready(())}).await;
    let mut reading=Box::pin(v.source(None));std::future::poll_fn(|cx|{assert!(std::future::Future::poll(reading.as_mut(),cx).is_pending());std::task::Poll::Ready(())}).await;
    p.xml.write_finish.notify_one();writing.await.unwrap().unwrap();freeing.await.unwrap();let error=reading.await.unwrap_err();assert_eq!(error.class_name(),"java.sql.SQLException");assert!(error.to_string().contains("SQLXML"));assert_eq!(p.xml.reads.load(Ordering::SeqCst),0);assert!(v.is_freed());assert_eq!(p.free_attempts.load(Ordering::SeqCst),1);
}

}

#[tokio::test]
async fn pooled_xml_cleanup_queued_behind_another_resource_reports_xml(){
    let control=Arc::new(Control::default());control.block_free.store(true,Ordering::SeqCst);
    let mut connection=pooled(control.clone()).await;
    let mut first=connection.create_sql_xml().await.unwrap();
    let mut second=connection.create_sql_xml().await.unwrap();
    let probes=control.xml.lock().unwrap().clone();assert_eq!(probes.len(),2);
    let first_free=tokio::spawn(async move{first.free().await});
    probes[0].release.started.notified().await;
    let mut queued=Box::pin(second.free());
    std::future::poll_fn(|cx|{assert!(std::future::Future::poll(queued.as_mut(),cx).is_pending());std::task::Poll::Ready(())}).await;
    drop(queued);
    assert_eq!(probes[1].release.calls.load(Ordering::SeqCst),0);
    let error=second.string().await.unwrap_err();
    assert_eq!(probes[1].reads.load(Ordering::SeqCst),0);
    probes[0].release.finish.notify_one();first_free.await.unwrap().unwrap();
    probes[1].release.started.notified().await;probes[1].release.finish.notify_one();
    second.free().await.unwrap();connection.close().await.unwrap();
    assert_eq!(probes[0].release.calls.load(Ordering::SeqCst),1);
    assert_eq!(probes[1].release.calls.load(Ordering::SeqCst),1);
    assert_eq!(control.returns.lock().unwrap().len(),1);
    assert_eq!(error.class_name(),"java.sql.SQLException");
    assert!(error.to_string().contains("SQLXML"),"{error}");
}
