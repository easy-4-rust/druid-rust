"""Extend all unchanged 28 LOB/XML test bodies with real Array-driver contracts."""
from pathlib import Path
import hashlib
import json

base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
source=base/'task7-pooled-xml-factory-contract2.rs'
original=source.read_text(encoding='utf-8')
payload=original.replace('struct Control {', 'struct Control {\n    arrays:Mutex<Vec<Arc<ArrayProbe>>>,',1)
method='''
    async fn create_array_of(&mut self, type_name:Option<&str>, elements:Option<Vec<RdbcObject>>) -> Result<Box<dyn Array>,RdbcError> {
        if self.closed {return Err(RdbcError::ConnectionDiscarded);}
        self.control.created.fetch_add(1,Ordering::SeqCst);
        self.control.factory_calls.lock().unwrap().push("create_array_of");
        if self.control.fail_create.load(Ordering::SeqCst) {return Err(fatal_error());}
        let probe=Arc::new(ArrayProbe { args:(type_name.map(str::to_owned),elements),
            maps:Mutex::new(vec![]), calls:Mutex::new(vec![]), frees:AtomicUsize::new(0),
            started:tokio::sync::Notify::new(), finish:tokio::sync::Notify::new(),
            block:self.control.block_free.load(Ordering::SeqCst), read_failure:AtomicBool::new(false),
            bad_metadata:AtomicBool::new(false), cursor_next_failure:AtomicBool::new(false), cursors:Mutex::new(vec![]) });
        self.control.arrays.lock().unwrap().push(probe.clone());
        Ok(Box::new(ArrayDriver {probe,freed:false,not_sync:std::cell::Cell::new(0)}))
    }
'''
needle='impl Connection for DriverConnection {'
assert payload.count(needle)==1
payload=payload.replace(needle,needle+method,1)
fixture=(base/'task7-pooled-array-fixture1.rs').read_text(encoding='utf-8')
needle='async fn task7_array_cursor_driver_fatal_error_discards_actual_owner_once() {\n    let control=Arc::new(Control::default()); let mut connection=pooled(control.clone()).await;'
assert fixture.count(needle)==1
fixture=fixture.replace(needle,needle+'\n    connection.set_exception_sorter(Arc::new(CountingSorter(control.clone())));',1)
destination=base/'task7-pooled-array-consumer1.rs'
with destination.open('x',encoding='utf-8',newline='\n') as output:
    output.write(payload+'\n'+fixture)
# The original consumer suffix from its first test through EOF is identical;
# only Control and DriverConnection fixture setup above that boundary changed.
boundary='#[tokio::test]\nasync fn task7_pooled_blob_factory_parent_close_and_distinct_resources()'
assert original[original.index(boundary):] == payload[payload.index(boundary):]
summary={'original_consumer_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),
         'consumer_sha256':hashlib.sha256(destination.read_bytes()).hexdigest(),
         'original_28_test_body_suffix_unchanged':True,'array_contract_count':5,
         'production_applied':False}
(base/'task7-pooled-array-consumer1.source.summary.json').write_text(json.dumps(summary,indent=2)+'\n',encoding='utf-8')
print(summary)
