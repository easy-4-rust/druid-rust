"""Preserve target admission precedence independently of the two-resource lock order."""
import hashlib,json,os,shutil,subprocess
from pathlib import Path
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008');oldr=base/'task7-blob-value-rdbc-candidate1';oldd=base/'task7-blob-value-druid-candidate1';rdbc=base/'task7-blob-value-rdbc-candidate2';dest=base/'task7-blob-value-druid-candidate2';rdbc.mkdir();dest.mkdir();sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
original=json.loads((oldd/'summary.json').read_text(encoding='utf-8'));extra=json.loads((oldd/'extra.summary.json').read_text(encoding='utf-8'));assert original['terminal'] and extra['terminal'];state={'production_applied':False,'commands':[],'terminal':False}
def run(name,command,env=None):
    log=dest/(name+'.log')
    with log.open('xb') as out:result=subprocess.run(command,env=env,stdout=out,stderr=subprocess.STDOUT,creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    state['commands'].append({'name':name,'command':command,'exit_code':result.returncode,'log':str(log),'log_sha256':sha(log)})
    (dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8');print(name,result.returncode,flush=True);return result.returncode
prior=base/'task7-pooled-blob-value-consumer3.rs';source=base/'task7-pooled-blob-value-consumer4.rs';source.write_bytes(prior.read_bytes()+b'\n'+(base/'task7-blob-value-priority1.rs').read_bytes());consumer=extra['commands'][0]['command'].copy();consumer[consumer.index(str(prior))]=str(source);consumer[consumer.index('-o')+1]=str(dest/'red.exe')
assert run('precedence-red-compile',consumer)==0
assert run('precedence-red-runtime',[str(dest/'red.exe'),'task7_blob_value_target_capability_precedes_closed_pattern_for_both_orders','--exact','--test-threads=1'])!=0
shutil.copytree(oldr/'src',rdbc/'src');shutil.copytree(oldd/'src',dest/'src');shutil.copytree(oldd/'resources',dest/'resources')
p=rdbc/'src/resource/standard_resource_value.rs';text=p.read_text(encoding='utf-8');needle='    fn require(&self, capability:ResourceOperations, operation:&\'static str)';assert text.count(needle)==1;text=text.replace(needle,'    /// 检查参数capability/operation的目标准入条件，返回成功或原生命周期/能力错误。\n    pub(crate) fn require(&self, capability:ResourceOperations, operation:&\'static str)');p.write_text(text,encoding='utf-8',newline='\n')
p=rdbc/'src/resource/rdbc_blob.rs';text=p.read_text(encoding='utf-8');needle='        let operation = "Blob#position(Blob,long)";\n';assert text.count(needle)==1;text=text.replace(needle,needle+'        // 先检查目标，避免模式的锁序改变目标关闭/能力错误的优先级。\n        self.value.require(required, operation)?;\n');p.write_text(text,encoding='utf-8',newline='\n')
assert run('format-priority-objects',['C:/Users/hnxyh/.rustup/toolchains/1.97.1-x86_64-pc-windows-msvc/bin/rustfmt.exe','--edition','2021',str(rdbc/'src/resource/standard_resource_value.rs'),str(rdbc/'src/resource/rdbc_blob.rs')])==0
library=next(row['command'] for row in original['commands'] if row['name']=='full-rdbc-compile').copy();library[library.index(str(oldr/'src/lib.rs'))]=str(rdbc/'src/lib.rs');library[library.index('--out-dir')+1]=str(rdbc)
for i,arg in enumerate(library):
    if arg.startswith('metadata='):library[i]='metadata=task7_blob_value_rdbc2'
assert run('full-rdbc-compile',library)==0
library=next(row['command'] for row in original['commands'] if row['name']=='full-druid-compile').copy();library[library.index(str(oldd/'src/lib.rs'))]=str(dest/'src/lib.rs');library[library.index('-o')+1]=str(dest/'libdruid.rlib')
for i,arg in enumerate(library):
    if arg.startswith('druid_rdbc='):library[i]='druid_rdbc='+str(rdbc/'libdruid_rdbc.rlib')
    if arg.startswith('metadata='):library[i]='metadata=task7_blob_value_druid2'
env=os.environ.copy();env.update(original['druid_environment']);env['CARGO_MANIFEST_DIR']=str(dest);state['druid_environment']=original['druid_environment']|{'CARGO_MANIFEST_DIR':str(dest)}
assert run('full-druid-compile',library,env)==0
consumer[consumer.index('-o')+1]=str(dest/'consumer.exe')
for i,arg in enumerate(consumer):
    if arg.startswith('druid_rdbc='):consumer[i]='druid_rdbc='+str(rdbc/'libdruid_rdbc.rlib')
    if arg.startswith('druid='):consumer[i]='druid='+str(dest/'libdruid.rlib')
consumer+=['-L','dependency='+str(rdbc)]
assert run('consumer-compile',consumer)==0
assert run('consumer-runtime',[str(dest/'consumer.exe'),'--test-threads=1'])==0
state['sources']={p.relative_to(dest).as_posix():sha(p) for p in (dest/'src').rglob('*.rs')};state['rdbc_sources']={p.relative_to(rdbc).as_posix():sha(p) for p in (rdbc/'src').rglob('*.rs')};state['consumer_sha256']=sha(source);state['rlib_sha256']=sha(dest/'libdruid.rlib');state['rdbc_rlib_sha256']=sha(rdbc/'libdruid_rdbc.rlib');state['exe_sha256']=sha(dest/'consumer.exe');state['terminal']=True
(dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
