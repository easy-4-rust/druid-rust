"""Bundle exact Struct sources, positive/negative results and fixed Java provenance."""
import datetime
import difflib
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import zipfile
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
root=Path('E:/workspaces/workspace-github-easy-4-rust/druid-rust');mirror=root/'.superpowers/sdd/2026-08-24-rdbc-single-standard-unification'
rdbc=base/'task7-struct-rdbc-candidate1';druid=base/'task7-struct-druid-candidate1';negative=base/'task7-struct-negative1'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
rdbc_state=json.loads((rdbc/'summary.json').read_text(encoding='utf-8'));druid_state=json.loads((druid/'summary.json').read_text(encoding='utf-8'));negative_state=json.loads((negative/'summary.json').read_text(encoding='utf-8'))
assert all(s['terminal'] for s in (rdbc_state,druid_state,negative_state))
assert rdbc_state['commands'][-1]['exit_code']==0 and druid_state['commands'][-1]['exit_code']==0
state={'observed':datetime.datetime.now(datetime.timezone.utc).isoformat(),'production_applied':False,'formal_cargo_certified':False,'mirrors':{},'source_manifest':{},'patches':[],'results':{}}
for folder,key in ((rdbc,'sources'),(druid,'sources')):
    expected=(rdbc_state if folder==rdbc else druid_state)[key]
    actual={p.relative_to(folder).as_posix():sha(p) for p in (folder/'src').rglob('*.rs')}
    assert actual==expected
assert druid_state['rdbc_sources']==rdbc_state['sources']
for folder,label in ((rdbc,'green-runtime'),(druid,'before-runtime'),(druid,'after-runtime')):
    result=re.search(r'test result: .*? (\d+) passed; (\d+) failed; (\d+) ignored;',(folder/(label+'.log')).read_text(encoding='utf-8'));assert result
    state['results'][label]={'passed':int(result[1]),'failed':int(result[2]),'ignored':int(result[3])}
assert state['results']=={'green-runtime':{'passed':4,'failed':0,'ignored':0},'before-runtime':{'passed':43,'failed':4,'ignored':0},'after-runtime':{'passed':47,'failed':0,'ignored':0}}
snapshot=json.loads((base/'task6-current-full-validation1.inputs.json').read_text(encoding='utf-8'))
state['changed_inputs']=[p for p,digest in snapshot['inputs'].items() if not (root/p).is_file() or sha(root/p)!=digest]
assert not state['changed_inputs'];state['frozen_input_count']=len(snapshot['inputs'])
draft=mirror/'drafts/task7-pooled-struct-factory1'
def new_file(path,payload):
    path.parent.mkdir(parents=True,exist_ok=True)
    with path.open('xb') as out:out.write(payload)
def copy(source,dest):
    new_file(dest,source.read_bytes());assert dest.read_bytes()==source.read_bytes()
    state['mirrors'][dest.relative_to(root).as_posix()]=sha(source)
patch=[]
for current,previous,label in ((rdbc,base/'task7-rdbc-source-candidate1','druid-rdbc'),(druid,base/'task7-array-source-candidate1','druid')):
    for path in sorted((current/'src').rglob('*.rs')):
        rel=path.relative_to(current);old=previous/rel
        if not old.exists() or old.read_bytes()!=path.read_bytes():
            copy(path,draft/'crates'/label/rel);state['patches'].append('crates/'+label+'/'+rel.as_posix())
            patch+=list(difflib.unified_diff(old.read_text(encoding='utf-8').splitlines(keepends=True) if old.exists() else [],path.read_text(encoding='utf-8').splitlines(keepends=True),fromfile='a/crates/'+label+'/'+rel.as_posix(),tofile='b/crates/'+label+'/'+rel.as_posix()))
assert len(state['patches'])==5,state['patches']
new_file(draft/'struct.patch',''.join(patch).encode('utf-8'))
provenance=base/'task7-struct-java-provenance1';provenance.mkdir()
state['java_sources']=rdbc_state['java_sources']
files=[]
for row in rdbc_state['java_sources'].values():
    path=Path(row['local_path']);assert sha(path)==row['sha256'];files.append((path,'java/'+path.name))
commit='33824c3dec1612711f9bb4e409319bcab2e4cd0e';path='core/src/main/java/com/alibaba/druid/pool/DruidPooledConnection.java'
payload=subprocess.check_output(['git','show',commit+':'+path],cwd='E:/workspaces/workspace-github/druid')
java=provenance/'DruidPooledConnection.java';new_file(java,payload)
method=re.search(rb'public Struct createStruct\(String typeName, Object\[\] attributes\) throws SQLException \{.*?\n    \}',payload,re.S);assert method and b'checkState();' in method[0] and b'return conn.createStruct(typeName, attributes);' in method[0]
state['java_druid']={'commit':commit,'path':path,'sha256':sha(java),'method_sha256':hashlib.sha256(method[0]).hexdigest()};files.append((java,'java/DruidPooledConnection.java'))
for folder in (rdbc,druid,negative):
    for p in sorted(folder.iterdir()):
        if p.is_file() and p.suffix in ('.log','.json','.rs'):files.append((p,folder.name+'/'+p.name))
for name in ('task7-struct-contract1.rs','task7-struct-rdbc1.py','task7-struct-negative1.py','task7-struct-pooled-consumer1-build.py','task7-struct-pooled-consumer1-build.summary.json','task7-pooled-struct-consumer1.rs','task7-struct-pooled1.py','task7-struct-record1.py'):
    files.append((base/name,name))
for source,rel in files:copy(source,draft/'acceptance'/rel)
state['artifact_sha256']={str(p):sha(p) for p in (rdbc/'libdruid_rdbc.rlib',rdbc/'green.exe',druid/'libdruid.rlib',druid/'before.exe',druid/'after.exe')}
archive=base/'task7-full-druid-rdbc-source-candidate9.zip'
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
state['archive']={'path':str(archive),'sha256':sha(archive),'source_count':len(state['source_manifest'])}
assert len(state['source_manifest'])==557
copy(archive,mirror/('task6-e-space-'+archive.name))
summary=base/'task7-pooled-struct-factory1-validation.summary.json';new_file(summary,(json.dumps(state,ensure_ascii=False,indent=2)+'\n').encode('utf-8'))
new_file(mirror/('task6-e-space-'+summary.name),summary.read_bytes())
body=f'''

## Struct 标准对象及池化/外部租约创建链路候选（2026-10-08）

固定OpenJDK8u来源Struct.java/Connection.java的SHA逐项核对。Struct仅三个JDBC方法、不继承Wrapper、没有free，默认随应用引用有效；显式Map重载绝不使用连接关联Map。固定Druid1.2.28提交{commit}的createStruct为checkState后直接委托，Git当前HEAD不同，因此另保存固定提交完整原件/方法SHA，不以当前检出替代基线。

新增独立resource/struct_value.rs标准Struct trait（Debug+Send+'static、exclusive &mut，不要求Sync），完整保留中文语义文档、属性声明顺序、SQL NULL和RdbcObject。Connection新增nullable类型名/属性数组owned工厂，默认明确不支持。Druid pooled工厂检查当前租约后委托并使用当前标准创建错误分类；返回对象无新增owner/registry/free状态机。ExternalConnectionLease原样委托，不嵌套原生池。已有druid::sql::Struct值快照没有冒充此trait，旧值类型退役/重导出仍待后续唯一树验收。

实际RED/GREEN：旧完整RDBC对同一外部消费者编译1/E0432与E0407，新增后完整127源严格-Dwarnings/forbidunsafe rlib0、同源四合同4/0。验证三方法、属性顺序/NULL/显式Map、nullable数组和类型名错误、typed driver错误、默认不支持、owned对象在连接关闭/销毁后仍可读取。含Cell的DriverStruct负向Sync实际1/E0277；擅加free负向实际1/E0407，原日志保留。

完整Druid基线与新候选都使用同一新RDBC重新严格编译0。原42测试体逐字不变，新增五项同源消费者：未接入Druid工厂的旧完整库43PASS4FAIL/exit101，接入后47PASS0FAIL/exit0；包含原全部Array/LOB/XML合同。新增池化类型名/顺序/映射、关闭后对象有效、NULL参数与原错误、关闭工厂零driver调用、创建fatal sort/discard一次及真实ExternalConnectionLease委托。此为当前标准创建错误策略的合同，不单独宣称Java所有异常分支/Filter回调差分已认证。

精确三RDBC/二Druid源差异在drafts/task7-pooled-struct-factory1，完整源码zip9含127RDBC/409Druid/21资源共557项，逐项解压核验SHA；zip SHA-256为{state['archive']['sha256']}。Java原件、实际命令/环境/log/消费者/负向源逐字节镜像，validation summary绑定实际库/exe与源码SHA。1167已枚举正式输入变化0，未新增生产源。

本轮未生产apply、正式Cargo、真实数据库/native worker/外部实池或Java运行时差分认证。Struct嵌套资源返回、SQLData/类型映射完整矩阵、旧Struct值快照唯一归属、Ref代理/返回链路、旧Access退役及真实async流式读取仍开放。原完整C1/C2按原进程继续，Task6 FIX_REQUIRED / Task7八义务 / Task8 INCOMPLETE / 18顶层OPEN。
'''
def append(path,addition):
    old=path.read_bytes();new_file(base/(path.name+'.before-struct1'),old)
    temp=path.with_name(path.name+'.struct1.tmp');new_file(temp,old+addition.encode('utf-8'));assert temp.read_bytes()==old+addition.encode('utf-8');os.replace(temp,path)
append(root/'docs/verification/rdbc-first/task7-standard-resource-candidates-2026-10-08.md',body)
line='\n\n2026-10-08 Struct标准与工厂候选：固定Java来源核对，三JDBC方法/无free/引用生命周期，不套用LOB监管。完整127RDBC严格rlib0，旧缺trait/factory E0432/E0407 RED到同源4/0；Cell Sync负向E0277/free负向E0407。两409源Druid库同新RDBC重建0，原42测试体保留，五新增合同旧43/4 RED到新47/0；池化/实际ExternalLease创建、nullable输入、错误、关闭后Struct可读通过。五源补丁/完整zip9/Java固定提交原件与命令日志SHA归档，1167冻结输入变化0。未生产apply/Cargo/DB差分认证，Ref/getter/旧Access/streaming仍开放；Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18 OPEN。详见task7-standard-resource-candidates-2026-10-08.md。\n'
append(root/'docs/verification/rdbc-first/status.md',line);append(mirror/'progress.md',line)
print(json.dumps({'summary':str(summary),'results':state['results'],'patches':state['patches'],'archive_sha256':state['archive']['sha256'],'mirrors':len(files),'changed_inputs':state['changed_inputs']},ensure_ascii=False),flush=True)
