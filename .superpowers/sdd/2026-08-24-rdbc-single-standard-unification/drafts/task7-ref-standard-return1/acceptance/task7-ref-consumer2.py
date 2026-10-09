"""Preserve the original 47-test source verbatim; strengthen raw Ref error to fatal and add parameter/nested-resource contracts."""
import hashlib,json,subprocess
from pathlib import Path
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008');dest=base/'task7-ref-druid-candidate1'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
contract=(base/'task7-ref-contract1.rs').read_text(encoding='utf-8')
contract=contract.replace('    fail: AtomicBool,','    fail: AtomicBool,\n    fatal: AtomicBool,')
contract=contract.replace('if self.probe.fail.load(Ordering::SeqCst) {Err(failure())}', 'if self.probe.fail.load(Ordering::SeqCst) {Err(if self.probe.fatal.load(Ordering::SeqCst) {RdbcError::SqlException(Box::new(SqlException::driver(96,"fatal ref driver failure").with_sql_state("08006")))} else {failure()})}')
flow=(base/'task7-ref-pooled-flow1.rs').read_text(encoding='utf-8')
flow=flow.replace('probe.fail.store(true,Ordering::SeqCst);assert_eq!(result.object().await.unwrap_err(),failure());','probe.fatal.store(true,Ordering::SeqCst);probe.fail.store(true,Ordering::SeqCst);let error=result.object().await.unwrap_err();assert_eq!(error.sql_exception().unwrap().sql_state(),Some("08006"));assert_eq!(error.sql_exception().unwrap().error_code(),96);')
extra='''
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
'''
original=(base/'task7-pooled-struct-consumer1.rs').read_bytes();source=base/'task7-pooled-ref-consumer2.rs';source.write_bytes(original+b'\nmod ref_contract {\n'+contract.encode()+b'\n'+flow.encode()+b'\n'+extra.encode()+b'\n}\n')
record=json.loads((dest/'summary.json').read_text(encoding='utf-8'))
command=next(r['command'] for r in record['commands'] if r['name']=='consumer-compile').copy();command[command.index(str(base/'task7-pooled-ref-consumer1.rs'))]=str(source);command[command.index('-o')+1]=str(dest/'consumer2.exe')
state={'production_applied':False,'source_sha256':sha(source),'prior_47_source_sha256':sha(base/'task7-pooled-struct-consumer1.rs'),'commands':[]}
def run(name,args):
    log=dest/(name+'.log')
    with log.open('xb') as out:result=subprocess.run(args,stdout=out,stderr=subprocess.STDOUT,creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    state['commands'].append({'name':name,'command':args,'exit_code':result.returncode,'log':str(log),'log_sha256':sha(log)})
    (dest/'contract2.summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8');print(name,result.returncode,flush=True);return result.returncode
assert run('consumer2-compile',command)==0
assert run('consumer2-runtime',[str(dest/'consumer2.exe'),'--test-threads=1'])==0
state['rlib_sha256']=sha(dest/'libdruid.rlib');state['rdbc_rlib_sha256']=sha(base/'task7-ref-rdbc-candidate1/libdruid_rdbc.rlib');state['exe_sha256']=sha(dest/'consumer2.exe');state['terminal']=True
(dest/'contract2.summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
