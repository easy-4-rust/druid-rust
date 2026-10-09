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
bases = {test.function: test for test in v.parse_test_functions(source_path, source)}
scalar_name = 'pooled_result_set_routes_all_scalar_getters_through_java_filter_overloads'
base = bases[scalar_name]
candidate_path = drafts / 'complete_scalar_filter_contract_draft.rs'
candidate_source = candidate_path.read_text(encoding='utf-8')
candidate = v.parse_test_functions(str(candidate_path.relative_to(root)), candidate_source)[0]
v.verify_observation_floor(base, v.CurrentTest(candidate.source_path, candidate.function, candidate.body, candidate.body))
original_events = re.findall(r'"result_set_get_[^"]+"', base.body)
candidate_events = re.findall(r'"result_set_get_[^"]+"', candidate.body)
assert len(original_events) == 18 and original_events == candidate_events
assert v.parse_counted_names(base.key_assertions) == v.parse_counted_names(candidate.key_assertions)
report = {'state': 'PREPARATION_SOURCE_OBSERVATION_ONLY_NOT_COMPILED_NOT_RUN_NOT_BOUND',
          'base': v.TASK6_BASE, 'scalar_base_identity': base.identity,
          'scalar_base_body_sha256': base.body_sha256,
          'scalar_candidate_sha256': hashlib.sha256(candidate_source.encode('utf-8')).hexdigest(),
          'scalar_assertions': candidate.key_assertions, 'scalar_ordered_events': candidate_events,
          'scalar_density_floor': 'PASS', 'next_original_contracts': []}
for name in ('pooled_result_set_routes_dynamic_statement_identity_through_filter',
             'pooled_result_set_routes_metadata_platform_handle_through_filter'):
    original = bases[name]
    output = drafts / ('next_filter_base_' + name + '.rs.txt')
    output.write_bytes(('#[tokio::test]\nasync fn ' + name + '() ' + original.body + '\n').encode('utf-8'))
    report['next_original_contracts'].append({'identity': original.identity,
        'body_sha256': original.body_sha256, 'key_assertions': original.key_assertions,
        'key_calls': original.key_calls, 'extracted_artifact': output.name,
        'artifact_sha256': hashlib.sha256(output.read_bytes()).hexdigest()})
(drafts / 'next_filter_contract_preparation_audit_20261007.json').write_bytes(
    (json.dumps(report, ensure_ascii=False, indent=2) + '\n').encode('utf-8'))
print('Scalar original assertion distribution, ordered18 events and density floor PASS; source-only')
print('Two complete next BASE functions extracted with authoritative metadata; not restored/run/bound')
