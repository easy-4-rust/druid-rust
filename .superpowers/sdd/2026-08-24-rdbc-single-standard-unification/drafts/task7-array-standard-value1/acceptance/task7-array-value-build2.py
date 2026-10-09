"""Resume candidate after retaining the initial command setup and fixture diagnostics."""
import hashlib,json,os,subprocess
from pathlib import Path
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008');oldr=base/'task7-ref-rdbc-candidate1';oldd=base/'task7-ref-druid-candidate1';rdbc=base/'task7-array-value-rdbc-candidate1';dest=base/'task7-array-value-druid-candidate1'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();state=json.loads((dest/'summary.json').read_text(encoding='utf-8'))
def run(name,command,env=None):
    log=dest/(name+'.log')
    with log.open('xb') as out:result=subprocess.run(command,env=env,stdout=out,stderr=subprocess.STDOUT,creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    state['commands'].append({'name':name,'command':command,'exit_code':result.returncode,'log':str(log),'log_sha256':sha(log)})
    (dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8');print(name,result.returncode,flush=True);return result.returncode
source=base/'task7-pooled-array-value-consumer2.rs';prior=base/'task7-pooled-ref-consumer2.rs'
initial=(base/'task7-pooled-array-value-consumer1.rs').read_text(encoding='utf-8');needle='let task=tokio::spawn(async move{child.result_set().await});'
assert initial.count(needle)==1;source.write_text(initial.replace(needle,'let task=tokio::spawn(async move{child.result_set().await.map(|_|())});'),encoding='utf-8',newline='\n')
consumer=state['commands'][0]['command'].copy();consumer[consumer.index(str(base/'task7-pooled-array-value-consumer1.rs'))]=str(source)
assert run('old-array-value-consumer-red2',consumer)!=0
red=(dest/'old-array-value-consumer-red2.log').read_text(encoding='utf-8');assert 'E0308' in red and 'E0277' not in red
record=json.loads((oldr/'summary.json').read_text(encoding='utf-8'));library=next(r['command'] for r in record['commands'] if r['name']=='full-rdbc-compile3').copy()
library[library.index(str(oldr/'src/lib.rs'))]=str(rdbc/'src/lib.rs');library[library.index('--out-dir')+1]=str(rdbc)
for i,arg in enumerate(library):
    if arg.startswith('metadata='):library[i]='metadata=task7_array_value_rdbc1'
assert run('full-rdbc-compile',library)==0
record=json.loads((oldd/'summary.json').read_text(encoding='utf-8'));row=next(r for r in record['commands'] if r['name']=='after-library');library=row['command'].copy()
library[library.index(str(oldd/'src/lib.rs'))]=str(dest/'src/lib.rs');library[library.index('-o')+1]=str(dest/'libdruid.rlib')
for i,arg in enumerate(library):
    if arg.startswith('druid_rdbc='):library[i]='druid_rdbc='+str(rdbc/'libdruid_rdbc.rlib')
    if arg.startswith('metadata='):library[i]='metadata=task7_array_value_druid1'
env=os.environ.copy();env.update(row['environment']);env['CARGO_MANIFEST_DIR']=str(dest);state['druid_environment']=row['environment']|{'CARGO_MANIFEST_DIR':str(dest)}
assert run('full-druid-old-export-red',library,env)!=0
assert 'RdbcArrayAccess' in (dest/'full-druid-old-export-red.log').read_text(encoding='utf-8')
p=dest/'src/core/mod.rs';text=p.read_text(encoding='utf-8');assert text.count('RdbcArrayAccess,')==1;text=text.replace('RdbcArrayAccess,','');p.write_text(text,encoding='utf-8',newline='\n')
assert run('full-druid-compile',library,env)==0
for i,arg in enumerate(consumer):
    if arg.startswith('druid_rdbc='):consumer[i]='druid_rdbc='+str(rdbc/'libdruid_rdbc.rlib')
    if arg.startswith('druid='):consumer[i]='druid='+str(dest/'libdruid.rlib')
consumer+=['-L','dependency='+str(rdbc)]
assert run('consumer-compile',consumer)==0
assert run('consumer-runtime',[str(dest/'consumer.exe'),'--test-threads=1'])==0
state['sources']={p.relative_to(dest).as_posix():sha(p) for p in (dest/'src').rglob('*.rs')};state['rdbc_sources']={p.relative_to(rdbc).as_posix():sha(p) for p in (rdbc/'src').rglob('*.rs')}
assert source.read_bytes().startswith(prior.read_bytes())
state['rlib_sha256']=sha(dest/'libdruid.rlib');state['rdbc_rlib_sha256']=sha(rdbc/'libdruid_rdbc.rlib');state['consumer_sha256']=sha(source);state['prior_58_sha256']=sha(prior);state['exe_sha256']=sha(dest/'consumer.exe');state['terminal']=True
(dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
