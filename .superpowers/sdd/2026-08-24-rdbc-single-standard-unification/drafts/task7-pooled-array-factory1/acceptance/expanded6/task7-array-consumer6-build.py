"""Extend actual driver fixtures and preserve the previous 36 test bodies."""
from pathlib import Path
import hashlib
import json
import re
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
source=base/'task7-pooled-array-consumer5.rs'
old=source.read_text(encoding='utf-8'); text=old
def replace(before,after):
    global text
    assert text.count(before)==1,(before,text.count(before))
    text=text.replace(before,after)
replace('struct Control {\n    arrays:', 'struct Control {\n    array_disabled:AtomicUsize,\n    arrays:')
replace('let probe=Arc::new(ArrayProbe { args:', 'let probe=Arc::new(ArrayProbe { supported:ResourceOperations::array() & !ResourceOperations::from_bits_retain(self.control.array_disabled.load(Ordering::SeqCst) as _), free_failures:AtomicUsize::new(0), free_fatal:AtomicBool::new(false), nonfatal_read:AtomicBool::new(false), args:')
replace('struct ArrayProbe {\n    args:', 'struct ArrayProbe {\n    supported:ResourceOperations,\n    free_failures:AtomicUsize,\n    free_fatal:AtomicBool,\n    nonfatal_read:AtomicBool,\n    args:')
replace('fn capabilities(&self) -> ResourceOperations { ResourceOperations::array() }','fn capabilities(&self) -> ResourceOperations { self.probe.supported }')
replace('self.probe.calls.lock().unwrap().push(name);\n        if self.probe.read_failure', '''self.probe.calls.lock().unwrap().push(name);
        let mut required=if name.starts_with("result_set") {ResourceOperations::RESULT_SET} else {ResourceOperations::READ};
        if name.contains("range") { required|=ResourceOperations::RANGE; }
        if name.contains("type_map") { required|=ResourceOperations::TYPE_MAP; }
        if !self.probe.supported.contains(required) {return Err(RdbcError::feature_not_supported(name));}
        if self.probe.nonfatal_read.load(Ordering::SeqCst) {return Err(array_nonfatal_error());}
        if self.probe.read_failure''')
replace('if self.probe.block { self.probe.finish.notified().await; }\n        self.freed = true;', '''if self.probe.block { self.probe.finish.notified().await; }
        if self.probe.free_failures.fetch_update(Ordering::SeqCst,Ordering::SeqCst,|n|n.checked_sub(1)).is_ok() {
            return Err(if self.probe.free_fatal.load(Ordering::SeqCst) {fatal_error()} else {array_nonfatal_error()});
        }
        self.freed = true;''')
text+='''
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
'''
pattern=r'(?ms)^async fn (task7_\w+)\([^)]*\)\s*\{.*?^\}'
before={m[1]:m[0] for m in re.finditer(pattern,old)}
after={m[1]:m[0] for m in re.finditer(pattern,text)}
assert len(before)==36 and len(after)==42,(len(before),len(after))
assert all(after[k]==v for k,v in before.items())
dest=base/'task7-pooled-array-consumer6.rs'
with dest.open('x',encoding='utf-8',newline='\n') as out:out.write(text)
runner=(base/'task7-pooled-array-contract5-run.py').read_text(encoding='utf-8').replace('consumer5.rs','consumer6.rs').replace('contract5.summary','contract6.summary').replace('before5','before6').replace('after5','after6')
with (base/'task7-pooled-array-contract6-run.py').open('x',encoding='utf-8',newline='\n') as out:out.write(runner)
receipt={'old_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'source_sha256':hashlib.sha256(dest.read_bytes()).hexdigest(),'prior_tests_verbatim':36,'tests':42,'new_tests':sorted(set(after)-set(before)),'production_applied':False,'candidate_changed':False}
with (base/'task7-array-consumer6-build.summary.json').open('x',encoding='utf-8') as out:json.dump(receipt,out,indent=2)
print(json.dumps(receipt),flush=True)
