from pathlib import Path
import subprocess, re, hashlib, json, sys

root = Path.cwd().resolve()
e = root / '.superpowers/sdd/2026-08-24-rdbc-single-standard-unification'
sys.path.insert(0, str(root / 'scripts'))
import verify_task6_deleted_test_parity as v
name = 'pooled_result_set_routes_resource_updates_and_preserves_sqlite_capability_errors'
base = next(b for b in v.derive_base_tests(root) if b.function == name)
original = subprocess.check_output(['git', 'show', v.TASK6_BASE + ':' + base.source_path]).decode('utf-8')
start = original.rfind('#[tokio::test]', 0, original.index('async fn ' + name))
end = original.index('\n#[test]', original.index('async fn ' + name))
test = original[start:end].strip() + '\n'
draft = e / 'drafts/complete_resource_update_base_original_20261007.rs.txt'
assert not draft.exists()
draft.write_text(test, encoding='utf-8', newline='\n')
manifest = {'fixed_base': v.TASK6_BASE, 'identity': base.identity,
            'body_sha256': base.body_sha256, 'key_assertions': base.key_assertions,
            'key_calls': base.key_calls, 'artifact_sha256': hashlib.sha256(draft.read_bytes()).hexdigest()}
(e / 'drafts/complete_resource_update_base_manifest_20261007.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')
test = test.replace('async fn ' + name, 'async fn task6_' + name)
imports = '''    use crate::core::{FilterAdapter, FilterChain, RdbcError};
    use druid_rdbc::result_set::ResultSetExt;
    use druid_rdbc::types::RdbcRowId;
    use resource_update_short_circuit_filter::ResourceUpdateShortCircuitFilter;
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> { log.lock().unwrap().clone() }
'''
test = test.replace('    let call_log =', imports + '    let call_log =', 1)
for slot, context in [(91, 'sqlite-resource-update-short-circuit'), (92, 'sqlite-resource-update-pass-through')]:
    start = test.index('    let physical = factory.create()')
    end = test.index('    let row_id =' if slot == 91 else '    macro_rules!', start)
    chain = 'filter_chain' if slot == 91 else 'pass_chain'
    test = test[:start] + f'''    let (mut statement, mut connection) = task6_sqlite_metadata_statement(
        &factory, {slot}, "{context}", Arc::new({chain}),
    ).await;
    let mut result_set = statement.execute_query("SELECT 1 AS value").await.unwrap();

''' + test[end:]
test = test.replace('Arc::new(PassThroughResultSetFilter)', 'Arc::new(FilterAdapter::new())')
test = re.sub(r'\(&mut connection, (\d+),', r'(ColumnIndex::new(\1).unwrap(),', test)
test = re.sub(r'\(&mut connection, ("[^"]*"),', r'(\1,', test)
test = test.replace('result_set.close_with_connection(&mut connection).unwrap();', 'result_set.close().await.unwrap();\n    statement.close().await.unwrap();\n    connection.close().await.unwrap();')
assert '&mut connection,' not in test
target = root / 'crates/druid/src/core/druid_pooled_result_set/diagnostic_tests.rs'
source = target.read_text(encoding='utf-8')
assert 'mod resource_update_short_circuit_filter;' not in source
target.write_text(source + '\nmod resource_update_short_circuit_filter;\n\n/// 恢复固定BASE十四种资源更新重载、精确错误和实际SQLite能力合同。\n' + test, encoding='utf-8', newline='\n')
fixture = target.parent / 'diagnostic_tests/resource_update_short_circuit_filter.rs'
assert not fixture.exists()
variants = [('Ref','reference'),('Blob','blob'),('Clob','clob'),('Array','array'),('RowId','row_id'),('NClob','n_clob'),('SqlXml','sql_xml')]
head = '''use crate::core::standard_result_set_filter_chain::StandardResultSetFilterChain;
use crate::core::{DruidError, ResultSetFilter};
use druid_rdbc::result_set::{ColumnIndex, ResultSetUpdate};
use std::sync::{Arc, Mutex};

/// 保留七类资源下标和标签更新的原始事件及短路错误。
/// 对应 Java: Filter#resultSet_updateRef/updateBlob/updateClob/updateArray/updateRowId/updateNClob/updateSQLXML。
pub(super) struct ResourceUpdateShortCircuitFilter {
    pub(super) calls: Arc<Mutex<Vec<String>>>,
}

impl ResourceUpdateShortCircuitFilter {
    fn record(&self, event: String) {
        self.calls.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(event);
    }
}

#[async_trait::async_trait]
impl ResultSetFilter for ResourceUpdateShortCircuitFilter {
    fn standard_result_set_update(&self, chain: &mut StandardResultSetFilterChain<'_>, column: ColumnIndex, update: ResultSetUpdate) -> Result<(), DruidError> {
        match update {
'''
for variant, stem in variants:
    head += f'            ResultSetUpdate::{variant}(value) => {{ self.record(format!("result_set_update_{stem}:{{}}:{{value:?}}", column.get())); Ok(()) }},\n'
head += '''            other => chain.result_set_update(column, other),
        }
    }
    fn standard_result_set_update_by_label(&self, chain: &mut StandardResultSetFilterChain<'_>, label: &str, update: ResultSetUpdate) -> Result<(), DruidError> {
        match update {
'''
for variant, stem in variants:
    head += f'            ResultSetUpdate::{variant}(value) => {{ self.record(format!("result_set_update_{stem}_by_label:{{label}}:{{value:?}}")); '
    if variant == 'SqlXml':
        head += 'if label == "fail" { Err(DruidError::DriverError("filtered resource update failure".to_owned())) } else { Ok(()) } },\n'
    else:
        head += 'Ok(()) },\n'
head += '''            other => chain.result_set_update_by_label(label, other),
        }
    }
}
'''
fixture.write_text(head, encoding='utf-8', newline='\n')
print('RESTORED complete resource update original contract', manifest['body_sha256'])
