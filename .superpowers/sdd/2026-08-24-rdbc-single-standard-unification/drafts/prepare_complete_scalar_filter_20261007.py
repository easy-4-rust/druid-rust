from pathlib import Path
import hashlib
import json
import re
import subprocess
import sys

root = Path.cwd().resolve()
sys.path.insert(0, str(root / 'scripts'))
import verify_task6_deleted_test_parity as v
drafts = root / '.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/drafts'
source_path = 'crates/druid/tests/result_set_filter_semantics_test.rs'
source = subprocess.check_output(['git', 'show', v.TASK6_BASE + ':' + source_path], cwd=root).decode('utf-8')
name = 'pooled_result_set_routes_all_scalar_getters_through_java_filter_overloads'
base = next(test for test in v.parse_test_functions(source_path, source) if test.function == name)
original = '#[tokio::test]\nasync fn ' + name + '() ' + base.body + '\n'
(drafts / 'complete_scalar_filter_base_original.rs.txt').write_bytes(original.encode('utf-8'))

current = (root / 'crates/druid/src/core/druid_pooled_result_set/diagnostic_tests.rs').read_text(encoding='utf-8')
object_start = current.index('async fn task6_pooled_result_set_routes_all_object_overloads_and_preserves_argument_identity')
start = current.index('    let factory = ToastyConnectionFactory::new', object_start)
end = current.index('    let empty_map = ', start)
setup = current[start:end].replace('75,', '73,').replace('sqlite-object-filter', 'sqlite-scalar-filter')
tail = base.body[base.body.index('    // Filter 在 raw ResultSet 前短路'):]
tail = re.sub(r'&mut connection,\s*(\d+)', lambda match: 'ColumnIndex::new(' + match.group(1) + ').unwrap()', tail)
tail = re.sub(r'&mut connection,\s*', '', tail)
tail = tail.replace('result_set.close_with_connection(&mut connection).unwrap();',
                    'result_set.close().await.unwrap();\n    statement.close().await.unwrap();\n    connection.close().await.unwrap();')
assert '&mut connection' not in tail
header = '''/// 完整保留九类标量原重载、顺序、短路和精确错误计数；候选未运行。
#[tokio::test]
async fn task6_pooled_result_set_routes_all_scalar_getters_through_java_filter_overloads() {
    use crate::core::{FilterChain, RdbcError};
    use druid_rdbc::result_set::ResultSetExt;
    use scalar_short_circuit_filter::ScalarShortCircuitFilter;
    use std::sync::Mutex;
    fn calls(log: &Arc<Mutex<Vec<String>>>) -> Vec<String> { log.lock().unwrap().clone() }
    let call_log = Arc::new(Mutex::new(Vec::new()));
    let mut filter_chain = FilterChain::new();
    filter_chain.add_result_set(Arc::new(ScalarShortCircuitFilter { calls: Arc::clone(&call_log) }));
'''
candidate = header + setup + tail + '\n'
assert base.body.count('assert_eq!') == candidate.count('assert_eq!')
assert base.body.count('assert!') == candidate.count('assert!')
for event in re.findall(r'"result_set_get_[^"]+"', base.body):
    assert event in candidate
assert 'filtered double failure' in candidate and 'statement.exception_count(), 1' in candidate
target = drafts / 'complete_scalar_filter_contract_draft.rs'
target.write_bytes(candidate.encode('utf-8'))
manifest = {'state': 'PREPARATION_NOT_COMPILED_NOT_RUN_NOT_BOUND', 'base': v.TASK6_BASE,
    'base_identity': base.identity, 'base_file_sha256': hashlib.sha256(source.encode()).hexdigest(),
    'base_body_sha256': base.body_sha256, 'assert_eq': base.body.count('assert_eq!'),
    'assert': base.body.count('assert!'), 'ordered_success_events': 18,
    'drafts': {path.name: hashlib.sha256(path.read_bytes()).hexdigest() for path in
        [drafts / 'complete_scalar_filter_base_original.rs.txt', target, drafts / 'complete_scalar_short_circuit_filter_draft.rs']}}
(drafts / 'complete_scalar_filter_preparation_manifest_20261007.json').write_bytes(
    (json.dumps(manifest, ensure_ascii=False, indent=2) + '\n').encode('utf-8'))
print('Complete scalar BASE observations preserved:', manifest['assert_eq'], manifest['assert'], '18 events')
