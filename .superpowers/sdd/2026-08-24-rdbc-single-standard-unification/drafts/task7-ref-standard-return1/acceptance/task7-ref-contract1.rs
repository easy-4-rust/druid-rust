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
        if self.probe.fail.load(Ordering::SeqCst) {Err(failure())} else {Ok(())}
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
