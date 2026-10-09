from pathlib import Path
import hashlib
import json

root = Path.cwd().resolve()
evidence = root / '.superpowers/sdd/2026-08-24-rdbc-single-standard-unification'
freeze = json.loads((evidence / 'task6-complete-object-filter-workspace-inputs-2026-10-07.json').read_text(encoding='utf-8'))
changed = [path for path, checksum in freeze['inputs'].items()
           if not (root / path).is_file() or hashlib.sha256((root / path).read_bytes()).hexdigest() != checksum]
assert not changed, changed
manifest_path = evidence / 'drafts/complete_scalar_filter_preparation_manifest_20261007.json'
manifest = json.loads(manifest_path.read_text(encoding='utf-8'))
manifest['drafts'] = {name: hashlib.sha256((manifest_path.parent / name).read_bytes()).hexdigest() for name in manifest['drafts']}
for name in ('complete_scalar_filter_guards_draft.rs', 'complete_scalar_read_probe_draft.rs',
             'complete_ordered_scalar_filter_draft.rs', 'complete_scalar_filter_default_order_draft.rs'):
    candidate = manifest_path.parent / name
    if candidate.is_file():
        manifest['drafts'][name] = hashlib.sha256(candidate.read_bytes()).hexdigest()
manifest['workspace_frozen_inputs_verified'] = len(freeze['inputs'])
manifest_path.write_bytes((json.dumps(manifest, ensure_ascii=False, indent=2) + '\n').encode('utf-8'))
print('Workspace frozen inputs unchanged:', len(freeze['inputs']))
print('Formatted preparation draft hashes recorded; not compiled/run/bound')
