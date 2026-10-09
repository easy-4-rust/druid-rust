"""Correct the fixture to use public SQLException accessor; retain initial E0616."""
import hashlib
import json
from pathlib import Path
import subprocess

base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
out=base/'task7-array-baseline-coherent1'
source=base/'task7-pooled-array-consumer2.rs'
prior=(base/'task7-pooled-array-consumer1.rs').read_text(encoding='utf-8')
assert prior.count('.sql_state.as_deref()')==1
source.write_text(prior.replace('.sql_state.as_deref()', '.sql_state()'),encoding='utf-8',newline='\n')
summary=json.loads((out/'consumers2.summary.json').read_text(encoding='utf-8'))
command=summary['commands'][-1]['command'].copy()
for i,arg in enumerate(command):
    if arg.endswith('task7-pooled-array-consumer1.rs'): command[i]=str(source)
exe=out/'array-red2.exe'
command[command.index('-o')+1]=str(exe)
state={'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'production_applied':False,'commands':[]}
for name,args in [('compile',command),('runtime',[str(exe),'--test-threads=1'])]:
    log=out/('array-red2-'+name+'.log')
    with log.open('xb') as output:
        result=subprocess.run(args,stdout=output,stderr=subprocess.STDOUT,creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    state['commands'].append({'name':name,'command':args,'exit_code':result.returncode,'log':str(log),
                              'log_sha256':hashlib.sha256(log.read_bytes()).hexdigest()})
    (out/'array-red2.summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
    print(json.dumps({'name':name,'exit_code':result.returncode}),flush=True)
    if name=='compile' and result.returncode: raise SystemExit(result.returncode)
