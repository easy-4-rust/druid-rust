"""Archive verified XML candidate without applying production changes."""
import datetime,difflib,hashlib,json,os,re,sys,zipfile
from pathlib import Path
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
root=Path('E:/workspaces/workspace-github-easy-4-rust/druid-rust')
mirror=root/'.superpowers/sdd/2026-08-24-rdbc-single-standard-unification'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
rdbc=base/'task7-xml-value-rdbc-candidate1';druid=base/'task7-xml-value-druid-candidate1'
oldr=base/'task7-clob-value-rdbc-candidate1';oldd=base/'task7-clob-value-druid-candidate1'
normal=json.loads((druid/'summary.json').read_text(encoding='utf-8'))
expanded=json.loads((druid/'expanded.summary.json').read_text(encoding='utf-8'))
assert normal['terminal'] and expanded['terminal']
state={'observed':datetime.datetime.now(datetime.timezone.utc).isoformat(),'production_applied':False,'formal_cargo_certified':False,'results':{},'mirrors':{},'patches':[],'removed_sources':[],'source_manifest':{}}
sys.path.insert(0,str(root/'scripts'));import verify_task6_method_parity as verifier
def functions(text):
    masked=verifier._mask_rust(text);result=[]
    for match in re.finditer(r'#\[(?:tokio::)?test[^\]]*\]\s*(?:async\s+)?fn\s+(\w+)',masked):
        opening=masked.index('{',match.end());closing=verifier._matching_brace(masked,opening)
        result.append((match[1],text[match.start():closing+1]))
    return result
prior=(base/'task7-pooled-clob-value-consumer3.rs').read_bytes();prior_tests=functions(prior.decode());assert len(prior_tests)==92
for name,count in [('task7-pooled-xml-value-consumer1.rs',102),('task7-pooled-xml-value-consumer2.rs',103)]:
    payload=(base/name).read_bytes();assert payload.startswith(prior)
    tests=functions(payload.decode());assert len(tests)==count and tests[:92]==prior_tests
state['prior_92_tests']=[{'name':name,'sha256':hashlib.sha256(body.encode()).hexdigest()}for name,body in prior_tests]
for folder,key in [(rdbc,'rdbc_sources'),(druid,'sources')]:
    assert {p.relative_to(folder).as_posix():sha(p)for p in(folder/'src').rglob('*.rs')}==normal[key]
assert len(normal['rdbc_sources'])==123 and len(normal['sources'])==409
for name,expected in [('consumer-runtime',(102,0)),('consumer2-runtime',(103,0)),('capability-runtime',(0,1)),('one-shot-runtime',(0,1)),('late-consumption-runtime',(0,1))]:
    match=re.search(r'test result: .*? (\d+) passed; (\d+) failed; (\d+) ignored;',(druid/(name+'.log')).read_text(encoding='utf-8'));assert match
    result={'passed':int(match[1]),'failed':int(match[2]),'ignored':int(match[3])};assert(result['passed'],result['failed'])==expected and result['ignored']==0;state['results'][name]=result
for variant in ['capability','one-shot','late-consumption']:
    folder=Path(expanded[variant]['root']);assert {p.relative_to(folder).as_posix():sha(p)for p in folder.rglob('*.rs')}==expanded[variant]['sources']
    changed=[p.relative_to(rdbc).as_posix()for p in(rdbc/'src').rglob('*.rs')if sha(p)!=sha(folder/'rdbc'/p.relative_to(rdbc))]
    expected='src/resource/sql_xml_value.rs'if variant=='one-shot'else'src/resource/rdbc_sql_xml.rs';assert changed==[expected];state[variant+'_changed_sources']=changed
    assert all(sha(p)==sha(folder/'druid'/p.relative_to(druid))for p in(druid/'src').rglob('*.rs'))
for record in [normal,expanded]:
    for row in record['commands']:
        assert sha(Path(row['log']))==row['log_sha256']
        if row['name'].endswith('-compile')or row['name']=='format-new-objects':assert row['exit_code']==0
for name,code in [('negative-sync','E0277'),('negative-old-access','E0432')]:
    row=next(r for r in expanded['commands']if r['name']==name);assert row['exit_code']!=0 and code in Path(row['log']).read_text(encoding='utf-8')
snapshot=json.loads((base/'task6-current-full-validation1.inputs.json').read_text(encoding='utf-8'))
state['changed_inputs']=[p for p,digest in snapshot['inputs'].items()if not(root/p).is_file()or sha(root/p)!=digest];assert not state['changed_inputs']
state['frozen_input_count']=len(snapshot['inputs']);assert state['frozen_input_count']==1167
frozen={p for p in snapshot['inputs']if p.startswith('crates/')and p.endswith('.rs')};actual={p.relative_to(root).as_posix()for p in(root/'crates').rglob('*.rs')};assert frozen==actual
state['crates_rs_added']=[];state['crates_rs_removed']=[]
acquisition=json.loads((base/'task8-java-jdbc42-source-acquisition1.summary.json').read_text(encoding='utf-8'))
java=next(row for row in acquisition['sources']if row['fqcn']=='java.sql.SQLXML');assert sha(Path(java['local_path']))==java['sha256'];state['fixed_java_source']=java
state['artifact_sha256']={str(p):sha(p)for p in[rdbc/'libdruid_rdbc.rlib',druid/'libdruid.rlib',druid/'consumer.exe',druid/'consumer2.exe']}
assert sha(rdbc/'libdruid_rdbc.rlib')==normal['rdbc_rlib_sha256'] and sha(druid/'libdruid.rlib')==normal['rlib_sha256']
assert sha(druid/'consumer.exe')==normal['exe_sha256'] and sha(druid/'consumer2.exe')==expanded['exe_sha256']
state['extern_inputs']={}
for record in[normal,expanded]:
    for row in record['commands']:
        for i,arg in enumerate(row['command'][:-1]):
            if arg=='--extern':p=Path(row['command'][i+1].split('=',1)[1]);state['extern_inputs'][str(p)]=sha(p)
# 先构建并核对完整差异，所有断言完成后才写入仓库证据目录。
patch=[];changed_payloads=[]
for current,previous,label in[(rdbc,oldr,'druid-rdbc'),(druid,oldd,'druid')]:
    relative={p.relative_to(current)for p in(current/'src').rglob('*.rs')}|{p.relative_to(previous)for p in(previous/'src').rglob('*.rs')}
    for rel in sorted(relative):
        before=(previous/rel).read_bytes()if(previous/rel).exists()else b'';after=(current/rel).read_bytes()if(current/rel).exists()else b''
        if before==after:continue
        name='crates/'+label+'/'+rel.as_posix();state['patches'].append(name)
        if(current/rel).exists():changed_payloads.append((current/rel,name))
        else:state['removed_sources'].append(name)
        patch+=list(difflib.unified_diff(before.decode().splitlines(keepends=True),after.decode().splitlines(keepends=True),fromfile='a/'+name,tofile='b/'+name))
assert len(state['patches'])==10 and len(state['removed_sources'])==5
files=[(p,p.name)for p in sorted(druid.glob('*.log'))]+[(druid/'summary.json','summary.json'),(druid/'expanded.summary.json','expanded.summary.json'),(Path(java['local_path']),'java/SQLXML.java')]
for name in['task7-xml-value-build1.py','task7-xml-value-contract1.rs','task7-xml-value-verify1.py','task7-xml-value-record1.py','task7-pooled-xml-value-consumer1.rs','task7-pooled-xml-value-consumer2.rs']:files.append((base/name,name))
for variant in['capability','one-shot','late-consumption']:files.append((Path(expanded[variant]['root'])/'rdbc'/expanded[variant]['changed_source'],variant+'/'+Path(expanded[variant]['changed_source']).name))
draft=mirror/'drafts/task7-xml-standard-value1'
def new_file(path,payload):
    path.parent.mkdir(parents=True,exist_ok=True)
    with path.open('xb')as out:out.write(payload)
def copy(source,dest):
    new_file(dest,source.read_bytes());assert sha(dest)==sha(source);state['mirrors'][dest.relative_to(root).as_posix()]=sha(source)
for source,name in changed_payloads:copy(source,draft/name)
new_file(draft/'xml.patch',''.join(patch).encode());new_file(draft/'removed_sources.json',(json.dumps(state['removed_sources'],indent=2)+'\n').encode())
for source,rel in files:copy(source,draft/'acceptance'/rel)
archive=base/'task7-full-druid-rdbc-source-candidate14.zip'
with zipfile.ZipFile(archive,'x',zipfile.ZIP_DEFLATED)as out:
    for folder,label in[(rdbc,'druid-rdbc'),(druid,'druid')]:
        for area in['src','resources']:
            if not(folder/area).exists():continue
            for p in sorted((folder/area).rglob('*')):
                if p.is_file():name='crates/'+label+'/'+p.relative_to(folder).as_posix();out.write(p,name);state['source_manifest'][name]=sha(p)
    for source,rel in files:out.write(source,'acceptance/'+rel)
    out.writestr('source_manifest.json',json.dumps(state['source_manifest'],indent=2)+'\n')
assert len(state['source_manifest'])==553
with zipfile.ZipFile(archive)as saved:
    for name,digest in state['source_manifest'].items():assert hashlib.sha256(saved.read(name)).hexdigest()==digest
state['archive']={'path':str(archive),'sha256':sha(archive),'source_entries':553,'evidence_files':len(files)};copy(archive,mirror/('task6-e-space-'+archive.name))
summary=base/'task7-xml-standard-value1-validation.summary.json';new_file(summary,(json.dumps(state,indent=2,ensure_ascii=False)+'\n').encode());copy(summary,mirror/('task6-e-space-'+summary.name))
def append(path,addition):
    old=path.read_bytes();new_file(base/(path.name+'.before-xml-value1'),old);temp=path.with_name(path.name+'.xml-value1.tmp');payload=old+addition.encode();new_file(temp,payload);assert temp.read_bytes()==payload;os.replace(temp,path)
body=f'''\n\n## SQLXML唯一标准值与一次性读写候选（2026-10-08）

固定OpenJDK SQLXML.java源码SHA核对。工厂接受Box<dyn SqlXml>，共享句柄复用既有Send独占容器；新增内部SqlXmlValue只保存驱动与读写概念状态，不新增驱动接口、owner或调度。四种读取共享一次机会，四种写入共享一次机会；能力拒绝及未取得驱动锁的取消不消费，真正进入驱动前即消费，错误、panic或取消不恢复。getter别名共享同一状态。Source/Result的None保留Java null厂商默认请求，UTF-16及原流身份不改写。退役最后SQLXML Access与四个无调用方旧资源上下文类型。

旧完整消费者与Druid旧重导出实际RED；完整123RDBC/409Druid严格编译0，原92项测试源码前缀及函数体逐字保留，新增十合同102/0。再补真实锁队列中free先于读取的锁后状态重查合同，同源103/0。覆盖16能力组合/八操作、共享读写机会、原始流写入与UTF-16、nullable框架、失败/panic/取消、并发读只进入一次、释放失败及取消后重试、实际池化父关闭与fatal分类。Send非Sync及旧Access导入负向编译E0277/E0432。

三个完整单源反例分别绕过文本读取能力、重新开放一次性机会、把读取消费移到驱动await之后；每个完整123RDBC/409Druid与同一103消费者严格编译0，精确合同均0PASS1FAIL。实际命令、日志、源码及rlib/exe/extern SHA归档。十项增改删补丁、完整zip14共553源码/资源逐项解压SHA核验，SHA {state['archive']['sha256']}。

1167正式冻结输入及crates rs集合变化0。仅候选，未生产apply、正式Cargo/Clippy/全仓fmt、API/SemVer、真实DB或Java运行时差分认证。源码复核另发现DruidSqlXml的cleanup_requested拒绝分支误用Blob诊断名称，当前103合同未独立覆盖该分支，待后继修复，不宣称该缺口已闭合。Struct旧DTO归属、返回/参数完整矩阵、ResultSet统一及真正async streaming仍开放。全仓原串行作业独立等待终态；Task6 FIX_REQUIRED、Task7八义务、Task8 INCOMPLETE、18顶层OPEN。
'''
append(root/'docs/verification/rdbc-first/task7-standard-resource-candidates-2026-10-08.md',body)
line='\n\n2026-10-08 SQLXML唯一标准值候选：工厂接管标准对象，退役最后Access及四旧上下文类型；共享一次性读写机会，进入驱动前消费，失败/panic/取消不恢复，未入驱动取消不消费。完整123RDBC/409Druid严格编译0，原92测试逐字保留，新增102/0及free排队后读取103/0；三个完整单源反例精确0/1，Sync/旧Access负向失败。十项补丁/完整zip14/命令日志与产物SHA归档，1167冻结及rs集合变化0。仅候选，未生产apply/Cargo/API/实库认证；Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18 OPEN。详见task7-standard-resource-candidates-2026-10-08.md。\n'
append(root/'docs/verification/rdbc-first/status.md',line);append(mirror/'progress.md',line)
print(json.dumps({'summary':str(summary),'results':state['results'],'archive':state['archive'],'changed_inputs':state['changed_inputs']},ensure_ascii=False),flush=True)
