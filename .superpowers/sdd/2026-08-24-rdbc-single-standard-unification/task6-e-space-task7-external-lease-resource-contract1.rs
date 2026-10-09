use druid_rdbc::connection::Connection;
use druid_rdbc::error::RdbcError;
use druid_rdbc::resource::{Array, Blob, Clob, NClob, SqlXml};
use druid_rdbc::types::{RdbcObject, Value};
use task7_external_lease::ExternalConnectionLease;
use std::cell::Cell;
use std::sync::{Arc,Mutex};
#[path="task7-connection-resource-blob-fixture1.rs"] mod blob_fixture;
#[path="task7-connection-resource-clob-fixture1.rs"] mod clob_fixture;
#[derive(Debug, Default)]
struct Calls { lob:Vec<&'static str>, xml:usize, arrays:Vec<(Option<String>,Option<Vec<RdbcObject>>)> }
#[derive(Debug)]
struct Driver { calls:Arc<Mutex<Calls>>, serial:Cell<usize> }
druid_rdbc::impl_standard_wrapper!(Driver => Connection);
#[async_trait::async_trait]
impl Connection for Driver {
 async fn create_blob(&mut self)->Result<Box<dyn Blob>,RdbcError> {
  self.serial.set(self.serial.get()+1);self.calls.lock().unwrap().lob.push("blob");Ok(blob_fixture::resource())
 }
 async fn create_clob(&mut self)->Result<Box<dyn Clob>,RdbcError> {
  self.calls.lock().unwrap().lob.push("clob");Ok(clob_fixture::resource())
 }
 async fn create_n_clob(&mut self)->Result<Box<dyn NClob>,RdbcError> {
  self.calls.lock().unwrap().lob.push("nclob");Ok(clob_fixture::national_resource())
 }
 async fn create_sql_xml(&mut self)->Result<Box<dyn SqlXml>,RdbcError> {
  self.calls.lock().unwrap().xml+=1;Err(RdbcError::UnsupportedOperation {operation:"driver_xml_sentinel"})
 }
 async fn create_array_of(&mut self,type_name:Option<&str>,elements:Option<Vec<RdbcObject>>)->Result<Box<dyn Array>,RdbcError> {
  self.calls.lock().unwrap().arrays.push((type_name.map(str::to_owned),elements));
  Err(RdbcError::UnsupportedOperation {operation:"driver_array_sentinel"})
 }
}
fn lease(calls:Arc<Mutex<Calls>>)->ExternalConnectionLease<Box<Box<dyn Connection>>> {
 let raw:Box<dyn Connection>=Box::new(Driver {calls,serial:Cell::new(0)});
 ExternalConnectionLease::new(Box::new(raw))
}
#[tokio::test]
async fn lease_contract_returns_driver_owned_lobs_without_parent_borrow() {
 let calls=Arc::new(Mutex::new(Calls::default()));
 let (mut blob,mut clob,mut nclob)={let mut connection=lease(calls.clone());
 (connection.create_blob().await.unwrap(),connection.create_clob().await.unwrap(),connection.create_n_clob().await.unwrap())};
 assert_eq!(blob.length().await.unwrap(),4);assert_eq!(clob.length().await.unwrap(),4);assert_eq!(nclob.length().await.unwrap(),4);
 blob.free().await.unwrap();clob.free().await.unwrap();nclob.free().await.unwrap();
 assert_eq!(calls.lock().unwrap().lob,vec!["blob","clob","nclob"]);
}
#[tokio::test]
async fn lease_contract_forwards_xml_array_requests_and_original_errors_exactly_once() {
 let calls=Arc::new(Mutex::new(Calls::default()));let mut connection=lease(calls.clone());
 assert!(matches!(connection.create_sql_xml().await,Err(RdbcError::UnsupportedOperation {operation:"driver_xml_sentinel"})));
 let elements=vec![RdbcObject::Integer(8),RdbcObject::Scalar(Value::Null),RdbcObject::String("x".into())];
 assert!(matches!(connection.create_array_of(Some("vendor.T"),Some(elements.clone())).await,Err(RdbcError::UnsupportedOperation {operation:"driver_array_sentinel"})));
 assert!(matches!(connection.create_array_of(None,None).await,Err(RdbcError::UnsupportedOperation {operation:"driver_array_sentinel"})));
 assert!(matches!(connection.create_array_of(Some(""),Some(Vec::new())).await,Err(RdbcError::UnsupportedOperation {operation:"driver_array_sentinel"})));
 let actual=calls.lock().unwrap();assert_eq!(actual.xml,1);
 assert_eq!(actual.arrays,vec![(Some("vendor.T".into()),Some(elements)),(None,None),(Some(String::new()),Some(Vec::new()))]);
}
