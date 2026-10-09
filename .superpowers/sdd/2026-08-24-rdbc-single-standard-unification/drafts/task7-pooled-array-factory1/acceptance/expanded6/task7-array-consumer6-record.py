"""Persist expanded Array contract evidence without changing frozen production inputs."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import zipfile
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
root=Path('E:/workspaces/workspace-github-easy-4-rust/druid-rust')
mirror=root/'.superpowers/sdd/2026-08-24-rdbc-single-standard-unification'
candidate=base/'task7-array-source-candidate1'; mutation=base/'task7-array-capability-mutation1'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
contract=json.loads((candidate/'contract6.summary.json').read_text(encoding='utf-8'))
negative=json.loads((mutation/'summary.json').read_text(encoding='utf-8'))
assert contract['terminal'] and contract['commands'][-1]['exit_code']==0
assert negative['terminal'] and negative['candidate_sources_unchanged']
compiled=json.loads((candidate/'compile2.summary.json').read_text(encoding='utf-8'))
sources={p.relative_to(candidate).as_posix():sha(p) for p in (candidate/'src').rglob('*.rs')}
assert sources==compiled['sources']
mutated={p.relative_to(mutation).as_posix():sha(p) for p in (mutation/'src').rglob('*.rs')}
assert sorted(k for k in sources if sources[k]!=mutated[k])==['src/core/druid_array.rs']
assert set(mutated)==set(sources)
snapshot=json.loads((base/'task6-current-full-validation1.inputs.json').read_text(encoding='utf-8'))
changed=[p for p,digest in snapshot['inputs'].items() if not (root/p).is_file() or sha(root/p)!=digest]
assert not changed,changed
state={'observed':datetime.datetime.now(datetime.timezone.utc).isoformat(),'production_applied':False,'formal_cargo_certified':False,'frozen_input_count':len(snapshot['inputs']),'changed_inputs':changed,'sources':sources,'mirrors':{},'results':{},'mutation_only_source':'src/core/druid_array.rs','spec':'docs/superpowers/plans/2026-08-24-rdbc-single-standard-unification.md#task-7','prior_source_archive':{'path':str(base/'task7-full-druid-rdbc-source-candidate8.zip'),'sha256':sha(base/'task7-full-druid-rdbc-source-candidate8.zip')}}
for label in ('before6','after6'):
    log=(candidate/(label+'-runtime.log')).read_text(encoding='utf-8')
    result=re.search(r'test result: .*? (\d+) passed; (\d+) failed; (\d+) ignored;',log)
    assert result
    state['results'][label]={'passed':int(result[1]),'failed':int(result[2]),'ignored':int(result[3])}
assert state['results']['before6']=={'passed':29,'failed':13,'ignored':0}
assert state['results']['after6']=={'passed':42,'failed':0,'ignored':0}
build=json.loads((base/'task7-array-consumer6-build.summary.json').read_text(encoding='utf-8'))
assert build['prior_tests_verbatim']==36 and build['tests']==42
state['consumer_build']=build
draft=mirror/'drafts/task7-pooled-array-factory1/acceptance/expanded6'
files=[]
for name in ('task7-array-consumer6-build.py','task7-array-consumer6-build.summary.json','task7-pooled-array-consumer6.rs','task7-pooled-array-contract6-run.py','task7-array-capability-mutation1.py','task7-array-consumer6-record.py'):
    files.append((base/name,name))
for p in sorted(candidate.glob('*6*.log')): files.append((p,'candidate/'+p.name))
files.append((candidate/'contract6.summary.json','candidate/contract6.summary.json'))
for p in sorted(mutation.iterdir()):
    if p.is_file() and p.suffix in ('.log','.json'):files.append((p,'mutation/'+p.name))
files.append((mutation/'src/core/druid_array.rs','mutation/src/core/druid_array.rs'))
def new_file(path,payload):
    path.parent.mkdir(parents=True,exist_ok=True)
    with path.open('xb') as out:out.write(payload)
for source,rel in files:
    dest=draft/rel;new_file(dest,source.read_bytes());assert dest.read_bytes()==source.read_bytes()
    state['mirrors'][dest.relative_to(root).as_posix()]=sha(source)
state['artifact_sha256']={str(p):sha(p) for p in (candidate/'after6.exe',candidate/'before6.exe',mutation/'libdruid.rlib',mutation/'capability_mutation.exe')}
archive=base/'task7-array-expanded-contract6-evidence.zip'
with zipfile.ZipFile(archive,'x',compression=zipfile.ZIP_DEFLATED) as out:
    for source,rel in files:out.write(source,rel)
    out.writestr('manifest.json',json.dumps(state,indent=2,ensure_ascii=False)+'\n')
with zipfile.ZipFile(archive) as check:
    assert check.testzip() is None
    for source,rel in files:assert check.read(rel)==source.read_bytes()
state['evidence_archive']={'path':str(archive),'sha256':sha(archive)}
dest=mirror/('task6-e-space-'+archive.name);new_file(dest,archive.read_bytes());assert dest.read_bytes()==archive.read_bytes()
summary=base/'task7-array-expanded-contract6-validation.summary.json'
new_file(summary,(json.dumps(state,ensure_ascii=False,indent=2)+'\n').encode('utf-8'))
new_file(mirror/('task6-e-space-'+summary.name),summary.read_bytes())
body=f'''

## Array 稀疏能力、错误重试与旧租约隔离（2026-10-08）

在完整Array候选源码不变的前提下扩展同源消费者：原36测试体逐字不变，新增六项，实际旧完整库编译0/运行101为29PASS13FAIL，新完整库编译0/运行0为42PASS0FAIL。

能力合同枚举READ/RANGE/TYPE_MAP/RESULT_SET全部16组合，对十个非free方法分别在Open和Freed状态调用，共320个操作断言；成功与不支持同时验证驱动调用计数，关闭错误先于不支持且不进入驱动。FREE始终支持，本轮不扩展到无FREE能力驱动或任意运行上下文掩码。

创建fatal保留08006/code77，Array创建工厂调用一次、sorter/discard各一次，后续创建不进入驱动；关闭连接创建零driver调用。读取fatal与非fatal分别保留08006/77和22000/88，前者失效租约拒绝后续底层读取，后者恢复后正常读取并可复用。释放两个错误分支均先关闭子游标，失败后驱动free可重试，子游标不重放关闭，最终free实际两次/子close一次，sorter一次，fatal才discard。真实返回holder再借用时，旧Array及游标拒绝使用/重复free-close不影响新代次，新资源与游标保持可用并各清理一次。

独立完整能力反例仅去掉DruidArray::require的不支持返回，驱动自身仍真实拒绝。409源库和同一consumer6严格编译0，精确稀疏矩阵实际exit101/0PASS1FAIL：不支持操作进入了驱动，调用数1/期望0。证明合同检查的是代理边界，不会被驱动最终返回相同0A000掩盖。候选409源码清单与原compile2逐项一致，反例唯一源码差异为druid_array.rs。

expanded6目录保存验收源、helper、真实命令/退出码/log及反例单源；证据zip SHA-256为{state['evidence_archive']['sha256']}，机器summary另绑定消费者/库/可执行产物SHA及原完整源码zip8。镜像逐字节核验，1167已枚举正式输入变化0。未生产apply、正式Cargo或真实DB认证；任务状态仍Task6 FIX_REQUIRED、Task7八义务/Task8 INCOMPLETE、18顶层OPEN。下一步继续Struct/Ref与资源返回链路及正式接入，保持完整验收范围。
'''
def append(path,addition):
    old=path.read_bytes();new_file(base/(path.name+'.before-array-expanded6'),old)
    temp=path.with_name(path.name+'.array-expanded6.tmp');new_file(temp,old+addition.encode('utf-8'))
    assert temp.read_bytes()==old+addition.encode('utf-8');os.replace(temp,path)
append(root/'docs/verification/rdbc-first/task7-standard-resource-candidates-2026-10-08.md',body)
line='\n\n2026-10-08 Array扩展合同：原36测试体逐字保留，新增六项；旧完整库29/13 RED，新候选42/0 GREEN。16能力组合×十操作×Open/Freed共320断言验证不支持/关闭零驱动调用；创建/读取/释放fatal与非fatal分类、free失败重试子close不重放、真实holder再借用旧Array/游标隔离通过。删除代理能力拒绝的完整反例严格编译0/精确运行0/1，调用数1/期望0；candidate409源码逐项不变、1167正式冻结变化0。expanded6证据/zip/SHA与机器summary逐字节镜像，未生产apply/Cargo/DB认证；Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18 OPEN。详见task7-standard-resource-candidates-2026-10-08.md。\n'
append(root/'docs/verification/rdbc-first/status.md',line);append(mirror/'progress.md',line)
print(json.dumps({'summary':str(summary),'results':state['results'],'mirrors':len(files),'changed_inputs':changed,'evidence_sha256':state['evidence_archive']['sha256']},ensure_ascii=False),flush=True)
