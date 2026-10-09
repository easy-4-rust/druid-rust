"""Keep module docs first; compile the complete isolated source candidate."""
from pathlib import Path
import hashlib
import json
import os
import subprocess
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
candidate=base/'task7-array-source-candidate1'
path=candidate/'src/core/mod.rs'; text=path.read_text(encoding='utf-8')
prefix='mod druid_array;\nmod druid_array_state;\n'
assert text.startswith(prefix)
text=text[len(prefix):].replace('pub mod abstract_oracle_exception_sorter;',prefix+'pub mod abstract_oracle_exception_sorter;',1)
path.write_text(text,encoding='utf-8',newline='\n')
tool=Path('C:/Users/hnxyh/.rustup/toolchains/1.97.1-x86_64-pc-windows-msvc/bin')
files=[candidate/'src/core'/name for name in ('druid_array.rs','druid_array_state.rs','druid_pooled_connection.rs','druid_pooled_result_set.rs','mod.rs')]
fmt=subprocess.run([str(tool/'rustfmt.exe'),'--edition','2021','--config','skip_children=true',*[str(p) for p in files]],capture_output=True)
(candidate/'rustfmt2.log').write_bytes(fmt.stdout+fmt.stderr)
if fmt.returncode: raise SystemExit(fmt.returncode)
baseline=json.loads((base/'task7-array-baseline-coherent1/summary.json').read_text(encoding='utf-8'))
record=next(row for row in baseline['commands'] if row['name']=='druid')
command=record['command'].copy()
for i,arg in enumerate(command):
    if arg==str(base/'task7-full-druid-source-candidate1/src/lib.rs'): command[i]=str(candidate/'src/lib.rs')
    if arg=='metadata=task7_array_coherent_druid1': command[i]='metadata=task7_pooled_array_green1'
command[command.index('-o')+1]=str(candidate/'libdruid.rlib')
env=os.environ.copy(); env.update(record['environment']); env['CARGO_MANIFEST_DIR']=str(candidate)
with (candidate/'compile2.log').open('xb') as output:
    result=subprocess.run(command,env=env,stdout=output,stderr=subprocess.STDOUT,creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
summary={'command':command,'environment':record['environment'] | {'CARGO_MANIFEST_DIR':str(candidate)},
         'exit_code':result.returncode,'production_applied':False,
         'sources':{p.relative_to(candidate).as_posix():hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted((candidate/'src').rglob('*.rs'))}}
(candidate/'compile2.summary.json').write_text(json.dumps(summary,indent=2)+'\n',encoding='utf-8')
print(json.dumps({'compile_exit':result.returncode,'candidate':str(candidate)}),flush=True)
raise SystemExit(result.returncode)
