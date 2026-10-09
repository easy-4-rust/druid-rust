"""核对全部479目标签名；只准备候选，不改正式账本或Rust。"""
from pathlib import Path
from dataclasses import replace
from functools import lru_cache
import hashlib
import json
import sys

root = Path.cwd()
sys.path.insert(0, str(root / "scripts"))
import verify_task6_method_parity as v

draft = root / ".superpowers/sdd/2026-08-24-rdbc-single-standard-unification/drafts"
prior = draft / "task6-method-rebinding-candidate-20261007.tsv"
assert hashlib.sha256(prior.read_bytes()).hexdigest() == "9f266d05760de5e0044b9eec16c9fca74d446031bbe13e72bb84a5b551a9c1ce"
rows = v.parse_rows(prior.read_bytes())
assert len(rows) == 479

@lru_cache(maxsize=None)
def source(path):
    return (root / path).read_text(encoding="utf-8")

@lru_cache(maxsize=None)
def direct(path, trait):
    try:
        return {m.name: m for m in v.parse_direct_trait_methods(source(path), trait)}
    except v.VerificationError:
        return {}

changes = []
updated = []
for row in rows:
    trait, name = row.target_symbol.rsplit("::", 1)
    method = direct(row.target_path, trait).get(name)
    if method is None:
        method = v.parse_target_method(source(row.target_path), trait, name)
    if row.target_signature != method.signature or row.target_signature_sha256 != method.signature_sha256:
        changes.append({"source_object": row.source_object, "method": row.method,
                        "path": row.target_path, "old": row.target_signature,
                        "new": method.signature, "old_hash": row.target_signature_sha256,
                        "new_hash": method.signature_sha256})
        updated.append(replace(row, target_signature=method.signature,
                               target_signature_sha256=method.signature_sha256))
    else:
        updated.append(row)

result = {"state": "AUDITED_NOT_BOUND_NOT_EXECUTED", "rows": len(rows),
          "target_signature_changes": changes}
audit = draft / "task6-method-target-signature-audit-20261007.json"
assert not audit.exists()
audit.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print(json.dumps(result, ensure_ascii=False, indent=2))
# 不自动接受类型、接收者、同步/异步或返回值变化。
assert all(c["old"].replace("_calendar:", "calendar:") == c["new"] for c in changes), "signature type/behavior drift requires separate review"
candidate = draft / "task6-method-rebinding-signature-candidate-20261007.tsv"
assert not candidate.exists()
candidate.write_bytes(v.render_rows(updated))
print("CANDIDATE_SHA256=" + hashlib.sha256(candidate.read_bytes()).hexdigest())
