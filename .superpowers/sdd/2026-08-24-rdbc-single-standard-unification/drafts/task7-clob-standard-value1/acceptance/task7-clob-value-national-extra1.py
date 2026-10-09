"""Exercise every explicit NClob method directly, in addition to its shared Clob view."""
import hashlib,json,subprocess
from pathlib import Path
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008');dest=base/'task7-clob-value-druid-candidate1';sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
expanded=json.loads((dest/'expanded.summary.json').read_text(encoding='utf-8'));assert expanded['terminal'];state={'terminal':False,'production_applied':False,'commands':[]}
prior=base/'task7-pooled-clob-value-consumer2.rs';source=base/'task7-pooled-clob-value-consumer3.rs';text=prior.read_text(encoding='utf-8');assert text.rstrip().endswith('}');source.write_text(text.rstrip()[:-1]+(base/'task7-clob-value-national-direct1.rs').read_text(encoding='utf-8')+'\n}\n',encoding='utf-8',newline='\n')
def run(name,command):
    log=dest/(name+'.log')
    with log.open('xb')as out:result=subprocess.run(command,stdout=out,stderr=subprocess.STDOUT,creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    state['commands'].append({'name':name,'command':command,'exit_code':result.returncode,'log':str(log),'log_sha256':sha(log)});(dest/'extra.summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8');print(name,result.returncode,flush=True);return result.returncode
command=expanded['commands'][0]['command'].copy();command[command.index(str(prior))]=str(source);command[command.index('-o')+1]=str(dest/'consumer3.exe');assert run('consumer3-compile',command)==0;assert run('consumer3-runtime',[str(dest/'consumer3.exe'),'--test-threads=1'])==0
state['consumer_sha256']=sha(source);state['exe_sha256']=sha(dest/'consumer3.exe');state['terminal']=True;(dest/'extra.summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
