import hashlib
import json
import os
from pathlib import Path
import subprocess
from datetime import datetime, timezone

b = Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
d = b / 'task7-standard-cargo-candidate1'
w = d / 'workspace'
old = json.loads((d / 'summary.json').read_text(encoding='utf-8'))
assert old['terminal'] and old['exit_code'] != 0 and not old['changed_inputs']
frozen = json.loads((d / 'frozen-inputs.json').read_text(encoding='utf-8'))
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
assert all(sha(w / name) == digest for name, digest in frozen.items())
toolchain = 'C:/Users/hnxyh/.rustup/toolchains/1.97.1-x86_64-pc-windows-msvc/bin'
env = os.environ.copy()
env.update(CARGO_TARGET_DIR='F:/codex_build_cache/druid_rust/task7_standard_cargo13_1971',
           CARGO_BUILD_JOBS='1', RUST_TEST_THREADS='1', TMP='F:/codex_tmp/task7_standard_cargo13',
           TEMP='F:/codex_tmp/task7_standard_cargo13', RUSTC=toolchain+'/rustc.exe',
           RUSTDOC=toolchain+'/rustdoc.exe', RUSTUP_TOOLCHAIN='1.97.1-x86_64-pc-windows-msvc',
           RUSTUP_AUTO_INSTALL='0')
Path(env['TMP']).mkdir(parents=True, exist_ok=True)
version = subprocess.run([env['RUSTC'], '--version'], capture_output=True, text=True, check=True).stdout.strip()
assert version.startswith('rustc 1.97.1 '), version
cmd = old['command']
s = dict(terminal=False, production_applied=False, command=cmd, rustc=version,
         environment={k:env[k] for k in ['RUSTC','RUSTDOC','RUSTUP_TOOLCHAIN','RUSTUP_AUTO_INSTALL','CARGO_TARGET_DIR']},
         started=datetime.now(timezone.utc).isoformat(), frozen_input_count=len(frozen),
         prior_setup_failure='cargo.log/summary.json: rustc proxy resolved stable; terminated; not a contract RED')
out = d / 'retry13.summary.json'
assert not out.exists()
def save():
    out.write_text(json.dumps(s, indent=2)+'\n', encoding='utf-8')
save()
log = d / 'cargo-retry13.log'
with log.open('xb') as f:
    r = subprocess.run(cmd, cwd=w, env=env, stdout=f, stderr=subprocess.STDOUT,
                       creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
changed = [name for name,digest in frozen.items() if not (w/name).is_file() or sha(w/name) != digest]
s.update(terminal=True, exit_code=r.returncode, changed_inputs=changed, log_sha256=sha(log),
         ended=datetime.now(timezone.utc).isoformat())
save()
assert not changed
assert r.returncode == 0, log.read_text(encoding='utf-8')[-5000:]
assert '31 passed; 0 failed;' in log.read_text(encoding='utf-8')
s.update(cargo_certified=True, runtime_tests=31)
save()
print('Cargo contract31 PASS; fixed Rust1.97.1; frozen inputs unchanged', flush=True)
