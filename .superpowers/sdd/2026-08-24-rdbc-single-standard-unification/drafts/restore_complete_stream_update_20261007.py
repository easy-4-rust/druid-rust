from pathlib import Path
import re

root = Path.cwd()
e = root / '.superpowers/sdd/2026-08-24-rdbc-single-standard-unification'
p = root / 'crates/druid/src/core/druid_pooled_result_set/diagnostic_tests.rs'
current = p.read_text(encoding='utf-8')
name = 'pooled_result_set_routes_stream_updates_and_preserves_sqlite_capability_errors'
assert 'async fn task6_' + name not in current
s = (e / 'drafts/complete_stream_update_base_original_20261007.rs.txt').read_text(encoding='utf-8')
s = s.replace('async fn ' + name, 'async fn task6_' + name)
s = s.replace('() {\n', '''() {
    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use stream_update_short_circuit_filter::StreamUpdateShortCircuitFilter;
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        log.lock().unwrap().clone()
    }
''', 1)
pattern = r'    let physical = factory.create\(\).await.unwrap\(\);.*?    let mut result_set = statement\n.*?        \.unwrap\(\);\n'
matches = list(re.finditer(pattern, s, re.S))
assert len(matches) == 2
for m, context_id, context_name, chain in reversed(list(zip(matches, [95,96], ['sqlite-stream-update-short-circuit','sqlite-stream-update-pass-through'], ['filter_chain','pass_chain']))):
    replacement = f'''    let (mut statement, mut connection) = task6_sqlite_metadata_statement(
        &factory, {context_id}, "{context_name}", Arc::new({chain}),
    ).await;
    let mut result_set = statement.execute_query("SELECT X'010203' AS value").await.unwrap();
'''
    s = s[:m.start()] + replacement + s[m.end():]
s = s.replace('PassThroughResultSetFilter', 'FilterAdapter::new()')
pat = re.compile(r'result_set\s*\.\s*(update_(?:ascii|binary|character|n_character)_stream\w*)\(')
matches = list(pat.finditer(s))
assert len(matches) == 45
for m in reversed(matches):
    start = m.end(); pos = start; depth = 1
    while depth:
        if s[pos] == '(': depth += 1
        elif s[pos] == ')': depth -= 1
        pos += 1
    args = []; token_start = start; nested = 0
    for i in range(start, pos-1):
        if s[i] in '([{': nested += 1
        elif s[i] in ')]}': nested -= 1
        elif s[i] == ',' and nested == 0:
            args.append(s[token_start:i].strip()); token_start = i+1
    tail = s[token_start:pos-1].strip()
    if tail: args.append(tail)
    assert args.pop(0) == '&mut connection'
    method = m[1]
    by_label = '_by_label' in method
    target, value = args[:2]
    value = value + '.cloned()' if value.startswith('Some(&') else value
    family = next(x for x in ['n_character','character','binary','ascii'] if method.startswith('update_'+x+'_stream'))
    variant = {'ascii':'AsciiStream','binary':'BinaryStream','character':'CharacterStream','n_character':'NCharacterStream'}[family]
    field = 'stream' if family in ['ascii','binary'] else 'reader'
    length_type = 'RdbcStreamLength' if field == 'stream' else 'RdbcCharacterLength'
    length = length_type + '::' + ('Int('+args[2]+')' if '_with_int_length' in method else 'Long('+args[2]+')' if '_with_length' in method else 'Unspecified')
    target = target if by_label else 'ColumnIndex::new('+target+').unwrap()'
    new_method = 'update_value_by_label' if by_label else 'update_value'
    replacement = f'result_set.{new_method}({target}, ResultSetUpdate::{variant} {{ {field}: {value}, length: {length} }})'
    s = s[:m.start()] + replacement + s[pos:]
s = s.replace('result_set.close_with_connection(&mut connection).unwrap();','result_set.close().await.unwrap();\n    statement.close().await.unwrap();\n    connection.close().await.unwrap();')
s = s.replace('!result_set.is_closed()', '!result_set.is_closed().unwrap()')
fixture = '''use crate::core::standard_result_set_filter_chain::StandardResultSetFilterChain;
use crate::core::{DruidError, ResultSetFilter};
use druid_rdbc::result_set::{ColumnIndex, ResultSetUpdate};
use druid_rdbc::types::{RdbcInputStream, RdbcReader, RdbcStreamLength, RdbcCharacterLength};
use std::sync::{Arc, Mutex};

/// 完整保留22个流更新重载的原事件、长度、共享流消费和精确错误。
/// 对应 Java: Filter#resultSet_updateAsciiStream/updateBinaryStream/updateCharacterStream/updateNCharacterStream。
pub(super) struct StreamUpdateShortCircuitFilter {
    pub(super) calls: Arc<Mutex<Vec<String>>>,
}
impl StreamUpdateShortCircuitFilter {
    fn record(&self, event: String) {
        self.calls.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(event);
    }
    fn record_stream(&self, name: &str, target: &str, by_label: bool, value: Option<RdbcInputStream>, length: RdbcStreamLength) -> Result<(), DruidError> {
        let presence = if value.is_some() { "some" } else { "none" };
        let label_suffix = if by_label { "_by_label" } else { "" };
        let (overload, tail) = match length {
            RdbcStreamLength::Unspecified => {
                // 只在原无长度下标入口消费一个字节；clone必须共享调用者读位置。
                if !by_label { if let Some(value) = value { let mut byte = [0u8;1]; assert_eq!(value.read(&mut byte)?, 1); } }
                ("", String::new())
            },
            RdbcStreamLength::Int(length) => ("_with_int_length", format!(":{length}")),
            RdbcStreamLength::Long(length) => ("_with_length", format!(":{length}")),
        };
        self.record(format!("result_set_update_{name}{label_suffix}{overload}:{target}:{presence}{tail}"));
        Ok(())
    }
    fn record_reader(&self, name: &str, target: &str, by_label: bool, value: Option<RdbcReader>, length: RdbcCharacterLength) -> Result<(), DruidError> {
        let presence = if value.is_some() { "some" } else { "none" };
        let label_suffix = if by_label { "_by_label" } else { "" };
        let long_overload = matches!(length, RdbcCharacterLength::Long(_));
        let (overload, tail) = match length {
            RdbcCharacterLength::Unspecified => {
                // UTF-16单位消费与原Reader合同相同，带长度入口不得预读。
                if !by_label { if let Some(value) = value { let mut unit = [0u16;1]; assert_eq!(value.read_utf16(&mut unit)?, 1); } }
                ("", String::new())
            },
            RdbcCharacterLength::Int(length) => ("_with_int_length", format!(":{length}")),
            RdbcCharacterLength::Long(length) => ("_with_length", format!(":{length}")),
        };
        self.record(format!("result_set_update_{name}{label_suffix}{overload}:{target}:{presence}{tail}"));
        if name == "n_character_stream" && by_label && long_overload && target == "fail" {
            Err(DruidError::DriverError("filtered stream update failure".to_owned()))
        } else { Ok(()) }
    }
}
#[async_trait::async_trait]
impl ResultSetFilter for StreamUpdateShortCircuitFilter {
'''
for by_label in [False,True]:
    method = 'standard_result_set_update' + ('_by_label' if by_label else '')
    arg = 'label: &str' if by_label else 'column: ColumnIndex'
    target = 'label' if by_label else '&column.get().to_string()'
    flag = str(by_label).lower()
    fixture += f'fn {method}(&self, chain: &mut StandardResultSetFilterChain<\'_>, {arg}, update: ResultSetUpdate) -> Result<(), DruidError> {{\nmatch update {{\n'
    for family,variant,field,helper in [('ascii','AsciiStream','stream','record_stream'),('binary','BinaryStream','stream','record_stream'),('character','CharacterStream','reader','record_reader'),('n_character','NCharacterStream','reader','record_reader')]:
        fixture += f'ResultSetUpdate::{variant} {{ {field}, length }} => self.{helper}("{family}_stream", {target}, {flag}, {field}, length),\n'
    fixture += ('other => chain.result_set_update_by_label(label, other),\n' if by_label else 'other => chain.result_set_update(column, other),\n') + '}\n}\n'
fixture += '}\n'
q = p.parent / 'diagnostic_tests/stream_update_short_circuit_filter.rs'
assert not q.exists(); q.write_bytes(fixture.encode())
p.write_bytes((current+'\nmod stream_update_short_circuit_filter;\n\n/// 恢复原22个流更新重载、共享读位置与真实SQLite能力错误。\n'+s).encode())
print('Full stream update original restored with canonical descriptors and faithful consumption fixture')
