use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{RdbcResourceAccess,RdbcSqlXmlAccess,ResourceHandleFactory,ResourceOperations};
use druid_rdbc::types::RdbcString;
use std::sync::{Arc,Mutex};
use std::sync::atomic::{AtomicUsize,Ordering};

/// SQLXML 驱动委托观测夹具；不自行实施标准的一次性读写门禁。
#[derive(Debug)]
struct XmlAccess {
    value: Mutex<RdbcString>,
    reads: AtomicUsize,
    writes: AtomicUsize,
    releases: AtomicUsize,
}
#[async_trait::async_trait]
impl RdbcResourceAccess for XmlAccess {
    fn capabilities(&self)->ResourceOperations {
        ResourceOperations::READ | ResourceOperations::WRITE | ResourceOperations::FREE
    }
    async fn free(&self)->Result<(),RdbcError> {
        self.releases.fetch_add(1,Ordering::SeqCst); Ok(())
    }
}
#[async_trait::async_trait]
impl RdbcSqlXmlAccess for XmlAccess {
    async fn string(&self)->Result<RdbcString,RdbcError> {
        self.reads.fetch_add(1,Ordering::SeqCst);
        Ok(self.value.lock().expect("value lock").clone())
    }
    async fn set_string(&self,value:&RdbcString)->Result<(),RdbcError> {
        self.writes.fetch_add(1,Ordering::SeqCst);
        *self.value.lock().expect("value lock")=value.clone(); Ok(())
    }
}
fn fixture()->(druid_rdbc::resource::RdbcSqlXml,Arc<XmlAccess>) {
    let access=Arc::new(XmlAccess {value:Mutex::new(RdbcString::from_utf16(vec![60,120,47,62])),reads:AtomicUsize::new(0),writes:AtomicUsize::new(0),releases:AtomicUsize::new(0)});
    (ResourceHandleFactory::sql_xml(access.clone()),access)
}
#[tokio::test]
async fn sqlxml_second_string_read_is_rejected() {
    let (xml,access)=fixture();
    assert_eq!(xml.string().await.expect("first read").as_utf16(),[60,120,47,62]);
    let second=xml.string().await;
    eprintln!("second_read_ok={} delegated_reads={}",second.is_ok(),access.reads.load(Ordering::SeqCst));
    assert!(second.is_err(),"SQLXML#getString consumes readable state");
    assert_eq!(access.reads.load(Ordering::SeqCst),1);
}
#[tokio::test]
async fn sqlxml_clone_cannot_read_consumed_value() {
    let (xml,access)=fixture();let alias=xml.clone();
    xml.string().await.expect("first read");
    let second=alias.get_string().await;
    eprintln!("clone_second_read_ok={} delegated_reads={}",second.is_ok(),access.reads.load(Ordering::SeqCst));
    assert!(second.is_err(),"alias shares consumed SQLXML readable state");
    assert_eq!(access.reads.load(Ordering::SeqCst),1);
}
#[tokio::test]
async fn sqlxml_second_string_write_is_rejected() {
    let (xml,access)=fixture();let alias=xml.clone();
    let first=RdbcString::from_utf16(vec![60,97,47,62]);
    let second_value=RdbcString::from_utf16(vec![60,98,47,62]);
    xml.set_string(&first).await.expect("first write");
    let second=alias.set_string(&second_value).await;
    eprintln!("second_write_ok={} delegated_writes={}",second.is_ok(),access.writes.load(Ordering::SeqCst));
    assert!(second.is_err(),"SQLXML#setString consumes writable state shared by clones");
    assert_eq!(access.writes.load(Ordering::SeqCst),1);
    assert_eq!(*access.value.lock().expect("value lock"),first);
}
#[tokio::test]
async fn sqlxml_free_rejects_read_write_and_releases_once_control() {
    let (xml,access)=fixture();let alias=xml.clone();
    xml.free().await.expect("free");alias.free().await.expect("repeat free");
    assert!(alias.string().await.is_err());
    assert!(alias.set_string(&RdbcString::from_utf16(vec![60,120,47,62])).await.is_err());
    assert_eq!(access.reads.load(Ordering::SeqCst),0);
    assert_eq!(access.writes.load(Ordering::SeqCst),0);
    assert_eq!(access.releases.load(Ordering::SeqCst),1);
}
