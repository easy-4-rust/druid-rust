"""Exercise XML lock handoff and full-source counterexamples against the same consumer."""
import hashlib,json,os,shutil,subprocess
from pathlib import Path
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008');rdbc=base/'task7-xml-value-rdbc-candidate1';dest=base/'task7-xml-value-druid-candidate1';sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
original=json.loads((dest/'summary.json').read_text(encoding='utf-8'));assert original['terminal'];state={'terminal':False,'production_applied':False,'commands':[]}
def run(name,command,env=None):
    log=dest/(name+'.log')
    with log.open('xb')as out:result=subprocess.run(command,env=env,stdout=out,stderr=subprocess.STDOUT,creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    state['commands'].append({'name':name,'command':command,'exit_code':result.returncode,'log':str(log),'log_sha256':sha(log)});(dest/'expanded.summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8');print(name,result.returncode,flush=True);return result.returncode
prior=base/'task7-pooled-xml-value-consumer1.rs';source=base/'task7-pooled-xml-value-consumer2.rs';text=prior.read_text(encoding='utf-8');assert text.rstrip().endswith('}')
extra='''
#[tokio::test]
async fn queued_read_rechecks_free_before_driver_entry(){
    let p=probe(ResourceOperations::sql_xml(),false);p.xml.write_block.store(true,Ordering::SeqCst);let v=value(p.clone());let alias=v.clone();let writing=tokio::spawn(async move{alias.set_string(&RdbcString::from("<held/>" )).await});p.xml.write_started.notified().await;
    let mut freeing=Box::pin(v.free());std::future::poll_fn(|cx|{assert!(std::future::Future::poll(freeing.as_mut(),cx).is_pending());std::task::Poll::Ready(())}).await;
    let mut reading=Box::pin(v.source(None));std::future::poll_fn(|cx|{assert!(std::future::Future::poll(reading.as_mut(),cx).is_pending());std::task::Poll::Ready(())}).await;
    p.xml.write_finish.notify_one();writing.await.unwrap().unwrap();freeing.await.unwrap();let error=reading.await.unwrap_err();assert_eq!(error.class_name(),"java.sql.SQLException");assert!(error.to_string().contains("SQLXML"));assert_eq!(p.xml.reads.load(Ordering::SeqCst),0);assert!(v.is_freed());assert_eq!(p.free_attempts.load(Ordering::SeqCst),1);
}
''';source.write_text(text.rstrip()[:-1]+extra+'\n}\n',encoding='utf-8',newline='\n')
consumer=next(row['command']for row in original['commands']if row['name']=='consumer-compile').copy();consumer[consumer.index(str(prior))]=str(source);consumer[consumer.index('-o')+1]=str(dest/'consumer2.exe');assert run('consumer2-compile',consumer)==0;assert run('consumer2-runtime',[str(dest/'consumer2.exe'),'--test-threads=1'])==0
for name,cfg,code in [('negative-sync','task7_require_xml_value_sync','E0277'),('negative-old-access','task7_require_old_xml_access','E0432')]:
    command=consumer.copy();command[command.index('-o')+1]=str(dest/(name+'.exe'));command+=['--cfg',cfg];assert run(name,command)!=0;assert code in(dest/(name+'.log')).read_text(encoding='utf-8')
for variant,test in [('capability','every_capability_combination_and_freed_precedence'),('one-shot','aliases_share_all_read_and_write_consumption_gates'),('late-consumption','cancelled_entered_read_or_write_consumes_only_its_direction')]:
    root=base/('task7-xml-value-'+variant+'-mutation1');root.mkdir();rr=root/'rdbc';dd=root/'druid';shutil.copytree(rdbc/'src',rr/'src');shutil.copytree(dest/'src',dd/'src');shutil.copytree(dest/'resources',dd/'resources')
    relative='src/resource/sql_xml_value.rs'if variant=='one-shot'else'src/resource/rdbc_sql_xml.rs';p=rr/relative;text=p.read_text(encoding='utf-8')
    if variant=='one-shot':needle='*available = false;';assert text.count(needle)==1;text=text.replace(needle,'*available = true;')
    elif variant=='capability':needle='self.payload(true, ResourceOperations::READ, "SQLXML#getString")';assert text.count(needle)==1;text=text.replace(needle,'self.payload(true, ResourceOperations::empty(), "SQLXML#getString")')
    else:
        start=text.index('    pub async fn string(&self)');end=text.index('    /// 设置参数value',start);text=text[:start]+'''    pub async fn string(&self)->Result<RdbcString,RdbcError>{
        let mut payload=self.value.lock(ResourceOperations::READ,"SQLXML#getString").await?;
        let result=payload.driver.string().await;
        payload.admit(true)?;
        result
    }
'''+text[end:]
    p.write_text(text,encoding='utf-8',newline='\n')
    library=next(row['command']for row in original['commands']if row['name']=='full-rdbc-compile').copy();library[library.index(str(rdbc/'src/lib.rs'))]=str(rr/'src/lib.rs');library[library.index('--out-dir')+1]=str(rr)
    for i,arg in enumerate(library):
        if arg.startswith('metadata='):library[i]='metadata=task7_xml_'+variant.replace('-','_')
    assert run(variant+'-rdbc-compile',library)==0
    library=next(row['command']for row in original['commands']if row['name']=='full-druid-compile').copy();library[library.index(str(dest/'src/lib.rs'))]=str(dd/'src/lib.rs');library[library.index('-o')+1]=str(dd/'libdruid.rlib')
    for i,arg in enumerate(library):
        if arg.startswith('druid_rdbc='):library[i]='druid_rdbc='+str(rr/'libdruid_rdbc.rlib')
        if arg.startswith('metadata='):library[i]='metadata=task7_xml_druid_'+variant.replace('-','_')
    env=os.environ.copy();env.update(original['druid_environment']);env['CARGO_MANIFEST_DIR']=str(dd);assert run(variant+'-druid-compile',library,env)==0
    command=consumer.copy();command[command.index('-o')+1]=str(root/'consumer.exe')
    for i,arg in enumerate(command):
        if arg.startswith('druid_rdbc='):command[i]='druid_rdbc='+str(rr/'libdruid_rdbc.rlib')
        if arg.startswith('druid='):command[i]='druid='+str(dd/'libdruid.rlib')
    command+=['-L','dependency='+str(rr)];assert run(variant+'-consumer-compile',command)==0;assert run(variant+'-runtime',[str(root/'consumer.exe'),'xml_value_contract::'+test,'--exact','--test-threads=1'])!=0
    state[variant]={'root':str(root),'changed_source':relative,'sources':{p.relative_to(root).as_posix():sha(p)for p in root.rglob('*.rs')},'rlib_sha256':sha(dd/'libdruid.rlib'),'rdbc_rlib_sha256':sha(rr/'libdruid_rdbc.rlib'),'exe_sha256':sha(root/'consumer.exe')}
state['consumer_sha256']=sha(source);state['exe_sha256']=sha(dest/'consumer2.exe');state['terminal']=True;(dest/'expanded.summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
