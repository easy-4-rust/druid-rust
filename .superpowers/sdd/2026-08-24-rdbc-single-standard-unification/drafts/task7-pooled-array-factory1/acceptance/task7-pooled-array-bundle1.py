"""Archive complete source and byte-verified evidence without applying production."""
import datetime
import difflib
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import zipfile
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
root=Path('E:/workspaces/workspace-github-easy-4-rust/druid-rust')
mirror=root/'.superpowers/sdd/2026-08-24-rdbc-single-standard-unification'
candidate=base/'task7-array-source-candidate1'
rdbc=base/'task7-rdbc-source-candidate1'
mutation=base/'task7-array-child-close-mutation1'
def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()
contracts=json.loads((candidate/'contract5.summary.json').read_text(encoding='utf-8'))
negative=json.loads((mutation/'summary.json').read_text(encoding='utf-8'))
assert contracts['terminal'] and contracts['commands'][-1]['exit_code']==0
assert negative['terminal'] and negative['candidate_sources_unchanged']
compiled=json.loads((candidate/'compile2.summary.json').read_text(encoding='utf-8'))
assert compiled['exit_code']==0 and all(sha(candidate/p)==digest for p,digest in compiled['sources'].items())
snapshot=json.loads((base/'task6-current-full-validation1.inputs.json').read_text(encoding='utf-8'))
changed=[name for name,digest in snapshot['inputs'].items() if not (root/name).is_file() or sha(root/name)!=digest]
assert not changed,changed
state={'observed':datetime.datetime.now(datetime.timezone.utc).isoformat(),'production_applied':False,'formal_cargo_certification':False,'changed_frozen_inputs':changed,'frozen_input_count':len(snapshot['inputs']),'mirrors':{},'patch_files':[],'source_manifest':{},'artifact_sha256':{}}
def new_file(path,payload):
    path.parent.mkdir(parents=True,exist_ok=True)
    with path.open('xb') as out: out.write(payload)
def copy(source,target):
    new_file(target,source.read_bytes())
    assert target.read_bytes()==source.read_bytes()
    state['mirrors'][target.relative_to(root).as_posix()]=sha(source)
draft=mirror/'drafts/task7-pooled-array-factory1'
previous=base/'task7-full-druid-source-candidate1'
patch=[]
for path in sorted((candidate/'src').rglob('*.rs')):
    rel=path.relative_to(candidate); before=previous/rel
    if not before.exists() or before.read_bytes()!=path.read_bytes():
        copy(path,draft/'crates/druid'/rel)
        state['patch_files'].append(rel.as_posix())
        patch.extend(difflib.unified_diff(before.read_text(encoding='utf-8').splitlines(keepends=True) if before.exists() else [],path.read_text(encoding='utf-8').splitlines(keepends=True),fromfile='a/crates/druid/'+rel.as_posix(),tofile='b/crates/druid/'+rel.as_posix()))
assert len(state['patch_files'])==5,state['patch_files']
new_file(draft/'array.patch',''.join(patch).encode('utf-8'))
evidence=[]
for folder in (candidate,base/'task7-array-baseline-coherent1',mutation):
    for p in sorted(folder.iterdir()):
        if p.is_file() and p.suffix in ('.json','.log'):
            evidence.append((p,folder.name+'/'+p.name))
for p in sorted(base.iterdir()):
    if p.is_file() and (p.name.startswith('task7-array-') or p.name.startswith('task7-pooled-array-') or p.name.startswith('task7-build-pooled-array-') or p.name.startswith('task7-compile-pooled-array-')) and p.suffix in ('.py','.rs','.json'):
        evidence.append((p,p.name))
for p,rel in evidence: copy(p,draft/'acceptance'/rel)
archive=base/'task7-full-druid-rdbc-source-candidate8.zip'
with zipfile.ZipFile(archive,'x',compression=zipfile.ZIP_DEFLATED,compresslevel=6) as out:
    for folder,label in ((candidate/'src','crates/druid/src'),(candidate/'resources','crates/druid/resources'),(rdbc/'src','crates/druid-rdbc/src')):
        for p in sorted(folder.rglob('*')):
            if p.is_file():
                name=label+'/'+p.relative_to(folder).as_posix()
                state['source_manifest'][name]=sha(p)
                out.write(p,name)
    for p,rel in evidence: out.write(p,'acceptance/'+rel)
    out.writestr('source-manifest.json',json.dumps(state['source_manifest'],indent=2)+'\n')
with zipfile.ZipFile(archive) as check:
    assert check.testzip() is None
    for name,digest in state['source_manifest'].items(): assert hashlib.sha256(check.read(name)).hexdigest()==digest
state['archive']={'path':str(archive),'sha256':sha(archive),'source_count':len(state['source_manifest'])}
copy(archive,mirror/('task6-e-space-'+archive.name))
for p in (candidate/'libdruid.rlib',base/'task7-array-baseline-coherent1/libdruid_rdbc.rlib',candidate/'before5.exe',candidate/'after5.exe',mutation/'child_close_mutation.exe'):
    state['artifact_sha256'][str(p)]=sha(p)
for label in ('before5','after5'):
    log=(candidate/(label+'-runtime.log')).read_text(encoding='utf-8')
    result=re.search(r'test result: .*? (\d+) passed; (\d+) failed; (\d+) ignored;',log)
    assert result
    state[label+'_result']={'passed':int(result[1]),'failed':int(result[2]),'ignored':int(result[3])}
assert state['before5_result']=={'passed':28,'failed':8,'ignored':0}
assert state['after5_result']=={'passed':36,'failed':0,'ignored':0}
summary=base/'task7-pooled-array-factory1-validation.summary.json'
new_file(summary,(json.dumps(state,indent=2,ensure_ascii=False)+'\n').encode('utf-8'))
target=mirror/('task6-e-space-'+summary.name)
new_file(target,summary.read_bytes()); assert target.read_bytes()==summary.read_bytes()
paragraph=f'''

## 完整 Druid Array 工厂与子游标监管候选（2026-10-08）

新增 DruidArray/DruidArrayState 两真实对象，覆写池化 create_array_of，保留 nullable type_name/elements 与全部十一方法委托。能力为驱动与运行上下文的交集，owner/lease/error 分类沿用已验证真实策略。四种 getResultSet 重载返回标准池化 ResultSet，get_statement 为 None，保留索引/值/NULL/类型映射与范围参数。

raw 游标在 registry 异步登记之前同步移交 Array state，然后释放 Array payload 锁；失败清理不等待自己持有的锁。Array free 先等待操作移交，再直接关闭子游标 actor，最后驱动 free 和 owner 通知；不在同一 coordinator 中排队等待自身。父级关闭期间拒绝的新游标仍先归属并排空。

完整409个Druid源/21资源/126个RDBC源候选，显式Rust1.97.1与同源31extern，严格-Dwarnings/forbidunsafe rlib编译exit0零输出。先重建一致工具链基线并复验原XML/LOB消费者28/0，避免旧混合缓存E0514。相同最终consumer5旧完整库编译0/运行101，28PASS8FAIL；新完整库编译0/运行0，36PASS0FAIL。原28测试体保留，Array八项覆盖nullable输入、四值/四游标重载、父级清理、free取消、打开失败、fatal丢弃一次、显式free先关子游标、创建与父级关闭并发及无Filter元数据延迟。

consumer3的Debug格式要求导致E0277是夹具编译错误；consumer4的34/1失败来自无Filter时构造不会预读metadata的错误前提，不是产品缺陷。consumer5只为打开失败用例配置Some(FilterChain::new())并新增无Filter延迟报错对照，未改变候选生产行为或放宽断言。原编译与失败日志全部保留。

同源ArrayDriver含Cell，正向36项已实际链接运行；负向要求Sync实际exit1/E0277。独立完整反例只去掉Array state的子游标close_core调用，库及相同消费者编译0，精确显式free测试实际exit101/0PASS1FAIL，raw closes为0/期望1，证明不会由驱动free或父级close伪造该清理保证。未改正式候选。

精确五源差异补丁与验收源/helper/原日志/摘要在drafts/task7-pooled-array-factory1；完整源码zip8 SHA-256为{state['archive']['sha256']}，共{len(state['source_manifest'])}源码/资源输入，并逐个解压核验SHA。validation summary绑定源码、实际产物、命令、反例和逐字节镜像。

本轮仅独立候选，未生产apply、正式Cargo、native worker、真实外部池或数据库认证。Array能力稀疏矩阵、错误重试/旧租约再借用完整矩阵、Struct/Ref、ResultSet资源getter返回、旧Access退役和真实async流式读取仍开放，不将候选GREEN计为Task7完成。当前1167已枚举正式输入逐项变化0，未新增生产源。

原完整验证runner57728/C1子55648继续以实际进程观察为准；当前流水线尚无全仓终态。Task6 FIX_REQUIRED / Task7八义务 / Task8 INCOMPLETE / 18顶层OPEN。
'''
def append_safe(path,text):
    old=path.read_bytes(); backup=base/(path.name+'.before-array-bundle1')
    new_file(backup,old)
    temporary=path.with_name(path.name+'.array-bundle1.tmp')
    new_file(temporary,old+text.encode('utf-8'))
    assert temporary.read_bytes()==old+text.encode('utf-8')
    os.replace(temporary,path)
append_safe(root/'docs/verification/rdbc-first/task7-standard-resource-candidates-2026-10-08.md',paragraph)
line=f'\n\n2026-10-08 Array完整池化候选：409Druid/21资源/126RDBC严格rlib0；同源consumer5旧28/8 RED到36/0 GREEN。四游标同步handoff、Array先关子游标后free、父级并发拒绝排空；修正无Filter metadata夹具前提并新增延迟对照，原失败日志保留。Cell负向Sync1/E0277；删除子游标关闭的完整反例编译0/精确运行0/1。五源补丁/完整zip8（SHA {state["archive"]["sha256"]}）/证据逐字节镜像与机器摘要保存，1167冻结输入变化0。未生产apply/Cargo/真实DB，Struct/Ref/getter/唯一树/streaming继续开放。Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18 OPEN。详见task7-standard-resource-candidates-2026-10-08.md。\n'
append_safe(root/'docs/verification/rdbc-first/status.md',line)
append_safe(mirror/'progress.md',line)
print(json.dumps({'summary':str(summary),'archive_sha256':state['archive']['sha256'],'mirrors':len(state['mirrors']),'changed_inputs':changed,'before':state['before5_result'],'after':state['after5_result']},ensure_ascii=False),flush=True)
