"""Record real pooled XML diagnostic RED/GREEN without broadening candidate scope."""
import datetime,difflib,hashlib,json,os,zipfile
from pathlib import Path
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008');root=Path('E:/workspaces/workspace-github-easy-4-rust/druid-rust')
mirror=root/'.superpowers/sdd/2026-08-24-rdbc-single-standard-unification';sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
old=base/'task7-xml-value-druid-candidate1';dest=base/'task7-xml-value-druid-candidate2';rdbc=base/'task7-xml-value-rdbc-candidate1'
summary=json.loads((dest/'summary.json').read_text(encoding='utf-8'));prior=json.loads((old/'summary.json').read_text(encoding='utf-8'))
assert summary['terminal'];assert summary['changed_sources']==['src/core/druid_sql_xml.rs']
assert {p.relative_to(dest).as_posix():sha(p)for p in(dest/'src').rglob('*.rs')}==summary['sources']
assert {p.relative_to(rdbc).as_posix():sha(p)for p in(rdbc/'src').rglob('*.rs')}==prior['rdbc_sources']
assert [p.relative_to(dest).as_posix()for p in(dest/'src').rglob('*.rs')if sha(p)!=sha(old/p.relative_to(dest))]==['src/core/druid_sql_xml.rs']
assert all(sha(p)==sha(old/p.relative_to(dest))for p in(dest/'resources').rglob('*')if p.is_file())
assert sha(dest/'libdruid.rlib')==summary['rlib_sha256'] and sha(dest/'consumer3.exe')==summary['exe_sha256']
assert sha(base/'task7-pooled-xml-value-consumer3.rs')==summary['consumer_sha256']
assert (base/'task7-pooled-xml-value-consumer3.rs').read_bytes().startswith((base/'task7-pooled-xml-value-consumer2.rs').read_bytes())
for row in summary['commands']:
    assert sha(Path(row['log']))==row['log_sha256']
    assert row['exit_code']==(101 if row['name']=='diagnostic-red'else 0)
assert '0 passed; 1 failed;'in(dest/'diagnostic-red.log').read_text(encoding='utf-8')
assert 'Blob'in(dest/'diagnostic-red.log').read_text(encoding='utf-8')
assert '104 passed; 0 failed; 0 ignored;'in(dest/'consumer3-runtime.log').read_text(encoding='utf-8')
snapshot=json.loads((base/'task6-current-full-validation1.inputs.json').read_text(encoding='utf-8'))
changed=[p for p,digest in snapshot['inputs'].items()if not(root/p).is_file()or sha(root/p)!=digest];assert not changed and len(snapshot['inputs'])==1167
assert {p for p in snapshot['inputs']if p.startswith('crates/')and p.endswith('.rs')}=={p.relative_to(root).as_posix()for p in(root/'crates').rglob('*.rs')}
state={'observed':datetime.datetime.now(datetime.timezone.utc).isoformat(),'production_applied':False,'formal_cargo_certified':False,'red':{'passed':0,'failed':1},'green':{'passed':104,'failed':0},'changed_sources':summary['changed_sources'],'changed_inputs':changed,'frozen_input_count':1167,'source_manifest':{},'extern_inputs':{}}
for row in summary['commands']:
    for i,arg in enumerate(row['command'][:-1]):
        if arg=='--extern':p=Path(row['command'][i+1].split('=',1)[1]);state['extern_inputs'][str(p)]=sha(p)
def new_file(path,payload):
    path.parent.mkdir(parents=True,exist_ok=True)
    with path.open('xb')as out:out.write(payload)
def copy(source,dest):new_file(dest,source.read_bytes());assert sha(source)==sha(dest)
draft=mirror/'drafts/task7-xml-cleanup-diagnostic1';p=dest/'src/core/druid_sql_xml.rs'
copy(p,draft/'crates/druid/src/core/druid_sql_xml.rs')
patch=''.join(difflib.unified_diff((old/'src/core/druid_sql_xml.rs').read_text(encoding='utf-8').splitlines(keepends=True),p.read_text(encoding='utf-8').splitlines(keepends=True),fromfile='a/crates/druid/src/core/druid_sql_xml.rs',tofile='b/crates/druid/src/core/druid_sql_xml.rs'))
new_file(draft/'diagnostic.patch',patch.encode())
files=[(p,p.name)for p in sorted(dest.glob('*.log'))]+[(dest/'summary.json','summary.json'),(dest/'druid_sql_xml.before.rs','druid_sql_xml.before.rs')]
for name in['task7-xml-cleanup-diagnostic1.py','task7-xml-cleanup-record1.py','task7-pooled-xml-value-consumer3.rs']:files.append((base/name,name))
for source,name in files:copy(source,draft/'acceptance'/name)
previous=base/'task7-full-druid-rdbc-source-candidate14.zip';previous_record=json.loads((base/'task7-xml-standard-value1-validation.summary.json').read_text(encoding='utf-8'));assert sha(previous)==previous_record['archive']['sha256']
archive=base/'task7-full-druid-rdbc-source-candidate15.zip'
with zipfile.ZipFile(previous)as before,zipfile.ZipFile(archive,'x',zipfile.ZIP_DEFLATED)as out:
    for info in before.infolist():
        if info.filename.startswith('acceptance/'):out.writestr(info.filename,before.read(info.filename))
    for folder,label in[(rdbc,'druid-rdbc'),(dest,'druid')]:
        for area in['src','resources']:
            if not(folder/area).exists():continue
            for source in sorted((folder/area).rglob('*')):
                if source.is_file():name='crates/'+label+'/'+source.relative_to(folder).as_posix();out.write(source,name);state['source_manifest'][name]=sha(source)
    for source,name in files:out.write(source,'acceptance/cleanup-diagnostic/'+name)
    out.writestr('source_manifest.json',json.dumps(state['source_manifest'],indent=2)+'\n')
assert len(state['source_manifest'])==553
with zipfile.ZipFile(archive)as saved:
    for name,digest in state['source_manifest'].items():assert hashlib.sha256(saved.read(name)).hexdigest()==digest
state['archive']={'path':str(archive),'sha256':sha(archive),'source_entries':553,'inherited_evidence_files':previous_record['archive']['evidence_files'],'new_evidence_files':len(files)}
copy(archive,mirror/('task6-e-space-'+archive.name))
record=base/'task7-xml-cleanup-diagnostic1-validation.summary.json';new_file(record,(json.dumps(state,indent=2,ensure_ascii=False)+'\n').encode());copy(record,mirror/('task6-e-space-'+record.name))
def append(path,text):
    old=path.read_bytes();new_file(base/(path.name+'.before-xml-cleanup1'),old);temp=path.with_name(path.name+'.xml-cleanup1.tmp');payload=old+text.encode();new_file(temp,payload);assert temp.read_bytes()==payload;os.replace(temp,path)
body=f'''\n\n## SQLXML排队清理诊断修复候选（2026-10-08）

通过实际DruidPooledConnection创建两个标准XML。第一个真实驱动free阻塞同一registry coordinator，第二个free显式poll进入Pending后取消调用方；第二个驱动尚未free、读取零驱动调用，清理仍由registry持有。第二个XML读取实际返回Blob关闭诊断，旧完整候选精确0PASS1FAIL RED。测试释放两驱动、关闭父连接并确认每个driver free一次及owner归还一次后再检查诊断，不以挂起/超时冒充目标失败。

后继完整409Druid候选仅druid_sql_xml.rs一处字符串Blob改SQLXML，RDBC123源码及全部资源不变，原103消费者源字节前缀保留。完整库与同一104消费者严格编译0、全部104/0 GREEN，修复对应已观察分支。此前三反例仍绑定修复前候选103范围，不声称已对104重跑；该后继仅诊断修复。原RED/新GREEN、完整源码/产物/extern SHA、补丁及zip15归档，553源码资源逐项解压SHA校验，SHA {state['archive']['sha256']}。

1167正式冻结输入及crates rs集合变化0。未生产apply、正式Cargo/Clippy/全仓fmt、API/SemVer或真实DB认证；Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。全仓串行原作业状态另记。
'''
append(root/'docs/verification/rdbc-first/task7-standard-resource-candidates-2026-10-08.md',body)
line='\n\n2026-10-08 SQLXML排队清理诊断候选：实际池化双XML/同registry，首驱动释放阻塞、次free入队后取消，次XML读取零driver调用但误报Blob；精确0/1 RED。仅一字符串改SQLXML，完整409Druid重建0、RDBC123不变，原103源前缀保留，同104消费者104/0 GREEN，两驱动free各一次/父归还一次。源码/日志/命令/产物SHA、单源补丁与zip15归档，1167冻结及rs集合变化0。仅候选，未生产apply/Cargo/API/DB；18顶层OPEN。详见task7-standard-resource-candidates-2026-10-08.md。\n'
append(root/'docs/verification/rdbc-first/status.md',line);append(mirror/'progress.md',line)
print(json.dumps({'record':str(record),'archive':state['archive'],'red':state['red'],'green':state['green'],'changed_inputs':changed},ensure_ascii=False),flush=True)
