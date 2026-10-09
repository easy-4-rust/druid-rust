"""Retry the exact full workspace scope with one compiler job after recorded allocation failure."""
import datetime,hashlib,json,os,re,subprocess
from pathlib import Path
base=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008');root=Path('E:/workspaces/workspace-github-easy-4-rust/druid-rust');prefix=base/'task6-current-workspace-serial2';sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
prior=json.loads((base/'task6-current-full-validation1.summary.json').read_text(encoding='utf-8'));assert prior['terminal'];workspace=next(row for row in prior['steps'] if row['name']=='workspace');assert workspace['exit_code']==101
assert sha(Path(workspace['log']))==workspace['log_sha256'];log_text=Path(workspace['log']).read_text(encoding='utf-8',errors='replace');assert 'memory allocation of' in log_text and 'os error 1455' in log_text
snapshot=json.loads((base/'task6-current-full-validation1.inputs.json').read_text(encoding='utf-8'))
def changes():return [p for p,digest in snapshot['inputs'].items() if not(root/p).is_file() or sha(root/p)!=digest]
assert not changes()
state={'started':datetime.datetime.now(datetime.timezone.utc).isoformat(),'runner_pid':os.getpid(),'previous_exit':101,'previous_log_sha256':workspace['log_sha256'],'scope_change':'compiler jobs only: -j1','frozen_input_count':len(snapshot['inputs']),'command':workspace['command']+['-j1'],'terminal':False,'environment':prior['environment']}
env=os.environ.copy();env.update(prior['environment']);env['PATH']=str(Path(env['RUSTC']).parent)+os.pathsep+env.get('PATH','')
log=Path(str(prefix)+'.log');summary=Path(str(prefix)+'.summary.json')
assert not summary.exists()
with log.open('xb') as output:
    child=subprocess.Popen(state['command'],cwd=root,env=env,stdout=output,stderr=subprocess.STDOUT)
    state['pid']=child.pid;state['log']=str(log);summary.write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8');print('workspace serial started',child.pid,flush=True);exit_code=child.wait()
state['exit_code']=exit_code;state['ended']=datetime.datetime.now(datetime.timezone.utc).isoformat();state['log_sha256']=sha(log);state['changed_inputs']=changes();state['terminal']=True
text=log.read_text(encoding='utf-8',errors='replace');results=re.findall(r'test result: .*? (\d+) passed; (\d+) failed; (\d+) ignored;',text);state['results']={'blocks':len(results),'passed':sum(int(r[0]) for r in results),'failed':sum(int(r[1]) for r in results),'ignored':sum(int(r[2]) for r in results)}
summary.write_text(json.dumps(state,indent=2)+'\n',encoding='utf-8');print(json.dumps({'exit_code':exit_code,'results':state['results'],'changed_inputs':state['changed_inputs']}),flush=True)
