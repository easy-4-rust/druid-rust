"""仅在完整候选静态PASS后，保护式绑定479行；实际运行验收仍OPEN。"""
from pathlib import Path
from dataclasses import fields
import hashlib
import json
import sys

root = Path.cwd()
sys.path.insert(0, str(root / "scripts"))
import verify_task6_method_parity as v

evidence = root / ".superpowers/sdd/2026-08-24-rdbc-single-standard-unification"
draft = evidence / "drafts"
matrix = root / v.DEFAULT_MATRIX
candidate = draft / "task6-method-rebinding-signature-candidate-20261007.tsv"
old_hash = "7a96fdbec7484fed1461a756422ef9b05d25a6f8b1dd10ce305e353e643787d3"
new_hash = "65ae308dfa52b6bc0b4548ade2b3834fd828495e52dd34e01d598078e3608abd"
sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
assert sha(matrix) == old_hash and sha(candidate) == new_hash
assert sha(root / v.DEFAULT_METHOD_CALL_INDEX) == "54aeeefe8b1eead36b44646199e4580dfa6fd682bdc6995340bba57b45db9c9e"
static_log = evidence / "task6-method-signature-candidate-static-2026-10-07.log"
output = static_log.read_text(encoding="utf-8-sig")
assert "TASK6_METHOD_ROWS=479\n" in output
assert f"TASK6_METHOD_SHA256={new_hash}\n" in output
assert "TASK6_METHOD_EVIDENCE_EXECUTED=no" in output
assert "TASK6_METHOD_PARITY_ERROR" not in output
generated_log = evidence / "task6-method-signature-candidate-generated-check-2026-10-07.log"
assert "PASS generated method behavior matrix: files=14" in generated_log.read_text(encoding="utf-8-sig")
freeze = json.loads((evidence / "task6-complete-stat-close-error-runtime3-inputs-2026-10-07.json").read_text(encoding="utf-8"))["inputs"]
assert len(freeze) == 1090 and all(sha(root / name) == value for name, value in freeze.items())
old = v.parse_rows(matrix.read_bytes())
new = v.parse_rows(candidate.read_bytes())
assert len(old) == len(new) == 479
changes = []
for before, after in zip(old, new):
    changed = [f.name for f in fields(before) if getattr(before, f.name) != getattr(after, f.name)]
    identity = (before.source_object, before.method)
    if not changed:
        continue
    if identity == ("PhysicalConnection", "execute"):
        assert changed == ["behavior_evidence"]
    else:
        assert before.source_object == "PhysicalResultSet"
        assert before.method in {"date", "time", "timestamp", "object_as", "update_value", "update_value_by_label"}
        assert set(changed) == {"target_signature", "target_signature_sha256"}
    changes.append({"source_object": identity[0], "method": identity[1], "fields": changed})
assert len(changes) == 7
backup = draft / "task6-method-matrix-before-validated-binding-20261007.tsv"
summary = evidence / "task6-method-validated-binding-2026-10-07.summary.json"
assert not backup.exists() and not summary.exists()
backup.write_bytes(matrix.read_bytes())
matrix.write_bytes(candidate.read_bytes())
assert sha(matrix) == new_hash
result = {"state": "BOUND_SOURCE_VALIDATED_RUNTIME_PENDING", "rows": 479,
          "old_sha256": old_hash, "new_sha256": new_hash, "changes": changes,
          "static_log_sha256": sha(static_log), "generated_log_sha256": sha(generated_log),
          "unchanged_build_inputs": len(freeze)}
summary.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
print(json.dumps(result))
