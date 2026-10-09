"""Preserve all 42 prior tests and add real Struct factory/lease consumers."""
import hashlib
import json
from pathlib import Path
import re
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
old=(base/'task7-pooled-array-consumer6.rs').read_text(encoding='utf-8')
text=old.replace('struct Control {','struct Control {\n    struct_args:Mutex<Vec<(Option<String>,Option<Vec<RdbcObject>>)>>,\n    structs:Mutex<Vec<Arc<StructProbe>>>,',1)
marker='impl Connection for DriverConnection {';assert text.count(marker)==1
method='''
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
'''
text=text.replace(marker,marker+method,1)
text+='''
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
'''
pattern=r'(?ms)^async fn (task7_\w+)\([^)]*\)\s*\{.*?^\}'
before={m[1]:m[0] for m in re.finditer(pattern,old)};after={m[1]:m[0] for m in re.finditer(pattern,text)}
assert len(before)==42 and len(after)==47 and all(after[k]==v for k,v in before.items())
dest=base/'task7-pooled-struct-consumer1.rs'
with dest.open('x',encoding='utf-8',newline='\n') as out:out.write(text)
receipt={'prior_tests_verbatim':42,'tests':47,'source_sha256':hashlib.sha256(dest.read_bytes()).hexdigest(),'new_tests':sorted(set(after)-set(before))}
with (base/'task7-struct-pooled-consumer1-build.summary.json').open('x',encoding='utf-8') as out:json.dump(receipt,out,indent=2)
print(json.dumps(receipt),flush=True)
