"""Remove proxy admission rejection; the unchanged driver still rejects capability."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
candidate=base/'task7-array-source-candidate1'
dest=base/'task7-array-capability-mutation1'
contract=json.loads((candidate/'contract6.summary.json').read_text(encoding='utf-8'))
assert contract['terminal'] and contract['commands'][-1]['exit_code']==0
dest.mkdir()
shutil.copytree(candidate/'src',dest/'src');shutil.copytree(candidate/'resources',dest/'resources')
path=dest/'src/core/druid_array.rs'; old=path.read_text(encoding='utf-8')
before='if !self.capabilities.contains(capability) {\n            return Err(RdbcError::feature_not_supported(operation));\n        }'
after='if !self.capabilities.contains(capability) {\n            let _ = operation;\n        }'
assert old.count(before)==1
path.write_text(old.replace(before,after),encoding='utf-8',newline='\n')
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
state={'production_applied':False,'mutation':{'file':'src/core/druid_array.rs','before':before,'after':after,'base_sha256':sha(candidate/'src/core/druid_array.rs'),'mutated_sha256':sha(path)},'commands':[]}
def run(name,command,env=None):
    log=dest/(name+'.log')
    with log.open('xb') as out: result=subprocess.run(command,env=env,stdout=out,stderr=subprocess.STDOUT,creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    state['commands'].append({'name':name,'command':command,'exit_code':result.returncode,'log':str(log),'log_sha256':sha(log)})
    (dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
    print(json.dumps({'name':name,'exit_code':result.returncode}),flush=True)
    return result.returncode
record=json.loads((candidate/'compile2.summary.json').read_text(encoding='utf-8'))
command=record['command'].copy();command[command.index(str(candidate/'src/lib.rs'))]=str(dest/'src/lib.rs');command[command.index('-o')+1]=str(dest/'libdruid.rlib')
for i,arg in enumerate(command):
    if arg.startswith('metadata='):command[i]='metadata=task7_array_capability_mutation1'
env=os.environ.copy();env.update(record['environment']);env['CARGO_MANIFEST_DIR']=str(dest)
state['environment']=record['environment'] | {'CARGO_MANIFEST_DIR':str(dest)}
assert run('full-library-compile',command,env)==0
command=next(r['command'] for r in contract['commands'] if r['name']=='after6-compile').copy()
for i,arg in enumerate(command):
    if arg.startswith('druid='):command[i]='druid='+str(dest/'libdruid.rlib')
exe=dest/'capability_mutation.exe';command[command.index('-o')+1]=str(exe)
assert run('consumer-compile',command)==0
assert run('capability-runtime',[str(exe),'--exact','task7_array_sparse_capability_matrix_and_closed_precedence','--test-threads=1'])==101
log=(dest/'capability-runtime.log').read_text(encoding='utf-8')
assert 'unsupported/closed must not touch driver' in log and '0 passed; 1 failed;' in log
state['terminal']=True;state['candidate_sources_unchanged']=all(sha(candidate/p)==digest for p,digest in record['sources'].items())
assert state['candidate_sources_unchanged']
state['source_sha256']=sha(base/'task7-pooled-array-consumer6.rs')
(dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
