"""Archive exact full Blob candidates, prior-scope counterexamples and admission-priority fix."""
import datetime,difflib,hashlib,json,os,re,zipfile
from pathlib import Path
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008');root=Path('E:/workspaces/workspace-github-easy-4-rust/druid-rust');mirror=root/'.superpowers/sdd/2026-08-24-rdbc-single-standard-unification';sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
oldr=base/'task7-blob-value-rdbc-candidate1';oldd=base/'task7-blob-value-druid-candidate1';rdbc=base/'task7-blob-value-rdbc-candidate2';druid=base/'task7-blob-value-druid-candidate2'
original=json.loads((oldd/'summary.json').read_text(encoding='utf-8'));expanded=json.loads((oldd/'expanded.summary.json').read_text(encoding='utf-8'));extra=json.loads((oldd/'extra.summary.json').read_text(encoding='utf-8'));final=json.loads((druid/'summary.json').read_text(encoding='utf-8'));assert all(r['terminal'] for r in (original,expanded,extra,final))
state={'observed':datetime.datetime.now(datetime.timezone.utc).isoformat(),'production_applied':False,'formal_cargo_certified':False,'results':{},'patches':[],'removed_sources':[],'mirrors':{},'source_manifest':{},'prior_test_blocks':original['prior_test_blocks']}
assert len(original['prior_test_blocks'])==67
for folder,record,key in ((oldr,original,'rdbc_sources'),(oldd,original,'sources'),(rdbc,final,'rdbc_sources'),(druid,final,'sources')):
    actual={p.relative_to(folder).as_posix():sha(p) for p in (folder/'src').rglob('*.rs')};assert actual==record[key]
for folder,name,expected in [(oldd,'consumer-runtime',(76,0)),(oldd,'consumer2-runtime',(77,0)),(oldd,'consumer3-runtime',(79,0)),(oldd,'capability-runtime',(0,1)),(oldd,'self-alias-runtime',(0,1)),(oldd,'lock-order-runtime',(0,1)),(druid,'precedence-red-runtime',(0,1)),(druid,'consumer-runtime',(80,0))]:
    match=re.search(r'test result: .*? (\d+) passed; (\d+) failed; (\d+) ignored;',(folder/(name+'.log')).read_text(encoding='utf-8'));assert match;result={'passed':int(match[1]),'failed':int(match[2]),'ignored':int(match[3])};assert (result['passed'],result['failed'])==expected and result['ignored']==0;state['results'][folder.name+'/'+name]=result
for variant in ('capability','self-alias','lock-order'):
    folder=Path(expanded[variant]['root']);actual={p.relative_to(folder).as_posix():sha(p) for p in folder.rglob('*.rs')};assert actual==expanded[variant]['sources'];changed=[p.relative_to(oldr).as_posix() for p in (oldr/'src').rglob('*.rs') if sha(p)!=sha(folder/'rdbc'/p.relative_to(oldr))];assert changed==['src/resource/rdbc_blob.rs'];state[variant+'_changed_sources']=changed
state['priority_changed_sources']=[p.relative_to(rdbc).as_posix() for p in (rdbc/'src').rglob('*.rs') if sha(p)!=sha(oldr/p.relative_to(rdbc))];assert set(state['priority_changed_sources'])=={'src/resource/rdbc_blob.rs','src/resource/standard_resource_value.rs'};assert final['sources']==original['sources']
snapshot=json.loads((base/'task6-current-full-validation1.inputs.json').read_text(encoding='utf-8'));state['changed_inputs']=[p for p,digest in snapshot['inputs'].items() if not(root/p).is_file() or sha(root/p)!=digest];assert not state['changed_inputs'];state['frozen_input_count']=len(snapshot['inputs'])
frozen={p for p in snapshot['inputs'] if p.startswith('crates/') and p.endswith('.rs')};actual={p.relative_to(root).as_posix() for p in (root/'crates').rglob('*.rs')};state['crates_rs_added']=sorted(actual-frozen);state['crates_rs_removed']=sorted(frozen-actual);assert actual==frozen
draft=mirror/'drafts/task7-blob-standard-value1'
def new_file(path,payload):
    path.parent.mkdir(parents=True,exist_ok=True)
    with path.open('xb') as out:out.write(payload)
def copy(source,dest):
    new_file(dest,source.read_bytes());assert dest.read_bytes()==source.read_bytes();state['mirrors'][dest.relative_to(root).as_posix()]=sha(source)
patch=[]
for current,previous,label in ((rdbc,base/'task7-array-value-rdbc-candidate1','druid-rdbc'),(druid,base/'task7-array-value-druid-candidate1','druid')):
    relative={p.relative_to(current) for p in (current/'src').rglob('*.rs')}|{p.relative_to(previous) for p in (previous/'src').rglob('*.rs')}
    for rel in sorted(relative):
        old=previous/rel;new=current/rel;before=old.read_bytes() if old.exists() else b'';after=new.read_bytes() if new.exists() else b''
        if before==after:continue
        name='crates/'+label+'/'+rel.as_posix();state['patches'].append(name)
        if new.exists():copy(new,draft/name)
        else:state['removed_sources'].append(name)
        patch+=list(difflib.unified_diff(before.decode('utf-8').splitlines(keepends=True),after.decode('utf-8').splitlines(keepends=True),fromfile='a/'+name,tofile='b/'+name))
assert len(state['patches'])==10 and state['removed_sources']==['crates/druid-rdbc/src/resource/rdbc_blob_access.rs']
new_file(draft/'blob.patch',''.join(patch).encode());new_file(draft/'removed_sources.json',(json.dumps(state['removed_sources'],indent=2)+'\n').encode())
files=[];acquisition=json.loads((base/'task8-java-jdbc42-source-acquisition1.summary.json').read_text(encoding='utf-8'));java=next(row for row in acquisition['sources'] if row['fqcn']=='java.sql.Blob');assert sha(Path(java['local_path']))==java['sha256'];state['java_source']=java;files.append((Path(java['local_path']),'java/Blob.java'))
for folder in (oldd,druid):
    for p in sorted(folder.iterdir()):
        if p.is_file() and p.suffix in ('.log','.json','.rs'):files.append((p,folder.name+'/'+p.name))
for name in ('task7-blob-pattern1.rs','task7-blob-handle1.rs','task7-blob-value-contract1.rs','task7-blob-value-lock-order2.rs','task7-blob-value-pooled-search1.rs','task7-blob-value-priority1.rs','task7-pooled-blob-value-consumer1.rs','task7-pooled-blob-value-consumer2.rs','task7-pooled-blob-value-consumer3.rs','task7-pooled-blob-value-consumer4.rs','task7-blob-value-build1.py','task7-blob-value-verify1.py','task7-blob-value-extra1.py','task7-blob-value-priority1.py','task7-blob-value-record1.py'):files.append((base/name,name))
for variant in ('capability','self-alias','lock-order'):files.append((Path(expanded[variant]['root'])/'rdbc/src/resource/rdbc_blob.rs',variant+'/rdbc_blob.rs'))
# 保留优先级修复前的两处源码；配合完整旧manifest能逐字重建三反例的正常基线。
for rel in state['priority_changed_sources']:files.append((oldr/rel,'pre-priority-baseline/'+rel))
for source,rel in files:copy(source,draft/'acceptance'/rel)
state['artifact_sha256']={str(p):sha(p) for p in (oldr/'libdruid_rdbc.rlib',oldd/'libdruid.rlib',oldd/'consumer.exe',oldd/'consumer2.exe',oldd/'consumer3.exe',rdbc/'libdruid_rdbc.rlib',druid/'libdruid.rlib',druid/'consumer.exe')}
assert state['artifact_sha256'][str(rdbc/'libdruid_rdbc.rlib')]==final['rdbc_rlib_sha256'];assert state['artifact_sha256'][str(druid/'libdruid.rlib')]==final['rlib_sha256'];assert state['artifact_sha256'][str(druid/'consumer.exe')]==final['exe_sha256']
state['extern_inputs']={}
for record in (original,expanded,extra,final):
    for row in record['commands']:
        command=row['command']
        for i,arg in enumerate(command[:-1]):
            if arg=='--extern':
                p=Path(command[i+1].split('=',1)[1]);assert p.is_file();state['extern_inputs'][str(p)]=sha(p)
archive=base/'task7-full-druid-rdbc-source-candidate12.zip'
with zipfile.ZipFile(archive,'x',zipfile.ZIP_DEFLATED) as out:
    for folder,label in ((rdbc,'druid-rdbc'),(druid,'druid')):
        for area in ('src','resources'):
            if not(folder/area).exists():continue
            for p in sorted((folder/area).rglob('*')):
                if p.is_file():name='crates/'+label+'/'+p.relative_to(folder).as_posix();out.write(p,name);state['source_manifest'][name]=sha(p)
    for source,rel in files:out.write(source,'acceptance/'+rel)
    out.writestr('source_manifest.json',json.dumps(state['source_manifest'],indent=2)+'\n')
assert len(state['source_manifest'])==558
with zipfile.ZipFile(archive) as saved:
    for name,digest in state['source_manifest'].items():assert hashlib.sha256(saved.read(name)).hexdigest()==digest
state['archive']={'path':str(archive),'sha256':sha(archive),'source_entries':558,'evidence_files':len(files)};copy(archive,mirror/('task6-e-space-'+archive.name))
summary=base/'task7-blob-standard-value1-validation.summary.json';new_file(summary,(json.dumps(state,indent=2,ensure_ascii=False)+'\n').encode());copy(summary,mirror/('task6-e-space-'+summary.name))
body=f'''

## Blob唯一标准值句柄、别名搜索及准入优先级候选（2026-10-08）

固定OpenJDK8u Blob.java来源SHA核对，保留十一项JDBC操作、1-based资源位置/0-based输入offset、原流与释放语义。删除旧RdbcBlobAccess、旧factory/context/Druid重导出及ResourceType::Blob；工厂接受Box<dyn Blob>，值句柄复用标准共享容器，不新增RDBC后台释放调度。

现有候选的position_blob(&mut dyn Blob)无法安全表示Java自身模式。新增BlobPattern是原模式参数的Rust别名表示而非第二套驱动端口：SameResource由真实驱动处理自身内容，Other独占借用另一个标准Blob；没有强制内容快照或固定搜索结果。两个共享句柄按稳定身份顺序取锁，Druid代理原样转发参数。此为候选公共签名变化，尚需正式API/SemVer门禁，不能声明兼容性已认证。

旧完整库同源RED为缺少BlobPattern及旧Access工厂E0308；完整128RDBC严格-Dwarnings/forbidunsafe编译0。完整409Druid旧重导出/旧参数边界RED后更新两源，严格编译0。仅迁移旧Blob驱动夹具的参数表示，原67测试块逐字比较一致；新增九合同76/0、真实占锁反向搜索77/0、池化别名与取消双锁79/0。128能力组合×十操作，支持FREE的64组合再验证Freed十操作零驱动调用；流原身份、范围/写入offset、失败/取消/panic重试、并发free一次及池化父关闭一次通过。

修复前的完整候选另有三个完整单源反例：移除length能力门禁、将自身模式固定返回1、改成总是先锁目标；各128RDBC/409Druid及同一77测试消费者编译0，精确合同均0PASS1FAIL。锁序反例在两个真实读取先占锁后使搜索排队，避免单线程无阻塞取锁掩盖死锁。Cell驱动Sync负向E0277、旧BlobAccess导入E0432。三反例绑定的是优先级修复前源码，未冒充修复后源码的重新执行。

随后补证目标不支持SEARCH且模式已关闭的错误优先级，原完整候选实际精确0/1 RED；独立后继候选先检查目标准入，再按原稳定序取锁并保留锁后重查。仅rdbc_blob.rs与standard_resource_value.rs两源变化（含格式化），Druid源全同；完整128/409严格重建0，同一全部80消费者80/0。保留原79源前缀及原失败记录，不以锁顺序改变旧目标准入错误语义。

十项最终源码增改删补丁与完整zip12归档，558源码/资源项逐项解压SHA核验，zip SHA-256 {state['archive']['sha256']}。原始/后继完整manifest、修复前两源码、实际命令/日志/Java/反例与产物及extern SHA逐字节镜像。1167正式冻结输入变化0，crates rs集合无新增删除。

本轮仅候选，未生产apply、正式Cargo/Clippy/全仓fmt、旧全套夹具迁移、真实DB或Java运行时差分认证。Clob/NClob/XML旧Access、Struct旧DTO、资源返回/参数全矩阵与真实async streaming仍开放。完整C1/C2已终态通过；原全仓内存/页面文件构建失败保留，-j1完整重跑另记，不与候选80/0混淆。Task6 FIX_REQUIRED、Task7八义务/Task8 INCOMPLETE、18顶层OPEN。
'''
def append(path,addition):
    old=path.read_bytes();new_file(base/(path.name+'.before-blob-value1'),old);temp=path.with_name(path.name+'.blob-value1.tmp');payload=old+addition.encode();new_file(temp,payload);assert temp.read_bytes()==payload;os.replace(temp,path)
append(root/'docs/verification/rdbc-first/task7-standard-resource-candidates-2026-10-08.md',body)
line='\n\n2026-10-08 Blob唯一标准值句柄候选：退役旧BlobAccess及factory/context/重导出，复用Send共享容器。BlobPattern安全表示自身/他资源，稳定双锁序不预读取；完整128RDBC/409Druid严格编译0，旧67测试块保留，扩展76/0、77/0、池化与取消79/0。修复前三个完整单源反例各精确0/1；目标能力/关闭模式优先级实际RED0/1，后继两源修复完整重建80/0。十源补丁/完整zip12/原始及后继源码与实际命令日志SHA归档，1167冻结输入及rs集合变化0。未生产apply/Cargo/API/实库认证，Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18 OPEN。详见task7-standard-resource-candidates-2026-10-08.md。\n'
append(root/'docs/verification/rdbc-first/status.md',line);append(mirror/'progress.md',line)
print(json.dumps({'summary':str(summary),'results':state['results'],'archive':state['archive'],'changed_inputs':state['changed_inputs']},ensure_ascii=False),flush=True)
