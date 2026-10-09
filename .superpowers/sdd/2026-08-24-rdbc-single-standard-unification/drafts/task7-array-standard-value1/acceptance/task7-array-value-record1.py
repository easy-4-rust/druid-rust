"""Archive full Array value candidate, exact counterexamples and frozen-input proof."""
import datetime,difflib,hashlib,json,os,re,zipfile
from pathlib import Path
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008');root=Path('E:/workspaces/workspace-github-easy-4-rust/druid-rust');mirror=root/'.superpowers/sdd/2026-08-24-rdbc-single-standard-unification'
rdbc=base/'task7-array-value-rdbc-candidate1';druid=base/'task7-array-value-druid-candidate1';sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
original=json.loads((druid/'summary.json').read_text(encoding='utf-8'));expanded=json.loads((druid/'expanded.summary.json').read_text(encoding='utf-8'));assert original['terminal'] and expanded['terminal']
state={'observed':datetime.datetime.now(datetime.timezone.utc).isoformat(),'production_applied':False,'formal_cargo_certified':False,'results':{},'patches':[],'removed_sources':[],'mirrors':{},'source_manifest':{}}
for folder,key in ((rdbc,'rdbc_sources'),(druid,'sources')):
    actual={p.relative_to(folder).as_posix():sha(p) for p in (folder/'src').rglob('*.rs')};assert actual==original[key]
for name,expected in [('consumer-runtime',(65,0)),('consumer3-runtime',(67,0)),('capability-runtime',(0,1)),('lock-recheck-runtime',(0,1))]:
    match=re.search(r'test result: .*? (\d+) passed; (\d+) failed; (\d+) ignored;',(druid/(name+'.log')).read_text(encoding='utf-8'));assert match
    result={'passed':int(match[1]),'failed':int(match[2]),'ignored':int(match[3])};assert (result['passed'],result['failed'])==expected and result['ignored']==0;state['results'][name]=result
for variant in ('capability','lock-recheck'):
    folder=Path(expanded[variant]['root']);actual={p.relative_to(folder).as_posix():sha(p) for p in folder.rglob('*.rs')};assert actual==expanded[variant]['sources']
    changed=[]
    for p in (rdbc/'src').rglob('*.rs'):
        rel=p.relative_to(rdbc);other=folder/'rdbc'/rel
        if sha(p)!=sha(other):changed.append(rel.as_posix())
    assert changed==['src/resource/standard_resource_value.rs'];state[variant+'_changed_sources']=changed
snapshot=json.loads((base/'task6-current-full-validation1.inputs.json').read_text(encoding='utf-8'))
state['changed_inputs']=[p for p,digest in snapshot['inputs'].items() if not (root/p).is_file() or sha(root/p)!=digest];assert not state['changed_inputs'];state['frozen_input_count']=len(snapshot['inputs'])
frozen={p for p in snapshot['inputs'] if p.startswith('crates/') and p.endswith('.rs')};actual={p.relative_to(root).as_posix() for p in (root/'crates').rglob('*.rs')};state['crates_rs_added']=sorted(actual-frozen);state['crates_rs_removed']=sorted(frozen-actual);assert actual==frozen
draft=mirror/'drafts/task7-array-standard-value1'
def new_file(path,payload):
    path.parent.mkdir(parents=True,exist_ok=True)
    with path.open('xb') as out:out.write(payload)
def copy(source,dest):
    new_file(dest,source.read_bytes());assert dest.read_bytes()==source.read_bytes();state['mirrors'][dest.relative_to(root).as_posix()]=sha(source)
patch=[]
for current,previous,label in ((rdbc,base/'task7-ref-rdbc-candidate1','druid-rdbc'),(druid,base/'task7-ref-druid-candidate1','druid')):
    relative={p.relative_to(current) for p in (current/'src').rglob('*.rs')}|{p.relative_to(previous) for p in (previous/'src').rglob('*.rs')}
    for rel in sorted(relative):
        old=previous/rel;new=current/rel;before=old.read_bytes() if old.exists() else b'';after=new.read_bytes() if new.exists() else b''
        if before==after:continue
        name='crates/'+label+'/'+rel.as_posix();state['patches'].append(name)
        if new.exists():copy(new,draft/name)
        else:state['removed_sources'].append(name)
        patch+=list(difflib.unified_diff(before.decode('utf-8').splitlines(keepends=True),after.decode('utf-8').splitlines(keepends=True),fromfile='a/'+name,tofile='b/'+name))
assert len(state['patches'])==8 and state['removed_sources']==['crates/druid-rdbc/src/resource/rdbc_array_access.rs']
new_file(draft/'array.patch',''.join(patch).encode());new_file(draft/'removed_sources.json',(json.dumps(state['removed_sources'],indent=2)+'\n').encode())
files=[]
acquisition=json.loads((base/'task8-java-jdbc42-source-acquisition1.summary.json').read_text(encoding='utf-8'));java=next(row for row in acquisition['sources'] if row['fqcn']=='java.sql.Array');assert sha(Path(java['local_path']))==java['sha256'];state['java_source']=java;files.append((Path(java['local_path']),'java/Array.java'))
for p in sorted(druid.iterdir()):
    if p.is_file() and p.suffix in ('.log','.json','.rs'):files.append((p,druid.name+'/'+p.name))
for name in ('task7-array-handle1.rs','task7-shared-resource-value1.rs','task7-shared-resource-release-guard1.rs','task7-array-value-contract1.rs','task7-array-value-boundary1.rs','task7-array-value-lock-race1.rs','task7-pooled-array-value-consumer1.rs','task7-pooled-array-value-consumer2.rs','task7-pooled-array-value-consumer3.rs','task7-array-value-build1.py','task7-array-value-build2.py','task7-array-value-verify1.py','task7-array-value-record1.py'):files.append((base/name,name))
for variant in ('capability','lock-recheck'):
    folder=Path(expanded[variant]['root']);files.append((folder/'rdbc/src/resource/standard_resource_value.rs',variant+'/standard_resource_value.rs'))
for source,rel in files:copy(source,draft/'acceptance'/rel)
state['artifact_sha256']={str(p):sha(p) for p in (rdbc/'libdruid_rdbc.rlib',druid/'libdruid.rlib',druid/'consumer.exe',druid/'consumer3.exe')}
assert state['artifact_sha256'][str(rdbc/'libdruid_rdbc.rlib')]==original['rdbc_rlib_sha256'];assert state['artifact_sha256'][str(druid/'libdruid.rlib')]==original['rlib_sha256'];assert state['artifact_sha256'][str(druid/'consumer3.exe')]==expanded['exe_sha256']
archive=base/'task7-full-druid-rdbc-source-candidate11.zip'
with zipfile.ZipFile(archive,'x',zipfile.ZIP_DEFLATED) as out:
    for folder,label in ((rdbc,'druid-rdbc'),(druid,'druid')):
        for area in ('src','resources'):
            if not (folder/area).exists():continue
            for p in sorted((folder/area).rglob('*')):
                if p.is_file():name='crates/'+label+'/'+p.relative_to(folder).as_posix();out.write(p,name);state['source_manifest'][name]=sha(p)
    for source,rel in files:out.write(source,'acceptance/'+rel)
    out.writestr('source_manifest.json',json.dumps(state['source_manifest'],indent=2)+'\n')
assert len(state['source_manifest'])==558
with zipfile.ZipFile(archive) as saved:
    for name,digest in state['source_manifest'].items():assert hashlib.sha256(saved.read(name)).hexdigest()==digest
state['archive']={'path':str(archive),'sha256':sha(archive),'source_entries':len(state['source_manifest']),'evidence_files':len(files)};copy(archive,mirror/('task6-e-space-'+archive.name))
summary=base/'task7-array-standard-value1-validation.summary.json';new_file(summary,(json.dumps(state,indent=2,ensure_ascii=False)+'\n').encode());copy(summary,mirror/('task6-e-space-'+summary.name))
body=f'''

## Array唯一标准值句柄与共享释放确认候选（2026-10-08）

固定OpenJDK8u Array.java来源SHA核对。删除旧RdbcArrayAccess及factory/context/Druid重导出、废弃ResourceType::Array；ResourceHandleFactory::array现在接受Box<dyn Array>，共享值容器只持有Send标准对象、身份、能力和释放确认状态，不增加第二个驱动端口或RDBC后台释放调度。原始ResultSet保持原样返回，Druid资源监管仍由既有registry/coordinator承担。

初始命令模板误用-o与夹具unwrap_err隐含Debug错误原日志保留，不计产品缺陷；修正夹具后旧完整库同源RED仅E0308旧Access入参。完整128源RDBC严格-Dwarnings/forbidunsafe编译0；完整409源Druid旧重导出E0432 RED，移除后严格编译0。原58项消费者源逐字保留，新增七合同后65/0，追加驱动入口计数锁交接和panic合同后67/0。

全部32种READ/RANGE/TYPE_MAP/RESULT_SET/FREE能力组合验证十方法、FREE门禁及不支持零驱动调用；支持释放的16组合另验证Freed状态十方法零驱动调用。克隆身份、别名、SQL NULL、显式映射、1-based范围、原始游标独立关闭、原错误与free失败重试通过。原始free取消/panic后不误报成功、不恢复读取，后续free重试确认；并发克隆free只调用driver一次；取消读取不消耗资源。实际池化Array装入值句柄后，free调用者取消与父connection关闭仍由Druid监管链完成，重试加入既有释放，底层free一次。

独立完整源码反例分别仅移除能力拒绝或取得锁后的状态重查：两组128RDBC/409Druid严格编译0、同一消费者编译0、精确合同均0PASS1FAIL。后者通过驱动入口计数证明不能靠驱动自身closed错误掩盖句柄漏检查。两个负向编译确认Cell驱动不满足Sync（E0277）、旧ArrayAccess不可导入（E0432）。

八项源码增改删补丁与完整zip11归档，558源码/资源项逐项解压SHA核验，zip SHA-256 {state['archive']['sha256']}。实际命令、日志、Java来源、消费者、反例及产物SHA逐字节镜像。1167正式冻结输入变化0，crates下rs集合无新增删除。

本轮仅候选，未生产apply、正式Cargo/Clippy/fmt、旧全套夹具迁移、真实DB或Java运行时差分认证。其余LOB/XML旧Access、Struct旧DTO归属、结果集返回/参数完整矩阵与真实async streaming仍开放；Task6 FIX_REQUIRED、Task7八义务/Task8 INCOMPLETE、18顶层OPEN。
'''
def append(path,addition):
    old=path.read_bytes();new_file(base/(path.name+'.before-array-value1'),old);temp=path.with_name(path.name+'.array-value1.tmp');payload=old+addition.encode();new_file(temp,payload);assert temp.read_bytes()==payload;os.replace(temp,path)
append(root/'docs/verification/rdbc-first/task7-standard-resource-candidates-2026-10-08.md',body)
line='\n\n2026-10-08 Array唯一标准值句柄候选：删除旧ArrayAccess及factory/context/重导出，Box标准对象共享Send独占调用与释放确认，无新增RDBC调度。旧完整消费者E0308 RED，完整128RDBC/409Druid严格编译0，保留原58源并扩展65/0、锁交接及panic67/0。32能力组合/十方法、取消与失败重试、池化父关闭driver一次通过；两个完整单源反例精确0/1，Cell Sync及旧Access负向编译失败。八源补丁/完整zip11/实际命令日志SHA归档，1167冻结输入及rs集合变化0。未生产apply/Cargo/实库认证，Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18 OPEN。详见task7-standard-resource-candidates-2026-10-08.md。\n'
append(root/'docs/verification/rdbc-first/status.md',line);append(mirror/'progress.md',line)
print(json.dumps({'summary':str(summary),'results':state['results'],'archive':state['archive'],'changed_inputs':state['changed_inputs']},ensure_ascii=False),flush=True)
