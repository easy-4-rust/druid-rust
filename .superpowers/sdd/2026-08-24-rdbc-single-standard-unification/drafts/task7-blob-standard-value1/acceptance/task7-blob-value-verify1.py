"""Compile negative consumers and full-source capability, alias and lock-order counterexamples."""
import hashlib,json,os,shutil,subprocess
from pathlib import Path
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008');dest=base/'task7-blob-value-druid-candidate1';rdbc=base/'task7-blob-value-rdbc-candidate1';sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
original=json.loads((dest/'summary.json').read_text(encoding='utf-8'));assert original['terminal'];state={'production_applied':False,'commands':[],'terminal':False}
def run(name,command,env=None):
    log=dest/(name+'.log')
    with log.open('xb') as out:result=subprocess.run(command,env=env,stdout=out,stderr=subprocess.STDOUT,creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    state['commands'].append({'name':name,'command':command,'exit_code':result.returncode,'log':str(log),'log_sha256':sha(log)})
    (dest/'expanded.summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8');print(name,result.returncode,flush=True);return result.returncode
prior=base/'task7-pooled-blob-value-consumer1.rs';source=base/'task7-pooled-blob-value-consumer2.rs';source.write_bytes(prior.read_bytes()+b'\n'+(base/'task7-blob-value-lock-order2.rs').read_bytes())
consumer=next(row['command'] for row in original['commands'] if row['name']=='consumer-compile').copy();consumer[consumer.index(str(prior))]=str(source);consumer[consumer.index('-o')+1]=str(dest/'consumer2.exe')
assert run('consumer2-compile',consumer)==0
assert run('consumer2-runtime',[str(dest/'consumer2.exe'),'--test-threads=1'])==0
for name,addition,diagnostic in [('negative-blob-sync','fn require_sync<T:Sync>(){}\nfn forbidden_sync(){require_sync::<BlobValueDriver>();}\n','E0277'),('negative-blob-access','use druid_rdbc::resource::RdbcBlobAccess;\n','E0432')]:
    p=dest/(name+'.rs');p.write_bytes(source.read_bytes()+addition.encode());command=consumer.copy();command[command.index(str(source))]=str(p);command[command.index('-o')+1]=str(dest/(name+'.rmeta'));command+=['--emit=metadata','--crate-name',name.replace('-','_')]
    assert run(name,command)!=0;assert diagnostic in (dest/(name+'.log')).read_text(encoding='utf-8')
for variant,needle,replacement,test in [
    ('capability','.lock(ResourceOperations::READ, "Blob#length")','.lock(ResourceOperations::empty(), "Blob#length")','task7_blob_value_all_128_capabilities_and_freed_precedence'),
    ('self-alias','if Arc::ptr_eq(&self.value, &pattern.value) {','if Arc::ptr_eq(&self.value, &pattern.value) { return Ok(Some(1)); }\n        if Arc::ptr_eq(&self.value, &pattern.value) {','task7_blob_value_self_pattern_is_delegated_without_snapshot'),
    ('lock-order','if self.resource_id() < pattern.resource_id() {','if true {','task7_blob_value_blocked_first_locks_preserve_global_order')]:
    mutation=base/('task7-blob-value-'+variant+'-mutation1');mutation.mkdir();mr=mutation/'rdbc';md=mutation/'druid';shutil.copytree(rdbc/'src',mr/'src');shutil.copytree(dest/'src',md/'src');shutil.copytree(dest/'resources',md/'resources')
    p=mr/'src/resource/rdbc_blob.rs';text=p.read_text(encoding='utf-8');assert text.count(needle)==1;p.write_text(text.replace(needle,replacement),encoding='utf-8',newline='\n')
    library=next(row['command'] for row in original['commands'] if row['name']=='full-rdbc-compile').copy();library[library.index(str(rdbc/'src/lib.rs'))]=str(mr/'src/lib.rs');library[library.index('--out-dir')+1]=str(mr)
    for i,arg in enumerate(library):
        if arg.startswith('metadata='):library[i]='metadata=blob_value_'+variant.replace('-','_')
    assert run(variant+'-rdbc-compile',library)==0
    library=next(row['command'] for row in original['commands'] if row['name']=='full-druid-compile').copy();library[library.index(str(dest/'src/lib.rs'))]=str(md/'src/lib.rs');library[library.index('-o')+1]=str(md/'libdruid.rlib')
    for i,arg in enumerate(library):
        if arg.startswith('druid_rdbc='):library[i]='druid_rdbc='+str(mr/'libdruid_rdbc.rlib')
        if arg.startswith('metadata='):library[i]='metadata=blob_value_druid_'+variant.replace('-','_')
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
state['source_sha256']=sha(source);state['exe_sha256']=sha(dest/'consumer2.exe');state['terminal']=True
(dest/'expanded.summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
