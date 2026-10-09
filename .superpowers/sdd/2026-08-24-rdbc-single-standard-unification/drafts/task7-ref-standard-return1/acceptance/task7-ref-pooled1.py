"""Compile complete Druid candidate against retired Ref Access, then run all previous and new contracts."""
import hashlib,json,os,shutil,subprocess
from pathlib import Path
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008');previous=base/'task7-struct-druid-candidate1';rdbc=base/'task7-ref-rdbc-candidate1';dest=base/'task7-ref-druid-candidate1';dest.mkdir()
shutil.copytree(previous/'src',dest/'src');shutil.copytree(previous/'resources',dest/'resources')
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
state={'production_applied':False,'commands':[]}
def run(name,command,env=None):
    log=dest/(name+'.log')
    with log.open('xb') as out:result=subprocess.run(command,env=env,stdout=out,stderr=subprocess.STDOUT,creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    state['commands'].append({'name':name,'command':command,'environment':None if env is None else {k:env[k] for k in ('CARGO_MANIFEST_DIR','CARGO_PKG_VERSION','CARGO_PKG_NAME','CARGO_PKG_RUST_VERSION')},'exit_code':result.returncode,'log':str(log),'log_sha256':sha(log)})
    (dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8');print(name,result.returncode,flush=True);return result.returncode
record=json.loads((previous/'summary.json').read_text(encoding='utf-8'));row=next(r for r in record['commands'] if r['name']=='after-library');command=row['command'].copy()
command[command.index(str(previous/'src/lib.rs'))]=str(dest/'src/lib.rs');command[command.index('-o')+1]=str(dest/'libdruid.rlib')
for i,arg in enumerate(command):
    if arg.startswith('druid_rdbc='):command[i]='druid_rdbc='+str(rdbc/'libdruid_rdbc.rlib')
    if arg.startswith('metadata='):command[i]='metadata=task7_ref_druid1'
env=os.environ.copy();env.update(row['environment']);env['CARGO_MANIFEST_DIR']=str(dest)
assert run('before-library',command,env)!=0
assert 'RdbcRefAccess' in (dest/'before-library.log').read_text(encoding='utf-8')
p=dest/'src/core/mod.rs';text=p.read_text(encoding='utf-8');assert text.count('RdbcRefAccess,')==1;text=text.replace('RdbcRefAccess,','');p.write_text(text,encoding='utf-8',newline='\n')
assert run('after-library',command,env)==0
original=base/'task7-pooled-struct-consumer1.rs';source=base/'task7-pooled-ref-consumer1.rs'
original_bytes=original.read_bytes();new=original_bytes+b'\nmod ref_contract {\n'+(base/'task7-ref-contract1.rs').read_bytes()+b'\n'+(base/'task7-ref-pooled-flow1.rs').read_bytes()+b'\n}\n';source.write_bytes(new)
assert new.startswith(original_bytes);state['prior_47_consumer_sha256']=sha(original);state['consumer_sha256']=sha(source)
consumer=next(r['command'] for r in record['commands'] if r['name']=='after-consumer-compile').copy();consumer[consumer.index(str(original))]=str(source);consumer[consumer.index('-o')+1]=str(dest/'consumer.exe')
for i,arg in enumerate(consumer):
    if arg.startswith('druid_rdbc='):consumer[i]='druid_rdbc='+str(rdbc/'libdruid_rdbc.rlib')
    if arg.startswith('druid='):consumer[i]='druid='+str(dest/'libdruid.rlib')
consumer+=['-L','dependency='+str(rdbc)]
assert run('consumer-compile',consumer)==0
assert run('consumer-runtime',[str(dest/'consumer.exe'),'--test-threads=1'])==0
state['sources']={p.relative_to(dest).as_posix():sha(p) for p in (dest/'src').rglob('*.rs')}
state['rdbc_sources']={p.relative_to(rdbc).as_posix():sha(p) for p in (rdbc/'src').rglob('*.rs')}
state['rlib_sha256']=sha(dest/'libdruid.rlib');state['exe_sha256']=sha(dest/'consumer.exe');state['terminal']=True
(dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
