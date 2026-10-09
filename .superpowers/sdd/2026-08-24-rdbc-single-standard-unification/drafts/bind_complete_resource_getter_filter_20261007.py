from pathlib import Path
import csv
import dataclasses
import io
import sys
import json
import hashlib

root = Path.cwd().resolve()
sys.path.insert(0, str(root / 'scripts'))
import verify_task6_deleted_test_parity as v
import render_task6_migration_document as render

evidence = root / '.superpowers/sdd/2026-08-24-rdbc-single-standard-unification'
summary = json.loads((evidence / 'task6-resource-getter-controls-2026-10-07.summary.json').read_text(encoding='utf-8'))
assert summary['state'] == 'TERMINAL_PASSED' and summary['exit_code'] == 0 and summary['failed'] == 0
assert summary['toolchain'] == '1.97.1'
log = root / summary['log']
assert hashlib.sha256(log.read_bytes()).hexdigest() == summary['log_sha256']
assert 'task6_pooled_result_set_routes_resource_getters_and_real_sqlite_streams ... ok' in log.read_text(encoding='utf-8-sig')
freeze = json.loads((evidence / 'task6-resource-getter-controls-inputs-2026-10-07.json').read_text(encoding='utf-8'))
for name, sha in freeze['inputs'].items():
    data=(root/name).read_bytes()
    if name=='crates/druid/tests/task6_resource_filter_contract_test.rs':
        # 唯一终态后调整是移除unused import；重构旧字节逐项核对，不能忽略文件。
        assert data.count(b'use druid::core::{FilterChain, RdbcError};')==1
        data=data.replace(b'use druid::core::{FilterChain, RdbcError};',b'use druid::core::{FilterChain, Pool, RdbcError};')
    assert hashlib.sha256(data).hexdigest()==sha, name

name = 'pooled_result_set_routes_resource_getters_and_real_sqlite_streams'
identity = 'crates/druid/tests/result_set_filter_semantics_test.rs#' + name
bases = v.derive_base_tests(root)
rows = v.parse_ledger(root / v.DEFAULT_LEDGER)
assert len(bases) == len(rows) == 1283
base_by = {b.identity: b for b in bases}
for row in rows:
    base = base_by[row.base_identity]
    assert tuple(getattr(row, f.name) for f in dataclasses.fields(v.LedgerRow)[:7]) == (
        base.source_path, base.function, base.identity, base.body_sha256,
        base.behavioral_theme, base.key_assertions, base.key_calls)
original = next(row for row in rows if row.base_identity == identity)
assert original.disposition.startswith(v.PARAMETERIZED_PREFIX)
candidate = dataclasses.replace(original, disposition=v.EXACT,
    target_source_path='crates/druid/src/core/druid_pooled_result_set/diagnostic_tests.rs',
    target_function='core::druid_pooled_result_set::diagnostic_tests::task6_' + name,
    case_id='-')
v.verify_ledger((base_by[identity],), (candidate,), root)
updated = tuple(candidate if row.base_identity == identity else row for row in rows)
buffer = io.StringIO(newline='')
writer = csv.writer(buffer, delimiter='\t', lineterminator='\n')
writer.writerow(v.LEDGER_HEADER)
writer.writerows(dataclasses.astuple(row) for row in updated)
(root / v.DEFAULT_LEDGER).write_bytes(buffer.getvalue().encode('utf-8'))
matrix = root / 'crates/druid/tests/task6_result_set_filter_canonical_matrix_test.rs'
text = matrix.read_text(encoding='utf-8')
line = '        "' + name + '",\n'
assert text.count(line) == 1
matrix.write_bytes(text.replace(line, '').encode('utf-8'))
(root / v.DEFAULT_METHOD_CALL_INDEX).write_bytes(v.render_method_call_index(updated).encode('utf-8'))
document = root / render.DOCUMENT
text = document.read_text(encoding='utf-8')
start = text.index(render.TEST_TABLE_HEADER)
end = text.index(render.FINAL_INVARIANTS_HEADING, start)
document.write_bytes((text[:start] + render.render_deleted_test_table(root / v.DEFAULT_LEDGER)
    + '\n' + text[end:]).encode('utf-8'))
print('Resource getter full BASE exact binding verified; all 1283 frozen metadata rows preserved')
print('EXACT', sum(row.disposition == v.EXACT for row in updated))
