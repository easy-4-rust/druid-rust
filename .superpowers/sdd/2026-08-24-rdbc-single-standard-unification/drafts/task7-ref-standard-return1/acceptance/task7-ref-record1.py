"""Archive exact complete candidates and actual Ref retirement evidence without applying production code."""
import datetime,difflib,hashlib,json,os,re,subprocess,zipfile
from pathlib import Path
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008');root=Path('E:/workspaces/workspace-github-easy-4-rust/druid-rust');mirror=root/'.superpowers/sdd/2026-08-24-rdbc-single-standard-unification'
rdbc=base/'task7-ref-rdbc-candidate1';druid=base/'task7-ref-druid-candidate1';mutation=base/'task7-ref-capability-mutation1'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
records=[json.loads((p/'summary.json').read_text(encoding='utf-8')) for p in (rdbc,druid,mutation)]
expanded=json.loads((druid/'contract2.summary.json').read_text(encoding='utf-8'))
assert all(record['terminal'] for record in records+[expanded])
assert expanded['commands'][-1]['exit_code']==0
state={'observed':datetime.datetime.now(datetime.timezone.utc).isoformat(),'production_applied':False,'formal_cargo_certified':False,'results':{},'patches':[],'removed_sources':[],'mirrors':{},'source_manifest':{}}
for folder,record in zip((rdbc,druid,mutation),records):
    actual={p.relative_to(folder).as_posix():sha(p) for p in (folder/'src').rglob('*.rs')};assert actual==record['sources']
assert records[1]['rdbc_sources']==records[0]['sources']
for folder,name in ((rdbc,'green-runtime'),(druid,'consumer-runtime'),(druid,'consumer2-runtime'),(mutation,'exact-matrix-runtime')):
    match=re.search(r'test result: .*? (\d+) passed; (\d+) failed; (\d+) ignored;',(folder/(name+'.log')).read_text(encoding='utf-8'));assert match
    state['results'][name]={'passed':int(match[1]),'failed':int(match[2]),'ignored':int(match[3])}
assert state['results']=={'green-runtime':{'passed':7,'failed':0,'ignored':0},'consumer-runtime':{'passed':56,'failed':0,'ignored':0},'consumer2-runtime':{'passed':58,'failed':0,'ignored':0},'exact-matrix-runtime':{'passed':0,'failed':1,'ignored':0}}
snapshot=json.loads((base/'task6-current-full-validation1.inputs.json').read_text(encoding='utf-8'))
state['changed_inputs']=[p for p,digest in snapshot['inputs'].items() if not (root/p).is_file() or sha(root/p)!=digest];assert not state['changed_inputs'];state['frozen_input_count']=len(snapshot['inputs'])
frozen_rs={p for p in snapshot['inputs'] if p.startswith('crates/') and p.endswith('.rs')};actual_rs={p.relative_to(root).as_posix() for p in (root/'crates').rglob('*.rs')}
state['crates_rs_added']=sorted(actual_rs-frozen_rs);state['crates_rs_removed']=sorted(frozen_rs-actual_rs);assert not state['crates_rs_added'] and not state['crates_rs_removed']
draft=mirror/'drafts/task7-ref-standard-return1'
def new_file(path,payload):
    path.parent.mkdir(parents=True,exist_ok=True)
    with path.open('xb') as out:out.write(payload)
def copy(source,dest):
    new_file(dest,source.read_bytes());assert dest.read_bytes()==source.read_bytes();state['mirrors'][dest.relative_to(root).as_posix()]=sha(source)
patch=[]
for current,previous,label in ((rdbc,base/'task7-struct-rdbc-candidate1','druid-rdbc'),(druid,base/'task7-struct-druid-candidate1','druid')):
    relative={p.relative_to(current) for p in (current/'src').rglob('*.rs')}|{p.relative_to(previous) for p in (previous/'src').rglob('*.rs')}
    for rel in sorted(relative):
        old=previous/rel;new=current/rel;before=old.read_bytes() if old.exists() else b'';after=new.read_bytes() if new.exists() else b''
        if before==after:continue
        name='crates/'+label+'/'+rel.as_posix();state['patches'].append(name)
        if new.exists():copy(new,draft/name)
        else:state['removed_sources'].append(name)
        patch+=list(difflib.unified_diff(before.decode('utf-8').splitlines(keepends=True),after.decode('utf-8').splitlines(keepends=True),fromfile='a/'+name,tofile='b/'+name))
assert len(state['patches'])==10,state['patches'];assert state['removed_sources']==['crates/druid-rdbc/src/resource/rdbc_ref_access.rs']
new_file(draft/'ref.patch',''.join(patch).encode('utf-8'));new_file(draft/'removed_sources.json',(json.dumps(state['removed_sources'],indent=2)+'\n').encode())
files=[];state['java_sources']=records[0]['java_sources']
for row in state['java_sources'].values():
    p=Path(row['local_path']);assert sha(p)==row['sha256'];files.append((p,'java/'+p.name))
provenance=base/'task7-ref-java-provenance1';provenance.mkdir();state['java_druid']={}
commit='33824c3dec1612711f9bb4e409319bcab2e4cd0e'
for name in ('DruidPooledResultSet','DruidPooledCallableStatement'):
    path='core/src/main/java/com/alibaba/druid/pool/'+name+'.java';payload=subprocess.check_output(['git','show',commit+':'+path],cwd='E:/workspaces/workspace-github/druid')
    p=provenance/(name+'.java');new_file(p,payload);methods=re.findall(rb'public Ref getRef\([^)]*\) throws SQLException \{.*?\n    \}',payload,re.S)
    assert len(methods)==2 and all(b'return ' in m and b'.getRef(' in m and b'checkException(t)' in m for m in methods)
    state['java_druid'][name]={'commit':commit,'path':path,'sha256':sha(p),'method_sha256':[hashlib.sha256(m).hexdigest() for m in methods]};files.append((p,'java/'+p.name))
for folder in (rdbc,druid,mutation):
    for p in sorted(folder.iterdir()):
        if p.is_file() and p.suffix in ('.log','.json','.rs'):files.append((p,folder.name+'/'+p.name))
for name in ('task7-ref-handle1.rs','task7-struct-handle1.rs','task7-ref-contract1.rs','task7-ref-pooled-flow1.rs','task7-pooled-ref-consumer1.rs','task7-pooled-ref-consumer2.rs','task7-ref-rdbc1.py','task7-ref-rdbc2.py','task7-ref-rdbc3.py','task7-ref-pooled1.py','task7-ref-consumer2.py','task7-ref-mutation1.py','task7-ref-record1.py'):
    files.append((base/name,name))
for source,rel in files:copy(source,draft/'acceptance'/rel)
state['artifact_sha256']={str(p):sha(p) for p in (rdbc/'libdruid_rdbc.rlib',rdbc/'green.exe',druid/'libdruid.rlib',druid/'consumer.exe',druid/'consumer2.exe')}
assert state['artifact_sha256'][str(rdbc/'libdruid_rdbc.rlib')]==expanded['rdbc_rlib_sha256'];assert state['artifact_sha256'][str(druid/'libdruid.rlib')]==expanded['rlib_sha256']
archive=base/'task7-full-druid-rdbc-source-candidate10.zip'
with zipfile.ZipFile(archive,'x',compression=zipfile.ZIP_DEFLATED) as out:
    for folder,label in ((rdbc/'src','crates/druid-rdbc/src'),(druid/'src','crates/druid/src'),(druid/'resources','crates/druid/resources')):
        for p in sorted(folder.rglob('*')):
            if p.is_file():
                name=label+'/'+p.relative_to(folder).as_posix();state['source_manifest'][name]=sha(p);out.write(p,name)
    for source,rel in files:out.write(source,'acceptance/'+rel)
    out.writestr('manifest.json',json.dumps(state,ensure_ascii=False,indent=2)+'\n')
with zipfile.ZipFile(archive) as check:
    assert check.testzip() is None
    for name,digest in state['source_manifest'].items():assert hashlib.sha256(check.read(name)).hexdigest()==digest
assert len(state['source_manifest'])==557
state['archive']={'path':str(archive),'sha256':sha(archive),'source_count':len(state['source_manifest'])};copy(archive,mirror/('task6-e-space-'+archive.name))
summary=base/'task7-ref-standard-return1-validation.summary.json';new_file(summary,(json.dumps(state,ensure_ascii=False,indent=2)+'\n').encode('utf-8'));new_file(mirror/('task6-e-space-'+summary.name),summary.read_bytes())
body=f'''

## Ref唯一标准端口退役旧Access及Struct资源返回候选（2026-10-08）

固定OpenJDK8u Ref/Struct来源SHA逐项核对；Ref是SQL结构值的逻辑引用，取得句柄不能物化属性，全部四JDBC方法必须实现，没有Wrapper/free/Connection#createRef。固定Druid1.2.28提交{commit}的ResultSet/Callable两组getRef重载均原样返回底层对象，仅getter本身错误交给checkException；完整Java原件与四方法SHA归档。

RdbcRef共享值句柄现在只持有Mutex<Box<dyn Ref>>，不再持有Access/ResourceLifecycle；删除rdbc_ref_access.rs、旧factory/context路径、Druid旧重导出及过时ResourceType::Ref分支。没有保留桥接driver接口。标准对象只要求Send，克隆共享身份及串行独占调用，Ref操作之外的FREE/STREAM位不对外宣称。不附加连接关闭失效或owner释放策略。新增RdbcStruct值句柄承载唯一标准Struct并加入RdbcObject::Struct，用于默认Ref目标及嵌套属性；保留属性延迟读取、SQL NULL、显式映射、克隆身份。PreparedInputParameter明确拒绝Struct的通用标量物化，原样保留对象供native adapter处理。

实际同源RED/GREEN：旧完整RDBC编译1/E0308旧Access入参及E0432/E0599缺Struct返回值；新完整127源最初E0004遗漏标量分支、随后-Dwarnings发现废弃ResourceType::Ref，修正后严格-Dwarnings/forbidunsafe完整rlib编译0，原七合同7/0。Cell驱动Sync负向E0277、擅加Ref free负向E0407、导入旧Access负向E0432均为实际编译失败。全部8种READ/WRITE/TYPE_MAP组合共32调用验证capability及驱动计数；仅删除句柄能力拒绝的完整127源反例仍严格编译0，同一精确矩阵实际0/1失败，唯一源差异rdbc_ref.rs。

完整409源Druid对新RDBC最初因旧重导出E0432失败；移除后严格完整rlib0。原47资源消费者源逐字保留，加入七Ref合同及两池化返回合同后实际56/0；进一步强化返回Ref的方法错误为08006/code96并新增参数/嵌套资源合同，实际58/0。池化ResultSet按下标/原列名返回同一标准Ref、父cursor/statement/connection关闭后仍由驱动语义决定，Ref目标Struct延迟属性、取消后共享锁释放、SQL NULL、原错误、Ref/setObject参数身份和嵌套Struct属性身份均有实际证据。08006原错误不额外触发Druid discard且连接保持可用，对应Java直接返回对象的边界。

完整源码zip10共127RDBC/409Druid/21资源557项逐项解压核对SHA，zip SHA-256为{state['archive']['sha256']}。十项源增改删差异、实际命令/环境/log、反例/负向源及Java固定原件逐字节镜像到drafts/task7-ref-standard-return1，summary绑定实际rlib/exe与全部源码。1167冻结输入变化0，crates下.rs新增/删除集合均为空。

本轮仅候选，未生产apply、正式Cargo/Clippy/fmt、旧全套测试夹具迁移、真实数据库或Java运行时差分认证。其余LOB/Array/XML旧Access端口、Struct旧DTO唯一归属、SQLData/类型映射全矩阵、Callable返回/Prepared driver全矩阵、真实async流式读取仍开放；不得把共享值句柄视为第二套标准接口或Task7全完成。Task6 FIX_REQUIRED / Task7八义务 / Task8 INCOMPLETE / 18顶层OPEN。
'''
def append(path,addition):
    old=path.read_bytes();new_file(base/(path.name+'.before-ref1'),old);temp=path.with_name(path.name+'.ref1.tmp');payload=old+addition.encode('utf-8');new_file(temp,payload);assert temp.read_bytes()==payload;os.replace(temp,path)
append(root/'docs/verification/rdbc-first/task7-standard-resource-candidates-2026-10-08.md',body)
line='\n\n2026-10-08 Ref唯一标准/Struct返回候选：删除旧RefAccess及factory/context/Druid重导出，不新增free/createRef/owner；共享句柄承载Send非Sync标准对象与延迟属性。完整127RDBC严格rlib0、七合同7/0、三个负向编译实际失败；能力反例完整编译0、同矩阵0/1。完整409Druid旧重导出E0432 RED后严格0，原47消费者源保留，扩展56/0及fatal原错误/参数/嵌套资源58/0。固定Java四getRef源码SHA、十项补丁/完整zip10/命令日志与实际产物SHA已归档，1167冻结输入0变化且crates rs集合无新增删除。未生产apply/Cargo/旧全套夹具迁移/真实DB认证；Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18 OPEN。详见task7-standard-resource-candidates-2026-10-08.md。\n'
append(root/'docs/verification/rdbc-first/status.md',line);append(mirror/'progress.md',line)
print(json.dumps({'summary':str(summary),'results':state['results'],'patches':state['patches'],'archive_sha256':state['archive']['sha256'],'evidence_files':len(files),'changed_inputs':state['changed_inputs']},ensure_ascii=False),flush=True)
