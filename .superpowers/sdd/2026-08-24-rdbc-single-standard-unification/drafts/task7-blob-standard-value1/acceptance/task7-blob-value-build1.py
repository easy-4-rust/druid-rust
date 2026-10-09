"""Retire old Blob Access using full source candidates and unchanged prior test bodies."""
import hashlib,json,os,re,shutil,subprocess
from pathlib import Path
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008');oldr=base/'task7-array-value-rdbc-candidate1';oldd=base/'task7-array-value-druid-candidate1';rdbc=base/'task7-blob-value-rdbc-candidate1';dest=base/'task7-blob-value-druid-candidate1'
rdbc.mkdir();dest.mkdir();sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest();state={'production_applied':False,'commands':[],'terminal':False}
def run(name,command,env=None):
    log=dest/(name+'.log')
    with log.open('xb') as out:result=subprocess.run(command,env=env,stdout=out,stderr=subprocess.STDOUT,creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    state['commands'].append({'name':name,'command':command,'exit_code':result.returncode,'log':str(log),'log_sha256':sha(log)})
    (dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8');print(name,result.returncode,flush=True);return result.returncode
prior=base/'task7-pooled-array-value-consumer3.rs';source=base/'task7-pooled-blob-value-consumer1.rs';old=prior.read_text(encoding='utf-8')
needle='''    async fn position_blob(&mut self, pattern:&mut dyn Blob, start:i64) -> Result<Option<i64>,RdbcError> {
        self.ensure_open()?;
        let length=i32::try_from(pattern.length().await?).map_err(|_| RdbcError::InvalidArgument("pattern exceeds i32".into()))?;
        let bytes=pattern.get_bytes(1,length).await?;
        self.position_bytes(&bytes,start).await
    }'''
replacement='''    async fn position_blob(&mut self, pattern:druid_rdbc::resource::BlobPattern<'_>, start:i64) -> Result<Option<i64>,RdbcError> {
        self.ensure_open()?;
        let bytes=match pattern {
            druid_rdbc::resource::BlobPattern::SameResource => self.bytes.clone(),
            druid_rdbc::resource::BlobPattern::Other(pattern) => {
                let length=i32::try_from(pattern.length().await?).map_err(|_| RdbcError::InvalidArgument("pattern exceeds i32".into()))?;
                pattern.get_bytes(1,length).await?
            }
        };
        self.position_bytes(&bytes,start).await
    }'''
assert old.count(needle)==1;adapted=old.replace(needle,replacement)
# 只迁移驱动夹具的参数表示；全部既有tokio测试块保持原字节语义。
test_blocks=lambda text:re.findall(r'#\[(?:tokio::)?test[^\]]*\][\s\S]*?(?=\n#\[|\Z)',text)
assert test_blocks(old)==test_blocks(adapted)
state['prior_test_blocks']=[hashlib.sha256(block.encode()).hexdigest() for block in test_blocks(old)]
source.write_text(adapted+'\n'+(base/'task7-blob-value-contract1.rs').read_text(encoding='utf-8'),encoding='utf-8',newline='\n')
record=json.loads((oldd/'expanded.summary.json').read_text(encoding='utf-8'));consumer=next(row['command'] for row in record['commands'] if row['name']=='consumer3-compile').copy();consumer[consumer.index(str(prior))]=str(source);consumer[consumer.index('-o')+1]=str(dest/'consumer.exe')
assert run('old-blob-value-consumer-red',consumer)!=0
red=(dest/'old-blob-value-consumer-red.log').read_text(encoding='utf-8');assert 'E0308' in red and 'BlobPattern' in red
shutil.copytree(oldr/'src',rdbc/'src');shutil.copytree(oldd/'src',dest/'src');shutil.copytree(oldd/'resources',dest/'resources');resource=rdbc/'src/resource'
shutil.copyfile(base/'task7-blob-handle1.rs',resource/'rdbc_blob.rs');shutil.copyfile(base/'task7-blob-pattern1.rs',resource/'blob_pattern.rs');(resource/'rdbc_blob_access.rs').unlink()
p=resource/'mod.rs';text=p.read_text(encoding='utf-8').replace('mod rdbc_blob_access;\n','').replace('pub use rdbc_blob_access::RdbcBlobAccess;\n','');text+='\nmod blob_pattern;\npub use blob_pattern::BlobPattern;\n';p.write_text(text,encoding='utf-8',newline='\n')
p=resource/'blob.rs';text=p.read_text(encoding='utf-8').replace('use crate::resource::{RdbcInputStream,','use crate::resource::{BlobPattern, RdbcInputStream,').replace('pattern: &mut dyn Blob','pattern: BlobPattern<\'_>');text=text.replace('两个资源均独占借用，无具体类型downcast。','Other独占借用另一资源；SameResource保留自身模式，不构造重叠可变引用或具体类型downcast。');p.write_text(text,encoding='utf-8',newline='\n')
p=resource/'rdbc_resource_factory.rs';text=p.read_text(encoding='utf-8').replace('RdbcBlob, RdbcBlobAccess,','Blob, RdbcBlob,');start=text.index('    /// Creates a detached `Blob` handle.');end=text.index('    /// Creates a detached `Clob` handle.',start)
text=text[:start]+'''    /// 将唯一标准Blob对象装入共享值句柄，参数为独立拥有的驱动对象，返回保留身份与释放状态的句柄。
    /// 对应 Java: java.sql.Blob；不提前读取内容，不增加驱动端口。
    #[must_use]
    pub fn blob(value: Box<dyn Blob>) -> RdbcBlob {
        RdbcBlob::from_standard(value)
    }

'''+text[end:];p.write_text(text,encoding='utf-8',newline='\n')
p=resource/'rdbc_resource_kind.rs';text=p.read_text(encoding='utf-8').replace('    /// An SQL `BLOB` resource.\n    Blob,\n','').replace('            Self::Blob => "Blob",\n','');p.write_text(text,encoding='utf-8',newline='\n')
fmt='C:/Users/hnxyh/.rustup/toolchains/1.97.1-x86_64-pc-windows-msvc/bin/rustfmt.exe'
assert run('format-new-objects',[fmt,'--edition','2021',str(resource/'rdbc_blob.rs'),str(resource/'blob_pattern.rs')])==0
record=json.loads((oldd/'summary.json').read_text(encoding='utf-8'));library=next(row['command'] for row in record['commands'] if row['name']=='full-rdbc-compile').copy();library[library.index(str(oldr/'src/lib.rs'))]=str(rdbc/'src/lib.rs');library[library.index('--out-dir')+1]=str(rdbc)
for i,arg in enumerate(library):
    if arg.startswith('metadata='):library[i]='metadata=task7_blob_value_rdbc1'
assert run('full-rdbc-compile',library)==0
row=next(row for row in record['commands'] if row['name']=='full-druid-compile');library=row['command'].copy();library[library.index(str(oldd/'src/lib.rs'))]=str(dest/'src/lib.rs');library[library.index('-o')+1]=str(dest/'libdruid.rlib')
for i,arg in enumerate(library):
    if arg.startswith('druid_rdbc='):library[i]='druid_rdbc='+str(rdbc/'libdruid_rdbc.rlib')
    if arg.startswith('metadata='):library[i]='metadata=task7_blob_value_druid1'
env=os.environ.copy();env.update(record['druid_environment']);env['CARGO_MANIFEST_DIR']=str(dest);state['druid_environment']=record['druid_environment']|{'CARGO_MANIFEST_DIR':str(dest)}
assert run('full-druid-old-boundary-red',library,env)!=0
red=(dest/'full-druid-old-boundary-red.log').read_text(encoding='utf-8');assert 'RdbcBlobAccess' in red and 'position_blob' in red
p=dest/'src/core/mod.rs';text=p.read_text(encoding='utf-8');assert text.count('RdbcBlobAccess,')==1;text=text.replace('RdbcBlobAccess,','');p.write_text(text,encoding='utf-8',newline='\n')
p=dest/'src/core/druid_blob.rs';text=p.read_text(encoding='utf-8').replace('use druid_rdbc::resource::{Blob,','use druid_rdbc::resource::{Blob, BlobPattern,').replace('pattern: &mut dyn Blob,','pattern: BlobPattern<\'_>,');p.write_text(text,encoding='utf-8',newline='\n')
assert run('full-druid-compile',library,env)==0
for i,arg in enumerate(consumer):
    if arg.startswith('druid_rdbc='):consumer[i]='druid_rdbc='+str(rdbc/'libdruid_rdbc.rlib')
    if arg.startswith('druid='):consumer[i]='druid='+str(dest/'libdruid.rlib')
consumer+=['-L','dependency='+str(rdbc)]
assert run('consumer-compile',consumer)==0
assert run('consumer-runtime',[str(dest/'consumer.exe'),'--test-threads=1'])==0
state['sources']={p.relative_to(dest).as_posix():sha(p) for p in (dest/'src').rglob('*.rs')};state['rdbc_sources']={p.relative_to(rdbc).as_posix():sha(p) for p in (rdbc/'src').rglob('*.rs')}
state['rlib_sha256']=sha(dest/'libdruid.rlib');state['rdbc_rlib_sha256']=sha(rdbc/'libdruid_rdbc.rlib');state['consumer_sha256']=sha(source);state['prior_67_sha256']=sha(prior);state['exe_sha256']=sha(dest/'consumer.exe');state['terminal']=True
(dest/'summary.json').write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8')
