"""Correct only the metadata failure premise; preserve all prior assertions."""
from pathlib import Path
import hashlib
import json
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
old=base/'task7-pooled-array-consumer4.rs'
text=old.read_text(encoding='utf-8')
start=text.index('async fn pooled(control:Arc<Control>)')
end=text.index('\n#[tokio::test]',start)
helper=text[start:end]
filtered=helper.replace('async fn pooled(', 'async fn pooled_with_array_filter(',1).replace('"blob-contract".into(),None,','"blob-contract".into(),Some(Arc::new(druid::core::FilterChain::new())),',1)
assert filtered!=helper and 'Some(Arc::new(druid::core::FilterChain::new()))' in filtered
text=text[:end]+'\n'+filtered+text[end:]
marker='async fn task7_array_cursor_open_failure_does_not_deadlock_parent_cleanup() {'
begin=text.index(marker)
end=text.index('\n#[tokio::test]',begin)
body=text[begin:end]
assert body.count('pooled(control.clone())')==1
text=text[:begin]+body.replace('pooled(control.clone())','pooled_with_array_filter(control.clone())',1)+text[end:]
text+='''
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
'''
dest=base/'task7-pooled-array-consumer5.rs'
with dest.open('x',encoding='utf-8',newline='\n') as out: out.write(text)
runner=(base/'task7-pooled-array-contract4-run.py').read_text(encoding='utf-8').replace('consumer4.rs','consumer5.rs').replace('contract4.summary','contract5.summary').replace('before4','before5').replace('after4','after5')
with (base/'task7-pooled-array-contract5-run.py').open('x',encoding='utf-8',newline='\n') as out: out.write(runner)
receipt={'old_sha256':hashlib.sha256(old.read_bytes()).hexdigest(),'source_sha256':hashlib.sha256(dest.read_bytes()).hexdigest(),'prior_tests_preserved':35,'additional_lazy_metadata_control':1,'candidate_source_changed':False,'production_applied':False,'prior_failure':'invalid premise: no Filter chain means constructor intentionally does not read metadata'}
with (base/'task7-array-consumer5-build.summary.json').open('x',encoding='utf-8') as out: json.dump(receipt,out,indent=2)
print(json.dumps(receipt),flush=True)
