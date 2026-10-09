"""接入完整合同候选；必须先取得冻结Object workspace真实通过终态。"""
from pathlib import Path
import hashlib
import json

root = Path.cwd().resolve()
evidence = root / '.superpowers/sdd/2026-08-24-rdbc-single-standard-unification'
drafts = evidence / 'drafts'
summary_path = evidence / 'task6-complete-object-filter-workspace-2026-10-07.summary.json'
assert summary_path.is_file(), 'Complete workspace terminal evidence required; do not mutate live inputs'
summary = json.loads(summary_path.read_text(encoding='utf-8'))
assert summary['state'] == 'TERMINAL_PASSED' and summary['exit_code'] == 0 and summary['failed'] == 0
log_path = root / summary['log']
assert hashlib.sha256(log_path.read_bytes()).hexdigest() == summary['log_sha256']
compiler_summary_path = evidence / 'task6-rust-1971-workspace-all-targets-check-2026-10-07.summary.json'
assert compiler_summary_path.is_file(), 'Default compiler verification terminal evidence required; preserve its live inputs'
compiler_summary = json.loads(compiler_summary_path.read_text(encoding='utf-8'))
assert compiler_summary['state'] == 'TERMINAL_PASSED' and compiler_summary['exit_code'] == 0
assert hashlib.sha256((root / compiler_summary['log']).read_bytes()).hexdigest() == compiler_summary['log_sha256']
freeze = json.loads((evidence / 'task6-complete-object-filter-workspace-inputs-2026-10-07.json').read_text(encoding='utf-8'))
assert all((root / path).is_file() and hashlib.sha256((root / path).read_bytes()).hexdigest() == checksum
           for path, checksum in freeze['inputs'].items()), 'Run source freeze changed; revalidate before application'
manifest = json.loads((drafts / 'complete_scalar_filter_preparation_manifest_20261007.json').read_text(encoding='utf-8'))
assert manifest['state'] == 'PREPARATION_NOT_COMPILED_NOT_RUN_NOT_BOUND'
for name, checksum in manifest['drafts'].items():
    assert hashlib.sha256((drafts / name).read_bytes()).hexdigest() == checksum, name

parent = root / 'crates/druid/src/core/druid_pooled_result_set'
test_file = parent / 'diagnostic_tests.rs'
original = test_file.read_text(encoding='utf-8')
fixture_map = {
    'complete_scalar_short_circuit_filter_draft.rs': 'scalar_short_circuit_filter.rs',
    'complete_scalar_read_probe_draft.rs': 'scalar_read_probe.rs',
    'complete_ordered_scalar_filter_draft.rs': 'ordered_scalar_filter.rs',
}
case_files = ('complete_scalar_filter_contract_draft.rs', 'complete_scalar_filter_guards_draft.rs',
              'complete_scalar_filter_default_order_draft.rs')
names = ('task6_pooled_result_set_routes_all_scalar_getters_through_java_filter_overloads',
         'task6_scalar_filter_all_overloads_obey_hidden_closed_and_expired_lease_guards',
         'task6_scalar_filter_preserves_all_native_overloads_order_short_circuit_and_error_unwind')
for name in names:
    assert 'async fn ' + name not in original, 'Do not duplicate an existing implementation'
for target in fixture_map.values():
    assert not (parent / 'diagnostic_tests' / target).exists(), target
addition = '\n'
for target in fixture_map.values():
    addition += 'mod ' + Path(target).stem + ';\n'
addition += '\n' + '\n'.join((drafts / name).read_text(encoding='utf-8') for name in case_files)
assert all(addition.count('async fn ' + name) == 1 for name in names)
for source, target in fixture_map.items():
    (parent / 'diagnostic_tests' / target).write_bytes((drafts / source).read_bytes())
test_file.write_bytes((original + addition).encode('utf-8'))
print('Three complete scalar contract tests and three single-object fixtures attached; no production routing change')
print('NOT_RUN_NOT_BOUND: run real RED/GREEN before ledger mutation')
