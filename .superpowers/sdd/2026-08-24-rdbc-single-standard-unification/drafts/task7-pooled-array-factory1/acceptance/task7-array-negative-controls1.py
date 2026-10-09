"""Real compiler negative control and child-close mutation against full source."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
candidate=base/'task7-array-source-candidate1'
dest=base/'task7-array-child-close-mutation1'
state={'production_applied':False,'commands':[]}
def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
def run(name,command,env=None):
    path=dest/(name+'.log')
    with path.open('xb') as output:
        result=subprocess.run(command,env=env,stdout=output,stderr=subprocess.STDOUT,creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    state['commands'].append({'name':name,'command':command,'exit_code':result.returncode,'log':str(path),'log_sha256':sha(path)})
    (dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
    print(json.dumps({'name':name,'exit_code':result.returncode}),flush=True)
    return result.returncode
dest.mkdir()
contracts=json.loads((candidate/'contract5.summary.json').read_text(encoding='utf-8'))
assert contracts['terminal'] and contracts['commands'][-1]['exit_code']==0
command=next(r['command'] for r in contracts['commands'] if r['name']=='after5-compile').copy()
source=base/'task7-pooled-array-consumer5.rs'
negative=dest/'array_not_sync.rs'
negative.write_bytes(source.read_bytes()+b'\nfn array_must_not_satisfy_sync() { fn requires_sync<T: Sync>() {} requires_sync::<ArrayDriver>(); }\n')
command[command.index(str(source))]=str(negative)
command[command.index('-o')+1]=str(dest/'array_not_sync.rmeta')
command+=['--emit=metadata']
code=run('array-not-sync',command)
log=(dest/'array-not-sync.log').read_text(encoding='utf-8')
assert code!=0 and 'E0277' in log and 'Cell<usize>' in log and 'cannot be shared between threads safely' in log
shutil.copytree(candidate/'src',dest/'src')
shutil.copytree(candidate/'resources',dest/'resources')
path=dest/'src/core/druid_array_state.rs'
text=path.read_text(encoding='utf-8')
old='for child in children {\n            child.close_core().await?;\n        }'
new='for child in children {\n            drop(child);\n        }'
assert text.count(old)==1
path.write_text(text.replace(old,new),encoding='utf-8',newline='\n')
state['mutation']={'file':'src/core/druid_array_state.rs','from':old,'to':new,'base_sha256':sha(candidate/'src/core/druid_array_state.rs'),'mutated_sha256':sha(path)}
compile_record=json.loads((candidate/'compile2.summary.json').read_text(encoding='utf-8'))
command=compile_record['command'].copy()
command[command.index(str(candidate/'src/lib.rs'))]=str(dest/'src/lib.rs')
command[command.index('-o')+1]=str(dest/'libdruid.rlib')
for i,arg in enumerate(command):
    if arg.startswith('metadata='): command[i]='metadata=task7_array_child_close_mutation1'
env=os.environ.copy(); env.update(compile_record['environment']); env['CARGO_MANIFEST_DIR']=str(dest)
state['environment']=compile_record['environment'] | {'CARGO_MANIFEST_DIR':str(dest)}
assert run('full-library-compile',command,env)==0
command=next(r['command'] for r in contracts['commands'] if r['name']=='after5-compile').copy()
for i,arg in enumerate(command):
    if arg.startswith('druid='): command[i]='druid='+str(dest/'libdruid.rlib')
exe=dest/'child_close_mutation.exe'; command[command.index('-o')+1]=str(exe)
assert run('consumer-compile',command)==0
test='task7_array_explicit_free_closes_child_before_connection_return'
assert run('child-close-runtime',[str(exe),'--exact',test,'--test-threads=1'])==101
log=(dest/'child-close-runtime.log').read_text(encoding='utf-8')
assert '0 passed; 1 failed;' in log and 'left: 0' in log and 'right: 1' in log
state['terminal']=True
state['source_sha256']=sha(source)
state['candidate_sources_unchanged']=all(sha(candidate/rel)==digest for rel,digest in compile_record['sources'].items())
assert state['candidate_sources_unchanged']
(dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
