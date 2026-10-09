import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
from datetime import datetime, timezone

b=Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
d=b/'task7-external-resource-consumer-candidate1'
w=d/'workspace'
e=Path('E:/workspaces/workspace-github-easy-4-rust/druid-rust')
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
old=json.loads((d/'integrated6.summary.json').read_text(encoding='utf-8'))
assert [r['exit_code'] for r in old['commands']]==[0,0,101]
frozen=json.loads((d/'integrated6-frozen-inputs9.json').read_text(encoding='utf-8'))
assert all(sha(w/p)==digest for p,digest in frozen.items())
red=(d/'integrated6-rdbc-complete.log').read_text(encoding='utf-8')
assert red.count('error[E0053]')==3 and 'create_blob' in red and 'create_clob' in red and 'create_n_clob' in red
terminal=dict(terminal=True,original_session=60792,exit_code=1,failed_gate='rdbc-complete',
    commands=old['commands'],frozen_input_count=len(frozen),changed_inputs=[],
    ruling='Actual original handle exited1 after nested Cargo101/E0053. Runner left terminal=false after assertion; this audited receipt supersedes its liveness, not its original logs. Wrapper/Druid full gates were not run.')
terminal_path=b/'task7-integrated6-terminal17.summary.json'
assert not terminal_path.exists()
terminal_path.write_text(json.dumps(terminal,indent=2)+'\n',encoding='utf-8')
ledger=e/'.superpowers/sdd/2026-08-24-rdbc-single-standard-unification'
for p in [terminal_path,d/'integrated6-rdbc-complete.log']:
    target=ledger/('task6-e-space-task7-terminal17-'+p.name)
    assert not target.exists()
    shutil.copyfile(p,target)
toolchain='C:/Users/hnxyh/.rustup/toolchains/1.97.1-x86_64-pc-windows-msvc/bin'
os.environ.update(PATH=toolchain+os.pathsep+os.environ['PATH'],
    RUSTUP_TOOLCHAIN='1.97.1-x86_64-pc-windows-msvc',RUSTUP_AUTO_INSTALL='0')
generator=e/'scripts/render_task6_method_default_evidence.py'
source=generator.read_text(encoding='utf-8')
needle='use druid_rdbc::types::{'
assert source.count(needle)==1
source=source.replace(needle,'use druid_rdbc::resource::{Blob, Clob, NClob};\n'+needle)
candidate=b/'task7-render-method-evidence17.py'
assert not candidate.exists()
candidate.write_text(source,encoding='utf-8',newline='\n')
sys.path.insert(0,str(e/'scripts'))
spec=importlib.util.spec_from_file_location('candidate_renderer17',candidate)
module=importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)
rows=module._load_rows(e,module.verifier.DEFAULT_MATRIX)
files=module.render_files(w,rows)
changes={}
allowed={module.GENERATED_DIRECTORY/'common.inc.rs',module.GENERATED_DIRECTORY/'connection.inc.rs'}
def normalize(t):
    t=t.replace('use druid_rdbc::resource::{Blob, Clob, NClob};','')
    for name in ['Blob','Clob','NClob']:
        t=t.replace('Result<Rdbc'+name+', RdbcError>','Result<Box<dyn '+name+'>, RdbcError>')
    return re.sub(r'\s+','',t)
for rel,payload in files.items():
    target=w/rel
    before=target.read_bytes()
    if before==payload:
        continue
    assert rel in allowed, rel
    assert normalize(before.decode('utf-8'))==normalize(payload.decode('utf-8')),rel
    changes[rel.as_posix()]=dict(before_sha256=sha(target),after_sha256=hashlib.sha256(payload).hexdigest())
    target.write_bytes(payload)
assert len(changes)==2,changes
contracts=b/'task7-standard-array-xml-candidate2'
c=json.loads((contracts/'summary.json').read_text(encoding='utf-8'))
assert c['terminal'] and c['runtime_tests']==31
for name,digest in c['sources'].items():
    source=contracts/'crates/druid-rdbc/tests'/name
    assert sha(source)==digest
    target=w/'crates/druid-rdbc/tests'/name
    target.parent.mkdir(parents=True,exist_ok=True)
    shutil.copyfile(source,target)
inputs={p.relative_to(w).as_posix():sha(p) for p in w.rglob('*') if p.is_file()}
(d/'integrated17-frozen-inputs.json').write_text(json.dumps(inputs,indent=2)+'\n',encoding='utf-8')
env=os.environ.copy()
env.update(CARGO_TARGET_DIR='F:/codex_build_cache/druid_rust/task6_lifetime_clean_1971_20261008',
    CARGO_BUILD_JOBS='1',RUST_TEST_THREADS='1',RUSTC=toolchain+'/rustc.exe',RUSTDOC=toolchain+'/rustdoc.exe',
    TMP='F:/codex_tmp/task7_integrated17',TEMP='F:/codex_tmp/task7_integrated17')
Path(env['TMP']).mkdir(parents=True,exist_ok=True)
s=dict(terminal=False,production_applied=False,commands=[],generated_changes=changes,
    frozen_input_count=len(inputs),generator_sha256=sha(candidate),original_generator_sha256=sha(generator),
    started=datetime.now(timezone.utc).isoformat(),
    ruling='Regenerate from current standard method signatures; only three return types and required resource imports change. All generated behavior bodies and selectors preserved. Add already-Cargo-verified independent31; retain prior Druidlib153/owner104 scoped evidence. Resume at actual failed gate; warnings remain open.')
def save():
    (d/'integrated17.summary.json').write_text(json.dumps(s,indent=2)+'\n',encoding='utf-8')
save()
for name,package in [('rdbc-complete','druid-rdbc'),('wrapper-complete','druid-wrapper'),('druid-complete','druid')]:
    cmd=[toolchain+'/cargo.exe','test','-p',package,'--all-features','--locked','--offline','--no-fail-fast','-j1']
    log=d/('integrated17-'+name+'.log')
    with log.open('xb') as out:
        r=subprocess.run(cmd,cwd=w,env=env,stdout=out,stderr=subprocess.STDOUT,
            creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    changed=[name for name,digest in inputs.items() if not (w/name).is_file() or sha(w/name)!=digest]
    s['commands'].append(dict(name=name,command=cmd,exit_code=r.returncode,log=str(log),sha256=sha(log),changed_inputs=changed))
    save()
    print(name,r.returncode,flush=True)
    if r.returncode or changed:
        s.update(terminal=True,exit_code=r.returncode or 1,ended=datetime.now(timezone.utc).isoformat())
        save()
        raise SystemExit(s['exit_code'])
s.update(terminal=True,exit_code=0,ended=datetime.now(timezone.utc).isoformat())
save()
