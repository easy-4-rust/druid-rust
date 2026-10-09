"""Validate and complete non-Cargo preparation without altering frozen sources."""
import hashlib
import json
import re
from pathlib import Path

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[3]
manifest_path = OUT / "complete_object_filter_preparation_manifest_2026-10-07.json"
manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
base = (OUT / "complete_object_filter_base_case.rs").read_text(encoding="utf-8")
name = manifest["base_case"]
candidate = base.replace(name, "task6_" + name)
candidate = candidate.replace("    let call_log =", "    use crate::core::{FilterChain, RdbcError, Value};\n    use druid_rdbc::types::{RdbcObject, RdbcTypeMap, RdbcTargetType};\n    use std::sync::Mutex;\n    use object_short_circuit_filter::ObjectShortCircuitFilter;\n    let call_log =", 1)
candidate = candidate.replace("calls(&call_log)", "call_log.lock().unwrap().clone()")
candidate = candidate.replace(".object_by_label_with_type_map(", ".object_with_type_map_by_label(")
candidate = candidate.replace(".object_typed_by_label(", ".object_as_by_label(")
candidate = candidate.replace(".object_typed(", ".object_as(")
candidate = candidate.replace("&mut connection, ", "").replace("&mut connection,\n", "")
candidate = re.sub(r"\.(object|object_with_type_map|object_as)\((\d+)", r".\1(ColumnIndex::new(\2).unwrap()", candidate)
candidate = candidate.replace("Value::Int(901)", "RdbcObject::Scalar(Value::Int(901))")
candidate = candidate.replace('Value::String("filtered-label-object".to_string())', 'RdbcObject::Scalar(Value::String("filtered-label-object".to_string()))')
start = candidate.index("    let factory = ToastyConnectionFactory")
end = candidate.index("    let mut result_set =", start)
candidate = candidate[:start] + '''    let factory = ToastyConnectionFactory::new("sqlite::memory:").await.unwrap();
    let mut connection = factory.create().await.unwrap();
    let resources = Arc::new(StandardResourceRegistry::default());
    let generation = resources.current_generation();
    let lease_active = Arc::new(AtomicBool::new(true));
    let filters = Some(Arc::new(filter_chain));
    let policy = StandardLogicalPolicy::new(
        75, "sqlite-object-filter".to_owned(), Arc::clone(&lease_active),
        Arc::new(AtomicBool::new(false)), filters.clone(), Some(true), generation,
        Arc::new(OwnerDispositionControl::new(Box::new(|_| {}))),
    );
    let admission = resources.begin_admission(generation).unwrap();
    let raw_statement = connection.create_statement(StatementOptions::default()).await.unwrap();
    let mut statement = DruidPooledStatement::new_standard(
        raw_statement, StatementOptions::default(), Arc::clone(&resources), policy,
        75, 1, Arc::new(AtomicU64::new(1)), Arc::new(AtomicU64::new(1)), lease_active, filters, None,
    );
    resources.register(&admission, Arc::clone(&statement.inner)).await.unwrap();
    drop(admission);
''' + candidate[end:]
candidate = candidate.replace(".execute_query_result_set(", ".execute_query(")
candidate = candidate.replace("result_set.close_with_connection(&mut connection).unwrap();", "result_set.close().await.unwrap();\n    statement.close().await.unwrap();\n    connection.close().await.unwrap();")
candidate = candidate.replace("calls: Arc::clone(&call_log),", "calls: Arc::clone(&call_log),\n        arguments: Arc::new(Mutex::new(Vec::new())),")
assert candidate.count("assert_eq!") == 10
candidate = candidate.replace("    let call_log = Arc::new(Mutex::new(Vec::new()));", "    let call_log = Arc::new(Mutex::new(Vec::new()));\n    let argument_log = Arc::new(Mutex::new(Vec::new()));")
candidate = candidate.replace("arguments: Arc::new(Mutex::new(Vec::new())),", "arguments: Arc::clone(&argument_log),")
candidate = candidate.replace('    let custom_target = RdbcTargetType::Custom("example.Target".to_string());', '    let custom_target = RdbcTargetType::Custom("example.Target".to_string());\n    let string_target = RdbcTargetType::String;')
candidate = candidate.replace('&RdbcTargetType::String)', '&string_target)')
error_start = candidate.index("    assert_eq!(\n        result_set.object_as_by_label(")
candidate = candidate[:error_start] + '''    // 补充观察原映射和目标类型引用，不能只比较重新构造后的内容。
    assert_eq!(
        *argument_log.lock().unwrap(),
        vec![std::ptr::from_ref(&empty_map) as usize,
             std::ptr::from_ref(&populated_map) as usize,
             std::ptr::from_ref(&custom_target) as usize,
             std::ptr::from_ref(&string_target) as usize],
    );
''' + candidate[error_start:]
assert candidate.count("assert_eq!") == 11
# The exact original ordered event strings must survive method-name translation.
original_events = re.findall(r'"(result_set_get_object[^"\n]*(?:\\"[^"\n]*)*)"', base)
assert original_events == re.findall(r'"(result_set_get_object[^"\n]*(?:\\"[^"\n]*)*)"', candidate)
draft = OUT / "complete_object_filter_canonical_case_draft.rs"
draft.write_text(candidate, encoding="utf-8", newline="\n")
manifest["artifacts"] = [
    {"path": str((OUT / name).relative_to(ROOT)), "sha256": ""}
    for name in ["complete_object_filter_base_case.rs", "complete_object_filter_base_fixture.rs",
                 "complete_object_filter_canonical_case_draft.rs", "complete_object_filter_request_draft.rs",
                 "complete_object_short_circuit_filter_draft.rs"]
]
for artifact in manifest["artifacts"]:
    artifact["sha256"] = hashlib.sha256((ROOT / artifact["path"]).read_bytes()).hexdigest()
manifest["canonical_setup"] = "actual SQLite factory, registered canonical Statement, connection id 75 and original context name, unpositioned SELECT 1 AS value"
manifest["candidate_assert_eq_count"] = 11
manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
freeze = json.loads((OUT / "n_string_filter_workspace_inputs_2026-10-07.json").read_text(encoding="utf-8"))
for name, expected in freeze["inputs"].items():
    assert hashlib.sha256((ROOT / name).read_bytes()).hexdigest() == expected, name
print("PASS: complete original 10 assertions and ordered events preserved; all 16 declared frozen inputs unchanged; preparation remains NOT_COMPILED/NOT_RUN/NOT_BOUND")
