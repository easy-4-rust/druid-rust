"""Rebuild immutable Task7 candidate using explicit Cargo-emitted 1.97 artifacts."""
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess

base = Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
target = Path('F:/codex_build_cache/druid_rust/task6_lifetime_clean_1971_20261008/debug/deps')
out = base / 'task7-array-baseline-coherent1'
out.mkdir(exist_ok=False)
artifact_log = base / 'task6-lifetime-current-artifact3.jsonl'
sha = lambda p: hashlib.sha256(Path(p).read_bytes()).hexdigest()
artifacts = {}
for line in artifact_log.read_text(encoding='utf-8').splitlines():
    try:
        row = json.loads(line)
    except ValueError:
        continue
    if row.get('reason') == 'compiler-artifact':
        files = [p for p in row['filenames'] if p.endswith(('.rlib', '.dll'))]
        if files:
            artifacts[row['target']['name']] = files[0]
state = {'artifact_log': str(artifact_log), 'artifact_log_sha256': sha(artifact_log),
         'production_applied': False, 'commands': [], 'sources': {}}
for name in ('task7-rdbc-source-candidate1', 'task7-full-druid-source-candidate1'):
    directory = base / name
    state['sources'][name] = {p.relative_to(directory).as_posix(): sha(p)
        for p in sorted((directory / 'src').rglob('*.rs'))}

def save():
    (out / 'summary.json').write_text(json.dumps(state, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')

def execute(name, command, env=None):
    log = out / (name + '.log')
    record = {'name': name, 'command': command, 'started': datetime.datetime.now(datetime.timezone.utc).isoformat(),
              'environment': env or {}, 'log': str(log)}
    state['commands'].append(record)
    save()
    runenv = os.environ.copy()
    runenv.update(env or {})
    with log.open('xb') as output:
        result = subprocess.run(command, env=runenv, stdout=output, stderr=subprocess.STDOUT,
                                creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    record.update(exit_code=result.returncode, log_sha256=sha(log))
    save()
    print(json.dumps({'name': name, 'exit_code': result.returncode, 'log': str(log)}), flush=True)
    if result.returncode:
        raise SystemExit(result.returncode)

def replace_dependencies(command):
    for i, value in enumerate(command):
        if value == '--extern':
            name = command[i+1].split('=', 1)[0]
            path = artifacts[name]
            command[i+1] = name + '=' + path
            state.setdefault('dependencies', {})[name] = {'path': path, 'sha256': sha(path)}
        if value == '-L' and command[i+1].startswith('dependency='):
            command[i+1] = 'dependency=' + str(target)
    return command

old = json.loads((base / 'task7-full-druid-source-compiler1.summary.json').read_text(encoding='utf-8'))
command = old['results']['rdbc-candidate1']['command'].copy()
command[command.index('--out-dir')+1] = str(out)
command[command.index('-C')+1] = 'metadata=task7_array_coherent_rdbc1'
execute('rdbc', replace_dependencies(command))
artifacts['druid_rdbc'] = str(out / 'libdruid_rdbc.rlib')
old = json.loads((base / 'task7-pooled-xml-green1.compile.summary.json').read_text(encoding='utf-8'))
command = old['command'].copy()
command[command.index('-o')+1] = str(out / 'libdruid.rlib')
for i, value in enumerate(command):
    if value == '-C' and command[i+1].startswith('metadata='):
        command[i+1] = 'metadata=task7_array_coherent_druid1'
command.extend(['-L', 'dependency=' + str(out)])
execute('druid', replace_dependencies(command), old['compile_environment'])
rustc = command[0]
consumer = base / 'task7-pooled-xml-factory-contract2.rs'
test_command = [rustc, '--edition=2021', '--test', '-D', 'warnings', str(consumer),
    '-L', 'dependency=' + str(target), '-L', 'dependency=' + str(out),
    '--extern', 'druid=' + str(out / 'libdruid.rlib'),
    '--extern', 'druid_rdbc=' + str(out / 'libdruid_rdbc.rlib'),
    '--extern', 'async_trait=' + artifacts['async_trait'],
    '--extern', 'tokio=' + artifacts['tokio'],
    '-o', str(out / 'xml-regression.exe')]
state['consumer_sha256'] = sha(consumer)
execute('xml-regression-compile', test_command)
execute('xml-regression-runtime', [str(out / 'xml-regression.exe'), '--test-threads=1'])
state['candidate_source_changes'] = [name + '/' + p
    for name, files in state['sources'].items() for p, digest in files.items()
    if sha(base / name / p) != digest]
state['terminal'] = True
save()
