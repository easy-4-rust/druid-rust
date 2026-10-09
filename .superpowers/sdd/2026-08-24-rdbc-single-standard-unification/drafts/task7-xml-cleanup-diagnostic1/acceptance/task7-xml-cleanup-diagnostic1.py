"""Reproduce wrong XML close diagnostic through actual pooled coordinator queue."""
import hashlib,json,os,shutil,subprocess
from pathlib import Path
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
old=base/'task7-xml-value-druid-candidate1';dest=base/'task7-xml-value-druid-candidate2'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
normal=json.loads((old/'summary.json').read_text(encoding='utf-8'))
expanded=json.loads((old/'expanded.summary.json').read_text(encoding='utf-8'))
assert normal['terminal'] and expanded['terminal']
dest.mkdir();shutil.copytree(old/'src',dest/'src');shutil.copytree(old/'resources',dest/'resources')
state={'terminal':False,'production_applied':False,'commands':[]}
def run(name,command,env=None):
    log=dest/(name+'.log')
    with log.open('xb')as out:result=subprocess.run(command,env=env,stdout=out,stderr=subprocess.STDOUT,creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    state['commands'].append({'name':name,'command':command,'exit_code':result.returncode,'log':str(log),'log_sha256':sha(log)})
    (dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8');print(name,result.returncode,flush=True);return result.returncode
prior=base/'task7-pooled-xml-value-consumer2.rs';source=base/'task7-pooled-xml-value-consumer3.rs'
extra='''
#[tokio::test]
async fn pooled_xml_cleanup_queued_behind_another_resource_reports_xml(){
    let control=Arc::new(Control::default());control.block_free.store(true,Ordering::SeqCst);
    let mut connection=pooled(control.clone()).await;
    let mut first=connection.create_sql_xml().await.unwrap();
    let mut second=connection.create_sql_xml().await.unwrap();
    let probes=control.xml.lock().unwrap().clone();assert_eq!(probes.len(),2);
    let first_free=tokio::spawn(async move{first.free().await});
    probes[0].release.started.notified().await;
    let mut queued=Box::pin(second.free());
    std::future::poll_fn(|cx|{assert!(std::future::Future::poll(queued.as_mut(),cx).is_pending());std::task::Poll::Ready(())}).await;
    drop(queued);
    assert_eq!(probes[1].release.calls.load(Ordering::SeqCst),0);
    let error=second.string().await.unwrap_err();
    assert_eq!(probes[1].reads.load(Ordering::SeqCst),0);
    probes[0].release.finish.notify_one();first_free.await.unwrap().unwrap();
    probes[1].release.started.notified().await;probes[1].release.finish.notify_one();
    second.free().await.unwrap();connection.close().await.unwrap();
    assert_eq!(probes[0].release.calls.load(Ordering::SeqCst),1);
    assert_eq!(probes[1].release.calls.load(Ordering::SeqCst),1);
    assert_eq!(control.returns.lock().unwrap().len(),1);
    assert_eq!(error.class_name(),"java.sql.SQLException");
    assert!(error.to_string().contains("SQLXML"),"{error}");
}
'''
source.write_bytes(prior.read_bytes()+extra.encode());state['consumer_sha256']=sha(source)
consumer=next(row['command']for row in expanded['commands']if row['name']=='consumer2-compile').copy()
consumer[consumer.index(str(prior))]=str(source);consumer[consumer.index('-o')+1]=str(dest/'consumer-red.exe')
assert run('consumer-red-compile',consumer)==0
test='pooled_xml_cleanup_queued_behind_another_resource_reports_xml'
assert run('diagnostic-red',[str(dest/'consumer-red.exe'),test,'--exact','--test-threads=1'])!=0
log=(dest/'diagnostic-red.log').read_text(encoding='utf-8');assert 'Blob'in log and '0 passed; 1 failed;'in log
p=dest/'src/core/druid_sql_xml.rs';payload=p.read_bytes();needle=b'rdbc_resource_closed("Blob")';assert payload.count(needle)==1
state['old_source_sha256']=sha(p);(dest/'druid_sql_xml.before.rs').write_bytes(payload)
p.write_bytes(payload.replace(needle,b'rdbc_resource_closed("SQLXML")'));state['new_source_sha256']=sha(p)
changed=[p.relative_to(dest).as_posix()for p in(dest/'src').rglob('*.rs')if sha(p)!=sha(old/p.relative_to(dest))];assert changed==['src/core/druid_sql_xml.rs'];state['changed_sources']=changed
library=next(row['command']for row in normal['commands']if row['name']=='full-druid-compile').copy()
library[library.index(str(old/'src/lib.rs'))]=str(dest/'src/lib.rs');library[library.index('-o')+1]=str(dest/'libdruid.rlib')
for i,arg in enumerate(library):
    if arg.startswith('metadata='):library[i]='metadata=task7_xml_cleanup_diagnostic1'
env=os.environ.copy();env.update(normal['druid_environment']);env['CARGO_MANIFEST_DIR']=str(dest);state['druid_environment']={key:env[key]for key in normal['druid_environment']}
assert run('full-druid-compile',library,env)==0
consumer[consumer.index('-o')+1]=str(dest/'consumer3.exe')
for i,arg in enumerate(consumer):
    if arg.startswith('druid='):consumer[i]='druid='+str(dest/'libdruid.rlib')
assert run('consumer3-compile',consumer)==0
assert run('consumer3-runtime',[str(dest/'consumer3.exe'),'--test-threads=1'])==0
assert '104 passed; 0 failed;'in(dest/'consumer3-runtime.log').read_text(encoding='utf-8')
state['sources']={p.relative_to(dest).as_posix():sha(p)for p in(dest/'src').rglob('*.rs')}
state['rlib_sha256']=sha(dest/'libdruid.rlib');state['exe_sha256']=sha(dest/'consumer3.exe');state['terminal']=True
(dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
