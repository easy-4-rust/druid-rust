use druid_rdbc::connection::Connection;
use druid_rdbc::error::{RdbcError,SqlException};
use druid_rdbc::resource::{Struct,ResourceOperations};
use druid_rdbc::types::{RdbcObject,RdbcTypeMap,RdbcTargetType,Value};
use std::cell::Cell;
use std::sync::{Arc,Mutex,atomic::{AtomicBool,Ordering}};

#[derive(Debug,Default)]
struct Probe {
    factory:Mutex<Vec<(Option<String>,Option<Vec<RdbcObject>>)>>,
    maps:Mutex<Vec<RdbcTypeMap>>,
    fail:AtomicBool,
}
fn failure() -> RdbcError {RdbcError::SqlException(Box::new(SqlException::driver(93,"struct driver error").with_sql_state("22000")))}
#[derive(Debug)]
struct DriverStruct {name:String,attributes:Vec<RdbcObject>,probe:Arc<Probe>,calls:Cell<usize>}
#[async_trait::async_trait]
impl Struct for DriverStruct {
    fn capabilities(&self)->ResourceOperations {ResourceOperations::READ|ResourceOperations::TYPE_MAP}
    async fn sql_type_name(&mut self)->Result<String,RdbcError> {
        self.calls.set(self.calls.get()+1);if self.probe.fail.load(Ordering::SeqCst) {return Err(failure());}Ok(self.name.clone())
    }
    async fn attributes(&mut self)->Result<Vec<RdbcObject>,RdbcError> {
        self.calls.set(self.calls.get()+1);if self.probe.fail.load(Ordering::SeqCst) {return Err(failure());}Ok(self.attributes.clone())
    }
    async fn attributes_with_type_map(&mut self,map:&RdbcTypeMap)->Result<Vec<RdbcObject>,RdbcError> {
        self.calls.set(self.calls.get()+1);self.probe.maps.lock().unwrap().push(map.clone());
        if self.probe.fail.load(Ordering::SeqCst) {return Err(failure());}
        let mut result=self.attributes.clone();
        if map.get("vendor.Type").is_some() && !result.is_empty() {result[0]=RdbcObject::String("explicit mapping".into());}
        Ok(result)
    }
}
#[derive(Debug)]
struct DriverConnection {probe:Arc<Probe>,closed:bool}
druid_rdbc::impl_standard_wrapper!(DriverConnection => Connection);
#[async_trait::async_trait]
impl Connection for DriverConnection {
    async fn create_struct(&mut self,name:Option<&str>,attributes:Option<Vec<RdbcObject>>)->Result<Box<dyn Struct>,RdbcError> {
        if self.closed {return Err(RdbcError::ConnectionDiscarded);}
        self.probe.factory.lock().unwrap().push((name.map(str::to_owned),attributes.clone()));
        let name=name.ok_or_else(||RdbcError::SqlException(Box::new(SqlException::driver(94,"null struct name").with_sql_state("HY009"))))?;
        if self.probe.fail.load(Ordering::SeqCst) {return Err(failure());}
        Ok(Box::new(DriverStruct {name:name.into(),attributes:attributes.unwrap_or_default(),probe:self.probe.clone(),calls:Cell::new(0)}))
    }
    fn is_closed(&self)->Result<bool,RdbcError> {Ok(self.closed)}
    async fn close(&mut self)->Result<(),RdbcError> {self.closed=true;Ok(())}
}
#[derive(Debug)]
struct DefaultConnection;
druid_rdbc::impl_standard_wrapper!(DefaultConnection => Connection);
#[async_trait::async_trait]
impl Connection for DefaultConnection {}
fn send_static<T:Send+'static>() {}
fn send_future<T:std::future::Future+Send>(future:T)->T {future}
fn values()->Vec<RdbcObject> {vec![Value::Int(7).into(),Value::Null.into(),RdbcObject::Bytes(vec![0,255])]}
fn map()->RdbcTypeMap {let mut map=RdbcTypeMap::new();map.insert("vendor.Type",RdbcTargetType::String);map}
#[tokio::test]
async fn task7_struct_owned_three_methods_preserve_order_and_explicit_map() {
    send_static::<DriverStruct>();let probe=Arc::new(Probe::default());let mut connection=DriverConnection {probe:probe.clone(),closed:false};
    let mut value=connection.create_struct(Some("schema.vendor.Type"),Some(values())).await.unwrap();
    assert_eq!(send_future(value.sql_type_name()).await.unwrap(),"schema.vendor.Type");assert_eq!(value.attributes().await.unwrap(),values());
    let map=map();let mapped=value.attributes_with_type_map(&map).await.unwrap();assert_eq!(mapped[0],RdbcObject::String("explicit mapping".into()));assert_eq!(mapped[1..],values()[1..]);
    assert_eq!(probe.maps.lock().unwrap().as_slice(),&[map]);assert_eq!(value.attributes().await.unwrap(),values());
    assert_eq!(value.capabilities(),ResourceOperations::READ|ResourceOperations::TYPE_MAP);
    connection.close().await.unwrap();drop(connection);
    assert_eq!(value.sql_type_name().await.unwrap(),"schema.vendor.Type");assert_eq!(value.attributes().await.unwrap(),values());
}
#[tokio::test]
async fn task7_struct_factory_preserves_nullable_attributes_and_name_error() {
    let probe=Arc::new(Probe::default());let mut connection=DriverConnection {probe:probe.clone(),closed:false};
    let mut absent=connection.create_struct(Some("vendor.Empty"),None).await.unwrap();assert!(absent.attributes().await.unwrap().is_empty());
    let mut empty=connection.create_struct(Some("vendor.Empty"),Some(vec![])).await.unwrap();assert!(empty.attributes().await.unwrap().is_empty());
    let error=connection.create_struct(None,None).await.unwrap_err();assert_eq!(error.sql_exception().unwrap().error_code(),94);assert_eq!(error.sql_exception().unwrap().sql_state(),Some("HY009"));
    let args=probe.factory.lock().unwrap();assert_eq!(args[0],(Some("vendor.Empty".into()),None));assert_eq!(args[1],(Some("vendor.Empty".into()),Some(vec![])));assert_eq!(args[2],(None,None));
}
#[tokio::test]
async fn task7_struct_driver_errors_remain_typed_for_all_methods() {
    let probe=Arc::new(Probe::default());let mut connection=DriverConnection {probe:probe.clone(),closed:false};let mut value=connection.create_struct(Some("vendor.Type"),Some(values())).await.unwrap();probe.fail.store(true,Ordering::SeqCst);
    for error in [value.sql_type_name().await.unwrap_err(),value.attributes().await.unwrap_err(),value.attributes_with_type_map(&map()).await.unwrap_err()] {assert_eq!(error,failure());}
    assert_eq!(connection.create_struct(Some("vendor.Type"),None).await.unwrap_err(),failure());
}
#[tokio::test]
async fn task7_struct_default_factory_is_explicitly_unsupported() {
    let mut connection=DefaultConnection;let error=connection.create_struct(Some("vendor.Type"),None).await.unwrap_err();assert_eq!(error.sql_exception().unwrap().sql_state(),Some("0A000"));
}
