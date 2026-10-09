"""Additional actual pooled alias forwarding and pair-search cancellation contracts."""
import hashlib,json,subprocess
from pathlib import Path
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008');dest=base/'task7-blob-value-druid-candidate1';sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
record=json.loads((dest/'summary.json').read_text(encoding='utf-8'));assert record['terminal'];state={'production_applied':False,'commands':[],'terminal':False}
prior=base/'task7-pooled-blob-value-consumer2.rs';source=base/'task7-pooled-blob-value-consumer3.rs';source.write_bytes(prior.read_bytes()+b'\n'+(base/'task7-blob-value-pooled-search1.rs').read_bytes())
expanded=json.loads((dest/'expanded.summary.json').read_text(encoding='utf-8'));command=next(row['command'] for row in expanded['commands'] if row['name']=='consumer2-compile').copy();command[command.index(str(prior))]=str(source);command[command.index('-o')+1]=str(dest/'consumer3.exe')
for name,cmd in [('consumer3-compile',command),('consumer3-runtime',[str(dest/'consumer3.exe'),'--test-threads=1'])]:
    log=dest/(name+'.log')
    with log.open('xb') as out:result=subprocess.run(cmd,stdout=out,stderr=subprocess.STDOUT,creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    state['commands'].append({'name':name,'command':cmd,'exit_code':result.returncode,'log':str(log),'log_sha256':sha(log)});(dest/'extra.summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8');print(name,result.returncode,flush=True);assert result.returncode==0
state['source_sha256']=sha(source);state['prior_source_sha256']=sha(prior);state['rlib_sha256']=sha(dest/'libdruid.rlib');state['exe_sha256']=sha(dest/'consumer3.exe');state['terminal']=True
(dest/'extra.summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
