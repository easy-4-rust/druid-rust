"""Read-only evidence for the existing admission gate; never bypass it."""
import hashlib
import json
import sys
from pathlib import Path

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[3]
sys.path.insert(0, str(ROOT / "scripts"))
import verify_druid_single_standard as verifier

core = ROOT / "crates/druid/src/core"
names = ("druid_pooled_connection.rs", "druid_pooled_statement.rs",
         "druid_pooled_prepared_statement.rs", "druid_pooled_callable_statement.rs",
         "database_meta_data_proxy_impl.rs", "druid_pooled_result_set.rs")
sources = {name: (core / name).read_text(encoding="utf-8") for name in names}
try:
    verifier.require_pending_admission_policy(
        (core / "standard_resource_registry.rs").read_text(encoding="utf-8"),
        (core / "standard_resource_admission.rs").read_text(encoding="utf-8"), sources,
    )
except verifier.VerificationError as error:
    gate = {"state": "FAIL", "reason": str(error)}
else:
    gate = {"state": "PASS", "reason": "source-only subcheck; no Cargo execution"}
blocks = verifier._impl_blocks(sources[names[0]], "Connection", "DruidPooledConnection")
assert len(blocks) == 1
tokens, _, opening, closing = blocks[0]
methods = verifier._method_bodies(tokens, opening, closing)
ordinary = methods["prepare_statement"]
delegate = methods["prepare_statement_with_generated_keys"]
markers = ("begin_admission", "prepare_statement_with_generated_keys(&effective_sql,options,generated_keys)", "DruidPooledPreparedStatement::new_standard")
positions = {marker: delegate.find(marker) for marker in markers}
result = {
    "state": "READ_ONLY_DIAGNOSIS_GATE_NOT_CHANGED",
    "current_source_gate": gate,
    "ordinary_method_body": ordinary,
    "generated_keys_delegate_marker_offsets": positions,
    "source_sha256": {name: hashlib.sha256((core / name).read_bytes()).hexdigest() for name in names},
    "required_verifier_regression_cases": [
        "accept exact ordinary GeneratedKeys::None delegation only with actual permitted delegate",
        "reject missing/ambiguous delegate, unrelated receiver and changed GeneratedKeys mode",
        "reject raw acquisition before admission including duplicated raw acquisition",
        "reject comments/string markers, dead branches, extra early raw calls and lost admission handoff",
        "retain direct historical producer checks and every other producer denominator",
    ],
    "limitation": "lexical offsets identify current wiring only; not a control-flow proof, runtime proof or gate waiver",
}
path = OUT / "current_admission_delegation_diagnosis_2026-10-07.json"
assert not path.exists(), "preserve prior diagnosis"
path.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
print(json.dumps({"gate": gate, "ordinary": ordinary, "offsets": positions}, ensure_ascii=False))
