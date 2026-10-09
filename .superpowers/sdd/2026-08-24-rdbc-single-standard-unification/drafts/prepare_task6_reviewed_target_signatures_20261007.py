"""按六个已核对的参数名变化准备候选，保留所有BASE及语义字段。"""
from pathlib import Path
from dataclasses import fields, replace
import hashlib
import json
import sys

root = Path.cwd()
sys.path.insert(0, str(root / "scripts"))
import verify_task6_method_parity as v
draft = root / ".superpowers/sdd/2026-08-24-rdbc-single-standard-unification/drafts"
prior = draft / "task6-method-rebinding-candidate-20261007.tsv"
rows = v.parse_rows(prior.read_bytes())
audit = json.loads((draft / "task6-method-target-signature-audit-20261007.json").read_text(encoding="utf-8"))
renames = {
    "date": {"_calendar:": "calendar:"},
    "time": {"_calendar:": "calendar:"},
    "timestamp": {"_calendar:": "calendar:"},
    "object_as": {"_column:": "column:", "_target_type:": "target_type:"},
    "update_value": {"_update:": "update:"},
    "update_value_by_label": {"column_label:": "_column_label:"},
}
assert len(rows) == 479 and len(audit["target_signature_changes"]) == 6
assert {c["method"] for c in audit["target_signature_changes"]} == set(renames)
changes = {(c["source_object"], c["method"]): c for c in audit["target_signature_changes"]}
updated = []
for row in rows:
    change = changes.get((row.source_object, row.method))
    if change is None:
        updated.append(row)
        continue
    assert row.source_object == "PhysicalResultSet"
    expected = row.target_signature
    for old, new in renames[row.method].items():
        assert expected.count(old) == 1
        expected = expected.replace(old, new)
    assert row.target_signature == change["old"] and expected == change["new"]
    current = v.parse_target_method((root / row.target_path).read_text(encoding="utf-8"),
                                    row.target_trait, row.target_symbol.rsplit("::", 1)[-1])
    assert current.signature == expected and current.signature_sha256 == change["new_hash"]
    candidate = replace(row, target_signature=current.signature,
                        target_signature_sha256=current.signature_sha256)
    assert all(getattr(candidate, f.name) == getattr(row, f.name)
               for f in fields(row) if f.name not in {"target_signature", "target_signature_sha256"})
    updated.append(candidate)
candidate_path = draft / "task6-method-rebinding-signature-candidate-20261007.tsv"
assert not candidate_path.exists()
candidate_path.write_bytes(v.render_rows(updated))
print("REVIEWED_PARAMETER_NAME_CHANGES=6; BASE_AND_SEMANTIC_FIELDS_UNCHANGED")
print("CANDIDATE_SHA256=" + hashlib.sha256(candidate_path.read_bytes()).hexdigest())
