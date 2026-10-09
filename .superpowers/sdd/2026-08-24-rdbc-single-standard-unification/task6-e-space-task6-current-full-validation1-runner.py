"""Run unchanged formal gates against a frozen checkout; retain actual exits."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path('E:/workspaces/workspace-github-easy-4-rust/druid-rust')
BASE = Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
PREFIX = 'task6-current-full-validation1'
TOOL = Path('C:/Users/hnxyh/.rustup/toolchains/1.97.1-x86_64-pc-windows-msvc/bin')
TARGET = 'F:/codex_build_cache/druid_rust/task6_lifetime_clean_1971_20261008'

def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def write_json(path, data):
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')

prior = json.loads((BASE / 'task6-lifetime-formal-inputs1.json').read_text(encoding='utf-8'))
old_changed = [name for group in ('inputs', 'control_inputs') for name, digest in prior[group].items()
               if not (ROOT / name).is_file() or sha(ROOT / name) != digest]
if old_changed:
    raise SystemExit('Prior input snapshot drift: ' + repr(old_changed))
tracked = set(prior['inputs']) | set(prior['control_inputs'])
tracked.update(p.relative_to(ROOT).as_posix() for p in (ROOT / 'scripts').rglob('*.py'))
tracked.update(p.relative_to(ROOT).as_posix() for p in (ROOT / 'docs/verification/rdbc-first').glob('*.tsv'))
snapshot = {name: sha(ROOT / name) for name in sorted(tracked)}
write_json(BASE / (PREFIX + '.inputs.json'), {'started': now(), 'inputs': snapshot,
    'prior_source_count': len(prior['inputs']), 'prior_control_count': len(prior['control_inputs'])})

env = os.environ.copy()
env['CARGO_TARGET_DIR'] = TARGET
env['RUSTC'] = str(TOOL / 'rustc.exe')
env['RUSTDOC'] = str(TOOL / 'rustdoc.exe')
env['PATH'] = str(TOOL) + os.pathsep + env['PATH']
env['RUST_TEST_THREADS'] = '1'
env['PYTHONUTF8'] = '1'
steps = [
    ('c1-source', [sys.executable, '-X', 'utf8', '-u', 'scripts/verify_task6_method_parity.py', '--no-execute']),
    ('wrapper-lib', [str(TOOL / 'cargo.exe'), 'test', '-p', 'druid-wrapper', '--lib', '--all-features', '--locked', '--offline', '--', '--show-output']),
    ('c1-runtime', [sys.executable, '-X', 'utf8', '-u', 'scripts/verify_task6_method_parity.py']),
    ('workspace', [str(TOOL / 'cargo.exe'), 'test', '--workspace', '--all-features', '--locked', '--offline', '--no-fail-fast']),
]
state = {'started': now(), 'runner_pid': os.getpid(), 'root': str(ROOT), 'environment': {
    k: env[k] for k in ('CARGO_TARGET_DIR', 'RUSTC', 'RUSTDOC', 'RUST_TEST_THREADS', 'PYTHONUTF8')},
    'steps': [], 'terminal': False}
state_path = BASE / (PREFIX + '.summary.json')
for name, command in steps:
    drift = [p for p, digest in snapshot.items() if not (ROOT / p).is_file() or sha(ROOT / p) != digest]
    if drift:
        state.update(terminal=True, stopped='INPUT_DRIFT', changed_inputs=drift, ended=now())
        write_json(state_path, state)
        raise SystemExit(2)
    log = BASE / (PREFIX + '.' + name + '.log')
    record = {'name': name, 'command': command, 'started': now(), 'log': str(log)}
    state['steps'].append(record)
    with log.open('xb') as output:
        process = subprocess.Popen(command, cwd=ROOT, env=env, stdout=output, stderr=subprocess.STDOUT)
        record['pid'] = process.pid
        write_json(state_path, state)
        print(json.dumps({'step': name, 'pid': process.pid, 'log': str(log)}), flush=True)
        code = process.wait()
    drift = [p for p, digest in snapshot.items() if not (ROOT / p).is_file() or sha(ROOT / p) != digest]
    record.update(exit_code=code, ended=now(), log_sha256=sha(log), changed_inputs=drift)
    write_json(state_path, state)
    print(json.dumps({'step': name, 'exit_code': code, 'changed_inputs': drift}), flush=True)
    if code or drift:
        state.update(terminal=True, stopped=name, ended=now())
        write_json(state_path, state)
        raise SystemExit(code if code else 2)
state.update(terminal=True, ended=now())
write_json(state_path, state)
