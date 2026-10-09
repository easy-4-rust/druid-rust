"""Full Clob/NClob standard-value candidate; production inputs remain frozen."""
import hashlib,json,os,shutil,subprocess,re
from pathlib import Path
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
oldr=base/'task7-blob-value-rdbc-candidate2';oldd=base/'task7-blob-value-druid-candidate2'
rdbc=base/'task7-clob-value-rdbc-candidate1';dest=base/'task7-clob-value-druid-candidate1'
rdbc.mkdir();dest.mkdir();sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
record=json.loads((oldd/'summary.json').read_text(encoding='utf-8'))
state={'production_applied':False,'terminal':False,'commands':[]}
def run(name,command,env=None):
    log=dest/(name+'.log')
    with log.open('xb') as out: result=subprocess.run(command,env=env,stdout=out,stderr=subprocess.STDOUT,creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    state['commands'].append({'name':name,'command':command,'exit_code':result.returncode,'log':str(log),'log_sha256':sha(log)})
    (dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8');print(name,result.returncode,flush=True);return result.returncode
prior=base/'task7-pooled-blob-value-consumer4.rs';source=base/'task7-pooled-clob-value-consumer1.rs'
text=prior.read_text(encoding='utf-8')
needle='''    async fn position_clob(&mut self, pattern:&mut dyn Clob, start:i64) -> Result<Option<i64>,RdbcError> {
        self.ensure_open()?;let count=i32::try_from(pattern.length().await?).map_err(|_|RdbcError::InvalidArgument("pattern too long".into()))?;
        let value=pattern.get_sub_string(1,count).await?;self.position_string(&value,start).await
    }'''
replacement='''    async fn position_clob(&mut self, pattern:druid_rdbc::resource::ClobPattern<'_>, start:i64) -> Result<Option<i64>,RdbcError> {
        self.ensure_open()?;
        let value=match pattern {
            druid_rdbc::resource::ClobPattern::SameResource => RdbcString::from_utf16(self.units.clone()),
            druid_rdbc::resource::ClobPattern::Other(pattern) => {
                let count=i32::try_from(pattern.length().await?).map_err(|_|RdbcError::InvalidArgument("pattern too long".into()))?;
                pattern.get_sub_string(1,count).await?
            }
        };
        self.position_string(&value,start).await
    }'''
assert text.count(needle)==1;text=text.replace(needle,replacement)
oldarg='resource.position_clob(&mut *pattern,1)';newarg='resource.position_clob(druid_rdbc::resource::ClobPattern::Other(&mut *pattern),1)'
assert text.count(oldarg)==2;text=text.replace(oldarg,newarg)
state['prior_consumer_sha256']=sha(prior);state['existing_test_argument_migrations']={'count':2,'old':oldarg,'new':newarg,'assertions_changed':False}
source.write_text(text+'\n'+(base/'task7-clob-value-contract1.rs').read_text(encoding='utf-8'),encoding='utf-8',newline='\n')
consumer=next(row['command'] for row in record['commands'] if row['name']=='consumer-compile').copy()
consumer[consumer.index(str(prior))]=str(source);consumer[consumer.index('-o')+1]=str(dest/'consumer.exe')
assert run('old-clob-value-consumer-red',consumer)!=0
red=(dest/'old-clob-value-consumer-red.log').read_text(encoding='utf-8');assert 'E0308' in red and 'ClobPattern' in red
shutil.copytree(oldr/'src',rdbc/'src');shutil.copytree(oldd/'src',dest/'src');shutil.copytree(oldd/'resources',dest/'resources');resource=rdbc/'src/resource'
# 每个对象一个文件；保留标准trait，删除第二套Access端口。
(resource/'clob_pattern.rs').write_text((resource/'blob_pattern.rs').read_text(encoding='utf-8').replace('Blob','Clob'),encoding='utf-8',newline='\n')
p=resource/'clob.rs';text=p.read_text(encoding='utf-8').replace('use crate::resource::{','use crate::resource::{ClobPattern,').replace('pattern: &mut dyn Clob',"pattern: ClobPattern<'_>")
p.write_text(text,encoding='utf-8',newline='\n')
methods=[
('length','', 'i64','READ','Clob#length','', '返回UTF-16字符单元长度，无参数；失败返回原数据库错误。'),
('get_sub_string','position: i64, length: i32','RdbcString','READ|RANGE','Clob#getSubString','position,length','读取1-based参数position起至多length个UTF-16单元，返回保留孤立代理项的原值。'),
('get_character_stream','','RdbcReader','READ|STREAM','Clob#getCharacterStream','','返回原驱动字符读取流，无参数；不提前读取或转换UTF-16。'),
('get_ascii_stream','','RdbcInputStream','READ|STREAM','Clob#getAsciiStream','','返回原驱动ASCII读取流，无参数；编码策略由驱动决定。'),
('position_string','pattern: &RdbcString, start: i64','Option<i64>','READ|SEARCH','Clob#position(String,long)','pattern,start','从1-based参数start搜索pattern，返回位置、None或原错误。'),
('set_string','position: i64, value: &RdbcString','i32','WRITE','Clob#setString','position,value','将value写至1-based参数position，返回UTF-16单元数或原错误。'),
('set_string_range','position: i64, value: &RdbcString, offset: i32, length: i32','i32','WRITE|RANGE','Clob#setString(long,String,int,int)','position,value,offset,length','将value的0-based offset起length单元写至1-based position，返回实际写入数。'),
('set_ascii_stream','position: i64','RdbcOutputStream','WRITE|STREAM','Clob#setAsciiStream','position','在1-based参数position返回原驱动ASCII写入流，不提前写入。'),
('set_character_stream','position: i64','RdbcWriter','WRITE|STREAM','Clob#setCharacterStream','position','在1-based参数position返回原驱动UTF-16字符写入器。'),
('truncate','length: i64','()','WRITE|TRUNCATE','Clob#truncate','length','截断至参数length个UTF-16单元，返回成功或原错误。'),
('get_character_stream_range','position: i64, length: i64','RdbcReader','READ|STREAM|RANGE','Clob#getCharacterStream(long,long)','position,length','返回1-based参数position起length单元的原字符流或原错误。'),
]
head='''use super::standard_resource_value::StandardResourceValue;
use crate::error::RdbcError;
use crate::resource::{Clob,ClobPattern,RdbcInputStream,RdbcOutputStream,RdbcReader,RdbcWriter,ResourceOperations};
use crate::types::RdbcString;
use std::{fmt,sync::Arc};
/// 字符大对象共享值句柄；对应 Java: java.sql.Clob。
/// 克隆保留同一Send标准对象、身份与释放确认，不提前物化字符。
#[derive(Clone)]
pub struct RdbcClob {value: Arc<StandardResourceValue<dyn Clob>>}
impl RdbcClob {
    /// 接管参数value，返回保留原驱动对象的Clob句柄。
    pub(crate) fn from_standard(value:Box<dyn Clob>)->Self {Self::from_standard_named(value,"Clob")}
    /// 接管参数value并保存name标准类型名，返回共享值；NClob视图保留相同状态。
    pub(crate) fn from_standard_named(value:Box<dyn Clob>,name:&'static str)->Self {
        let capabilities=value.capabilities() & ResourceOperations::clob();
        Self {value:Arc::new(StandardResourceValue::new(value,capabilities,name))}
    }
'''
for name,args,ret,caps,op,call,doc in methods:
    flags=' | '.join('ResourceOperations::'+c for c in caps.split('|'))
    head+=f'    /// {doc} 对应 Java: {op}。\n    pub async fn {name}(&self{", " if args else ""}{args})->Result<{ret},RdbcError>{{self.value.lock({flags},"{op}").await?.{name}({call}).await}}\n'
blob=(resource/'rdbc_blob.rs').read_text(encoding='utf-8');start=blob.index('    /// 从1-based参数start搜索pattern所引用');end=blob.index('    /// 从1-based参数position覆盖',start)
head+=blob[start:end].replace('Blob','Clob').replace('position_blob','position_clob')
head+='''    /// 幂等释放，无参数；失败可重试，取消后拒绝读取直到释放确认。对应 Java: Clob#free。
    pub async fn free(&self)->Result<(),RdbcError>{self.value.free("Clob#free",|value|value.free()).await}
    /// 返回是否已确认成功释放，无参数（Rust扩展）。
    #[must_use] pub fn is_freed(&self)->bool {self.value.is_freed()}
    /// 返回稳定共享身份，无参数（Rust扩展）。
    #[must_use] pub fn resource_id(&self)->&str {self.value.resource_id()}
    /// 返回驱动声明与Clob标准操作的交集，无参数（Rust扩展）。
    #[must_use] pub fn capabilities(&self)->ResourceOperations {self.value.capabilities()}
}
impl fmt::Debug for RdbcClob {fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result {f.debug_struct("RdbcClob").field("value",&self.value).finish_non_exhaustive()}}
impl PartialEq for RdbcClob {fn eq(&self,other:&Self)->bool {Arc::ptr_eq(&self.value,&other.value)}}
impl Eq for RdbcClob {}
'''
(resource/'rdbc_clob.rs').write_text(head,encoding='utf-8',newline='\n')
head='''use crate::error::RdbcError;
use crate::resource::{Clob,NClob,RdbcClob,RdbcInputStream,RdbcOutputStream,RdbcReader,RdbcWriter,ResourceOperations};
use crate::types::RdbcString;
/// 国家字符大对象共享值句柄；对应 Java: java.sql.NClob extends Clob。
/// 显式委托继承操作，不使用Deref模拟继承；Clob视图共享对象、身份与释放状态。
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct RdbcNClob {clob:RdbcClob}
impl RdbcNClob {
    /// 接管参数value国家字符对象，返回保留NClob身份的句柄；上转不复制内容。
    pub(crate) fn from_standard(value:Box<dyn NClob>)->Self {let value:Box<dyn Clob>=value;Self {clob:RdbcClob::from_standard_named(value,"NClob")}}
    /// 返回共享同一对象、身份与释放状态的Clob视图，无参数。
    #[must_use] pub const fn as_clob(&self)->&RdbcClob {&self.clob}
'''
for name,args,ret,caps,op,call,doc in methods:
    head+=f'    /// {doc} 对应 Java: {op}。\n    pub async fn {name}(&self{", " if args else ""}{args})->Result<{ret},RdbcError>{{self.clob.{name}({call}).await}}\n'
head+='''    /// 从1-based参数start搜索pattern资源，返回位置、None或原错误。对应 Java: Clob#position(Clob,long)。
    pub async fn position_clob(&self,pattern:&RdbcClob,start:i64)->Result<Option<i64>,RdbcError>{self.clob.position_clob(pattern,start).await}
    /// 幂等释放共享资源，无参数；失败返回原错误。对应 Java: Clob#free。
    pub async fn free(&self)->Result<(),RdbcError>{self.clob.free().await}
    /// 返回共享资源是否已确认释放，无参数。
    #[must_use] pub fn is_freed(&self)->bool {self.clob.is_freed()}
    /// 返回与Clob视图相同的稳定身份，无参数。
    #[must_use] pub fn resource_id(&self)->&str {self.clob.resource_id()}
    /// 返回实际支持的Clob操作集合，无参数。
    #[must_use] pub fn capabilities(&self)->ResourceOperations {self.clob.capabilities()}
}
'''
(resource/'rdbc_n_clob.rs').write_text(head,encoding='utf-8',newline='\n')
for name,typ in [('clob','RdbcClobAccess'),('n_clob','RdbcNClobAccess')]:
    (resource/f'rdbc_{name}_access.rs').unlink()
    p=resource/'mod.rs';text=p.read_text(encoding='utf-8').replace(f'mod rdbc_{name}_access;\n','').replace(f'pub use rdbc_{name}_access::{typ};\n','');p.write_text(text,encoding='utf-8',newline='\n')
p=resource/'mod.rs';p.write_text(p.read_text(encoding='utf-8')+'\nmod clob_pattern;\npub use clob_pattern::ClobPattern;\n',encoding='utf-8',newline='\n')
p=resource/'rdbc_resource_factory.rs';text=p.read_text(encoding='utf-8').replace('RdbcClobAccess,','Clob,').replace('RdbcNClobAccess,','NClob,');start=text.index('    /// Creates a detached `Clob` handle.');end=text.index('    /// 将独立拥有的标准Ref',start)
text=text[:start]+'''    /// 接管参数value唯一标准Clob，返回保留身份及释放状态的共享句柄。对应 Java: java.sql.Clob。
    #[must_use] pub fn clob(value:Box<dyn Clob>)->RdbcClob {RdbcClob::from_standard(value)}
    /// 接管参数value唯一标准NClob，返回保留国家字符身份及共享Clob视图的句柄。对应 Java: java.sql.NClob。
    #[must_use] pub fn n_clob(value:Box<dyn NClob>)->RdbcNClob {RdbcNClob::from_standard(value)}

'''+text[end:];p.write_text(text,encoding='utf-8',newline='\n')
p=resource/'rdbc_resource_kind.rs';text=p.read_text(encoding='utf-8')
for name in ['Clob','NClob']:
    text=re.sub(r'    /// An SQL `[^`]+` resource\.\n    '+name+r',\n','',text).replace(f'            Self::{name} => "{name}",\n','')
p.write_text(text,encoding='utf-8',newline='\n')
fmt='C:/Users/hnxyh/.rustup/toolchains/1.97.1-x86_64-pc-windows-msvc/bin/rustfmt.exe'
assert run('format-new-objects',[fmt,'--edition','2021',*[str(resource/name) for name in ['rdbc_clob.rs','rdbc_n_clob.rs','clob_pattern.rs','clob.rs']]])==0
library=next(row['command'] for row in record['commands'] if row['name']=='full-rdbc-compile').copy();library[library.index(str(oldr/'src/lib.rs'))]=str(rdbc/'src/lib.rs');library[library.index('--out-dir')+1]=str(rdbc)
for i,arg in enumerate(library):
    if arg.startswith('metadata='):library[i]='metadata=task7_clob_value_rdbc1'
assert run('full-rdbc-compile',library)==0
library=next(row['command'] for row in record['commands'] if row['name']=='full-druid-compile').copy();library[library.index(str(oldd/'src/lib.rs'))]=str(dest/'src/lib.rs');library[library.index('-o')+1]=str(dest/'libdruid.rlib')
for i,arg in enumerate(library):
    if arg.startswith('druid_rdbc='):library[i]='druid_rdbc='+str(rdbc/'libdruid_rdbc.rlib')
    if arg.startswith('metadata='):library[i]='metadata=task7_clob_value_druid1'
env=os.environ.copy();env.update(record['druid_environment']);env['CARGO_MANIFEST_DIR']=str(dest);state['druid_environment']=record['druid_environment']|{'CARGO_MANIFEST_DIR':str(dest)}
assert run('full-druid-old-boundary-red',library,env)!=0
red=(dest/'full-druid-old-boundary-red.log').read_text(encoding='utf-8');assert 'RdbcClobAccess' in red and 'position_clob' in red
p=dest/'src/core/mod.rs';text=p.read_text(encoding='utf-8').replace('RdbcClobAccess,','').replace('RdbcNClobAccess,','');p.write_text(text,encoding='utf-8',newline='\n')
for name in ['druid_clob.rs','druid_n_clob.rs']:
    p=dest/'src/core'/name;text=p.read_text(encoding='utf-8').replace('    Clob,','    Clob, ClobPattern,').replace('pattern: &mut dyn Clob,',"pattern: ClobPattern<'_>,");p.write_text(text,encoding='utf-8',newline='\n')
assert run('full-druid-compile',library,env)==0
for i,arg in enumerate(consumer):
    if arg.startswith('druid_rdbc='):consumer[i]='druid_rdbc='+str(rdbc/'libdruid_rdbc.rlib')
    if arg.startswith('druid='):consumer[i]='druid='+str(dest/'libdruid.rlib')
consumer+=['-L','dependency='+str(rdbc)]
assert run('consumer-compile',consumer)==0
assert run('consumer-runtime',[str(dest/'consumer.exe'),'--test-threads=1'])==0
state['sources']={p.relative_to(dest).as_posix():sha(p) for p in (dest/'src').rglob('*.rs')};state['rdbc_sources']={p.relative_to(rdbc).as_posix():sha(p) for p in (rdbc/'src').rglob('*.rs')}
state['rlib_sha256']=sha(dest/'libdruid.rlib');state['rdbc_rlib_sha256']=sha(rdbc/'libdruid_rdbc.rlib');state['consumer_sha256']=sha(source);state['exe_sha256']=sha(dest/'consumer.exe');state['terminal']=True
(dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
