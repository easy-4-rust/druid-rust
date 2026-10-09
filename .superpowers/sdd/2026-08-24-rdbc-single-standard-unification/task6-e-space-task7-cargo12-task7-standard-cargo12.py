import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
from datetime import datetime, timezone

b = Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
original = b / 'task7-external-resource-consumer-candidate1'
frozen = json.loads((original / 'integrated6-frozen-inputs9.json').read_text(encoding='utf-8'))
d = b / 'task7-standard-cargo-candidate1'
w = d / 'workspace'
w.mkdir(parents=True, exist_ok=False)
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
for name, digest in frozen.items():
    source = original / 'workspace' / name
    assert sha(source) == digest, name
    target = w / name
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, target)
    assert sha(target) == digest
contracts = b / 'task7-standard-array-xml-candidate2'
c = json.loads((contracts / 'summary.json').read_text(encoding='utf-8'))
assert c['terminal'] and c['runtime_tests'] == 31
for name, digest in c['sources'].items():
    source = contracts / 'crates/druid-rdbc/tests' / name
    assert sha(source) == digest
    target = w / 'crates/druid-rdbc/tests' / name
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, target)
inputs = {p.relative_to(w).as_posix(): sha(p) for p in w.rglob('*') if p.is_file()}
(d / 'frozen-inputs.json').write_text(json.dumps(inputs, indent=2) + '\n', encoding='utf-8')
cmd = ['C:/Users/hnxyh/.rustup/toolchains/1.97.1-x86_64-pc-windows-msvc/bin/cargo.exe',
       'test', '-p', 'druid-rdbc', '--test', 'resource_standard_contract_test',
       '--all-features', '--locked', '--offline', '-j1', '--', '--test-threads=1']
env = os.environ.copy()
env.update(CARGO_TARGET_DIR='F:/codex_build_cache/druid_rust/task7_standard_cargo12_1971',
           CARGO_BUILD_JOBS='1', RUST_TEST_THREADS='1', TMP='F:/codex_tmp/task7_standard_cargo12',
           TEMP='F:/codex_tmp/task7_standard_cargo12')
Path(env['TMP']).mkdir(parents=True, exist_ok=True)
s = dict(terminal=False, production_applied=False, started=datetime.now(timezone.utc).isoformat(),
         command=cmd, workspace=str(w), frozen_input_count=len(inputs),
         source_snapshot='integrated6 frozen1147 plus independent contract31; original workspace untouched',
         target_dir=env['CARGO_TARGET_DIR'])
def save():
    (d / 'summary.json').write_text(json.dumps(s, indent=2) + '\n', encoding='utf-8')
save()
log = d / 'cargo.log'
with log.open('xb') as out:
    r = subprocess.run(cmd, cwd=w, env=env, stdout=out, stderr=subprocess.STDOUT,
                       creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
changed = [name for name, digest in inputs.items()
           if not (w / name).is_file() or sha(w / name) != digest]
s.update(terminal=True, exit_code=r.returncode, ended=datetime.now(timezone.utc).isoformat(),
         changed_inputs=changed, log_sha256=sha(log))
save()
assert not changed, changed
assert r.returncode == 0, log.read_text(encoding='utf-8')[-5000:]
assert '31 passed; 0 failed;' in log.read_text(encoding='utf-8')
s.update(cargo_certified=True, runtime_tests=31)
save()
print('formal RDBC contract target: 31 PASS / 0 FAIL; frozen inputs unchanged', flush=True)
