"""Actual RED followed by complete RDBC source candidate and identical consumers."""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
dest=base/'task7-struct-rdbc-candidate1';dest.mkdir()
source=base/'task7-struct-contract1.rs'
state={'production_applied':False,'source_sha256':hashlib.sha256(source.read_bytes()).hexdigest(),'commands':[]}
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
def run(name,command):
    log=dest/(name+'.log')
    with log.open('xb') as out: result=subprocess.run(command,stdout=out,stderr=subprocess.STDOUT,creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    state['commands'].append({'name':name,'command':command,'exit_code':result.returncode,'log':str(log),'log_sha256':sha(log)})
    (dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
    print(json.dumps({'name':name,'exit_code':result.returncode}),flush=True)
    return result.returncode
old=base/'task7-array-baseline-coherent1'
template=json.loads((old/'array-red2.summary.json').read_text(encoding='utf-8'))['commands'][0]['command']
command=[];skip=False
for i,arg in enumerate(template):
    if skip:skip=False;continue
    if arg=='--extern' and template[i+1].startswith('druid='):skip=True;continue
    command.append(str(source) if arg.endswith('task7-pooled-array-consumer2.rs') else arg)
command[command.index('-o')+1]=str(dest/'red.exe')
assert run('red-compile',command)!=0
red=(dest/'red-compile.log').read_text(encoding='utf-8');assert 'E0432' in red and 'E0407' in red and 'Struct' in red and 'create_struct' in red
shutil.copytree(base/'task7-rdbc-source-candidate1/src',dest/'src')
trait='''use crate::error::RdbcError;
use crate::resource::ResourceOperations;
use crate::types::{RdbcObject, RdbcTypeMap};

/// SQL结构类型的独占标准对象；对应 Java: java.sql.Struct。
/// 属性按声明顺序保留，包括SQL NULL及嵌套对象；支持此类型的驱动必须实现全部三个方法。
/// 默认有效期持续到应用不再持有引用，不附加连接关闭失效或free方法；只要求Send，不要求Sync。
#[async_trait::async_trait]
pub trait Struct: std::fmt::Debug + Send + 'static {
    /// 返回当前对象真实支持的操作集合（Rust扩展），无参数。
    fn capabilities(&self) -> ResourceOperations;
    /// 返回完整限定SQL类型名，无参数；数据库访问或不支持时返回错误。
    /// 对应 Java: Struct#getSQLTypeName。
    async fn sql_type_name(&mut self) -> Result<String, RdbcError>;
    /// 按声明顺序返回属性对象，无参数；使用连接关联的类型映射，无匹配项时使用标准映射。
    /// 概念上逐属性执行getObject；数据库访问或不支持时返回错误。对应 Java: Struct#getAttributes()。
    async fn attributes(&mut self) -> Result<Vec<RdbcObject>, RdbcError>;
    /// 使用参数type_map显式映射属性，返回声明顺序的对象；无匹配项时使用标准映射。
    /// 此重载绝不使用连接的类型映射；数据库访问或不支持时返回错误。
    /// 对应 Java: Struct#getAttributes(Map)。
    async fn attributes_with_type_map(&mut self, type_map: &RdbcTypeMap) -> Result<Vec<RdbcObject>, RdbcError>;
}
'''
(dest/'src/resource/struct_value.rs').write_text(trait,encoding='utf-8',newline='\n')
path=dest/'src/resource/mod.rs';text=path.read_text(encoding='utf-8');text+='\nmod struct_value;\npub use struct_value::Struct;\n';path.write_text(text,encoding='utf-8',newline='\n')
path=dest/'src/connection/connection.rs';text=path.read_text(encoding='utf-8')
marker='pub trait Connection: Wrapper + Send + \'static {';assert text.count(marker)==1
method='''
    /// 按参数type_name与有序attributes创建独立拥有的SQL结构对象，返回对象或驱动错误。
    /// 对应 Java: Connection#createStruct；None保留原nullable输入供驱动验证。
    /// 类型名为NULL、数据库错误或连接已关闭时返回SQLException；不支持该类型时明确报错。
    async fn create_struct(
        &mut self,
        _type_name: Option<&str>,
        _attributes: Option<Vec<crate::types::RdbcObject>>,
    ) -> Result<Box<dyn crate::resource::Struct>, RdbcError> {
        Err(RdbcError::feature_not_supported("Connection#create_struct"))
    }
'''
path.write_text(text.replace(marker,marker+method,1),encoding='utf-8',newline='\n')
baseline=json.loads((old/'summary.json').read_text(encoding='utf-8'))
library=next(r['command'] for r in baseline['commands'] if r['name']=='rdbc').copy()
library[library.index(str(base/'task7-rdbc-source-candidate1/src/lib.rs'))]=str(dest/'src/lib.rs')
library[library.index('--out-dir')+1]=str(dest)
for i,arg in enumerate(library):
    if arg.startswith('metadata='):library[i]='metadata=task7_struct_rdbc1'
assert run('full-rdbc-compile',library)==0
green=command.copy();green[green.index('-o')+1]=str(dest/'green.exe')
for i,arg in enumerate(green):
    if arg.startswith('druid_rdbc='):green[i]='druid_rdbc='+str(dest/'libdruid_rdbc.rlib')
assert run('green-compile',green)==0
assert run('green-runtime',[str(dest/'green.exe'),'--test-threads=1'])==0
state['sources']={p.relative_to(dest).as_posix():sha(p) for p in (dest/'src').rglob('*.rs')}
acquisition=json.loads((base/'task8-java-jdbc42-source-acquisition1.summary.json').read_text(encoding='utf-8'))
state['java_sources']={}
for row in acquisition['sources']:
    if row['fqcn'] in ('java.sql.Struct','java.sql.Connection'):
        assert sha(Path(row['local_path']))==row['sha256'];state['java_sources'][row['fqcn']]=row
assert len(state['java_sources'])==2
state['terminal']=True
(dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
