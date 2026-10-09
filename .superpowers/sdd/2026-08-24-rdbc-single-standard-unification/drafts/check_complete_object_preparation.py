"""Refresh draft hashes after formatting and verify unchanged Cargo inputs."""
import hashlib
import json
import subprocess
from pathlib import Path

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[3]
path = OUT / "complete_object_filter_preparation_manifest_2026-10-07.json"
manifest = json.loads(path.read_text(encoding="utf-8"))
assert manifest["state"] == "PREPARATION_NOT_COMPILED_NOT_RUN_NOT_BOUND"
source = subprocess.check_output(["git", "show", f"{manifest['base']}:{manifest['base_source']}"], cwd=ROOT)
assert hashlib.sha256(source).hexdigest() == manifest["base_source_sha256"]
original = source.decode("utf-8")
start = original.index(f"async fn {manifest['base_case']}()")
opening = original.index("{", start)
depth = 0
for end in range(opening, len(original)):
    depth += (original[end] == "{") - (original[end] == "}")
    if depth == 0:
        original_case = original[start:end + 1]
        break
else:
    raise AssertionError("unclosed frozen BASE case")
assert hashlib.sha256(original_case.encode()).hexdigest() == manifest["base_function_sha256"]
assert (OUT / "complete_object_filter_base_case.rs").read_text(encoding="utf-8") == "#[tokio::test]\n" + original_case + "\n"
known = {Path(artifact["path"]).name for artifact in manifest["artifacts"]}
for name in ["complete_object_filter_guard_draft.rs", "complete_object_read_probe_draft.rs",
             "complete_object_filter_default_identity_draft.rs", "complete_object_payload_draft.rs",
             "complete_ordered_object_filter_draft.rs", "complete_object_filter_order_draft.rs",
             "complete_object_filter_expired_lease_draft.rs"]:
    if name not in known:
        manifest["artifacts"].append({"path": str((OUT / name).relative_to(ROOT)), "sha256": ""})
for artifact in manifest["artifacts"]:
    artifact["sha256"] = hashlib.sha256((ROOT / artifact["path"]).read_bytes()).hexdigest()
assert (OUT / "complete_object_filter_base_case.rs").read_text(encoding="utf-8").count("assert_eq!") == 10
assert (OUT / "complete_object_filter_canonical_case_draft.rs").read_text(encoding="utf-8").count("assert_eq!") == 11
freeze = json.loads((OUT / "n_string_filter_workspace_inputs_2026-10-07.json").read_text(encoding="utf-8"))
for name, expected in freeze["inputs"].items():
    assert hashlib.sha256((ROOT / name).read_bytes()).hexdigest() == expected, name
path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
print(f"PASS: exact frozen BASE source/function; {len(manifest['artifacts'])} draft hashes current; original 10 + one pointer-identity assertion; all 16 frozen inputs unchanged")
