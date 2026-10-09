from pathlib import Path
from datetime import datetime,timezone
from collections import Counter
import hashlib,json,subprocess
from integration_preflight404 import validate_preflight,IntegrationRefused

b=Path(__file__).parent
w=Path('F:/codex_build_cache/d177')
e=Path('E:/workspaces/workspace-github-easy-4-rust/druid-rust')
d=b/'task7-integration-preflight404'
d.mkdir(exist_ok=False)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
frozen_path=b/'task7-candidate-verification403/frozen-inputs.json'
frozen=json.loads(frozen_path.read_text())
assert len(frozen)==1278
assert all(sha(w/p)==h for p,h in frozen.items())
old_path=b/'task7-e-integration-current-snapshot383/summary.json'
old=json.loads(old_path.read_text())
rows=[]
for prior in old['files']:
    p=prior['path']
    pre=sha(e/p) if (e/p).is_file() else None
    assert pre==prior['e_preimage_sha256'],f'E preimage changed: {p}'
    rows.append(dict(path=p,candidate_sha256=frozen[p],e_preimage_sha256=pre,
        state='MISSING_IN_E' if pre is None else 'IDENTICAL' if pre==frozen[p] else 'DIFFERENT',
        prior383_candidate_sha256=prior['candidate_sha256'],
        candidate_changed_since383=frozen[p]!=prior['candidate_sha256'],
        prior_static_review=prior.get('review'),applied=False))
helper='scripts/windows_bounded_process.py'
assert helper not in {r['path'] for r in rows}
assert not (e/helper).exists()
rows.append(dict(path=helper,candidate_sha256=frozen[helper],e_preimage_sha256=None,state='MISSING_IN_E',candidate_changed_since383=True,applied=False))
assert len(rows)==1258
for row in old['e_only']:
    assert sha(e/row['path'])==row['e_preimage_sha256']
snapshot=dict(scope='CURRENT403_PREIMAGE_SNAPSHOT_NOT_APPLY_OR_ACCEPTANCE',observed_at=datetime.now(timezone.utc).isoformat(),
    head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=e,text=True).strip(),
    candidate_frozen_manifest_sha256=sha(frozen_path),candidate_frozen_count=1278,selected_source_count=1258,
    counts=dict(Counter(r['state'] for r in rows)),files=rows,e_only=old['e_only'],
    candidate_changed_since383=[r['path'] for r in rows if r['candidate_changed_since383']],
    production_applied=False,retirement_authorized=False)
(d/'snapshot.json').write_text(json.dumps(snapshot,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
summary_path=b/'task7-candidate-verification403/summary.json'
summary=json.loads(summary_path.read_text())
try:
    result=validate_preflight(w,e,frozen,snapshot,summary)
except IntegrationRefused as error:
    result=dict(state='REFUSED',reason=str(error),production_applied=False)
assert result['state']=='REFUSED' and result['reason']=='verification still running'
(d/'live-refusal.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
python='C:/Espressif/tools/python/python.exe'
cmd=[python,'-B','-X','utf8','-m','unittest','test_integration_preflight388.PreflightTests','test_integration_command_identity394.CommandIdentityTests','test_integration_preflight404.Candidate403Tests']
with (d/'tests.log').open('wb') as out:
    r=subprocess.run(cmd,cwd=b,stdout=out,stderr=subprocess.STDOUT)
assert r.returncode==0
assert all(sha(w/p)==h for p,h in frozen.items())
record=dict(scope='PREFLIGHT_TOOL_AND_PREIMAGE_ONLY_NOT_PRODUCT_COMPLETION',tests_command=cmd,tests_exit_code=r.returncode,
    tests_log_sha256=sha(d/'tests.log'),snapshot_sha256=sha(d/'snapshot.json'),
    live_refusal=result,candidate_frozen_inputs_unchanged=1278,formal_preimages_unchanged=1257,
    new_helper_formal_path_unoccupied=True,production_applied=False,
    checker_sha256=sha(b/'integration_preflight404.py'),runner403_sha256=sha(b/'verify_candidate403.py'))
(d/'summary.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8')
print(json.dumps(record))
print((d/'tests.log').read_text(encoding='utf-8')[-500:])
