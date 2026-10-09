"""Resolve fixture collection inference without changing production or test assertions."""
import hashlib,json,subprocess,shutil
from pathlib import Path
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008');rdbc=base/'task7-clob-value-rdbc-candidate1';dest=base/'task7-clob-value-druid-candidate1';sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
state=json.loads((dest/'summary.json').read_text(encoding='utf-8'));assert not state['terminal'];state['fixture_repair']='explicit Vec collection in fixture ASCII conversion; original E0283 log/source retained'
for name in ['task7-clob-value-contract1.rs','task7-pooled-clob-value-consumer1.rs']:
    p=base/name;backup=base/(p.stem+'-initial'+p.suffix);assert not backup.exists();shutil.copyfile(p,backup);text=p.read_text(encoding='utf-8');needle=".map(|v|u8::try_from(*v).unwrap_or(b'?')).collect()";assert text.count(needle)==1;p.write_text(text.replace(needle,needle[:-2]+'::<Vec<_>>()'),encoding='utf-8',newline='\n')
def run(name,command):
    log=dest/(name+'.log')
    with log.open('xb')as out:result=subprocess.run(command,stdout=out,stderr=subprocess.STDOUT,creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    state['commands'].append({'name':name,'command':command,'exit_code':result.returncode,'log':str(log),'log_sha256':sha(log)});(dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8');print(name,result.returncode,flush=True);return result.returncode
command=next(row['command']for row in state['commands']if row['name']=='consumer-compile');assert run('consumer-compile2',command)==0;assert run('consumer-runtime',[str(dest/'consumer.exe'),'--test-threads=1'])==0
state['sources']={p.relative_to(dest).as_posix():sha(p)for p in(dest/'src').rglob('*.rs')};state['rdbc_sources']={p.relative_to(rdbc).as_posix():sha(p)for p in(rdbc/'src').rglob('*.rs')};state['rlib_sha256']=sha(dest/'libdruid.rlib');state['rdbc_rlib_sha256']=sha(rdbc/'libdruid_rdbc.rlib');state['consumer_sha256']=sha(base/'task7-pooled-clob-value-consumer1.rs');state['exe_sha256']=sha(dest/'consumer.exe');state['terminal']=True
(dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
