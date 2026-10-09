from pathlib import Path
root=Path.cwd()
folder=root/'crates/druid/src/core/druid_pooled_result_set/diagnostic_tests'
probe=folder/'resource_update_defaults_result_set_probe.rs'
harness=folder/'resource_update_defaults_chain_harness.rs'
assert not probe.exists() and not harness.exists()
resources=[('reference','Ref','RdbcRef'),('blob','Blob','RdbcBlob'),('clob','Clob','RdbcClob'),('array','Array','RdbcArray'),('row_id','RowId','RdbcRowId'),('n_clob','NClob','RdbcNClob'),('sql_xml','SqlXml','RdbcSqlXml')]
s='''use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{ResultSetMetaData, ResultSetMetaDataSnapshot};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetUpdate};
use std::sync::{Arc, Mutex};
/// 观察原生资源更新十四重载的完整参数，不改写 NULL 或 RowId 身份。
/// 对应 Java: java.sql.ResultSet#updateRef/updateBlob/updateClob/updateArray/updateRowId/updateNClob/updateSQLXML。
pub(super) struct ResourceUpdateDefaultsResultSetProbe { calls: Arc<Mutex<Vec<String>>>, closed: bool }
impl ResourceUpdateDefaultsResultSetProbe {
    /// 创建开放探针；参数为共享日志，返回结果集。
    pub(super) fn new(calls: Arc<Mutex<Vec<String>>>) -> Self { Self { calls, closed: false } }
    fn record(&self, column: &str, by_label: bool, update: ResultSetUpdate) -> Result<(), RdbcError> {
        let (method, value) = match update {
'''
for stem,variant,_ in resources:
    s+=f'            ResultSetUpdate::{variant}(value) => ("update_{stem}", format!("{{value:?}}")),\n'
s+='''            _ => return Err(RdbcError::InvalidArgument("需要原资源更新描述符".to_owned())),
        };
        let suffix = if by_label { "_by_label" } else { "" };
        self.calls.lock().unwrap().push(format!("physical:{method}{suffix}:{column}:{value}"));
        Ok(())
    }
}
druid_rdbc::impl_standard_wrapper!(ResourceUpdateDefaultsResultSetProbe => ResultSet);
#[async_trait::async_trait]
impl ResultSet for ResourceUpdateDefaultsResultSetProbe {
    async fn next(&mut self) -> Result<bool, RdbcError> { Err(RdbcError::UnsupportedOperation { operation: "resource_update_defaults_next" }) }
    fn update_value(&mut self, column: ColumnIndex, update: ResultSetUpdate) -> Result<(), RdbcError> { self.record(&column.get().to_string(), false, update) }
    fn update_value_by_label(&mut self, label: &str, update: ResultSetUpdate) -> Result<(), RdbcError> { self.record(label, true, update) }
    fn find_column(&self, label: &str) -> Result<ColumnIndex, RdbcError> {
        self.calls.lock().unwrap().push(format!("physical:find_column:{label}")); ColumnIndex::new(23)
    }
    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> { Ok(Arc::new(ResultSetMetaDataSnapshot::new(Vec::new()))) }
    async fn close(&mut self) -> Result<(), RdbcError> { self.closed = true; Ok(()) }
    fn is_closed(&self) -> Result<bool, RdbcError> { Ok(self.closed) }
}
'''
probe.write_text(s,encoding='utf-8')
s='''use crate::core::{DruidError, FilterChain, ResultSetFilter, ResultSetFilterContext};
use druid_rdbc::resource::{RdbcRef, RdbcBlob, RdbcClob, RdbcArray, RdbcNClob, RdbcSqlXml};
use druid_rdbc::types::RdbcRowId;
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetUpdate};
use std::sync::Arc;
/// 原十四个资源更新入口桥接到实际标准链，不改变资源或参数。
/// 对应 Java: com.alibaba.druid.filter.FilterChainImpl。
pub(super) struct ResourceUpdateDefaultsChainHarness { chain: FilterChain }
impl ResourceUpdateDefaultsChainHarness {
    /// 创建链桥接；无参数，返回空链。
    pub(super) fn new() -> Self { Self { chain: FilterChain::new() } }
    /// 注册原顺序过滤器；参数为过滤器，返回完成状态。
    pub(super) fn add_result_set(&mut self, filter: Arc<dyn ResultSetFilter>) { self.chain.add_result_set(filter); }
'''
for stem,variant,ty in resources:
    for label in [False,True]:
        name='result_set_update_'+stem+('_by_label' if label else '')
        arg='label: &str' if label else 'position: usize'
        target='label' if label else 'ColumnIndex::new(position).map_err(DruidError::from)?'
        method='standard_result_set_update'+('_by_label' if label else '')
        s+=f'''    /// 委托原资源更新；参数保留驱动、上下文、列与可空资源，返回真实驱动或过滤器结果。
    pub(super) fn {name}(&self, raw: &mut dyn ResultSet, context: &ResultSetFilterContext, {arg}, value: Option<{ty}>) -> Result<(), DruidError> {{
        self.chain.{method}(raw, context, {target}, ResultSetUpdate::{variant}(value))
    }}
'''
s+='}\n'
harness.write_text(s,encoding='utf-8')
d=root/'.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/drafts'
body=(d/'complete_resource_update_defaults_base_original_20261007.rs.txt').read_text(encoding='utf-8')
body=body.replace('let physical = PhysicalResultSetProbe::new(Arc::clone(&physical_log), false);','let mut physical = ResourceUpdateDefaultsResultSetProbe::new(Arc::clone(&physical_log));').replace('let mut filter_chain = FilterChain::new();','let mut filter_chain = ResourceUpdateDefaultsChainHarness::new();').replace('PassThroughResultSetFilter','FilterAdapter::new()').replace('&physical','&mut physical')
body=body.replace('{\n','''{
    use crate::core::{FilterAdapter, ResultSetFilterContext};
    use druid_rdbc::types::RdbcRowId;
    use resource_update_defaults_result_set_probe::ResourceUpdateDefaultsResultSetProbe;
    use resource_update_defaults_chain_harness::ResourceUpdateDefaultsChainHarness;
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> { log.lock().unwrap().clone() }
''',1)
name='task6_result_set_resource_update_filter_defaults_preserve_all_fourteen_overloads'
main=folder.with_suffix('.rs')
s=main.read_text(encoding='utf-8')
assert name not in s
main.write_text(s+'\nmod resource_update_defaults_result_set_probe;\nmod resource_update_defaults_chain_harness;\n\n/// 原七资源双入口、NULL 与 RowId 值以及十四完整事件保持不变。\n#[test]\nfn '+name+'() '+body+'\n',encoding='utf-8')
p=root/'scripts/verify_task6_deleted_test_parity.py'
s=p.read_text(encoding='utf-8')
anchor='            "task6_result_set_resource_filter_defaults_preserve_all_physical_overload_identities",\n'
assert s.count(anchor)==1
p.write_text(s.replace(anchor,anchor+'            "'+name+'",\n'),encoding='utf-8')
s=(d/'bind_complete_resource_defaults_20261007.py').read_text(encoding='utf-8').replace('result_set_resource_filter_defaults_preserve_all_physical_overload_identities','result_set_resource_update_filter_defaults_preserve_all_fourteen_overloads').replace('resource-defaults','resource-update-defaults').replace('Resource defaults','Resource update defaults')
(d/'bind_complete_resource_update_defaults_20261007.py').write_text(s,encoding='utf-8')
s=(d/'audit_complete_resource_defaults_source_floor_20261007.py').read_text(encoding='utf-8').replace('resource-defaults','resource-update-defaults')
(d/'audit_complete_resource_update_defaults_source_floor_20261007.py').write_text(s,encoding='utf-8')
print('Full original resource update defaults attached; formal ledger unchanged')
