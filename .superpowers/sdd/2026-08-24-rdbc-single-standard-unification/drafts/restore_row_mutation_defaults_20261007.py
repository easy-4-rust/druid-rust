from pathlib import Path
root=Path.cwd()
folder=root/'crates/druid/src/core/druid_pooled_result_set/diagnostic_tests'
probe=folder/'row_mutation_defaults_result_set_probe.rs'
harness=folder/'row_mutation_defaults_chain_harness.rs'
assert not probe.exists() and not harness.exists()
operations=[('insert_row','InsertRow'),('update_row','UpdateRow'),('delete_row','DeleteRow'),('refresh_row','RefreshRow'),('cancel_row_updates','CancelRowUpdates'),('move_to_insert_row','MoveToInsertRow'),('move_to_current_row','MoveToCurrentRow')]
s='''use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{ResultSetMetaData, ResultSetMetaDataSnapshot};
use druid_rdbc::result_set::ResultSet;
use std::sync::{Arc, Mutex};
/// 观察七个原生无参行修改方法，完整保留原调用顺序。
/// 对应 Java: java.sql.ResultSet 的七个行修改方法。
pub(super) struct RowMutationDefaultsResultSetProbe { calls: Arc<Mutex<Vec<String>>>, closed: bool }
impl RowMutationDefaultsResultSetProbe {
    /// 创建开放探针；参数为共享日志，返回结果集。
    pub(super) fn new(calls: Arc<Mutex<Vec<String>>>) -> Self { Self { calls, closed: false } }
    fn record(&self, method: &str) { self.calls.lock().unwrap().push(format!("physical:{method}")); }
}
druid_rdbc::impl_standard_wrapper!(RowMutationDefaultsResultSetProbe => ResultSet);
#[async_trait::async_trait]
impl ResultSet for RowMutationDefaultsResultSetProbe {
    async fn next(&mut self) -> Result<bool, RdbcError> { Err(RdbcError::UnsupportedOperation { operation: "row_mutation_defaults_next" }) }
'''
for method,_ in operations:
    s+=f'    fn {method}(&mut self) -> Result<(), RdbcError> {{ self.record("{method}"); Ok(()) }}\n'
s+='''    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> { Ok(Arc::new(ResultSetMetaDataSnapshot::new(Vec::new()))) }
    async fn close(&mut self) -> Result<(), RdbcError> { self.closed = true; self.record("close"); Ok(()) }
    fn is_closed(&self) -> Result<bool, RdbcError> { Ok(self.closed) }
}
'''
probe.write_text(s,encoding='utf-8')
s='''use crate::core::{DruidError, FilterChain, ResultSetFilter, ResultSetFilterContext, ResultSetRowOperation};
use druid_rdbc::result_set::ResultSet;
use std::sync::Arc;
/// 原行修改入口通过实际标准过滤器链惰性委托到同名驱动方法。
/// 对应 Java: com.alibaba.druid.filter.FilterChainImpl。
pub(super) struct RowMutationDefaultsChainHarness { chain: FilterChain }
impl RowMutationDefaultsChainHarness {
    /// 创建链桥接；无参数，返回空链。
    pub(super) fn new() -> Self { Self { chain: FilterChain::new() } }
    /// 注册原顺序过滤器；参数为过滤器，返回完成状态。
    pub(super) fn add_result_set(&mut self, filter: Arc<dyn ResultSetFilter>) { self.chain.add_result_set(filter); }
'''
for method,variant in operations:
    s+=f'''    /// 委托原行修改；参数为驱动和上下文，返回真实过滤器或驱动结果。
    pub(super) fn result_set_{method}(&self, raw: &mut dyn ResultSet, context: &ResultSetFilterContext) -> Result<(), DruidError> {{
        self.chain.standard_result_set_row_operation(context, ResultSetRowOperation::{variant}, &mut || raw.{method}().map_err(DruidError::from))
    }}
'''
s+='}\n'
harness.write_text(s,encoding='utf-8')
d=root/'.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/drafts'
body=(d/'complete_row_mutation_defaults_base_original_20261007.rs.txt').read_text(encoding='utf-8')
body=body.replace('let physical = PhysicalResultSetProbe::new(Arc::clone(&call_log), false);','let mut physical = RowMutationDefaultsResultSetProbe::new(Arc::clone(&call_log));').replace('let mut chain = FilterChain::new();','let mut chain = RowMutationDefaultsChainHarness::new();').replace('PassThroughResultSetFilter','FilterAdapter::new()').replace('&physical,','&mut physical,')
body=body.replace('{\n','''{
    use crate::core::{FilterAdapter, ResultSetFilterContext};
    use row_mutation_defaults_result_set_probe::RowMutationDefaultsResultSetProbe;
    use row_mutation_defaults_chain_harness::RowMutationDefaultsChainHarness;
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> { log.lock().unwrap().clone() }
''',1)
name='task6_result_set_row_mutation_filter_defaults_delegate_every_physical_method'
main=folder.with_suffix('.rs')
s=main.read_text(encoding='utf-8'); assert name not in s
main.write_text(s+'\nmod row_mutation_defaults_result_set_probe;\nmod row_mutation_defaults_chain_harness;\n\n/// 原七个行修改默认委托与完整有序原生日志保持不变。\n#[test]\nfn '+name+'() '+body+'\n',encoding='utf-8')
p=root/'scripts/verify_task6_deleted_test_parity.py'; s=p.read_text(encoding='utf-8')
anchor='            "task6_result_set_resource_update_filter_defaults_preserve_all_fourteen_overloads",\n'
assert s.count(anchor)==1
p.write_text(s.replace(anchor,anchor+'            "'+name+'",\n'),encoding='utf-8')
s=(d/'bind_complete_resource_update_defaults_20261007.py').read_text(encoding='utf-8').replace('result_set_resource_update_filter_defaults_preserve_all_fourteen_overloads','result_set_row_mutation_filter_defaults_delegate_every_physical_method').replace('resource-update-defaults-green','row-mutation-defaults-initial').replace('Resource update defaults','Row mutation defaults')
(d/'bind_complete_row_mutation_defaults_20261007.py').write_text(s,encoding='utf-8')
s=(d/'audit_complete_resource_update_defaults_source_floor_20261007.py').read_text(encoding='utf-8').replace('resource-update-defaults','row-mutation-defaults')
(d/'audit_complete_row_mutation_defaults_source_floor_20261007.py').write_text(s,encoding='utf-8')
print('Full original row mutation defaults attached; formal ledger unchanged')
