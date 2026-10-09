"""Record full C1/C2 success, first workspace build failure, and verified serial retry state."""
import ast,datetime,hashlib,json,os,subprocess,uuid
from pathlib import Path
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008');root=Path('E:/workspaces/workspace-github-easy-4-rust/druid-rust');mirror=root/'.superpowers/sdd/2026-08-24-rdbc-single-standard-unification';sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
prior=json.loads((base/'task6-current-full-validation1.summary.json').read_text(encoding='utf-8'));assert prior['terminal'];steps={row['name']:row for row in prior['steps']};assert steps['c1-runtime']['exit_code']==0 and steps['workspace']['exit_code']==101
snapshot=json.loads((base/'task6-current-full-validation1.inputs.json').read_text(encoding='utf-8'));changed=[p for p,digest in snapshot['inputs'].items() if not(root/p).is_file() or sha(root/p)!=digest];assert not changed
serial=json.loads((base/'task6-current-workspace-serial2.summary.json').read_text(encoding='utf-8'))
pid=int(serial['pid']);query=f'Get-CimInstance Win32_Process -Filter "ProcessId = {pid}" | Select-Object ProcessId,ParentProcessId,Name,CommandLine | ConvertTo-Json -Compress'
payload=subprocess.check_output(['powershell.exe','-NoProfile','-Command',query],text=True,encoding='utf-8').strip();process=json.loads(payload) if payload else None
source=(root/'scripts/verify_task6_method_parity.py').read_text(encoding='utf-8');tree=ast.parse(source);c2_function=next(node for node in tree.body if isinstance(node,ast.FunctionDef) and 'C2 authoritative verifier' in (ast.get_docstring(node) or ''))
proof=ast.get_source_segment(source,c2_function);assert 'base_tests=1283 .*executed=yes' in proof and 'result.returncode != 0' in proof
state={'observed':datetime.datetime.now(datetime.timezone.utc).isoformat(),'c1_runtime':steps['c1-runtime'],'c2_proof_guard':{'function':c2_function.name,'source_sha256':sha(root/'scripts/verify_task6_method_parity.py'),'function_sha256':hashlib.sha256(proof.encode()).hexdigest()},'workspace_first':steps['workspace'],'serial_retry':serial,'live_process':process,'changed_inputs':changed,'frozen_input_count':len(snapshot['inputs'])}
def replace(path,payload):
    path.parent.mkdir(parents=True,exist_ok=True)
    if path.exists():
        backup=base/(path.name+'.before-serial2-'+uuid.uuid4().hex)
        with backup.open('xb') as out:out.write(path.read_bytes())
    temp=path.with_name(path.name+'.serial2-'+uuid.uuid4().hex+'.tmp')
    with temp.open('xb') as out:out.write(payload)
    assert temp.read_bytes()==payload;os.replace(temp,path)
summary=base/'task6-current-workspace-serial2.observation.json';replace(summary,(json.dumps(state,indent=2,ensure_ascii=False)+'\n').encode())
for p in (summary,base/'task6-current-workspace-serial2-runner.py',base/'task6-current-workspace-serial2-record.py',base/'task6-current-workspace-serial2.summary.json'):
    target=mirror/('task6-e-space-'+p.name);replace(target,p.read_bytes());assert sha(target)==sha(p)
if serial['terminal']:
    p=Path(serial['log']);assert sha(p)==serial['log_sha256'];replace(mirror/('task6-e-space-'+p.name),p.read_bytes())
status=('终态exit '+str(serial['exit_code'])) if serial['terminal'] else ('已验证LIVE' if process else '未获得存活证明，须继续核实')
body=f'''# 当前完整C1/C2终态与全仓串行重跑（2026-10-08）

正式当前源码完整C1已exit0，479方法/442选择器、executed=yes。原日志SHA {steps['c1-runtime']['log_sha256']}，结束时间{steps['c1-runtime']['ended']}。冻结验证器先要求完整C2 exit0且输出base_tests=1283/executed=yes，再执行C1证据目标；其原函数与文件SHA已绑定observation.json。这里依据实际C1终态及已冻结的强制C2门禁确认通过，不伪造或声称保存了未单独输出的C2子stdout。

原全workspace/all-features/locked/offline/no-fail-fast实际Cargo101，日志SHA {steps['workspace']['log_sha256']}。日志包含多次memory allocation失败、mmap os error1455（页面文件太小）及后续缺失rlib诊断；没有test result块，属于构建失败，不能统计为业务测试失败或全仓通过。完整原日志保留，不修改页面文件、测试范围、断言或超时。

在相同Rust1.97.1、缓存、features、源码和RUST_TEST_THREADS=1下，新增完整重跑仅限制编译并发-j1。原会话79769、Cargo PID {pid}；本次观察{state['observed']}，状态{status}。进程事实与原命令保存在observation.json；无终态不能推定成功，不重新启动仍存活作业。

1167已枚举冻结输入变化0。原全仓失败与串行重跑分开记录；候选资源独立验证也不能替代该全仓门禁。Task6仍FIX_REQUIRED，Task7八义务/Task8 INCOMPLETE，18顶层OPEN。
'''
replace(root/'docs/verification/rdbc-first/task6-current-workspace-serial-retry-2026-10-08.md',body.encode())
marker='2026-10-08 当前完整C1/C2终态及全仓构建内存失败'
line='\n\n'+marker+'：正式C1 exit0/479方法442选择器/executed=yes，并经冻结强制门禁确认完整C2 1283身份executed=yes通过。随后全workspace/allfeatures/no-fail-fast Cargo101，日志内存分配失败/os1455及缺失rlib，0测试结果块，非业务测试终态。原日志SHA 9ca325f8...保留，1167输入变化0。完整范围仅-j1降低编译并发重跑，session79769，等待原进程终态；不改系统页面文件/断言/features。Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18 OPEN。详见task6-current-workspace-serial-retry-2026-10-08.md。\n'
for p in (root/'docs/verification/rdbc-first/status.md',mirror/'progress.md'):
    if marker not in p.read_text(encoding='utf-8'):replace(p,p.read_bytes()+line.encode())
print(json.dumps({'report':'task6-current-workspace-serial-retry-2026-10-08.md','serial_status':status,'changed_inputs':changed},ensure_ascii=False),flush=True)
