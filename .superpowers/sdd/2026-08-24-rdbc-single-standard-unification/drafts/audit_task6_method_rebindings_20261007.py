"""审计所有 C1 证据与当前 C2 索引，生成候选；不修改正式台账或 Rust。"""
from pathlib import Path
import dataclasses, hashlib, json, sys
root = Path.cwd()
sys.path.insert(0, str(root / 'scripts'))
import verify_task6_method_parity as v
matrix = root / v.DEFAULT_MATRIX
index = root / v.DEFAULT_METHOD_CALL_INDEX
rows = v.parse_rows(matrix.read_bytes())
calls = v.parse_method_call_index(index.read_bytes())
updated = []
changes = []
for row in rows:
    selectors = v.parse_behavior_evidence(row.behavior_evidence)
    candidates = set(v.behavior_candidates(row, calls, root))
    valid = all(s in candidates for s in selectors) if candidates else selectors == (v.fallback_selector(row),)
    if valid or row.disposition == 'APPROVED_BREAK':
        updated.append(row)
        continue
    new = v.select_behavior_evidence(row, calls, root)
    candidate = dataclasses.replace(row, behavior_evidence=new)
    # 固定原签名、BASE、目标、转换及全部其他字段。
    for field in dataclasses.fields(row):
        if field.name != 'behavior_evidence':
            assert getattr(candidate, field.name) == getattr(row, field.name)
    updated.append(candidate)
    changes.append({'source_object':row.source_object, 'method':row.method,
        'old':row.behavior_evidence, 'new':new, 'candidate_count':len(candidates)})
draft = root / '.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/drafts'
candidate = draft / 'task6-method-rebinding-candidate-20261007.tsv'
candidate.write_bytes(v.render_rows(updated))
result = {'state':'PREPARED_NOT_BOUND_NOT_EXECUTED', 'rows':len(rows), 'changed':len(changes),
    'current_matrix_sha256':hashlib.sha256(matrix.read_bytes()).hexdigest(),
    'current_index_sha256':hashlib.sha256(index.read_bytes()).hexdigest(),
    'candidate_sha256':hashlib.sha256(candidate.read_bytes()).hexdigest(), 'changes':changes}
(draft / 'task6-method-rebinding-candidate-20261007.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
print('C1_ROWS',len(rows),'PREPARED_REBINDINGS',len(changes))
print('FORMAL_MATRIX_UNCHANGED; ORIGINAL_SIGNATURE_AND_TARGET_FIELDS_PRESERVED')
