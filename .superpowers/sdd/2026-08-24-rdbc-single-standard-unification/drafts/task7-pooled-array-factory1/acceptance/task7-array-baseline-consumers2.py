"""Retain original fixture dead-code policy and compile identical consumers."""
import hashlib
import json
from pathlib import Path
import subprocess

base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
directory=base/'task7-array-baseline-coherent1'
previous=json.loads((directory/'summary.json').read_text(encoding='utf-8'))
template=previous['commands'][-1]['command']
assert previous['commands'][-1]['name']=='xml-regression-compile'
state={'previous_setup_error':'First consumer compile omitted original -A dead_code; five unused fixture diagnostics, no tests executed.',
       'production_applied':False,'commands':[]}
def run(name,command):
    path=directory/(name+'.log')
    record={'name':name,'command':command,'log':str(path)}
    state['commands'].append(record)
    with path.open('xb') as output:
        process=subprocess.run(command,stdout=output,stderr=subprocess.STDOUT,creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    record.update(exit_code=process.returncode,log_sha256=hashlib.sha256(path.read_bytes()).hexdigest())
    (directory/'consumers2.summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
    print(json.dumps({'name':name,'exit_code':process.returncode}),flush=True)
    return process.returncode

for label,source in [('xml-control2','task7-pooled-xml-factory-contract2.rs'),('array-red1','task7-pooled-array-consumer1.rs')]:
    command=template.copy()
    for i,arg in enumerate(command):
        if arg.endswith('task7-pooled-xml-factory-contract2.rs'): command[i]=str(base/source)
    executable=directory/(label+'.exe')
    command[command.index('-o')+1]=str(executable)
    command.extend(['-A','dead_code'])
    code=run(label+'-compile',command)
    if code: raise SystemExit(code)
    code=run(label+'-runtime',[str(executable),'--test-threads=1'])
    if label=='xml-control2' and code: raise SystemExit(code)
state['terminal']=True
(directory/'consumers2.summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
