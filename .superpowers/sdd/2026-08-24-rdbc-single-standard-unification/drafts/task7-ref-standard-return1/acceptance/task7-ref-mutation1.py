"""Remove only the handle capability gate; same exact matrix must detect the regression."""
import hashlib,json,shutil,subprocess
from pathlib import Path
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008');old=base/'task7-ref-rdbc-candidate1';dest=base/'task7-ref-capability-mutation1';dest.mkdir();shutil.copytree(old/'src',dest/'src')
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
p=dest/'src/resource/rdbc_ref.rs';text=p.read_text(encoding='utf-8');marker='Err(RdbcError::feature_not_supported(operation))';assert text.count(marker)==1;text=text.replace(marker,'{ let _ = operation; Ok(()) }');p.write_text(text,encoding='utf-8',newline='\n')
record=json.loads((old/'summary.json').read_text(encoding='utf-8'));state={'production_applied':False,'commands':[]}
def run(name,command):
    log=dest/(name+'.log')
    with log.open('xb') as out:result=subprocess.run(command,stdout=out,stderr=subprocess.STDOUT,creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    state['commands'].append({'name':name,'command':command,'exit_code':result.returncode,'log':str(log),'log_sha256':sha(log)})
    (dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8');print(name,result.returncode,flush=True);return result.returncode
library=next(r['command'] for r in record['commands'] if r['name']=='full-rdbc-compile3').copy();library[library.index(str(old/'src/lib.rs'))]=str(dest/'src/lib.rs');library[library.index('--out-dir')+1]=str(dest)
for i,arg in enumerate(library):
    if arg.startswith('metadata='):library[i]='metadata=task7_ref_mutation1'
assert run('library',library)==0
consumer=next(r['command'] for r in record['commands'] if r['name']=='green-consumer-compile').copy();consumer[consumer.index('-o')+1]=str(dest/'consumer.exe')
for i,arg in enumerate(consumer):
    if arg.startswith('druid_rdbc='):consumer[i]='druid_rdbc='+str(dest/'libdruid_rdbc.rlib')
assert run('consumer-compile',consumer)==0
assert run('exact-matrix-runtime',[str(dest/'consumer.exe'),'--exact','task7_ref_all_eight_capability_combinations_reject_before_driver','--test-threads=1'])==101
assert '0 passed; 1 failed' in (dest/'exact-matrix-runtime.log').read_text(encoding='utf-8')
state['changed_sources']=[p.relative_to(dest).as_posix() for p in (dest/'src').rglob('*.rs') if sha(p)!=sha(old/p.relative_to(dest))];assert state['changed_sources']==['src/resource/rdbc_ref.rs']
state['sources']={p.relative_to(dest).as_posix():sha(p) for p in (dest/'src').rglob('*.rs')};state['terminal']=True
(dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
