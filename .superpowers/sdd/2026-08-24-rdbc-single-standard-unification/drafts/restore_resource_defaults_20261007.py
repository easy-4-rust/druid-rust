from pathlib import Path
import hashlib

root = Path.cwd()
folder = root / 'crates/druid/src/core/druid_pooled_result_set/diagnostic_tests'
probe = folder / 'resource_defaults_result_set_probe.rs'
harness = folder / 'resource_defaults_chain_harness.rs'
assert not probe.exists() and not harness.exists()
resources = [('ref','Ref','RdbcRef','reference'),('blob','Blob','RdbcBlob','blob'),('clob','Clob','RdbcClob','clob'),('array','Array','RdbcArray','array'),('url','Url','RdbcUrl','url'),('row_id','RowId','RdbcRowId','row_id'),('n_clob','NClob','RdbcNClob','n_clob'),('sql_xml','SqlXml','RdbcSqlXml','sql_xml')]
source = '''use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{ResultSetMetaData, ResultSetMetaDataSnapshot};
use druid_rdbc::resource::{RdbcInputStream, RdbcReader};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetRead, ResultSetStreamKind};
use druid_rdbc::types::{RdbcObject, Value};
use std::sync::{Arc, Mutex};
/// 观察默认资源读取的全部原生重载，原 SQL NULL 与日志完整保留。
/// 对应 Java: java.sql.ResultSet 的十三类资源 getter。
pub(super) struct ResourceDefaultsResultSetProbe {
    calls: Arc<Mutex<Vec<String>>>,
    closed: bool,
}
impl ResourceDefaultsResultSetProbe {
    /// 创建原生观察器；参数为共享日志，返回开放的结果集。
    pub(super) fn new(calls: Arc<Mutex<Vec<String>>>) -> Self { Self { calls, closed: false } }
    fn record(&self, family: &str, column: Option<ColumnIndex>, label: Option<&str>) {
        let event = match label {
            Some(label) => format!("physical:{family}_by_label:{label}"),
            None => format!("physical:{family}:{}", column.expect("原下标必须保留").get()),
        };
        self.calls.lock().unwrap().push(event);
    }
    fn resource_family(request: &ResultSetRead) -> Result<&'static str, RdbcError> {
        match request {
'''
for _,variant,_,family in resources:
    source += f'            ResultSetRead::{variant} => Ok("{family}"),\n'
source += '''            _ => Err(RdbcError::UnsupportedOperation { operation: "resource_defaults_read" }),
        }
    }
    fn stream_family(kind: ResultSetStreamKind) -> Result<&'static str, RdbcError> {
        match kind {
            ResultSetStreamKind::Ascii => Ok("ascii_stream"),
            ResultSetStreamKind::Unicode => Ok("unicode_stream"),
            ResultSetStreamKind::Binary => Ok("binary_stream"),
            _ => Err(RdbcError::UnsupportedOperation { operation: "resource_defaults_stream" }),
        }
    }
}
druid_rdbc::impl_standard_wrapper!(ResourceDefaultsResultSetProbe => ResultSet);
#[async_trait::async_trait]
impl ResultSet for ResourceDefaultsResultSetProbe {
    async fn next(&mut self) -> Result<bool, RdbcError> { Err(RdbcError::UnsupportedOperation { operation: "resource_defaults_next" }) }
    fn find_column(&self, label: &str) -> Result<ColumnIndex, RdbcError> {
        self.calls.lock().unwrap().push(format!("physical:find_column:{label}"));
        ColumnIndex::new(3)
    }
    fn read_object(&self, column: ColumnIndex, request: &ResultSetRead) -> Result<RdbcObject, RdbcError> {
        self.record(Self::resource_family(request)?, Some(column), None);
        Ok(RdbcObject::Scalar(Value::Null))
    }
    fn read_object_by_label(&self, label: &str, request: &ResultSetRead) -> Result<RdbcObject, RdbcError> {
        self.record(Self::resource_family(request)?, None, Some(label));
        Ok(RdbcObject::Scalar(Value::Null))
    }
    fn input_stream(&self, column: ColumnIndex, kind: ResultSetStreamKind) -> Result<Option<RdbcInputStream>, RdbcError> {
        self.record(Self::stream_family(kind)?, Some(column), None); Ok(None)
    }
    fn input_stream_by_label(&self, label: &str, kind: ResultSetStreamKind) -> Result<Option<RdbcInputStream>, RdbcError> {
        self.record(Self::stream_family(kind)?, None, Some(label)); Ok(None)
    }
    fn reader(&self, column: ColumnIndex, national: bool) -> Result<Option<RdbcReader>, RdbcError> {
        self.record(if national { "n_character_stream" } else { "character_stream" }, Some(column), None); Ok(None)
    }
    fn reader_by_label(&self, label: &str, national: bool) -> Result<Option<RdbcReader>, RdbcError> {
        self.record(if national { "n_character_stream" } else { "character_stream" }, None, Some(label)); Ok(None)
    }
    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> { Ok(Arc::new(ResultSetMetaDataSnapshot::new(Vec::new()))) }
    async fn close(&mut self) -> Result<(), RdbcError> { self.closed = true; Ok(()) }
    fn is_closed(&self) -> Result<bool, RdbcError> { Ok(self.closed) }
}
'''
probe.write_text(source, encoding='utf-8')
source = '''use crate::core::{DruidError, FilterChain, ResultSetFilter, ResultSetFilterContext};
use druid_rdbc::resource::{RdbcArray, RdbcBlob, RdbcClob, RdbcNClob, RdbcRef, RdbcSqlXml, RdbcInputStream, RdbcReader};
use druid_rdbc::types::{RdbcObject, RdbcRowId, RdbcUrl};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetRead, ResultSetStreamKind};
use std::sync::Arc;
/// 原重载入口桥接到实际标准过滤器链，资源按真实类型转换。
/// 对应 Java: com.alibaba.druid.filter.FilterChainImpl。
pub(super) struct ResourceDefaultsChainHarness { chain: FilterChain }
impl ResourceDefaultsChainHarness {
    /// 创建默认链桥接；无参数，返回空链。
    pub(super) fn new() -> Self { Self { chain: FilterChain::new() } }
    /// 注册过滤器；参数为过滤器，返回完成状态。
    pub(super) fn add_result_set(&mut self, filter: Arc<dyn ResultSetFilter>) { self.chain.add_result_set(filter); }
'''
for stem,variant,ty,_ in resources:
    for label in [False,True]:
        name = 'result_set_get_'+stem+('_by_label' if label else '')
        arg = 'label: &str' if label else 'position: usize'
        column = 'None' if label else 'Some(ColumnIndex::new(position).map_err(DruidError::from)?)'
        target = 'Some(label)' if label else 'None'
        source += f'''    /// 委托原资源重载；参数为驱动、上下文与原列身份，返回同类型可空资源或错误。
    pub(super) fn {name}(&self, raw: &mut dyn ResultSet, context: &ResultSetFilterContext, {arg}) -> Result<Option<{ty}>, DruidError> {{
        let value = self.chain.standard_result_set_resource(raw, context, {column}, &ResultSetRead::{variant}, {target})?;
        match value {{
            RdbcObject::{variant}(resource) => Ok(Some(resource)),
            value if value.is_null() => Ok(None),
            _ => Err(DruidError::DriverError("原资源类型不匹配".to_owned())),
        }}
    }}
'''
for stem,kind in [('ascii_stream','Ascii'),('unicode_stream','Unicode'),('binary_stream','Binary'),('character_stream',False),('n_character_stream',True)]:
    for label in [False,True]:
        name='result_set_get_'+stem+('_by_label' if label else '')
        arg='label: &str' if label else 'position: usize'
        column='None' if label else 'Some(ColumnIndex::new(position).map_err(DruidError::from)?)'
        target='Some(label)' if label else 'None'
        reader=isinstance(kind,bool)
        ty='RdbcReader' if reader else 'RdbcInputStream'
        method='reader' if reader else 'input_stream'
        descriptor=str(kind).lower() if reader else 'ResultSetStreamKind::'+kind
        source+=f'''    /// 委托原流重载；参数保留驱动、上下文、列身份，返回原可空流或错误。
    pub(super) fn {name}(&self, raw: &mut dyn ResultSet, context: &ResultSetFilterContext, {arg}) -> Result<Option<{ty}>, DruidError> {{
        self.chain.standard_result_set_{method}(raw, context, {column}, {target}, {descriptor})
    }}
'''
source+='}\n'
harness.write_text(source,encoding='utf-8')
body=(root / '.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/drafts/complete_resource_defaults_base_original_20261007.rs.txt').read_text(encoding='utf-8')
body=body.replace('let physical = PhysicalResultSetProbe::new(Arc::clone(&call_log), false);','let mut physical = ResourceDefaultsResultSetProbe::new(Arc::clone(&call_log));').replace('let mut chain = FilterChain::new();','let mut chain = ResourceDefaultsChainHarness::new();').replace('PassThroughResultSetFilter','FilterAdapter::new()').replace('&physical','&mut physical')
imports='''
    use crate::core::{FilterAdapter, ResultSetFilterContext};
    use resource_defaults_result_set_probe::ResourceDefaultsResultSetProbe;
    use resource_defaults_chain_harness::ResourceDefaultsChainHarness;
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> { log.lock().unwrap().clone() }
'''
body=body.replace('{\n','{\n'+imports,1)
main=folder.with_suffix('.rs')
text=main.read_text(encoding='utf-8')
name='task6_result_set_resource_filter_defaults_preserve_all_physical_overload_identities'
assert name not in text
main.write_text(text+'\nmod resource_defaults_result_set_probe;\nmod resource_defaults_chain_harness;\n\n/// 完整保留原十三家族双重载与二十六条默认委托事件。\n#[test]\nfn '+name+'() '+body,encoding='utf-8')
verifier=root/'scripts/verify_task6_deleted_test_parity.py'
text=verifier.read_text(encoding='utf-8')
anchor='            "task6_result_set_navigation_filter_defaults_delegate_every_physical_method",\n'
assert text.count(anchor)==1
verifier.write_text(text.replace(anchor,anchor+'            "'+name+'",\n'),encoding='utf-8')
print('Full original resource default contract attached; ledger unchanged')
