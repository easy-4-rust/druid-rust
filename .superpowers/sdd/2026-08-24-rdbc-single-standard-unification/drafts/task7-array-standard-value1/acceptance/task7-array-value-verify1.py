"""Expand race/panic contracts, negative compilations and full-source counterexamples."""
import hashlib,json,os,shutil,subprocess
from pathlib import Path
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008');dest=base/'task7-array-value-druid-candidate1';rdbc=base/'task7-array-value-rdbc-candidate1'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();original=json.loads((dest/'summary.json').read_text(encoding='utf-8'));assert original['terminal']
state={'production_applied':False,'commands':[],'terminal':False}
def run(name,command,env=None):
    log=dest/(name+'.log')
    with log.open('xb') as out:result=subprocess.run(command,env=env,stdout=out,stderr=subprocess.STDOUT,creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    state['commands'].append({'name':name,'command':command,'exit_code':result.returncode,'log':str(log),'log_sha256':sha(log)})
    (dest/'expanded.summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8');print(name,result.returncode,flush=True);return result.returncode
prior=base/'task7-pooled-array-value-consumer2.rs';source=base/'task7-pooled-array-value-consumer3.rs'
source.write_bytes(prior.read_bytes()+b'\n'+(base/'task7-array-value-boundary1.rs').read_bytes()+b'\n'+(base/'task7-array-value-lock-race1.rs').read_bytes());assert source.read_bytes().startswith((base/'task7-pooled-ref-consumer2.rs').read_bytes())
consumer=next(r['command'] for r in original['commands'] if r['name']=='consumer-compile').copy();consumer[consumer.index(str(prior))]=str(source);consumer[consumer.index('-o')+1]=str(dest/'consumer3.exe')
assert run('consumer3-compile',consumer)==0
assert run('consumer3-runtime',[str(dest/'consumer3.exe'),'--test-threads=1'])==0
for name,addition,diagnostic in [('negative-array-sync','fn require_sync<T:Sync>(){}\nfn rejected_sync(){require_sync::<ArrayDriver>();}\n','E0277'),('negative-array-access','use druid_rdbc::resource::RdbcArrayAccess;\n','E0432')]:
    p=dest/(name+'.rs');p.write_bytes(source.read_bytes()+addition.encode());command=consumer.copy();command[command.index(str(source))]=str(p);command[command.index('-o')+1]=str(dest/(name+'.rmeta'));command+=['--emit=metadata','--crate-name',name.replace('-','_')]
    assert run(name,command)!=0;assert diagnostic in (dest/(name+'.log')).read_text(encoding='utf-8')
for variant,needle,replacement,test in [
    ('capability','        self.require_capability(capability,operation)\n','        let _=(capability,operation); Ok(())\n','task7_array_value_all_capabilities_gate_before_driver_and_after_free'),
    ('lock-recheck','        // 等待锁期间可能已经释放，必须在调用驱动前再次检查关闭与能力优先级。\n        self.require(capability,operation)?;\n','        // 反例：故意移除取得锁后的关闭检查。\n','task7_array_value_waiting_read_rechecks_release_after_lock_handoff')]:
    mutation=base/('task7-array-value-'+variant+'-mutation1');mutation.mkdir();mr=mutation/'rdbc';md=mutation/'druid';shutil.copytree(rdbc/'src',mr/'src');shutil.copytree(dest/'src',md/'src');shutil.copytree(dest/'resources',md/'resources')
    p=mr/'src/resource/standard_resource_value.rs';text=p.read_text(encoding='utf-8');assert text.count(needle)==1;text=text.replace(needle,replacement);p.write_text(text,encoding='utf-8',newline='\n')
    library=next(r['command'] for r in original['commands'] if r['name']=='full-rdbc-compile').copy();library[library.index(str(rdbc/'src/lib.rs'))]=str(mr/'src/lib.rs');library[library.index('--out-dir')+1]=str(mr)
    for i,arg in enumerate(library):
        if arg.startswith('metadata='):library[i]='metadata=array_value_'+variant.replace('-','_')
    assert run(variant+'-rdbc-compile',library)==0
    library=next(r['command'] for r in original['commands'] if r['name']=='full-druid-compile').copy();library[library.index(str(dest/'src/lib.rs'))]=str(md/'src/lib.rs');library[library.index('-o')+1]=str(md/'libdruid.rlib')
    for i,arg in enumerate(library):
        if arg.startswith('druid_rdbc='):library[i]='druid_rdbc='+str(mr/'libdruid_rdbc.rlib')
        if arg.startswith('metadata='):library[i]='metadata=array_value_druid_'+variant.replace('-','_')
    env=os.environ.copy();env.update(original['druid_environment']);env['CARGO_MANIFEST_DIR']=str(md)
    assert run(variant+'-druid-compile',library,env)==0
    command=consumer.copy();command[command.index('-o')+1]=str(mutation/'consumer.exe')
    for i,arg in enumerate(command):
        if arg.startswith('druid_rdbc='):command[i]='druid_rdbc='+str(mr/'libdruid_rdbc.rlib')
        if arg.startswith('druid='):command[i]='druid='+str(md/'libdruid.rlib')
    command+=['-L','dependency='+str(mr)]
    assert run(variant+'-consumer-compile',command)==0
    assert run(variant+'-runtime',[str(mutation/'consumer.exe'),test,'--exact','--test-threads=1'])!=0
    state[variant]={'root':str(mutation),'test':test,'sources':{p.relative_to(mutation).as_posix():sha(p) for p in mutation.rglob('*.rs')},'rlib_sha256':sha(mr/'libdruid_rdbc.rlib'),'druid_rlib_sha256':sha(md/'libdruid.rlib'),'exe_sha256':sha(mutation/'consumer.exe')}
state['source_sha256']=sha(source);state['prior_58_sha256']=sha(base/'task7-pooled-ref-consumer2.rs');state['exe_sha256']=sha(dest/'consumer3.exe');state['terminal']=True
(dest/'expanded.summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
