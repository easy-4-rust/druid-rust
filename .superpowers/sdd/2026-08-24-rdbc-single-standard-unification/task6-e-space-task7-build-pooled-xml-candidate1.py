from pathlib import Path
import json,subprocess,os
b=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008');stage=b/'task7-full-druid-source-candidate1';core=stage/'src/core'
red=json.loads((b/'task7-sqlxml-one-shot-native-red1.summary.json').read_text());assert red['compile_exit']==0 and red['runtime_exit']==101 and red['failed']==3
# 先前真实生产 RED 支持实现状态修复；新同源池化验收仍沿用原编译句柄，不提前宣称结果。
payload="""use druid_rdbc::error::{RdbcError, SqlException};
use druid_rdbc::resource::SqlXml;

/// 在独占驱动对象旁保存 SQLXML 的一次性读写机会，取消也不恢复已进入驱动的操作。
/// 对应 Java: java.sql.SQLXML 的 readable/writable 概念状态。
pub(crate) struct DruidSqlXmlPayload {
    pub(crate) driver: Box<dyn SqlXml>,
    readable: bool,
    writable: bool,
}
impl DruidSqlXmlPayload {
    /// 接管驱动对象，返回读写机会尚未消费的状态；驱动自身仍可拒绝不可用方向。
    pub(crate) fn new(driver: Box<dyn SqlXml>) -> Self {
        Self { driver, readable: true, writable: true }
    }
    /// 根据 read 选择读取或写入机会；首次调用消费机会，再次调用返回标准 SQL 异常。
    pub(crate) fn admit(&mut self, read: bool) -> Result<(), RdbcError> {
        let available = if read { &mut self.readable } else { &mut self.writable };
        if !*available {
            return Err(RdbcError::SqlException(Box::new(SqlException::new(
                0, Some("HY010".to_owned()), Some(format!("SQLXML is no longer {}", if read { "readable" } else { "writable" })),
            ))));
        }
        // 必须在任何驱动 await 前消费；future 被取消不允许再次执行同方向操作。
        *available = false;
        Ok(())
    }
}
"""
(core/'druid_sql_xml_payload.rs').write_text(payload,encoding='utf-8',newline='\n')
s=(core/'druid_blob_state.rs').read_text().replace('DruidBlobState','DruidSqlXmlState').replace('use druid_rdbc::resource::Blob;','use druid_rdbc::resource::SqlXml;\nuse super::druid_sql_xml_payload::DruidSqlXmlPayload;').replace('Box<dyn Blob>','Box<dyn SqlXml>').replace('tokio::sync::Mutex<Box<dyn SqlXml>>','tokio::sync::Mutex<DruidSqlXmlPayload>').replace('tokio::sync::Mutex::new(payload)','tokio::sync::Mutex::new(DruidSqlXmlPayload::new(payload))').replace('self.payload.lock().await.free().await','self.payload.lock().await.driver.free().await').replace('二进制大对象','XML 对象').replace('BlobProxyImpl','SQLXMLProxyImpl')
(core/'druid_sql_xml_state.rs').write_text(s,encoding='utf-8',newline='\n')
s=(core/'druid_blob.rs').read_text();prefix=s[:s.index('#[async_trait::async_trait]')]
prefix=prefix.replace('druid_blob_state::DruidBlobState','druid_sql_xml_state::DruidSqlXmlState').replace('DruidBlobState','DruidSqlXmlState').replace('DruidBlob','DruidSqlXml').replace('use druid_rdbc::resource::{Blob, RdbcInputStream, RdbcOutputStream, ResourceOperations};','use druid_rdbc::resource::{SqlXml, RdbcInputStream, RdbcOutputStream, RdbcReader, RdbcWriter, RdbcXmlRepresentationType, RdbcXmlSource, RdbcXmlResult, ResourceOperations};\nuse druid_rdbc::types::RdbcString;\nuse super::druid_sql_xml_payload::DruidSqlXmlPayload;').replace('Box<dyn Blob>','Box<dyn SqlXml>').replace('BlobProxyImpl','SQLXMLProxyImpl').replace('二进制大对象','XML 对象')
# XML 仅声明真实委托的读、写、流和释放能力，不宣传 LOB 搜索/切片。
for flag in ['SEARCH','TRUNCATE','RANGE']:
 start=prefix.index('        if driver_capabilities.contains(ResourceOperations::'+flag+')');end=prefix.index('\n        }',start)+10;prefix=prefix[:start]+prefix[end:]
prefix=prefix.replace('async fn payload(\n        &self,','async fn payload(\n        &self,\n        read: bool,').replace("tokio::sync::MutexGuard<'_, Box<dyn SqlXml>>","tokio::sync::MutexGuard<'_, DruidSqlXmlPayload>").replace('let payload = self.state.payload.lock().await;','let mut payload = self.state.payload.lock().await;').replace('        Ok(payload)','        payload.admit(read)?;\n        Ok(payload)')
assert 'use druid_rdbc::resource::' in prefix and 'SqlXml,' in prefix
methods=[('binary_stream','', 'RdbcInputStream',True,'READ | ResourceOperations::STREAM',''),('set_binary_stream','', 'RdbcOutputStream',False,'WRITE | ResourceOperations::STREAM',''),('character_stream','', 'RdbcReader',True,'READ | ResourceOperations::STREAM',''),('set_character_stream','', 'RdbcWriter',False,'WRITE | ResourceOperations::STREAM',''),('string','', 'RdbcString',True,'READ',''),('set_string',', value: &RdbcString','()',False,'WRITE','value'),('source',', representation: Option<&RdbcXmlRepresentationType>','RdbcXmlSource',True,'READ','representation'),('result',', representation: Option<&RdbcXmlRepresentationType>','RdbcXmlResult',False,'WRITE','representation')]
body='#[async_trait::async_trait]\nimpl SqlXml for DruidSqlXml {\n    fn capabilities(&self) -> ResourceOperations { self.capabilities }\n'
for name,args,ret,read,caps,params in methods:
 body+=f'    async fn {name}(&mut self{args}) -> Result<{ret}, RdbcError> {{\n        let result = self.payload({str(read).lower()}, ResourceOperations::{caps}, "SQLXML#{name}").await?.driver.{name}({params}).await;\n        self.state.context.classify_operation_result(result)\n    }}\n'
free=s[s.index('    async fn free('):];body+=free
(core/'druid_sql_xml.rs').write_text(prefix+body,encoding='utf-8',newline='\n')
p=core/'mod.rs';s=p.read_text();assert 'mod druid_sql_xml;' not in s;s=s.replace('mod druid_clob_state;','mod druid_clob_state;\nmod druid_sql_xml;\nmod druid_sql_xml_state;\nmod druid_sql_xml_payload;');p.write_text(s,encoding='utf-8',newline='\n')
p=core/'druid_pooled_connection.rs';s=p.read_text();start=s.index('    /// 创建受当前租约监管的标准 NClob');end=s.index('    async fn commit(',start);factory=s[start:end].replace('NClob','SqlXml').replace('create_n_clob','create_sql_xml').replace('createNClob','createSQLXML').replace('druid_n_clob::DruidSqlXml','druid_sql_xml::DruidSqlXml');factory=factory.replace('super::resource_runtime_capabilities::ResourceRuntimeCapabilities::large_object()','super::resource_runtime_capabilities::ResourceRuntimeCapabilities::READ\n                | super::resource_runtime_capabilities::ResourceRuntimeCapabilities::WRITE\n                | super::resource_runtime_capabilities::ResourceRuntimeCapabilities::STREAM\n                | super::resource_runtime_capabilities::ResourceRuntimeCapabilities::FREE');s=s[:end]+factory+s[end:];p.write_text(s,encoding='utf-8',newline='\n')
files=[core/n for n in ['druid_sql_xml.rs','druid_sql_xml_state.rs','druid_sql_xml_payload.rs','mod.rs','druid_pooled_connection.rs']]
q=subprocess.run(['C:/Users/hnxyh/.rustup/toolchains/1.97.1-x86_64-pc-windows-msvc/bin/rustfmt.exe','--edition','2021']+[str(p) for p in files],capture_output=True);assert q.returncode==0,q.stderr
r=json.loads((b/'task7-pooled-character-green2.compile.summary.json').read_text());c=[x.replace('task7_pooled_character_green2','task7_pooled_xml_green1').replace('libdruid_character_factory2.rlib','libdruid_xml_factory1.rlib') for x in r['command']];env=os.environ.copy();env.update(r['compile_environment'])
with (b/'task7-pooled-xml-green1.compile.log').open('wb') as f:q=subprocess.run(c,env=env,stdout=f,stderr=subprocess.STDOUT)
r.update(command=c,exit=q.returncode,rustfmt_exit=0);(b/'task7-pooled-xml-green1.compile.summary.json').write_text(json.dumps(r,indent=2));print('full_compile',q.returncode);print((b/'task7-pooled-xml-green1.compile.log').read_text()[-5000:])
