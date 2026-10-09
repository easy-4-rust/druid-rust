"""覆盖再生成的只读来源预检；明确不宣称编译覆盖通过。"""
from pathlib import Path
import hashlib
import json
import sys

root = Path.cwd()
sys.path.insert(0, str(root / "scripts"))
import regenerate_task6_coverage_manifest as regeneration
import verify_wrapper_standard_impls as verifier

obligations = regeneration.canonical_requirements(root)
regeneration.validate_canonical_source_requirements(root, obligations)
old_raw = (root / verifier.DEFAULT_COVERAGE_MANIFEST).read_bytes()
old = json.loads(old_raw)
old_rows = {
    (family["family"], requirement["requirement_id"]): requirement
    for family in old["families"]
    for requirement in family["requirements"]
}
rows = []
for family in verifier.COVERAGE_FAMILY_ORDER:
    for requirement_id, (kind, path, function, anchor) in zip(
        verifier.COVERAGE_REQUIREMENT_IDS, obligations[family], strict=True
    ):
        prior = old_rows[(family, requirement_id)]
        current_hash = hashlib.sha256((root / path).read_bytes()).hexdigest()
        rows.append({
            "family": family, "requirement_id": requirement_id,
            "kind": kind, "path": path.as_posix(), "function": function,
            "anchor": anchor, "current_source_sha256": current_hash,
            "stored_source_sha256": prior["source_sha256"],
            "source_hash_stale": current_hash != prior["source_sha256"],
            "source_binding_changed": any((
                prior["requirement_kind"] != kind,
                Path(prior["source_path"]) != path,
                prior["source_function"] != function,
                prior["anchor_id"] != anchor,
            )),
        })
assert len(rows) == 105 and len(obligations) == 7
result = {
    "scope": "source preflight only; compiler/runtime NOT_RUN",
    "families": 7, "source_obligations": 105,
    "old_manifest_sha256": hashlib.sha256(old_raw).hexdigest(),
    "stale_source_rows": sum(row["source_hash_stale"] for row in rows),
    "changed_binding_rows": sum(row["source_binding_changed"] for row in rows),
    "rows": rows,
}
output = root / ".superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-coverage-regeneration-preflight-2026-10-07.json"
assert not output.exists()
output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print("COVERAGE_SOURCE_PREFLIGHT=PASS")
print("FAMILIES=7; SOURCE_OBLIGATIONS=105; COMPILER_EXECUTED=no")
print("STALE_SOURCE_ROWS=" + str(result["stale_source_rows"]))
print("CHANGED_SOURCE_BINDING_ROWS=" + str(result["changed_binding_rows"]))
