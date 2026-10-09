from pathlib import Path
import hashlib
import json
import subprocess

root = Path.cwd().resolve()
paths = subprocess.check_output(['git', 'ls-files', '--cached', '--others', '--exclude-standard'], cwd=root, text=True).splitlines()
inputs = {}
for path in sorted(set(paths)):
    if path in {'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml'} or (
        path.startswith(('crates/', 'vendor/')) and (path.endswith('.rs') or path.endswith('/Cargo.toml'))):
        file = root / path
        if file.is_file():
            inputs[path] = hashlib.sha256(file.read_bytes()).hexdigest()
output = {'scope': 'all tracked and nonignored untracked crates/vendor Rust and Cargo inputs plus root Cargo/toolchain; docs/scripts excluded',
          'session_id': 11062, 'inputs': inputs}
target = root / '.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-rust-1971-workspace-inputs-2026-10-07.json'
assert not target.exists(), 'Preserve original run input freeze'
target.write_bytes((json.dumps(output, ensure_ascii=False, indent=2) + '\n').encode('utf-8'))
print('Frozen workspace inputs:', len(inputs))
