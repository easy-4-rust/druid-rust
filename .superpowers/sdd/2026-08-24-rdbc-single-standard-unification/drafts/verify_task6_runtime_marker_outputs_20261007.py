from pathlib import Path
import hashlib, json, sys
root = Path.cwd()
sys.path.insert(0,str(root/'scripts'))
import verify_task6_deleted_test_parity as v
evidence = root/'.superpowers/sdd/2026-08-24-rdbc-single-standard-unification'
rows = v.parse_ledger(root/v.DEFAULT_LEDGER)
selected = [r for r in rows if r.target_source_path == 'crates/druid/tests/task6_buffered_result_set_canonical_matrix_test.rs']
expected = {r.target_function for r in selected}
cases = {}
for r in selected:
    case = v.disposition_case_id(r)
    if case is not None:
        cases.setdefault(r.target_function, []).append(case)
old = (evidence/'task6-runtime-buffered-marker-nocapture-2026-10-07.log').read_text(encoding='utf-8-sig')
new = (evidence/'task6-runtime-buffered-marker-show-output-2026-10-07.log').read_text(encoding='utf-8-sig')
assert '7 passed; 0 failed;' in old and '7 passed; 0 failed;' in new
assert v.missing_target_functions({'buffered':expected},{'buffered':old})
assert not v.missing_target_functions({'buffered':expected},{'buffered':new})
assert not v.missing_parameterized_case_digests(cases,new)
freeze = json.loads((evidence/'task6-complete-stat-close-error-runtime-inputs-2026-10-07.json').read_text(encoding='utf-8'))
assert all(hashlib.sha256((root/n).read_bytes()).hexdigest() == s for n,s in freeze['inputs'].items())
log = evidence/'task6-complete-stat-close-error-runtime-2026-10-07.log'
summary = {'state':'TERMINAL_FAILED','exit_code':1,'session_id':37151,
    'scope':'full C2 runtime verifier; no aggregate test count available',
    'error':log.read_text(encoding='utf-8-sig').strip(), 'log':log.relative_to(root).as_posix(),
    'log_sha256':hashlib.sha256(log.read_bytes()).hexdigest(), 'unchanged_frozen_inputs':len(freeze['inputs']),
    'reproduction':{'complete_target_passed':7,'failed':0,'show_output_function_and_case_proof':'PASS',
       'parameterized_cases':sum(len(c) for c in cases.values())}}
(evidence/'task6-complete-stat-close-error-runtime-2026-10-07.summary.json').write_text(json.dumps(summary,indent=2)+'\n',encoding='utf-8')
print(json.dumps(summary))
