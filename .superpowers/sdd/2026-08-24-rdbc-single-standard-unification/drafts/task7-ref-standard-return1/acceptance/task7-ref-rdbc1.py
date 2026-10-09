"""Retire the Ref Access driver port in complete F-only RDBC candidate, with actual RED/GREEN."""
import hashlib,json,shutil,subprocess
from pathlib import Path
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
old=base/'task7-struct-rdbc-candidate1';dest=base/'task7-ref-rdbc-candidate1';dest.mkdir()
source=base/'task7-ref-contract1.rs';sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
state={'production_applied':False,'commands':[],'consumer_sha256':sha(source)}
def run(name,command):
    log=dest/(name+'.log')
    with log.open('xb') as out:result=subprocess.run(command,stdout=out,stderr=subprocess.STDOUT,creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    state['commands'].append({'name':name,'command':command,'exit_code':result.returncode,'log':str(log),'log_sha256':sha(log)})
    (dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8');print(name,result.returncode,flush=True);return result.returncode
record=json.loads((old/'summary.json').read_text(encoding='utf-8'))
consumer=next(r['command'] for r in record['commands'] if r['name']=='green-compile').copy()
consumer[consumer.index(str(base/'task7-struct-contract1.rs'))]=str(source)
consumer[consumer.index('-o')+1]=str(dest/'red.exe')
assert run('red-consumer-compile',consumer)!=0
red=(dest/'red-consumer-compile.log').read_text(encoding='utf-8');assert 'E0308' in red and 'RdbcRefAccess' in red and 'RdbcStruct' in red
shutil.copytree(old/'src',dest/'src')
shutil.copyfile(base/'task7-ref-handle1.rs',dest/'src/resource/rdbc_ref.rs')
shutil.copyfile(base/'task7-struct-handle1.rs',dest/'src/resource/rdbc_struct.rs')
# Delete the obsolete second driver interface, rather than retaining an adapter to it.
(dest/'src/resource/rdbc_ref_access.rs').unlink()
p=dest/'src/resource/mod.rs';text=p.read_text(encoding='utf-8').replace('mod rdbc_ref_access;\n','').replace('pub use rdbc_ref_access::RdbcRefAccess;\n','')
text+='\nmod rdbc_struct;\npub use rdbc_struct::RdbcStruct;\n';p.write_text(text,encoding='utf-8',newline='\n')
p=dest/'src/resource/rdbc_resource_factory.rs';text=p.read_text(encoding='utf-8').replace('RdbcRef, RdbcRefAccess,','RdbcRef, Ref, RdbcStruct, Struct,')
start=text.index('    /// Creates a detached `Ref` handle.');end=text.index('    /// Creates a detached `SQLXML` handle.',start)
text=text[:start]+'''    /// 将独立拥有的标准Ref装入共享值句柄，参数为唯一驱动对象，返回保持引用身份的句柄。
    /// 对应 Java: java.sql.Ref；不物化属性，不新增free或连接创建接口。
    #[must_use]
    pub fn reference(value: Box<dyn Ref>) -> RdbcRef {
        RdbcRef::from_standard(value)
    }

    /// 将独立拥有的标准Struct装入共享值句柄，参数为唯一驱动对象，返回延迟读取属性的句柄。
    /// 对应 Java: java.sql.Struct；用于getObject及嵌套属性，不新增驱动端口。
    #[must_use]
    pub fn structure(value: Box<dyn Struct>) -> RdbcStruct {
        RdbcStruct::from_standard(value)
    }

'''+text[end:];p.write_text(text,encoding='utf-8',newline='\n')
p=dest/'src/types/rdbc_object.rs';text=p.read_text(encoding='utf-8').replace('RdbcRef, RdbcSqlXml','RdbcRef, RdbcStruct, RdbcSqlXml')
text=text.replace('    Ref(RdbcRef),','    Ref(RdbcRef),\n    /// SQL结构类型对象；对应 Java: java.sql.Struct，保留引用身份及延迟属性读取。\n    Struct(RdbcStruct),')
text=text.replace('            Self::Ref(_) => formatter.write_str("<Ref>"),','            Self::Ref(_) => formatter.write_str("<Ref>"),\n            Self::Struct(_) => formatter.write_str("<Struct>"),')
p.write_text(text,encoding='utf-8',newline='\n')
p=dest/'src/resource/ref_value.rs';text=p.read_text(encoding='utf-8').replace('/// 对象独立拥有，仅要求Send，不要求Sync。各方法返回实际值或数据库错误。','''/// 对象独立拥有，仅要求Send，不要求Sync。SQL REF是结构值的逻辑指针，取得引用不物化目标属性。
/// 支持该数据类型的驱动必须实现全部四个JDBC方法；不继承Wrapper，没有free或Connection#createRef。
/// 引用可作为列值或结构属性，也可通过PreparedStatement#setRef写入数据库。各方法返回实际值或数据库错误。''')
text=text.replace('/// 对应 Java: Ref#getObject()；按默认映射读取目标对象。','/// 对应 Java: Ref#getObject()；按连接关联映射读取目标，无匹配映射时返回Struct对象。')
text=text.replace('/// 对应 Java: Ref#getObject(Map)；显式映射读取目标对象。','/// 对应 Java: Ref#getObject(Map)；使用参数type_map的完整SQL结构类型名与SQLData目标映射读取对象。')
p.write_text(text,encoding='utf-8',newline='\n')
library=next(r['command'] for r in record['commands'] if r['name']=='full-rdbc-compile').copy()
library[library.index(str(old/'src/lib.rs'))]=str(dest/'src/lib.rs');library[library.index('--out-dir')+1]=str(dest)
for i,arg in enumerate(library):
    if arg.startswith('metadata='):library[i]='metadata=task7_ref_rdbc1'
assert run('full-rdbc-compile',library)==0
consumer[consumer.index('-o')+1]=str(dest/'green.exe')
for i,arg in enumerate(consumer):
    if arg.startswith('druid_rdbc='):consumer[i]='druid_rdbc='+str(dest/'libdruid_rdbc.rlib')
assert run('green-consumer-compile',consumer)==0
assert run('green-runtime',[str(dest/'green.exe'),'--test-threads=1'])==0
state['sources']={p.relative_to(dest).as_posix():sha(p) for p in (dest/'src').rglob('*.rs')}
acquisition=json.loads((base/'task8-java-jdbc42-source-acquisition1.summary.json').read_text(encoding='utf-8'))
state['java_sources']={}
for row in acquisition['sources']:
    if row['fqcn'] in ('java.sql.Ref','java.sql.Struct'):
        assert sha(Path(row['local_path']))==row['sha256'];state['java_sources'][row['fqcn']]=row
assert len(state['java_sources'])==2
state['terminal']=True
(dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
