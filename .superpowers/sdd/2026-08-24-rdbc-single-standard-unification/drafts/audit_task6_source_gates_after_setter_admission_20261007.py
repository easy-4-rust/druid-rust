"""只读核对现有源码门禁；不执行Cargo，不将子检查替代完整验收。"""
from pathlib import Path
import json
import sys

root = Path.cwd()
sys.path.insert(0, str(root / "scripts"))
import verify_wrapper_standard_impls as wrapper
import verify_druid_single_standard as druid

checks = []

def check(name, action):
    try:
        result = action()
        checks.append({"check": name, "state": "PASS", "result": result})
    except (OSError, UnicodeError, wrapper.VerificationError, druid.VerificationError) as error:
        checks.append({"check": name, "state": "FAIL", "error": str(error)})

adapters = wrapper.task_adapter_rows(root / wrapper.DEFAULT_ADAPTERS, root / wrapper.DEFAULT_RUNTIME_EVIDENCE)
for row in adapters:
    check("adapter:" + row.implementation_type + ":" + row.target_trait,
          lambda row=row: wrapper.verify_adapter_source(root, row))
check("wrapper_21_consumers", lambda: wrapper.verify_consumer_rows(root, root / wrapper.DEFAULT_CONSUMERS))
check("sqlx_ownership", lambda: wrapper.verify_sqlx_ownership(root))
check("family_executor_call_chains", lambda: wrapper.verify_family_executor_call_chains(root))
check("trace_record_callsites", lambda: wrapper.verify_trace_record_callsites(root))
check("coverage_manifest_source_bindings", lambda: len(wrapper.load_coverage_manifest(root)))
check("druid_pending_admission_source_policy", lambda: druid.require_pending_admission_policy(
    (root / "crates/druid/src/core/standard_resource_registry.rs").read_text(encoding="utf-8"),
    (root / "crates/druid/src/core/standard_resource_admission.rs").read_text(encoding="utf-8"),
    {path.name: path.read_text(encoding="utf-8") for path in (
        root / "crates/druid/src/core/druid_pooled_connection.rs",
        root / "crates/druid/src/core/druid_pooled_statement.rs",
        root / "crates/druid/src/core/druid_pooled_prepared_statement.rs",
        root / "crates/druid/src/core/druid_pooled_callable_statement.rs",
        root / "crates/druid/src/core/database_meta_data_proxy_impl.rs",
        root / "crates/druid/src/core/druid_pooled_result_set.rs",
    )}))
result = {"scope": "named source subchecks only; no runtime/compiler/API proof",
          "adapters": len(adapters), "checks": checks,
          "passed": sum(c["state"] == "PASS" for c in checks),
          "failed": sum(c["state"] == "FAIL" for c in checks)}
output = root / ".superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-source-gates-after-setter-admission-2026-10-07.json"
assert not output.exists()
output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
print("SOURCE_SUBCHECKS_PASS=" + str(result["passed"]))
print("SOURCE_SUBCHECKS_FAIL=" + str(result["failed"]))
for item in checks:
    if item["state"] == "FAIL":
        print(json.dumps(item, ensure_ascii=False))
sys.exit(1 if result["failed"] else 0)
