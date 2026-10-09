"""Retire the final legacy resource access/context using full standard XML source candidates."""
import hashlib,json,os,shutil,subprocess
from pathlib import Path
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008');oldr=base/'task7-clob-value-rdbc-candidate1';oldd=base/'task7-clob-value-druid-candidate1';rdbc=base/'task7-xml-value-rdbc-candidate1';dest=base/'task7-xml-value-druid-candidate1'
rdbc.mkdir();dest.mkdir();sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();original=json.loads((oldd/'summary.json').read_text(encoding='utf-8'));extra=json.loads((oldd/'extra.summary.json').read_text(encoding='utf-8'));assert original['terminal']and extra['terminal'];state={'terminal':False,'production_applied':False,'commands':[]}
def run(name,command,env=None):
    log=dest/(name+'.log')
    with log.open('xb')as out:result=subprocess.run(command,env=env,stdout=out,stderr=subprocess.STDOUT,creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    state['commands'].append({'name':name,'command':command,'exit_code':result.returncode,'log':str(log),'log_sha256':sha(log)});(dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8');print(name,result.returncode,flush=True);return result.returncode
prior=base/'task7-pooled-clob-value-consumer3.rs';source=base/'task7-pooled-xml-value-consumer1.rs';source.write_bytes(prior.read_bytes()+b'\n'+(base/'task7-xml-value-contract1.rs').read_bytes());state['prior_consumer_sha256']=sha(prior)
consumer=extra['commands'][0]['command'].copy();consumer[consumer.index(str(prior))]=str(source);consumer[consumer.index('-o')+1]=str(dest/'consumer.exe');assert run('old-xml-value-consumer-red',consumer)!=0
red=(dest/'old-xml-value-consumer-red.log').read_text(encoding='utf-8');assert 'E0308' in red and 'RdbcSqlXmlAccess' in red
shutil.copytree(oldr/'src',rdbc/'src');shutil.copytree(oldd/'src',dest/'src');shutil.copytree(oldd/'resources',dest/'resources');resource=rdbc/'src/resource'
payload='''use crate::error::{RdbcError,SqlException};
use crate::resource::SqlXml;
/// 标准SQLXML独占对象与一次性读写机会（Rust平台映射）；对应 Java: java.sql.SQLXML。
/// 仅保留Send驱动及概念状态，不定义第二套驱动接口，不持有owner或后台任务。
pub(crate) struct SqlXmlValue {pub(crate) driver:Box<dyn SqlXml>,readable:bool,writable:bool}
impl SqlXmlValue {
    /// 接管参数driver，返回尚未消费读写机会的状态；驱动仍决定原始可用方向。
    pub(crate) fn new(driver:Box<dyn SqlXml>)->Self{Self{driver,readable:true,writable:true}}
    /// 根据参数read选择方向，首次消费返回成功；再次返回HY010，失败或取消不恢复机会。
    pub(crate) fn admit(&mut self,read:bool)->Result<(),RdbcError>{
        let available=if read{&mut self.readable}else{&mut self.writable};
        if !*available{return Err(RdbcError::SqlException(Box::new(SqlException::new(0,Some("HY010".to_owned()),Some(format!("SQLXML is no longer {}",if read{"readable"}else{"writable"}))))));}
        // 在任何驱动await之前消费；调用者取消不能重放同一方向的操作。
        *available=false;Ok(())
    }
}
''';(resource/'sql_xml_value.rs').write_text(payload,encoding='utf-8',newline='\n')
methods=[
('binary_stream','','RdbcInputStream',True,'READ|STREAM','SQLXML#getBinaryStream','','返回原驱动二进制读取流，无参数；不提前读取或编码。'),
('set_binary_stream','','RdbcOutputStream',False,'WRITE|STREAM','SQLXML#setBinaryStream','','返回原驱动二进制写入流，无参数；绑定前由调用者完成流关闭。'),
('character_stream','','RdbcReader',True,'READ|STREAM','SQLXML#getCharacterStream','','返回保留UTF-16的原字符读取流，无参数。'),
('set_character_stream','','RdbcWriter',False,'WRITE|STREAM','SQLXML#setCharacterStream','','返回原驱动字符写入器，无参数；调用者负责完成关闭。'),
('string','','RdbcString',True,'READ','SQLXML#getString','','返回原XML文本并消费读取机会，无参数；不替换孤立代理项。'),
('set_string','value:&RdbcString','()',False,'WRITE','SQLXML#setString','value','设置参数value的原UTF-16文本并消费写入机会，返回成功或原错误。'),
('source',"representation:Option<&RdbcXmlRepresentationType>",'RdbcXmlSource',True,'READ','SQLXML#getSource','representation','按参数representation返回原Source；None保留Java null的厂商默认请求。'),
('result',"representation:Option<&RdbcXmlRepresentationType>",'RdbcXmlResult',False,'WRITE','SQLXML#setResult','representation','按参数representation返回原Result；None保留Java null，不改写框架身份。'),
]
head='''use super::sql_xml_value::SqlXmlValue;
use super::standard_resource_value::StandardResourceValue;
use crate::error::RdbcError;
use crate::resource::{SqlXml,RdbcInputStream,RdbcOutputStream,RdbcReader,RdbcWriter,RdbcXmlRepresentationType,RdbcXmlSource,RdbcXmlResult,ResourceOperations};
use crate::types::RdbcString;
use std::{fmt,sync::Arc};
/// SQLXML共享值句柄；对应 Java: java.sql.SQLXML。
/// 克隆共享同一标准对象、一次性读写状态、身份与释放确认；只要求驱动Send。
#[derive(Clone)]
pub struct RdbcSqlXml{value:Arc<StandardResourceValue<SqlXmlValue>>}
impl RdbcSqlXml{
    /// 接管参数value唯一标准SQLXML，返回未提前读取或写入的共享句柄。
    pub(crate) fn from_standard(value:Box<dyn SqlXml>)->Self{let capabilities=value.capabilities()&ResourceOperations::sql_xml();Self{value:Arc::new(StandardResourceValue::new(Box::new(SqlXmlValue::new(value)),capabilities,"SQLXML"))}}
    async fn payload(&self,read:bool,capability:ResourceOperations,operation:&'static str)->Result<tokio::sync::MutexGuard<'_,Box<SqlXmlValue>>,RdbcError>{
        let mut payload=self.value.lock(capability,operation).await?;
        // 生命周期及能力已在锁前后检查；仅真正取得驱动独占权后消费一次性机会。
        payload.admit(read)?;Ok(payload)
    }
'''
for name,args,ret,read,caps,op,call,doc in methods:
    flags=' | '.join('ResourceOperations::'+c for c in caps.split('|'));head+=f'    /// {doc} 对应 Java: {op}。\n    pub async fn {name}(&self{", "if args else ""}{args})->Result<{ret},RdbcError>{{self.payload({str(read).lower()},{flags},"{op}").await?.driver.{name}({call}).await}}\n'
for alias,name,ret in [('get_binary_stream','binary_stream','RdbcInputStream'),('get_character_stream','character_stream','RdbcReader'),('get_string','string','RdbcString')]:head+=f'    /// 对应Java getter命名的同一{name}入口，无参数；返回原驱动值并共享一次性读取机会。\n    pub async fn {alias}(&self)->Result<{ret},RdbcError>{{self.{name}().await}}\n'
head+='''    /// 幂等释放，无参数；失败或取消不误报成功，后续free重试确认。对应 Java: SQLXML#free。
    pub async fn free(&self)->Result<(),RdbcError>{self.value.free("SQLXML#free",|payload|payload.driver.free()).await}
    /// 返回是否已确认成功释放，无参数（Rust扩展）。
    #[must_use]pub fn is_freed(&self)->bool{self.value.is_freed()}
    /// 返回稳定共享身份，无参数（Rust扩展）。
    #[must_use]pub fn resource_id(&self)->&str{self.value.resource_id()}
    /// 返回驱动声明与SQLXML标准操作的交集，无参数（Rust扩展）。
    #[must_use]pub fn capabilities(&self)->ResourceOperations{self.value.capabilities()}
}
impl fmt::Debug for RdbcSqlXml{fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result{f.debug_struct("RdbcSqlXml").field("value",&self.value).finish_non_exhaustive()}}
impl PartialEq for RdbcSqlXml{fn eq(&self,other:&Self)->bool{Arc::ptr_eq(&self.value,&other.value)}}
impl Eq for RdbcSqlXml{}
''';(resource/'rdbc_sql_xml.rs').write_text(head,encoding='utf-8',newline='\n')
removed={'rdbc_sql_xml_access':'pub use rdbc_sql_xml_access::RdbcSqlXmlAccess;','rdbc_resource_access':'pub use rdbc_resource_access::RdbcResourceAccess;','rdbc_resource_context':'pub(crate) use rdbc_resource_context::ResourceLifecycle;','rdbc_resource_kind':'pub(crate) use rdbc_resource_kind::ResourceType;','rdbc_resource_state':'pub(crate) use rdbc_resource_state::ResourceLifecycleState;'}
p=resource/'mod.rs';text=p.read_text(encoding='utf-8')
for name,export in removed.items():
    (resource/(name+'.rs')).unlink();assert text.count('mod '+name+';')==1 and text.count(export)==1;text=text.replace('mod '+name+';\n','').replace(export+'\n','')
text+='\nmod sql_xml_value;\n';p.write_text(text,encoding='utf-8',newline='\n')
p=resource/'rdbc_resource_factory.rs';text=p.read_text(encoding='utf-8');text=text.replace('use crate::error::RdbcError;\n','').replace('use std::sync::Arc;\n','').replace('Struct, RdbcResourceAccess, RdbcSqlXml, RdbcSqlXmlAccess,','Struct, RdbcSqlXml, SqlXml,').replace('    ResourceLifecycle, ResourceType,\n','').replace('#[allow(dead_code)] // Context-bearing constructors are reserved for Task 7 standard resources.\n','')
start=text.index('    /// Creates a detached `SQLXML` handle.');text=text[:start]+'''    /// 接管参数value唯一标准SQLXML，返回共享身份、一次性读写和释放确认的句柄。对应 Java: java.sql.SQLXML。
    #[must_use]pub fn sql_xml(value:Box<dyn SqlXml>)->RdbcSqlXml{RdbcSqlXml::from_standard(value)}
}
''';p.write_text(text,encoding='utf-8',newline='\n')
fmt='C:/Users/hnxyh/.rustup/toolchains/1.97.1-x86_64-pc-windows-msvc/bin/rustfmt.exe';assert run('format-new-objects',[fmt,'--edition','2021',str(resource/'rdbc_sql_xml.rs'),str(resource/'sql_xml_value.rs'),str(resource/'rdbc_resource_factory.rs')])==0
library=next(row['command']for row in original['commands']if row['name']=='full-rdbc-compile').copy();library[library.index(str(oldr/'src/lib.rs'))]=str(rdbc/'src/lib.rs');library[library.index('--out-dir')+1]=str(rdbc)
for i,arg in enumerate(library):
    if arg.startswith('metadata='):library[i]='metadata=task7_xml_value_rdbc1'
assert run('full-rdbc-compile',library)==0
library=next(row['command']for row in original['commands']if row['name']=='full-druid-compile').copy();library[library.index(str(oldd/'src/lib.rs'))]=str(dest/'src/lib.rs');library[library.index('-o')+1]=str(dest/'libdruid.rlib')
for i,arg in enumerate(library):
    if arg.startswith('druid_rdbc='):library[i]='druid_rdbc='+str(rdbc/'libdruid_rdbc.rlib')
    if arg.startswith('metadata='):library[i]='metadata=task7_xml_value_druid1'
env=os.environ.copy();env.update(original['druid_environment']);env['CARGO_MANIFEST_DIR']=str(dest);state['druid_environment']=original['druid_environment']|{'CARGO_MANIFEST_DIR':str(dest)}
assert run('full-druid-old-boundary-red',library,env)!=0;assert 'RdbcResourceAccess' in(dest/'full-druid-old-boundary-red.log').read_text(encoding='utf-8')
p=dest/'src/core/mod.rs';text=p.read_text(encoding='utf-8').replace('RdbcResourceAccess,','').replace('RdbcSqlXmlAccess,','');p.write_text(text,encoding='utf-8',newline='\n')
assert run('full-druid-compile',library,env)==0
for i,arg in enumerate(consumer):
    if arg.startswith('druid_rdbc='):consumer[i]='druid_rdbc='+str(rdbc/'libdruid_rdbc.rlib')
    if arg.startswith('druid='):consumer[i]='druid='+str(dest/'libdruid.rlib')
consumer+=['-L','dependency='+str(rdbc)];assert run('consumer-compile',consumer)==0;assert run('consumer-runtime',[str(dest/'consumer.exe'),'--test-threads=1'])==0
state['sources']={p.relative_to(dest).as_posix():sha(p)for p in(dest/'src').rglob('*.rs')};state['rdbc_sources']={p.relative_to(rdbc).as_posix():sha(p)for p in(rdbc/'src').rglob('*.rs')};state['rlib_sha256']=sha(dest/'libdruid.rlib');state['rdbc_rlib_sha256']=sha(rdbc/'libdruid_rdbc.rlib');state['consumer_sha256']=sha(source);state['exe_sha256']=sha(dest/'consumer.exe');state['terminal']=True
(dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
