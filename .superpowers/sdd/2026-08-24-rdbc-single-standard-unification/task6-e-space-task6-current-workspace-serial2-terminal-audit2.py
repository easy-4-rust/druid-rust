"""Record full serial test terminal with exact failures, retaining frozen runner evidence."""
import hashlib,json,re,shutil
from pathlib import Path
b=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
root=Path('E:/workspaces/workspace-github-easy-4-rust/druid-rust')
mirror=root/'.superpowers/sdd/2026-08-24-rdbc-single-standard-unification'
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
summary=b/'task6-current-workspace-serial2.summary.json';s=json.loads(summary.read_text())
assert s['terminal'] and s['exit_code']==101 and not s['changed_inputs']
log=Path(s['log']);assert sha(log)==s['log_sha256'];text=log.read_text(encoding='utf-8')
blocks=re.findall(r'test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;',text)
counts={'blocks':len(blocks),'passed':sum(int(x[0])for x in blocks),'failed':sum(int(x[1])for x in blocks),'ignored':sum(int(x[2])for x in blocks)}
assert counts==s['results']=={'blocks':262,'passed':3913,'failed':9,'ignored':0}
failures=re.findall(r'^test (.+) \.\.\. FAILED$',text,re.M);assert len(failures)==9
assert text.rstrip().endswith('`-p druid-admin --test grpc_network_e2e_test`')
evidence={'terminal':True,'scope':s['command'],'exit_code':101,'results':counts,'failed_target':'druid-admin/grpc_network_e2e_test','failed_tests':failures,'frozen_input_count':1167,'changed_inputs':[],'log_sha256':sha(log),'runner_summary_sha256':sha(summary),'ended':s['ended'],'root_cause_certified':False}
out=b/'task6-current-workspace-serial2-terminal-audit2.summary.json'
with out.open('x',encoding='utf-8')as f:f.write(json.dumps(evidence,indent=2,ensure_ascii=False)+'\n')
for p in[out,Path(__file__),b/'task6-current-workspace-serial2-terminal-record1.py']:
    target=mirror/('task6-e-space-'+p.name);assert not target.exists();shutil.copyfile(p,target);assert sha(p)==sha(target)
marker='2026-10-08 串行全仓正式终态：3913PASS/9FAIL'
line='\n\n'+marker+'，0ignored/262结果块，实际Cargo101；唯一失败目标Admin grpc_network_e2e_test为37/9。原1167冻结输入变化0，日志SHA '+sha(log)+'，全部后续目标与doctest结束。旧内存构建失败与旧3876/44保留历史，不扩大到当前输入；本轮池寿命失败未重现，网络根因尚未认证。生产冻结随原作业终态结束；Task6 FIX_REQUIRED/Task7八义务/Task8 INCOMPLETE/18顶层OPEN。机器证据task6-e-space-task6-current-workspace-serial2-terminal-audit2.summary.json。\n'
for p in[root/'docs/verification/rdbc-first/task6-current-workspace-serial-retry-2026-10-08.md',root/'docs/verification/rdbc-first/status.md',mirror/'progress.md']:
    assert marker not in p.read_text(encoding='utf-8')
    with p.open('ab')as f:f.write(line.encode())
print(json.dumps(evidence,ensure_ascii=False),flush=True)
