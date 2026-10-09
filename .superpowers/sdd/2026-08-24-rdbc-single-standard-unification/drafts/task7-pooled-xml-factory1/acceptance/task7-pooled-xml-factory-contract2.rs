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
    async fn position_clob(&mut self, pattern:&mut dyn Clob, start:i64) -> Result<Option<i64>,RdbcError> {
        self.ensure_open()?;let count=i32::try_from(pattern.length().await?).map_err(|_|RdbcError::InvalidArgument("pattern too long".into()))?;
        let value=pattern.get_sub_string(1,count).await?;self.position_string(&value,start).await
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
        assert_eq!(resource.position_clob(&mut *pattern,1).await.unwrap(),Some(1));
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
            resource.position_string(&text,1).await.err().unwrap(),resource.position_clob(&mut *pattern,1).await.err().unwrap(),resource.set_string(1,&text).await.err().unwrap(),resource.set_string_range(1,&text,0,1).await.err().unwrap(),
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
