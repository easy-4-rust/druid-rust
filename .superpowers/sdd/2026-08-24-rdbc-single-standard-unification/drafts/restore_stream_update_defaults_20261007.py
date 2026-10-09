from pathlib import Path
root=Path.cwd()
folder=root/'crates/druid/src/core/druid_pooled_result_set/diagnostic_tests'
probe=folder/'stream_update_defaults_result_set_probe.rs'
harness=folder/'stream_update_defaults_chain_harness.rs'
assert not probe.exists() and not harness.exists()
families=[('ascii_stream','AsciiStream','RdbcInputStream','stream','RdbcStreamLength'),('binary_stream','BinaryStream','RdbcInputStream','stream','RdbcStreamLength'),('character_stream','CharacterStream','RdbcReader','reader','RdbcCharacterLength'),('n_character_stream','NCharacterStream','RdbcReader','reader','RdbcCharacterLength')]
s='''use druid_rdbc::error::RdbcError;
use druid_rdbc::metadata::{ResultSetMetaData, ResultSetMetaDataSnapshot};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetUpdate};
use std::sync::{Arc, Mutex};
/// 观察流更新原始重载和长度，不读取、关闭或改写调用者资源。
/// 对应 Java: java.sql.ResultSet#updateAsciiStream/updateBinaryStream/updateCharacterStream/updateNCharacterStream。
pub(super) struct StreamUpdateDefaultsResultSetProbe { calls: Arc<Mutex<Vec<String>>>, closed: bool }
impl StreamUpdateDefaultsResultSetProbe {
    /// 创建开放探针；参数为共享日志，返回结果集。
    pub(super) fn new(calls: Arc<Mutex<Vec<String>>>) -> Self { Self { calls, closed: false } }
    fn record(&self, column: &str, by_label: bool, update: ResultSetUpdate) -> Result<(), RdbcError> {
        // 保留原描述符身份，只观察可空句柄与长度，不提前消费共享流。
        let (family, present, length) = match update {
'''
for _,variant,_,field,_ in families:
    s+=f'            ResultSetUpdate::{variant} {{ {field}, length }} => ("{variant}", {field}.is_some(), format!("{{length:?}}")),\n'
s+='''            _ => return Err(RdbcError::InvalidArgument("需要原流更新描述符".to_owned())),
        };
        let method = if by_label { "update_value_by_label" } else { "update_value" };
        self.calls.lock().unwrap().push(format!("physical:{method}:{column}:{family}:{present}:{length}"));
        Ok(())
    }
}
druid_rdbc::impl_standard_wrapper!(StreamUpdateDefaultsResultSetProbe => ResultSet);
#[async_trait::async_trait]
impl ResultSet for StreamUpdateDefaultsResultSetProbe {
    async fn next(&mut self) -> Result<bool, RdbcError> { Err(RdbcError::UnsupportedOperation { operation: "stream_update_defaults_next" }) }
    fn update_value(&mut self, column: ColumnIndex, update: ResultSetUpdate) -> Result<(), RdbcError> { self.record(&column.get().to_string(), false, update) }
    fn update_value_by_label(&mut self, label: &str, update: ResultSetUpdate) -> Result<(), RdbcError> { self.record(label, true, update) }
    fn find_column(&self, label: &str) -> Result<ColumnIndex, RdbcError> { self.calls.lock().unwrap().push(format!("physical:find_column:{label}")); ColumnIndex::new(23) }
    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> { Ok(Arc::new(ResultSetMetaDataSnapshot::new(Vec::new()))) }
    async fn close(&mut self) -> Result<(), RdbcError> { self.closed = true; Ok(()) }
    fn is_closed(&self) -> Result<bool, RdbcError> { Ok(self.closed) }
}
'''
probe.write_text(s,encoding='utf-8')
s='''use crate::core::{DruidError, FilterChain, ResultSetFilter, ResultSetFilterContext};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetUpdate};
use druid_rdbc::types::{RdbcInputStream, RdbcReader, RdbcStreamLength, RdbcCharacterLength};
use std::sync::Arc;
/// 原二十二个流更新入口桥接到真实标准链，长度与共享资源原样交接。
/// 对应 Java: com.alibaba.druid.filter.FilterChainImpl。
pub(super) struct StreamUpdateDefaultsChainHarness { chain: FilterChain }
impl StreamUpdateDefaultsChainHarness {
    /// 创建链桥接；无参数，返回空链。
    pub(super) fn new() -> Self { Self { chain: FilterChain::new() } }
    /// 注册原顺序过滤器；参数为过滤器，返回完成状态。
    pub(super) fn add_result_set(&mut self, filter: Arc<dyn ResultSetFilter>) { self.chain.add_result_set(filter); }
'''
for stem,variant,ty,field,length_ty in families:
    for label in [False,True]:
        for length in ['Unspecified','Int','Long']:
            if stem=='n_character_stream' and length=='Int': continue
            suffix={'Unspecified':'','Int':'_with_int_length','Long':'_with_length'}[length]
            name='result_set_update_'+stem+('_by_label' if label else '')+suffix
            arg='label: &str' if label else 'position: usize'
            extra='' if length=='Unspecified' else ', length: '+('i32' if length=='Int' else 'i64')
            descriptor=length_ty+'::'+length+('' if length=='Unspecified' else '(length)')
            target='label' if label else 'ColumnIndex::new(position).map_err(DruidError::from)?'
            method='standard_result_set_update'+('_by_label' if label else '')
            s+=f'''    /// 委托原流更新重载；参数保留驱动、上下文、列、可空流与原长度，返回真实结果。
    pub(super) fn {name}(&self, raw: &mut dyn ResultSet, context: &ResultSetFilterContext, {arg}, value: Option<{ty}>{extra}) -> Result<(), DruidError> {{
        self.chain.{method}(raw, context, {target}, ResultSetUpdate::{variant} {{ {field}: value, length: {descriptor} }})
    }}
'''
s+='}\n'
harness.write_text(s,encoding='utf-8')
d=root/'.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/drafts'
body=(d/'complete_stream_update_defaults_base_original_20261007.rs.txt').read_text(encoding='utf-8')
body=body.replace('let physical = PhysicalResultSetProbe::new(Arc::clone(&physical_log), false);','let mut physical = StreamUpdateDefaultsResultSetProbe::new(Arc::clone(&physical_log));').replace('let mut filter_chain = FilterChain::new();','let mut filter_chain = StreamUpdateDefaultsChainHarness::new();').replace('PassThroughResultSetFilter','FilterAdapter::new()').replace('&physical,','&mut physical,')
body=body.replace('{\n','''{
    use crate::core::{FilterAdapter, ResultSetFilterContext};
    use druid_rdbc::types::{RdbcInputStream, RdbcReader};
    use stream_update_defaults_result_set_probe::StreamUpdateDefaultsResultSetProbe;
    use stream_update_defaults_chain_harness::StreamUpdateDefaultsChainHarness;
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> { log.lock().unwrap().clone() }
''',1)
name='task6_result_set_stream_update_filter_defaults_preserve_all_twenty_two_overloads'
main=folder.with_suffix('.rs'); s=main.read_text(encoding='utf-8'); assert name not in s
main.write_text(s+'\nmod stream_update_defaults_result_set_probe;\nmod stream_update_defaults_chain_harness;\n\n/// 原二十二重载、长度边界与四个完整资源内容断言保持不变。\n#[test]\nfn '+name+'() '+body+'\n',encoding='utf-8')
p=root/'scripts/verify_task6_deleted_test_parity.py'; s=p.read_text(encoding='utf-8')
anchor='            "task6_result_set_row_mutation_filter_defaults_delegate_every_physical_method",\n'; assert s.count(anchor)==1
p.write_text(s.replace(anchor,anchor+'            "'+name+'",\n'),encoding='utf-8')
s=(d/'bind_complete_row_mutation_defaults_20261007.py').read_text(encoding='utf-8').replace('result_set_row_mutation_filter_defaults_delegate_every_physical_method','result_set_stream_update_filter_defaults_preserve_all_twenty_two_overloads').replace('row-mutation-defaults','stream-update-defaults').replace('Row mutation defaults','Stream update defaults')
(d/'bind_complete_stream_update_defaults_20261007.py').write_text(s,encoding='utf-8')
s=(d/'audit_complete_row_mutation_defaults_source_floor_20261007.py').read_text(encoding='utf-8').replace('row-mutation-defaults','stream-update-defaults')
(d/'audit_complete_stream_update_defaults_source_floor_20261007.py').write_text(s,encoding='utf-8')
print('Full original stream defaults attached; formal ledger unchanged')
