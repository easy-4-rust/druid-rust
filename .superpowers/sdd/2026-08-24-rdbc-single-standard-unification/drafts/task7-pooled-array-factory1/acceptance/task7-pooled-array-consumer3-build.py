"""Add explicit Array-child free and concurrent admission rejection contracts."""
from pathlib import Path
import hashlib
import json
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
source=(base/'task7-pooled-array-consumer2.rs').read_text(encoding='utf-8')
source=source.replace('    bad_metadata: AtomicBool,','    bad_metadata: AtomicBool,\n    block_cursor:AtomicBool,\n    cursor_started:tokio::sync::Notify,\n    cursor_finish:tokio::sync::Notify,',1)
source=source.replace('bad_metadata:AtomicBool::new(false), cursor_next_failure:',
    'bad_metadata:AtomicBool::new(false), block_cursor:AtomicBool::new(false), cursor_started:tokio::sync::Notify::new(), cursor_finish:tokio::sync::Notify::new(), cursor_next_failure:',1)
needle='''        self.check("result_set")?; Ok(self.cursor(self.all()))'''
assert source.count(needle)==1
source=source.replace(needle,'''        self.check("result_set")?;
        self.probe.cursor_started.notify_one();
        if self.probe.block_cursor.load(Ordering::SeqCst) { self.probe.cursor_finish.notified().await; }
        Ok(self.cursor(self.all()))''')
source+='''
#[tokio::test]
async fn task7_array_explicit_free_closes_child_before_connection_return() {
    let control=Arc::new(Control::default()); let mut connection=pooled(control.clone()).await;
    let mut array=connection.create_array_of(None,Some(array_values())).await.unwrap(); let probe=array_probe(&control);
    let mut cursor=array.result_set().await.unwrap(); assert!(cursor.next().await.unwrap());
    array.free().await.unwrap(); assert!(control.returns.lock().unwrap().is_empty());
    assert_eq!(probe.cursors.lock().unwrap()[0].closes.load(Ordering::SeqCst),1);
    assert!(cursor.next().await.is_err()); cursor.close().await.unwrap();
    assert!(array.values().await.is_err()); array.free().await.unwrap(); connection.close().await.unwrap();
    assert_eq!(probe.frees.load(Ordering::SeqCst),1); assert_eq!(probe.cursors.lock().unwrap()[0].closes.load(Ordering::SeqCst),1);
}
#[tokio::test]
async fn task7_array_parent_close_during_cursor_creation_drains_rejected_admission() {
    let control=Arc::new(Control::default()); let mut connection=pooled(control.clone()).await;
    let mut array=connection.create_array_of(None,Some(array_values())).await.unwrap(); let probe=array_probe(&control);
    probe.block_cursor.store(true,Ordering::SeqCst); let mut open=Box::pin(array.result_set());
    tokio::select! { _=probe.cursor_started.notified()=>{}, result=&mut open=>panic!("cursor should block: {result:?}") }
    let mut parent=Box::pin(connection.close());
    assert!(tokio::time::timeout(Duration::from_millis(30),&mut parent).await.is_err());
    assert!(control.returns.lock().unwrap().is_empty()); probe.cursor_finish.notify_one();
    let result=tokio::time::timeout(Duration::from_secs(2),&mut open).await.expect("rejected admission must drain");
    assert!(result.is_err()); drop(open);
    tokio::time::timeout(Duration::from_secs(2),&mut parent).await.expect("parent must finish").unwrap(); drop(parent);
    assert_eq!(probe.frees.load(Ordering::SeqCst),1); assert_eq!(probe.cursors.lock().unwrap()[0].closes.load(Ordering::SeqCst),1);
    assert_eq!(control.returns.lock().unwrap().len(),1);
}
'''
destination=base/'task7-pooled-array-consumer3.rs'
with destination.open('x',encoding='utf-8',newline='\n') as output: output.write(source)
print(json.dumps({'sha256':hashlib.sha256(destination.read_bytes()).hexdigest(),'tests':35,'production_applied':False}))
