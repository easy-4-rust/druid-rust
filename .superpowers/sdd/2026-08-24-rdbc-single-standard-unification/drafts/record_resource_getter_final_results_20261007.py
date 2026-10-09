from pathlib import Path
import json,hashlib,re
root=Path.cwd()
e=root/'.superpowers/sdd/2026-08-24-rdbc-single-standard-unification'
freeze=json.loads((e/'task6-resource-getter-final-inputs-2026-10-07.json').read_text())
assert all(hashlib.sha256((root/n).read_bytes()).hexdigest()==h for n,h in freeze['inputs'].items())
for stem,session,exit_code,expected in [('rdbc-regression',95630,0,(474,0)),('wrapper-regression',96537,0,(7,0)),('c2',32117,1,(0,0))]:
    path=e/f'task6-resource-getter-{stem}-2026-10-07.log'
    text=path.read_text(encoding='utf-8-sig')
    blocks=re.findall(r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;',text)
    passed=sum(int(x[0]) for x in blocks);failed=sum(int(x[1]) for x in blocks)
    assert (passed,failed)==expected
    data={'state':'TERMINAL_PASSED' if exit_code==0 else 'TERMINAL_FAILED','exit_code':exit_code,'session_id':session,'toolchain':'1.97.1','log':path.relative_to(root).as_posix(),'log_sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'passed':passed,'failed':failed,'ignored':sum(int(x[2]) for x in blocks),'result_blocks':len(blocks),'unchanged_frozen_inputs':len(freeze['inputs']),'scope':{'rdbc-regression':'complete druid-rdbc package including doctest','wrapper-regression':'three complete affected Wrapper integration targets','c2':'complete C2 --no-execute source gate'}[stem]}
    if stem=='c2':
        assert 'pooled_result_set_routes_resource_updates_and_preserves_sqlite_capability_errors' in text
        data['next_failure']='pooled_result_set_routes_resource_updates_and_preserves_sqlite_capability_errors'
    (e/f'task6-resource-getter-{stem}-2026-10-07.summary.json').write_text(json.dumps(data,indent=2)+'\n',encoding='utf-8')
    print(stem,passed,failed,data['log_sha256'])
