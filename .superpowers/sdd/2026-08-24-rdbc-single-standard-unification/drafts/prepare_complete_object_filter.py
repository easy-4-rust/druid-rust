"""Preserve the complete frozen Object Filter case before touching Cargo inputs."""
import hashlib
import json
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).resolve().parent
BASE = "5afe8609b177544ff7d18461aa84f69efe8ee90e"
SOURCE = "crates/druid/tests/result_set_filter_semantics_test.rs"
NAME = "pooled_result_set_routes_all_object_overloads_and_preserves_argument_identity"


def block(text, marker):
    start = text.index(marker)
    opening = text.index("{", start)
    depth = 0
    for end in range(opening, len(text)):
        depth += (text[end] == "{") - (text[end] == "}")
        if depth == 0:
            return text[start:end + 1]
    raise AssertionError("unclosed BASE block")


def write_new(name, text):
    path = OUT / name
    assert not path.exists(), f"preserve existing artifact: {path}"
    path.write_text(text, encoding="utf-8", newline="\n")
    return {"path": str(path.relative_to(ROOT)), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}


raw = subprocess.check_output(["git", "show", f"{BASE}:{SOURCE}"], cwd=ROOT)
source = raw.decode("utf-8")
case = block(source, f"async fn {NAME}()")
fixture = block(source, "struct ObjectShortCircuitFilter")
fixture_impl = block(source, "impl ObjectShortCircuitFilter")
filter_impl = block(source, "impl ResultSetFilter for ObjectShortCircuitFilter")
identity = block(source, "fn type_map_identity(")
assert case.count("assert_eq!") == 10
artifacts = [
    write_new("complete_object_filter_base_case.rs", "#[tokio::test]\n" + case + "\n"),
    write_new("complete_object_filter_base_fixture.rs", "\n\n".join([fixture, fixture_impl, filter_impl, identity]) + "\n"),
]

# These are interface translations, not new behavior or reduced assertions.
candidate = case.replace(NAME, "task6_" + NAME)
candidate = candidate.replace(".object_by_label_with_type_map(", ".object_with_type_map_by_label(")
candidate = candidate.replace(".object_typed_by_label(", ".object_as_by_label(")
candidate = candidate.replace(".object_typed(", ".object_as(")
candidate = candidate.replace("&mut connection, ", "")
candidate = candidate.replace("&mut connection,\n", "")
candidate = re.sub(r"\.(object|object_with_type_map|object_as)\((\d+)",
                   r".\1(ColumnIndex::new(\2).unwrap()", candidate)
candidate = candidate.replace("Value::Int(901)", "RdbcObject::Scalar(Value::Int(901))")
candidate = candidate.replace('Value::String("filtered-label-object".to_string())',
                              'RdbcObject::Scalar(Value::String("filtered-label-object".to_string()))')
candidate = candidate.replace(".execute_query_result_set(", ".execute_query(")
candidate = candidate.replace("result_set.close_with_connection(&mut connection).unwrap();",
                              "result_set.close().await.unwrap();\n    statement.close().await.unwrap();")
# Actual canonical connection/statement setup must be supplied after the running
# workspace reaches terminal state; this draft intentionally is outside Cargo.
assert candidate.count("assert_eq!") == 10
artifacts.append(write_new("complete_object_filter_canonical_case_draft.rs", "#[tokio::test]\n" + candidate + "\n"))
manifest = {
    "state": "PREPARATION_NOT_COMPILED_NOT_RUN_NOT_BOUND",
    "base": BASE,
    "base_source": SOURCE,
    "base_source_sha256": hashlib.sha256(raw).hexdigest(),
    "base_case": NAME,
    "base_function_sha256": hashlib.sha256(case.encode()).hexdigest(),
    "assert_eq_count": 10,
    "scope": "all ordinary/map/typed index-label overloads, seven ordered success events, exact DriverError and actual statement exception count",
    "required_additional_controls": [
        "borrowed type-map and target-type reference identity",
        "no-map ordinary request distinct from map=None and empty map",
        "RdbcObject resource/opaque return identity without Value flattening",
        "registration order, default delegation, short circuit, error unwind",
        "hidden column and closed/expired lease reject before hook",
    ],
    "artifacts": artifacts,
}
write_new("complete_object_filter_preparation_manifest_2026-10-07.json", json.dumps(manifest, ensure_ascii=False, indent=2) + "\n")
print(json.dumps({"state": manifest["state"], "assert_eq_count": 10, "artifacts": len(artifacts)}, ensure_ascii=False))
