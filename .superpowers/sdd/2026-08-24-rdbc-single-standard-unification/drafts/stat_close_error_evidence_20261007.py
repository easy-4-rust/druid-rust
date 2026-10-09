from pathlib import Path
import hashlib, json, re, sys
root = Path.cwd()
evidence = root / '.superpowers/sdd/2026-08-24-rdbc-single-standard-unification'
stem = 'task6-complete-stat-close-error-' + sys.argv[2] + '-2026-10-07'
freeze_path = evidence / (stem.replace('-2026', '-inputs-2026') + '.json')
def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()
if sys.argv[1] == 'freeze':
    assert not freeze_path.exists()
    files = {p for folder in ['crates', 'vendor'] for p in (root/folder).rglob('*')
        if p.is_file() and (p.suffix in ['.rs', '.proto', '.sql'] or p.name == 'Cargo.toml')}
    files.update(root / n for n in ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml'])
    result = {'toolchain':'1.97.1', 'inputs':{p.relative_to(root).as_posix():sha(p) for p in sorted(files)}}
    freeze_path.write_text(json.dumps(result, indent=2) + '\n', encoding='utf-8')
    print('Frozen inputs', len(files))
else:
    freeze = json.loads(freeze_path.read_text(encoding='utf-8'))
    assert all(sha(root / n) == s for n,s in freeze['inputs'].items())
    log = evidence / (stem + '.log')
    content = log.read_text(encoding='utf-8-sig')
    counts = [tuple(map(int,m)) for m in re.findall(r'test result: .*? (\d+) passed; (\d+) failed; (\d+) ignored;', content)]
    assert counts
    exit_code, session_id = map(int, sys.argv[3:5])
    passed, failed, ignored = (sum(c[i] for c in counts) for i in range(3))
    result = {'state':'TERMINAL_PASSED' if exit_code == 0 and failed == 0 else 'TERMINAL_FAILED',
      'exit_code':exit_code, 'session_id':session_id, 'toolchain':'1.97.1', 'passed':passed,
      'failed':failed, 'ignored':ignored, 'result_blocks':len(counts), 'result_counts':counts,
      'scope':'druid complete lib' if sys.argv[2] == 'initial' else 'six complete druid consumer targets',
      'log':log.relative_to(root).as_posix(), 'log_sha256':sha(log), 'unchanged_frozen_inputs':len(freeze['inputs'])}
    dest = evidence / (stem + '.summary.json')
    assert not dest.exists()
    dest.write_text(json.dumps(result,indent=2) + '\n', encoding='utf-8')
    print(json.dumps(result))
