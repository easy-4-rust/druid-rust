"""Archive exact Clob/NClob candidate source, real logs, negative tests and source fidelity."""
import datetime,difflib,hashlib,json,os,re,sys,zipfile
from pathlib import Path
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008');root=Path('E:/workspaces/workspace-github-easy-4-rust/druid-rust');mirror=root/'.superpowers/sdd/2026-08-24-rdbc-single-standard-unification';sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
oldr=base/'task7-blob-value-rdbc-candidate2';oldd=base/'task7-blob-value-druid-candidate2';rdbc=base/'task7-clob-value-rdbc-candidate1';druid=base/'task7-clob-value-druid-candidate1'
normal=json.loads((druid/'summary.json').read_text(encoding='utf-8'));expanded=json.loads((druid/'expanded.summary.json').read_text(encoding='utf-8'));extra=json.loads((druid/'extra.summary.json').read_text(encoding='utf-8'));assert normal['terminal'] and expanded['terminal'] and extra['terminal']
state={'observed':datetime.datetime.now(datetime.timezone.utc).isoformat(),'production_applied':False,'formal_cargo_certified':False,'results':{},'mirrors':{},'patches':[],'removed_sources':[],'source_manifest':{}}
sys.path.insert(0,str(root/'scripts'));import verify_task6_method_parity as method_verifier
def functions(text):
    masked=method_verifier._mask_rust(text);result=[]
    for match in re.finditer(r'#\[(?:tokio::)?test[^\]]*\]\s*(?:async\s+)?fn\s+(\w+)',masked):
        opening=masked.index('{',match.end());closing=method_verifier._matching_brace(masked,opening);result.append((match[1],text[match.start():closing+1]))
    return result
prior=functions((base/'task7-pooled-blob-value-consumer4.rs').read_text(encoding='utf-8'));assert len(prior)==80
migration=normal['existing_test_argument_migrations'];assert migration['count']==2 and not migration['assertions_changed'];state['existing_test_argument_migrations']=migration
for name,expected in [('task7-pooled-clob-value-consumer1.rs',89),('task7-pooled-clob-value-consumer2.rs',90),('task7-pooled-clob-value-consumer3.rs',92)]:
    text=(base/name).read_text(encoding='utf-8');assert len(functions(text))==expected;normalized=text.replace(migration['new'],migration['old']);assert functions(normalized)[:80]==prior
state['prior_80_tests']=[{'name':name,'sha256':hashlib.sha256(body.encode()).hexdigest()}for name,body in prior]
for folder,key in [(rdbc,'rdbc_sources'),(druid,'sources')]:assert {p.relative_to(folder).as_posix():sha(p)for p in (folder/'src').rglob('*.rs')}==normal[key]
for name,expected in [('consumer-runtime',(89,0)),('consumer2-runtime',(90,0)),('consumer3-runtime',(92,0)),('capability-runtime',(0,1)),('self-alias-runtime',(0,1)),('lock-order-runtime',(0,1))]:
    match=re.search(r'test result: .*? (\d+) passed; (\d+) failed; (\d+) ignored;',(druid/(name+'.log')).read_text(encoding='utf-8'));assert match;result={'passed':int(match[1]),'failed':int(match[2]),'ignored':int(match[3])};assert(result['passed'],result['failed'])==expected and result['ignored']==0;state['results'][name]=result
for variant in ['capability','self-alias','lock-order']:
    folder=Path(expanded[variant]['root']);assert {p.relative_to(folder).as_posix():sha(p)for p in folder.rglob('*.rs')}==expanded[variant]['sources'];changed=[p.relative_to(rdbc).as_posix()for p in (rdbc/'src').rglob('*.rs')if sha(p)!=sha(folder/'rdbc'/p.relative_to(rdbc))];assert changed==['src/resource/rdbc_clob.rs'];state[variant+'_changed_sources']=changed
snapshot=json.loads((base/'task6-current-full-validation1.inputs.json').read_text(encoding='utf-8'));state['changed_inputs']=[p for p,digest in snapshot['inputs'].items()if not(root/p).is_file()or sha(root/p)!=digest];assert not state['changed_inputs'];state['frozen_input_count']=len(snapshot['inputs'])
frozen={p for p in snapshot['inputs']if p.startswith('crates/')and p.endswith('.rs')};actual={p.relative_to(root).as_posix()for p in (root/'crates').rglob('*.rs')};assert frozen==actual;state['crates_rs_added']=[];state['crates_rs_removed']=[]
acquisition=json.loads((base/'task8-java-jdbc42-source-acquisition1.summary.json').read_text(encoding='utf-8'));java=[]
for fqcn in ['java.sql.Clob','java.sql.NClob']:
    row=next(row for row in acquisition['sources']if row['fqcn']==fqcn);p=Path(row['local_path']);assert sha(p)==row['sha256'];java.append((p,'java/'+p.name))
state['fixed_java_sources']=[row for row in acquisition['sources']if row['fqcn']in ['java.sql.Clob','java.sql.NClob']]
state['artifact_sha256']={str(p):sha(p)for p in [rdbc/'libdruid_rdbc.rlib',druid/'libdruid.rlib',druid/'consumer.exe',druid/'consumer2.exe',druid/'consumer3.exe']}
assert state['artifact_sha256'][str(rdbc/'libdruid_rdbc.rlib')]==normal['rdbc_rlib_sha256'];assert state['artifact_sha256'][str(druid/'libdruid.rlib')]==normal['rlib_sha256'];assert state['artifact_sha256'][str(druid/'consumer2.exe')]==expanded['exe_sha256'];assert state['artifact_sha256'][str(druid/'consumer3.exe')]==extra['exe_sha256']
state['extern_inputs']={}
for record in [normal,expanded,extra]:
    for row in record['commands']:
        for i,arg in enumerate(row['command'][:-1]):
            if arg=='--extern':p=Path(row['command'][i+1].split('=',1)[1]);assert p.is_file();state['extern_inputs'][str(p)]=sha(p)
# 全部检查先于E盘写入；新增证据使用独占创建，报告使用原像+原子替换。
draft=mirror/'drafts/task7-clob-standard-value1'
def new_file(path,payload):
    path.parent.mkdir(parents=True,exist_ok=True)
    with path.open('xb')as out:out.write(payload)
def copy(source,dest):new_file(dest,source.read_bytes());assert dest.read_bytes()==source.read_bytes();state['mirrors'][dest.relative_to(root).as_posix()]=sha(source)
patch=[]
for current,previous,label in [(rdbc,oldr,'druid-rdbc'),(druid,oldd,'druid')]:
    relative={p.relative_to(current)for p in(current/'src').rglob('*.rs')}|{p.relative_to(previous)for p in(previous/'src').rglob('*.rs')}
    for rel in sorted(relative):
        before=(previous/rel).read_bytes()if(previous/rel).exists()else b'';after=(current/rel).read_bytes()if(current/rel).exists()else b''
        if before==after:continue
        name='crates/'+label+'/'+rel.as_posix();state['patches'].append(name)
        if(current/rel).exists():copy(current/rel,draft/name)
        else:state['removed_sources'].append(name)
        patch+=list(difflib.unified_diff(before.decode().splitlines(keepends=True),after.decode().splitlines(keepends=True),fromfile='a/'+name,tofile='b/'+name))
assert len(state['patches'])==12 and len(state['removed_sources'])==2
new_file(draft/'clob.patch',''.join(patch).encode());new_file(draft/'removed_sources.json',(json.dumps(state['removed_sources'],indent=2)+'\n').encode())
files=[(p,p.name)for p in sorted(druid.glob('*.log'))]+[(druid/'summary.json','summary.json'),(druid/'expanded.summary.json','expanded.summary.json'),(druid/'extra.summary.json','extra.summary.json')]+java
for name in ['task7-clob-value-build1.py','task7-clob-value-contract1.rs','task7-clob-value-contract1-initial.rs','task7-clob-value-fixture-repair1.py','task7-clob-value-verify1.py','task7-clob-value-record1.py','task7-clob-value-national-direct1.rs','task7-clob-value-national-extra1.py','task7-pooled-clob-value-consumer1.rs','task7-pooled-clob-value-consumer1-initial.rs','task7-pooled-clob-value-consumer2.rs','task7-pooled-clob-value-consumer3.rs']:files.append((base/name,name))
for variant in ['capability','self-alias','lock-order']:files.append((Path(expanded[variant]['root'])/'rdbc/src/resource/rdbc_clob.rs',variant+'/rdbc_clob.rs'))
for source,rel in files:copy(source,draft/'acceptance'/rel)
archive=base/'task7-full-druid-rdbc-source-candidate13.zip'
with zipfile.ZipFile(archive,'x',zipfile.ZIP_DEFLATED)as out:
    for folder,label in [(rdbc,'druid-rdbc'),(druid,'druid')]:
        for area in ['src','resources']:
            if not(folder/area).exists():continue
            for p in sorted((folder/area).rglob('*')):
                if p.is_file():name='crates/'+label+'/'+p.relative_to(folder).as_posix();out.write(p,name);state['source_manifest'][name]=sha(p)
    for source,rel in files:out.write(source,'acceptance/'+rel)
    out.writestr('source_manifest.json',json.dumps(state['source_manifest'],indent=2)+'\n')
assert len(state['source_manifest'])==557
with zipfile.ZipFile(archive)as saved:
    for name,digest in state['source_manifest'].items():assert hashlib.sha256(saved.read(name)).hexdigest()==digest
state['archive']={'path':str(archive),'sha256':sha(archive),'source_entries':557,'evidence_files':len(files)};copy(archive,mirror/('task6-e-space-'+archive.name))
summary=base/'task7-clob-standard-value1-validation.summary.json';new_file(summary,(json.dumps(state,indent=2,ensure_ascii=False)+'\n').encode());copy(summary,mirror/('task6-e-space-'+summary.name))
def append(path,addition):
    old=path.read_bytes();new_file(base/(path.name+'.before-clob-value1'),old);temp=path.with_name(path.name+'.clob-value1.tmp');payload=old+addition.encode();new_file(temp,payload);assert temp.read_bytes()==payload;os.replace(temp,path)
body=f'''

## Clob/NClob唯一标准值句柄、共享视图及UTF-16候选（2026-10-08）

固定OpenJDK8u Clob/NClob来源SHA核对，保留十三JDBC操作；NClob仅继承Clob，不增加方法。工厂接管Box标准对象，退役两旧Access、context工厂与kind重导出。NClob上转不复制驱动对象，Clob视图/克隆共享身份、能力与释放确认；诊断保留NClob名称。复用既有Send共享容器，无新增RDBC调度。ClobPattern安全表示自身/另一标准模式，目标准入在双锁排序前检查；不强制快照，驱动处理真实搜索。新参数是候选公共签名变更，API/SemVer未认证。

旧完整库实际E0308/缺ClobPattern RED；完整127RDBC严格编译0，完整409Druid旧重导出/旧参数实际RED后严格编译0。首次消费者夹具ASCII collect类型推断E0283保留原source/log，显式Vec修复，不改变生产或断言。旧80项测试逐函数核对：仅两处模式参数包装为Other，归一化后全部原函数体逐字一致，断言不变；驱动夹具新增SameResource表示。新增九合同89/0，再补真实阻塞首锁反向搜索90/0。Clob/NClob各128能力组合覆盖十二非free方法与FREE门禁，释放后零驱动调用。UTF-16代理对/孤立代理项、1-based资源位置/0-based输入offset、Reader/Writer/ASCII原身份和实际写入、国家字符共享视图、自身start、错误优先级、取消/panic/失败重试、并发free一次、实际池化父关闭driver一次通过。

Cell Sync及旧两Access导入负向编译实际E0277/E0432。三个完整单源反例分别绕过length能力、固定自身搜索1、总是目标先取锁：每个127RDBC/409Druid及相同90消费者完整编译0，精确运行均0PASS1FAIL，唯一差异rdbc_clob.rs。另补按UUID实际排序确定两种准入优先级顺序，并直接调用NClob全部继承公开方法、原流身份及池化Clob/NClob交叉搜索，正常同源码扩展92/0；反例范围为90消费者，不冒充92重跑。源/日志/实际命令/env/rlib/exe/extern与固定Java来源SHA归档，十二源增改删补丁、完整zip13共557源码/资源项逐项解压核验，SHA {state['archive']['sha256']}。

1167正式冻结输入及crates rs集合变化0。候选未生产apply、正式Cargo/Clippy/全仓fmt、旧全套夹具迁移、真实DB或Java运行时差分认证。SQLXML旧Access、Struct旧DTO、资源返回/参数全矩阵与真正async streaming仍开放。完整Task6 FIX_REQUIRED、Task7八义务/Task8 INCOMPLETE、18顶层OPEN；全仓串行运行另计，不与候选90/0混淆。
'''
append(root/'docs/verification/rdbc-first/task7-standard-resource-candidates-2026-10-08.md',body)
line='\n\n2026-10-08 Clob/NClob唯一标准值句柄候选：两旧Access/context/kind退役，国家字符视图共享身份与释放确认，ClobPattern保留自身/他资源模式。旧完整RED后127RDBC/409Druid严格编译0；原80测试仅两参数表示迁移，归一化函数体一致、断言不变，新增89/0、锁序90/0、直接NClob继承方法及池化交叉搜索92/0。两类各128能力组合/十二方法、UTF-16/流原身份/取消与父关闭通过；三个完整单源反例各精确0/1，Cell Sync/旧Access负向失败。十二源补丁/完整zip13/原命令日志SHA归档，1167冻结及rs集合变化0。未生产apply/Cargo/API/实库认证；Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18 OPEN。详见task7-standard-resource-candidates-2026-10-08.md。\n'
append(root/'docs/verification/rdbc-first/status.md',line);append(mirror/'progress.md',line)
print(json.dumps({'summary':str(summary),'results':state['results'],'archive':state['archive'],'changed_inputs':state['changed_inputs']},ensure_ascii=False),flush=True)
