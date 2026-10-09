"""Rebuild both complete Druid variants with the same new RDBC crate."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
previous=base/'task7-array-source-candidate1';rdbc=base/'task7-struct-rdbc-candidate1'
old=base/'task7-struct-druid-baseline1';candidate=base/'task7-struct-druid-candidate1'
for dest in (old,candidate):
    dest.mkdir();shutil.copytree(previous/'src',dest/'src');shutil.copytree(previous/'resources',dest/'resources')
state={'production_applied':False,'commands':[],'source_sha256':hashlib.sha256((base/'task7-pooled-struct-consumer1.rs').read_bytes()).hexdigest()}
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def run(name,command,env=None):
    log=candidate/(name+'.log')
    with log.open('xb') as out:result=subprocess.run(command,env=env,stdout=out,stderr=subprocess.STDOUT,creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    state['commands'].append({'name':name,'command':command,'environment':env if env is None else {k:env[k] for k in ('CARGO_MANIFEST_DIR','CARGO_PKG_VERSION','CARGO_PKG_NAME','CARGO_PKG_RUST_VERSION')},'exit_code':result.returncode,'log':str(log),'log_sha256':sha(log)})
    (candidate/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8');print(json.dumps({'name':name,'exit_code':result.returncode}),flush=True)
    return result.returncode
record=json.loads((previous/'compile2.summary.json').read_text(encoding='utf-8'))
def library(dest,label):
    command=record['command'].copy();command[command.index(str(previous/'src/lib.rs'))]=str(dest/'src/lib.rs');command[command.index('-o')+1]=str(dest/'libdruid.rlib')
    for i,arg in enumerate(command):
        if arg.startswith('druid_rdbc='):command[i]='druid_rdbc='+str(rdbc/'libdruid_rdbc.rlib')
        if arg.startswith('metadata='):command[i]='metadata=task7_struct_druid_'+label
    env=os.environ.copy();env.update(record['environment']);env['CARGO_MANIFEST_DIR']=str(dest)
    assert run(label+'-library',command,env)==0
def consumer(dest,label):
    contract=json.loads((previous/'contract6.summary.json').read_text(encoding='utf-8'))
    command=next(r['command'] for r in contract['commands'] if r['name']=='after6-compile').copy()
    command[command.index(str(base/'task7-pooled-array-consumer6.rs'))]=str(base/'task7-pooled-struct-consumer1.rs')
    for i,arg in enumerate(command):
        if arg.startswith('druid_rdbc='):command[i]='druid_rdbc='+str(rdbc/'libdruid_rdbc.rlib')
        if arg.startswith('druid='):command[i]='druid='+str(dest/'libdruid.rlib')
    command+=['-L','dependency='+str(rdbc)]
    exe=candidate/(label+'.exe');command[command.index('-o')+1]=str(exe)
    assert run(label+'-consumer-compile',command)==0
    return run(label+'-runtime',[str(exe),'--test-threads=1'])
library(old,'before')
assert consumer(old,'before')==101
method='''
    /// 创建独立拥有的SQL结构对象，保留nullable名称与有序属性；返回原驱动对象或分类错误。
    /// 对应 Java: DruidPooledConnection#createStruct。Struct无free且默认随应用引用有效，不能加入LOB释放状态机。
    async fn create_struct(
        &mut self,
        type_name: Option<&str>,
        attributes: Option<Vec<druid_rdbc::types::RdbcObject>>,
    ) -> Result<Box<dyn druid_rdbc::resource::Struct>, RdbcError> {
        self.standard_ref()?;
        let result = self.standard_mut()?.create_struct(type_name, attributes).await;
        self.classify_result(result)
    }
'''
path=candidate/'src/core/druid_pooled_connection.rs';text=path.read_text(encoding='utf-8');marker='    async fn create_array_of('
assert text.count(marker)==1
# Put the new method before the existing factory's doc comments.
start=text.rfind('    ///',0,text.index(marker));start=text.rfind('    ///',0,start)
text=text[:start]+method+'\n'+text[start:];path.write_text(text,encoding='utf-8',newline='\n')
path=candidate/'src/core/external_connection_lease.rs';text=path.read_text(encoding='utf-8');marker='    async fn create_array_of('
assert text.count(marker)==1
method='''
    /// 将SQL结构创建的nullable类型名与有序属性原样交给外部池连接；返回独立拥有的对象或原错误。
    /// 对应 Java: Connection#createStruct，不引入第二层资源所有者。
    async fn create_struct(
        &mut self,
        type_name: Option<&str>,
        attributes: Option<Vec<druid_rdbc::types::RdbcObject>>,
    ) -> Result<Box<dyn druid_rdbc::resource::Struct>, RdbcError> {
        self.connection_mut().create_struct(type_name, attributes).await
    }
'''
text=text.replace(marker,method+'\n'+marker,1);path.write_text(text,encoding='utf-8',newline='\n')
library(candidate,'after')
assert consumer(candidate,'after')==0
state['sources']={p.relative_to(candidate).as_posix():sha(p) for p in (candidate/'src').rglob('*.rs')}
state['rdbc_sources']={p.relative_to(rdbc).as_posix():sha(p) for p in (rdbc/'src').rglob('*.rs')}
state['terminal']=True
(candidate/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
