from pathlib import Path
import hashlib,json,os,re,subprocess,time
b=Path(__file__).parent
w=Path('F:/codex_build_cache/d177')
e=Path('E:/workspaces/workspace-github-easy-4-rust/druid-rust')
d=b/'task7-candidate-verification403'
d.mkdir(exist_ok=False)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
parent=json.loads((b/'task7-final-candidate-workspace319/summary.json').read_text())
assert parent['terminal'] and parent['exit_code']==1
paths=set(json.loads((b/'task7-final-candidate-workspace319/frozen-inputs.json').read_text()))
paths.add('scripts/windows_bounded_process.py')
frozen={p:sha(w/p) for p in sorted(paths)}
assert len(frozen)==1278
(d/'frozen-inputs.json').write_text(json.dumps(frozen,indent=2)+'\n',encoding='utf-8')
s=dict(terminal=False,pid=os.getpid(),production_applied=False,frozen_input_count=len(frozen),commands=[],original_c1_c2_timeouts_preserved=True,child_priority='normal inherited',parent319_terminal_sha256=sha(b/'task7-final-candidate-workspace319/summary.json'))
def save():(d/'summary.json').write_text(json.dumps(s,indent=2)+'\n',encoding='utf-8')
save()
tool=Path('C:/Users/hnxyh/.rustup/toolchains/1.97.1-x86_64-pc-windows-msvc/bin')
env=os.environ.copy()
env.update(RUSTC=str(tool/'rustc.exe'),RUSTDOC=str(tool/'rustdoc.exe'),RUSTUP_TOOLCHAIN='1.97.1-x86_64-pc-windows-msvc',RUSTUP_AUTO_INSTALL='0',CARGO_BUILD_JOBS='1',CARGO_TARGET_DIR='F:/codex_build_cache/druid_rust/task7_row_set177_target',DRUID_RDBC_EXTERNAL_TARGET_ROOT='F:/codex_build_cache/druid_rust/task7_row_set177_external',TMP='F:/codex_tmp/task7_standard_cargo13',TEMP='F:/codex_tmp/task7_standard_cargo13',GIT_DIR=str(e/'.git'),GIT_WORK_TREE=str(w),PYTHONUTF8='1',PYTHONDONTWRITEBYTECODE='1')
env['PATH']=str(tool)+os.pathsep+env['PATH']
for key in list(env):
 if key.startswith(('DRUID_DATABASE_','DRUID_CONTRACT_','DRUID_JDBC_','DRUID_REQUIRE_DATABASE_PROFILE_','DRUID_LIVE_DB_')):env.pop(key)
common=['--workspace','--all-features','--locked','--offline','-j1']
commands=[('wrapper-original-prebuild',[str(tool/'cargo.exe'),'test','--offline','--locked','-p','druid-wrapper','--all-features','--lib','--','--show-output'])]
commands += [('workspace-complete',[str(tool/'cargo.exe'),'test']+common+['--no-fail-fast']),('workspace-strict',[str(tool/'cargo.exe'),'clippy']+common+['--all-targets','--','-D','warnings']),('workspace-doc',[str(tool/'cargo.exe'),'doc']+common+['--no-deps']),('format-check',[str(tool/'cargo.exe'),'fmt','--all','--','--check']),('c1-complete-runtime',['C:/Espressif/tools/python/python.exe','-X','utf8','scripts/verify_task6_method_parity.py','--repository-root',str(w)])]
try:
 for name,command in commands:
  assert all(sha(w/p)==h for p,h in frozen.items())
  s['running_command']=name;save()
  log=d/(name+'.log');started=time.monotonic()
  with log.open('wb') as out:
   result=subprocess.run(command,cwd=w,env=env,stdout=out,stderr=subprocess.STDOUT)
  text=log.read_text(encoding='utf-8',errors='replace')
  blocks=re.findall(r'^test result: .*?(\d+) passed; (\d+) failed; (\d+) ignored;',text,re.M)
  changed=[p for p,h in frozen.items() if sha(w/p)!=h]
  record=dict(name=name,command=command,exit_code=result.returncode,elapsed_seconds=time.monotonic()-started,log=str(log),sha256=sha(log),passed=sum(int(x) for x,y,z in blocks),failed=sum(int(y) for x,y,z in blocks),ignored=sum(int(z) for x,y,z in blocks),changed_inputs=changed)
  s['commands'].append(record);save();print(json.dumps(record),flush=True)
  if changed:raise RuntimeError('candidate inputs changed')
 s.update(terminal=True,exit_code=0 if all(c['exit_code']==0 for c in s['commands']) else 1)
 s.pop('running_command',None);save();print('CANDIDATE403_TERMINAL',s['exit_code'],flush=True)
except Exception as error:
 s.update(terminal=True,exit_code=1,error=str(error));save();raise
