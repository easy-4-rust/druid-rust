"""Full source candidate; preserve old-consumer RED before applying Access retirement."""
import hashlib,json,os,shutil,subprocess
from pathlib import Path
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
oldr=base/'task7-ref-rdbc-candidate1';oldd=base/'task7-ref-druid-candidate1'
rdbc=base/'task7-array-value-rdbc-candidate1';dest=base/'task7-array-value-druid-candidate1'
rdbc.mkdir();dest.mkdir()
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
state={'production_applied':False,'commands':[],'terminal':False}
def run(name,command,env=None):
    log=dest/(name+'.log')
    with log.open('xb') as out:result=subprocess.run(command,env=env,stdout=out,stderr=subprocess.STDOUT,creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    state['commands'].append({'name':name,'command':command,'exit_code':result.returncode,'log':str(log),'log_sha256':sha(log)})
    (dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8');print(name,result.returncode,flush=True);return result.returncode
prior=base/'task7-pooled-ref-consumer2.rs';source=base/'task7-pooled-array-value-consumer1.rs'
source.write_bytes(prior.read_bytes()+b'\n'+(base/'task7-array-value-contract1.rs').read_bytes())
oldrecord=json.loads((oldd/'contract2.summary.json').read_text(encoding='utf-8'))
consumer=oldrecord['commands'][0]['command'].copy();consumer[consumer.index(str(prior))]=str(source);consumer[consumer.index('-o')+1]=str(dest/'consumer.exe')
assert run('old-array-value-consumer-red',consumer)!=0
assert 'E0308' in (dest/'old-array-value-consumer-red.log').read_text(encoding='utf-8')
shutil.copytree(oldr/'src',rdbc/'src');shutil.copytree(oldd/'src',dest/'src');shutil.copytree(oldd/'resources',dest/'resources')
resource=rdbc/'src/resource'
for template,target in [('task7-array-handle1.rs','rdbc_array.rs'),('task7-shared-resource-value1.rs','standard_resource_value.rs'),('task7-shared-resource-release-guard1.rs','standard_resource_release_guard.rs')]:shutil.copyfile(base/template,resource/target)
p=resource/'standard_resource_value.rs';text=p.read_text(encoding='utf-8');text=text.replace('free(&self, release:','free(&self, operation: &\'static str, release:').replace('ResourceOperations::FREE,"resource#free"','ResourceOperations::FREE,operation');p.write_text(text,encoding='utf-8',newline='\n')
p=resource/'rdbc_array.rs';text=p.read_text(encoding='utf-8').replace('self.value.free(|value|value.free())','self.value.free("Array#free",|value|value.free())');p.write_text(text,encoding='utf-8',newline='\n')
(resource/'rdbc_array_access.rs').unlink()
p=resource/'mod.rs';text=p.read_text(encoding='utf-8');text=text.replace('mod rdbc_array_access;\n','').replace('pub use rdbc_array_access::RdbcArrayAccess;\n','');text+='\nmod standard_resource_value;\nmod standard_resource_release_guard;\n';p.write_text(text,encoding='utf-8',newline='\n')
p=resource/'rdbc_resource_factory.rs';text=p.read_text(encoding='utf-8').replace('RdbcArray, RdbcArrayAccess,','Array, RdbcArray,');start=text.index('    /// Creates a detached `Array` handle.');end=text.index('    /// Creates a detached `Blob` handle.',start)
text=text[:start]+'''    /// 将唯一标准Array对象装入共享值句柄，参数为独立拥有的驱动对象，返回保留身份及释放状态的句柄。
    /// 对应 Java: java.sql.Array；范围、映射和游标直接委托，不增加驱动端口。
    #[must_use]
    pub fn array(value: Box<dyn Array>) -> RdbcArray {
        RdbcArray::from_standard(value)
    }

'''+text[end:];p.write_text(text,encoding='utf-8',newline='\n')
p=resource/'rdbc_resource_kind.rs';text=p.read_text(encoding='utf-8').replace('    /// An SQL `ARRAY` resource.\n    Array,\n','').replace('            Self::Array => "Array",\n','');p.write_text(text,encoding='utf-8',newline='\n')
record=json.loads((oldr/'summary.json').read_text(encoding='utf-8'));library=next(r['command'] for r in record['commands'] if r['name']=='full-rdbc-compile3').copy()
library[library.index(str(oldr/'src/lib.rs'))]=str(rdbc/'src/lib.rs');library[library.index('-o')+1]=str(rdbc/'libdruid_rdbc.rlib')
for i,arg in enumerate(library):
    if arg.startswith('metadata='):library[i]='metadata=task7_array_value_rdbc1'
assert run('full-rdbc-compile',library)==0
record=json.loads((oldd/'summary.json').read_text(encoding='utf-8'));row=next(r for r in record['commands'] if r['name']=='after-library');library=row['command'].copy()
library[library.index(str(oldd/'src/lib.rs'))]=str(dest/'src/lib.rs');library[library.index('-o')+1]=str(dest/'libdruid.rlib')
for i,arg in enumerate(library):
    if arg.startswith('druid_rdbc='):library[i]='druid_rdbc='+str(rdbc/'libdruid_rdbc.rlib')
    if arg.startswith('metadata='):library[i]='metadata=task7_array_value_druid1'
env=os.environ.copy();env.update(row['environment']);env['CARGO_MANIFEST_DIR']=str(dest);state['druid_environment']=row['environment']|{'CARGO_MANIFEST_DIR':str(dest)}
assert run('full-druid-old-export-red',library,env)!=0
assert 'RdbcArrayAccess' in (dest/'full-druid-old-export-red.log').read_text(encoding='utf-8')
p=dest/'src/core/mod.rs';text=p.read_text(encoding='utf-8');assert text.count('RdbcArrayAccess,')==1;text=text.replace('RdbcArrayAccess,','');p.write_text(text,encoding='utf-8',newline='\n')
assert run('full-druid-compile',library,env)==0
for i,arg in enumerate(consumer):
    if arg.startswith('druid_rdbc='):consumer[i]='druid_rdbc='+str(rdbc/'libdruid_rdbc.rlib')
    if arg.startswith('druid='):consumer[i]='druid='+str(dest/'libdruid.rlib')
consumer+=['-L','dependency='+str(rdbc)]
assert run('consumer-compile',consumer)==0
assert run('consumer-runtime',[str(dest/'consumer.exe'),'--test-threads=1'])==0
state['sources']={p.relative_to(dest).as_posix():sha(p) for p in (dest/'src').rglob('*.rs')};state['rdbc_sources']={p.relative_to(rdbc).as_posix():sha(p) for p in (rdbc/'src').rglob('*.rs')}
state['rlib_sha256']=sha(dest/'libdruid.rlib');state['rdbc_rlib_sha256']=sha(rdbc/'libdruid_rdbc.rlib');state['consumer_sha256']=sha(source);state['prior_58_sha256']=sha(prior);state['exe_sha256']=sha(dest/'consumer.exe');state['terminal']=True
(dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
