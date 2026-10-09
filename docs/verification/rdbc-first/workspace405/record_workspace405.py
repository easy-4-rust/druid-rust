from pathlib import Path
from datetime import datetime,timezone
import hashlib,json,re,shutil
b=Path(__file__).parent
w=Path('F:/codex_build_cache/d177')
e=Path('E:/workspaces/workspace-github-easy-4-rust/druid-rust')
run=b/'task7-candidate-verification403'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
summary=json.loads((run/'summary.json').read_text())
record=next(r for r in summary['commands'] if r['name']=='workspace-complete')
assert record['exit_code']==0 and record['changed_inputs']==[]
assert record['passed']==4198 and record['failed']==record['ignored']==0
expected=['test','--workspace','--all-features','--locked','--offline','-j1','--no-fail-fast']
assert record['command'][1:]==expected
log=Path(record['log'])
assert sha(log)==record['sha256']
text=log.read_text(encoding='utf-8',errors='replace')
blocks=re.findall(r'^test result: .*?(\d+) passed; (\d+) failed; (\d+) ignored;',text,re.M)
assert tuple(sum(int(row[i]) for row in blocks) for i in range(3))==(4198,0,0)
frozen=json.loads((run/'frozen-inputs.json').read_text())
assert len(frozen)==1278 and all(sha(w/p)==h for p,h in frozen.items())
d=e/'docs/verification/rdbc-first/workspace405'
assert not d.exists()
d.mkdir()
files={'workspace-complete.log':log,'frozen-inputs.json':run/'frozen-inputs.json','verify_candidate403.py':b/'verify_candidate403.py','record_workspace405.py':Path(__file__)}
hashes={}
for name,source in files.items():
    shutil.copyfile(source,d/name)
    assert sha(source)==sha(d/name)
    hashes[name]=sha(d/name)
evidence=dict(scope='CURRENT403_COMPLETE_WORKSPACE_TEST_ONLY_NOT_ALL_GATES',observed_at=datetime.now(timezone.utc).isoformat(),
    command=record,result_blocks=len(blocks),frozen_inputs_unchanged=1278,
    full_runner_terminal=summary['terminal'],observed_running_command=summary.get('running_command'),
    product_candidate_integrated=False,top_level_tasks_remaining=18,file_sha256=hashes)
(d/'summary.json').write_text(json.dumps(evidence,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
(d/'README.md').write_text('# 当前候选完整 Workspace 测试证据 405\n\n403 候选完整全 features 测试实际 exit 0：4198 PASS、0 FAIL、0 ignored。\n日志结果块重新统计及 SHA 均一致，1278 冻结输入逐项核验不变。\n\n这份证据只证明本次完整测试命令，不证明后续 Clippy、doc、fmt、完整 C1/C2、\nLLVM coverage、API、SemVer、MSRV、feature 矩阵或发布验收已通过。\n候选尚未集成到正式源码，18 顶层任务仍开放。所有材料以普通文件保存。\n',encoding='utf-8')
ignore=e/'.gitignore'
line='!/docs/verification/rdbc-first/workspace405/**/*.json\n!/docs/verification/rdbc-first/workspace405/**/*.log\n'
with ignore.open('a',encoding='utf-8',newline='') as out:out.write(line)
attrs=e/'.gitattributes'
with attrs.open('a',encoding='utf-8',newline='') as out:out.write('/docs/verification/rdbc-first/workspace405/** -text whitespace=cr-at-eol\n')
note=f'\n\n2026-10-09 修改后403完整Workspace终态405：原workspace/all-features/locked/offline/-j1/no-fail-fast实际exit0，4198PASS/0FAIL/0ignored，日志SHA {record["sha256"]}；日志重新统计与原结果一致，1278冻结输入逐项不变。本轮来源包含402候选修复，不能与旧319混用。证据及执行源码以普通文件保存在docs/verification/rdbc-first/workspace405/，不新增ZIP/PDB/编译产物。同一session56719继续严格Clippy及doc/fmt/完整C1-C2；这些后续门禁尚未取得整轮终态。正式E业务候选未集成，LLVM/API/SemVer/MSRV/feature/实库全矩阵及18顶层任务仍OPEN。\n'
for relative in ['.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/progress.md','docs/verification/rdbc-first/status.md','docs/verification/rdbc-first/task7-standard-resource-candidates-2026-10-08.md']:
    with (e/relative).open('a',encoding='utf-8',newline='') as out:out.write(note)
print(json.dumps(dict(directory=str(d),passed=4198,failed=0,frozen_inputs_unchanged=1278,log_sha256=record['sha256'],full_runner_terminal=summary['terminal'])))
