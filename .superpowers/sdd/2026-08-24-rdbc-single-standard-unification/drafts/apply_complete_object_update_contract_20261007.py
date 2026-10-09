"""在全仓终态与输入冻结一致后接入合同；明确保留已复现的网络失败门禁。"""
from pathlib import Path
import hashlib
import json
import subprocess

root = Path.cwd().resolve()
evidence = root / '.superpowers/sdd/2026-08-24-rdbc-single-standard-unification'
drafts = evidence / 'drafts'
summary_path = evidence / 'task6-complete-navigation-filter-workspace-2026-10-07.summary.json'
assert summary_path.is_file(), 'Workspace terminal evidence required; preserve live build inputs'
summary = json.loads(summary_path.read_text(encoding='utf-8'))
assert summary['toolchain'] == '1.97.1'
assert hashlib.sha256((root / summary['log']).read_bytes()).hexdigest() == summary['log_sha256']
if summary['state'] == 'TERMINAL_PASSED':
    assert summary['exit_code'] == 0 and summary['failed'] == 0
else:
    # 网络失败阻止完整验收，但不阻止独立RDBC合同实施；不重写全仓失败证据。
    assert summary['state'] == 'TERMINAL_FAILED' and summary['exit_code'] == 101
    expected_failures = {
        'remote_http_and_grpc_use_tls_and_require_authentication',
        'reset_result_survives_real_tonic_half_close_and_reconnect_without_reexecution',
        'resync_cutoff_preserves_live_update_queued_after_snapshot_sampling',
        'resync_required_supersedes_pending_with_monotonic_registry_full_snapshot',
        'runtime_shutdown_flushes_last_aggregated_batch_over_grpc',
        'same_command_id_with_different_payload_fails_closed',
        'shutdown_timeout_reports_only_actual_pending_and_queued_work',
        'snapshot_transaction_capacity_is_enforced_before_staging',
        'two_w_plus_one_unique_commands_fail_closed_without_successor_execution',
        'unavailable_admin_shutdown_reports_exporter_and_client_work_as_unflushed',
        'validated_w_plus_one_commands_survive_teardown_backpressure_and_live_traffic_resumes',
    }
    assert summary['failed'] == len(summary['failed_tests']) == 11
    assert set(summary['failed_tests']) == expected_failures
    reproduction = json.loads((evidence / 'task6-navigation-admin-network-reproduction-2026-10-07.summary.json').read_text(encoding='utf-8'))
    assert reproduction['state'] == 'TERMINAL_FAILED' and reproduction['exit_code'] == 101
    assert reproduction['toolchain'] == '1.97.1' and reproduction['failed'] == 11
    assert set(reproduction['failed_tests']) == expected_failures
    assert hashlib.sha256((root / reproduction['log']).read_bytes()).hexdigest() == reproduction['log_sha256']
    print('Known Admin network baseline remains RED; no full-workspace or release gate is closed')
freeze = json.loads((evidence / 'task6-complete-navigation-filter-workspace-inputs-2026-10-07.json').read_text(encoding='utf-8'))
assert len(freeze['inputs']) == 1023
assert all((root / name).is_file() and hashlib.sha256((root / name).read_bytes()).hexdigest() == sha
           for name, sha in freeze['inputs'].items()), 'Workspace inputs changed after verification'

# 对应 workspace 的 Cargo 必须已经终止；其他仓库的进程不受此脚本影响。
processes = subprocess.run(
    ['powershell', '-NoProfile', '-Command',
     "Get-CimInstance Win32_Process -Filter \"name='cargo.exe'\" | Select-Object -ExpandProperty CommandLine"],
    capture_output=True, text=True, check=True,
).stdout
assert 'task6_rust_1971_20261007' not in processes
assert 'test --workspace' not in processes, 'A workspace regression is still live'

manifest = json.loads((drafts / 'complete_object_update_source_audit_20261007.json').read_text(encoding='utf-8'))
assert manifest['state'] == 'SOURCE_ONLY_PREPARATION_NOT_COMPILED_NOT_RUN_NOT_BOUND'
assert manifest['observation_floor'] == 'PASS'
assert manifest['base_assertions'] == manifest['candidate_assertions'] == 'assert:1;assert_eq:8'
for name, sha in manifest['artifacts'].items():
    assert hashlib.sha256((drafts / name).read_bytes()).hexdigest() == sha, name

parent = root / 'crates/druid/src/core/druid_pooled_result_set'
test_file = parent / 'diagnostic_tests.rs'
original = test_file.read_text(encoding='utf-8')
function = 'task6_pooled_result_set_routes_object_updates_and_preserves_sqlite_capability_errors'
assert 'async fn ' + function not in original, 'Existing implementation must not be duplicated'
fixture_map = {
    'complete_object_update_filter_20261007.rs.txt': 'object_update_short_circuit_filter.rs',
    'complete_object_update_vendor_probe_20261007.rs.txt': 'object_update_vendor_probe.rs',
}
for target in fixture_map.values():
    assert not (parent / 'diagnostic_tests' / target).exists(), target
addition = '\n' + ''.join('mod ' + Path(target).stem + ';\n' for target in fixture_map.values())
addition += '\n' + (drafts / 'complete_object_update_contract_20261007.rs.txt').read_text(encoding='utf-8')
assert addition.count('async fn ' + function) == 1
for source, target in fixture_map.items():
    (parent / 'diagnostic_tests' / target).write_bytes((drafts / source).read_bytes())
test_file.write_bytes((original + addition).encode('utf-8'))
print('Complete original Object update test and two single-object fixtures attached')
print('NOT_RUN_NOT_BOUND: no production routing or formal ledger change')
