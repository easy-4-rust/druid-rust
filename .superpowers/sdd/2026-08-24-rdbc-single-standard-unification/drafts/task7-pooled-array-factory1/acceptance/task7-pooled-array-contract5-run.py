"""Run identical full consumers against before/after complete libraries."""
import hashlib
import json
from pathlib import Path
import subprocess
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
old=base/'task7-array-baseline-coherent1'
candidate=base/'task7-array-source-candidate1'
source=base/'task7-pooled-array-consumer5.rs'
template=json.loads((old/'array-red2.summary.json').read_text(encoding='utf-8'))['commands'][0]['command']
state={'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'production_applied':False,'commands':[]}
def run(name,command):
    log=candidate/(name+'.log')
    with log.open('xb') as output:
        result=subprocess.run(command,stdout=output,stderr=subprocess.STDOUT,creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    record={'name':name,'command':command,'exit_code':result.returncode,'log':str(log),'log_sha256':hashlib.sha256(log.read_bytes()).hexdigest()}
    state['commands'].append(record)
    (candidate/'contract5.summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
    print(json.dumps({'name':name,'exit_code':result.returncode}),flush=True)
    return result.returncode
for label,library in [('before5',old/'libdruid.rlib'),('after5',candidate/'libdruid.rlib')]:
    command=template.copy()
    for i,arg in enumerate(command):
        if arg.endswith('task7-pooled-array-consumer2.rs'): command[i]=str(source)
        if arg.startswith('druid='): command[i]='druid='+str(library)
    exe=candidate/(label+'.exe'); command[command.index('-o')+1]=str(exe)
    if run(label+'-compile',command): raise SystemExit(1)
    code=run(label+'-runtime',[str(exe),'--test-threads=1'])
    if label=='after5' and code: raise SystemExit(code)
state['terminal']=True
(candidate/'contract5.summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
