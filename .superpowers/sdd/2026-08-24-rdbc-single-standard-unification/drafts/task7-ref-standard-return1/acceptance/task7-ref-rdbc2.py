"""Resume exact failed candidate compilation; preserve initial errors and original contract."""
import hashlib,json,subprocess
from pathlib import Path
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008');dest=base/'task7-ref-rdbc-candidate1'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
state=json.loads((dest/'summary.json').read_text(encoding='utf-8'))
def run(name,command):
    log=dest/(name+'.log')
    with log.open('xb') as out:result=subprocess.run(command,stdout=out,stderr=subprocess.STDOUT,creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    state['commands'].append({'name':name,'command':command,'exit_code':result.returncode,'log':str(log),'log_sha256':sha(log)})
    (dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8');print(name,result.returncode,flush=True);return result.returncode
library=next(r['command'] for r in state['commands'] if r['name']=='full-rdbc-compile').copy()
assert run('full-rdbc-compile2',library)==0
consumer=state['commands'][0]['command'].copy();consumer[consumer.index('-o')+1]=str(dest/'green.exe')
for i,arg in enumerate(consumer):
    if arg.startswith('druid_rdbc='):consumer[i]='druid_rdbc='+str(dest/'libdruid_rdbc.rlib')
assert run('green-consumer-compile',consumer)==0
assert run('green-runtime',[str(dest/'green.exe'),'--test-threads=1'])==0
for name,addition,diagnostic in [('negative-sync','fn require_sync<T: Sync>() {}\nfn forbidden() {require_sync::<DriverRef>();require_sync::<DriverStruct>();}\n','E0277'),('negative-free','#[async_trait::async_trait]\nimpl Ref for ForbiddenRef {fn capabilities(&self)->ResourceOperations {ResourceOperations::empty()} async fn base_type_name(&mut self)->Result<String,RdbcError>{Err(failure())} async fn object(&mut self)->Result<RdbcObject,RdbcError>{Err(failure())} async fn object_with_type_map(&mut self,_:&RdbcTypeMap)->Result<RdbcObject,RdbcError>{Err(failure())} async fn set_object(&mut self,_:RdbcObject)->Result<(),RdbcError>{Err(failure())} async fn free(&mut self)->Result<(),RdbcError>{Ok(())}}\n#[derive(Debug)] struct ForbiddenRef;\n','E0407'),('negative-old-access','use druid_rdbc::resource::RdbcRefAccess;\n','E0432')]:
    source=dest/(name+'.rs');source.write_text((base/'task7-ref-contract1.rs').read_text(encoding='utf-8')+'\n'+addition,encoding='utf-8',newline='\n')
    command=consumer.copy();command[command.index(str(base/'task7-ref-contract1.rs'))]=str(source);command[command.index('-o')+1]=str(dest/(name+'.rmeta'))
    command+=['--emit=metadata','--crate-name',name.replace('-','_')]
    assert run(name,command)!=0;assert diagnostic in (dest/(name+'.log')).read_text(encoding='utf-8')
state['sources']={p.relative_to(dest).as_posix():sha(p) for p in (dest/'src').rglob('*.rs')}
acquisition=json.loads((base/'task8-java-jdbc42-source-acquisition1.summary.json').read_text(encoding='utf-8'));state['java_sources']={}
for row in acquisition['sources']:
    if row['fqcn'] in ('java.sql.Ref','java.sql.Struct'):
        assert sha(Path(row['local_path']))==row['sha256'];state['java_sources'][row['fqcn']]=row
assert len(state['java_sources'])==2
state['terminal']=True;state['rlib_sha256']=sha(dest/'libdruid_rdbc.rlib');state['exe_sha256']=sha(dest/'green.exe')
(dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
