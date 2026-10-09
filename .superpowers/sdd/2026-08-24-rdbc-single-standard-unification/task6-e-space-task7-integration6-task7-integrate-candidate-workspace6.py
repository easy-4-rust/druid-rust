import datetime
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import zipfile

b = Path('F:/codex_build_cache/druid_rust/task6_e_disk_full_evidence_20261008')
repo = Path('E:/workspaces/workspace-github-easy-4-rust/druid-rust')
d = b / 'task7-external-resource-consumer-candidate1'
w = (d / 'workspace').resolve()
sha = lambda p: hashlib.sha256(p.read_bytes()).hexdigest()
wrapper = json.loads((b / 'task7-wrapper-legacy-tests-candidate1/summary.json').read_text())
external = json.loads((d / 'short-runtime4.summary.json').read_text())
xml = json.loads((b / 'task7-wrapper-xml-capability-candidate2/summary.json').read_text())
assert wrapper['terminal'], 'Original wrapper job still active: do not mutate its sources'
assert external['terminal'] and external['exit_code'] == 0, 'External Cargo runtime must finish before workspace mutation'
assert xml['terminal'], 'XML capability RED/GREEN must finish before integration'
assert all(c['exit_code'] == 0 for c in wrapper['commands'] if '-standard-' in c['name'])
assert xml['original_unchanged']
receipt = d / 'integrated6.summary.json'
assert not receipt.exists()
s = dict(terminal=False, production_applied=False, commands=[], source_changes={}, quarantined={})
def save():
    receipt.write_text(json.dumps(s, indent=2) + '\n', encoding='utf-8')
with zipfile.ZipFile(b / 'task7-full-druid-rdbc-wrapper-source-candidate19.zip') as z:
    manifest = json.loads(z.read('source_manifest.json'))
    assert len(manifest) == 689
    for rel, digest in manifest.items():
        data = z.read(rel)
        assert hashlib.sha256(data).hexdigest() == digest
        p = (w / rel).resolve()
        assert p.is_relative_to(w)
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_bytes(data)
    extras = []
    for crate in ['druid-rdbc', 'druid', 'druid-wrapper']:
        for p in (w / 'crates' / crate / 'src').rglob('*'):
            if p.is_file() and p.relative_to(w).as_posix() not in manifest:
                extras.append(p.resolve())
    expected = {f'crates/druid-rdbc/src/resource/{name}.rs' for name in [
        'rdbc_array_access', 'rdbc_blob_access', 'rdbc_clob_access', 'rdbc_n_clob_access',
        'rdbc_ref_access', 'rdbc_resource_access', 'rdbc_resource_context',
        'rdbc_resource_kind', 'rdbc_resource_state', 'rdbc_sql_xml_access']}
    expected.add('crates/druid-wrapper/src/sqlx/sqlx_streaming_result_set.rs')
    assert {p.relative_to(w).as_posix() for p in extras} == expected
    quarantine = (d / 'quarantine6').resolve()
    assert not quarantine.exists()
    for p in extras:
        rel = p.relative_to(w)
        target = (quarantine / rel).resolve()
        assert p.is_relative_to(w) and target.is_relative_to(quarantine)
        assert not target.exists()
        target.parent.mkdir(parents=True, exist_ok=True)
        s['quarantined'][rel.as_posix()] = sha(p)
        shutil.move(str(p), str(target))
mapping = {}
for name in ['rdbc_blob_test.rs', 'rdbc_clob_test.rs']:
    mapping['crates/druid-rdbc/tests/' + name] = b / 'task7-legacy-lob-tests-candidate1' / name
mapping['crates/druid-rdbc/tests/rdbc_resource_context_test.rs'] = b / 'task7-resource-context-tests-candidate1/rdbc_resource_context_test.rs'
callable_dir = b / 'task7-callable-resource-tests-candidate1'
for p in (callable_dir / 'tests').rglob('*.rs'):
    rel = Path('crates/druid') / p.relative_to(callable_dir)
    if (repo / rel).is_file() and p.read_bytes() != (repo / rel).read_bytes():
        mapping[rel.as_posix()] = p
mapping['crates/druid/tests/task6_rdbc_extended_types_test_parity.rs'] = b / 'task7-extended-resource-tests-candidate1/task6_rdbc_extended_types_test_parity.rs'
for p in (b / 'task7-wrapper-legacy-tests-candidate1').glob('*.rs'):
    mapping['crates/druid-wrapper/tests/' + p.name] = p
assert len(mapping) == 19, len(mapping)  # External consumer is already migrated in this workspace.
for rel, p in mapping.items():
    assert p.is_file() and (w / rel).is_file()
    s['source_changes'][rel] = dict(before_sha256=sha(w / rel), candidate_sha256=sha(p))
    shutil.copyfile(p, w / rel)
part01 = 'crates/druid-wrapper/tests/task6_pool_deep_branch_coverage_test_parity_part_01.rs'
shutil.copyfile(b / 'task7-wrapper-xml-capability-candidate2' / Path(part01).name, w / part01)
old = 'impl SqlXml for StubSqlXmlAccess {\n    fn capabilities(&self) -> ResourceOperations {\n        ResourceOperations::READ | ResourceOperations::FREE'
new = 'impl SqlXml for StubSqlXmlAccess {\n    fn capabilities(&self) -> ResourceOperations {\n        // Source仍不支持，不能宣称包含Source的整组READ能力。\n        ResourceOperations::FREE'
for p in (w / 'crates/druid-wrapper/tests').glob('task6_pool_deep_branch_coverage_test_parity_part_*.rs'):
    if p.name != Path(part01).name:
        t = p.read_text(encoding='utf-8')
        assert t.count(old) == 1
        p.write_text(t.replace(old, new), encoding='utf-8', newline='\n')
    s['source_changes'][p.relative_to(w).as_posix()]['final_sha256'] = sha(p)
assert all(sha(w / rel) == digest for rel, digest in manifest.items())
owner_source = b / 'task7-planned-contract-tests-candidate1/crates/druid/tests/resource_owner_cancellation_test.rs'
owner_target = w / 'crates/druid/tests/resource_owner_cancellation_test.rs'
assert owner_source.is_file() and not owner_target.exists()
shutil.copyfile(owner_source, owner_target)
s['new_contract_tests'] = {owner_target.relative_to(w).as_posix(): sha(owner_target)}
s['canonical_source_files'] = 689
s['started'] = datetime.datetime.now(datetime.timezone.utc).isoformat()
save()
env = os.environ.copy()
env.update(json.loads((d / 'retry2.summary.json').read_text())['environment'])
tmp = Path('F:/codex_tmp/task7_integrated6')
tmp.mkdir(parents=True, exist_ok=False)
env.update(TMP=str(tmp), TEMP=str(tmp), CARGO_BUILD_JOBS='1', RUST_TEST_THREADS='1',
           CARGO_TARGET_DIR='F:/codex_build_cache/druid_rust/task6_lifetime_clean_1971_20261008')
cargo = env['CARGO']
def run(name, args):
    command = [cargo] + args
    log = d / ('integrated6-' + name + '.log')
    with log.open('xb') as out:
        r = subprocess.run(command, cwd=w, env=env, stdout=out, stderr=subprocess.STDOUT,
                           creationflags=subprocess.BELOW_NORMAL_PRIORITY_CLASS)
    s['commands'].append(dict(name=name, command=command, exit_code=r.returncode,
                              log=str(log), sha256=sha(log)))
    save()
    print(name, r.returncode, flush=True)
    assert r.returncode == 0
run('druid-unit', ['test', '-p', 'druid', '--lib', '--all-features', '--locked', '--offline', '-j1'])
run('resource-owner-contract', ['test', '-p', 'druid', '--test', 'resource_owner_cancellation_test', '--all-features', '--locked', '--offline', '-j1'])
run('rdbc-complete', ['test', '-p', 'druid-rdbc', '--all-features', '--locked', '--offline', '-j1'])
run('wrapper-complete', ['test', '-p', 'druid-wrapper', '--all-features', '--locked', '--offline', '--no-fail-fast', '-j1'])
run('druid-complete', ['test', '-p', 'druid', '--all-features', '--locked', '--offline', '--no-fail-fast', '-j1'])
s.update(terminal=True, ended=datetime.datetime.now(datetime.timezone.utc).isoformat())
save()
