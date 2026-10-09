#!/usr/bin/env python3
"""Atomically regenerate the seven-family Task 6 compiler-coverage manifest."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import tempfile
import time
from pathlib import Path

import verify_wrapper_standard_impls as verifier


def canonical_requirements(
    root: Path,
) -> dict[str, tuple[tuple[str, Path, str, str], ...]]:
    """Resolve the ordered 7x15 source obligations without trusting stale identities."""

    return {
        family: tuple(
            verifier.canonical_coverage_requirement(family, requirement_id)
            for requirement_id in verifier.COVERAGE_REQUIREMENT_IDS
        )
        for family in verifier.COVERAGE_FAMILY_ORDER
    }


def validate_source_function_unique(
    root: Path, source_path: Path, source_function: str, label: str
) -> None:
    """Fail before runtime builds when one obligation source binding is stale."""

    target = root / source_path
    if not target.is_file():
        verifier.fail(f"coverage preflight source is missing: {label}:{source_path}")
    ranges = verifier.function_line_ranges(
        target.read_text(encoding="utf-8"), source_function
    )
    if len(ranges) != 1:
        verifier.fail(
            "coverage preflight source function is missing/ambiguous: "
            f"{label}:{source_path}:{source_function}:{ranges}"
        )


def validate_canonical_source_requirements(
    root: Path,
    obligations: dict[str, tuple[tuple[str, Path, str, str], ...]],
) -> None:
    """Validate all 7x15 current source bindings and live executor call chains cheaply."""

    for family in verifier.COVERAGE_FAMILY_ORDER:
        rows = obligations.get(family, ())
        if len(rows) != len(verifier.COVERAGE_REQUIREMENT_IDS):
            verifier.fail(f"coverage preflight denominator drift: {family}:{len(rows)}")
        for requirement_id, (_, source_path, source_function, _) in zip(
            verifier.COVERAGE_REQUIREMENT_IDS, rows, strict=True
        ):
            validate_source_function_unique(
                root,
                source_path,
                source_function,
                f"{family}:{requirement_id}",
            )
    verifier.verify_sqlx_ownership(root)
    verifier.verify_family_executor_call_chains(root)
    verifier.verify_trace_record_callsites(root)


def exact_compiler_name(
    root: Path,
    functions: list[object],
    known_paths: tuple[Path, ...],
    source_path: Path,
    source_function: str,
) -> str:
    """Select one positive compiler identity bound to the exact source-function range."""

    source = (root / source_path).read_text(encoding="utf-8")
    ranges = verifier.function_line_ranges(source, source_function)
    if len(ranges) != 1:
        verifier.fail(
            f"coverage regeneration source function is missing/ambiguous: "
            f"{source_path}:{source_function}:{ranges}"
        )
    start, end = ranges[0]
    candidates: list[str] = []
    for function in functions:
        if not isinstance(function, dict) or set(function) != {
            "name",
            "count",
            "filenames",
            "regions",
            "branches",
            "mcdc_records",
        }:
            verifier.fail("coverage regeneration function schema is not exact")
        name = function["name"]
        count = function["count"]
        filenames = function["filenames"]
        regions = function["regions"]
        if (
            not isinstance(name, str)
            or isinstance(count, bool)
            or not isinstance(count, int)
            or not isinstance(filenames, list)
            or not isinstance(regions, list)
        ):
            verifier.fail("coverage regeneration function types are invalid")
        # Runtime evidence tests are non-generic free functions. Their Rust v0
        # top-level value identity starts with `_RNv` and ends with the exact
        # length-prefixed source leaf. Async lowering also emits nested
        # closure/coroutine symbols that contain the same leaf plus a suffix
        # (for example `_RNCNv...<leaf>0B3_`); those are implementation details,
        # never the compiler identity committed by this protocol.
        exact_leaf = f"{len(source_function)}{source_function}"
        top_level_test_identity = name.startswith("_RNv") and name.endswith(exact_leaf)
        compiler_generated_nested = any(
            marker in name
            for marker in ("{closure}", "{{closure}}", "coroutine", "generator")
        )
        normalized = tuple(
            verifier.normalize_compiler_path(filename, known_paths)
            for filename in filenames
        )
        if (
            normalized != (source_path,)
            or not top_level_test_identity
            or compiler_generated_nested
            or count <= 0
        ):
            continue
        positive_exact_region = False
        for region in regions:
            if not isinstance(region, list) or len(region) != 8:
                verifier.fail("coverage regeneration region schema is not exact")
            file_id = region[5]
            if isinstance(file_id, bool) or not isinstance(file_id, int):
                verifier.fail("coverage regeneration region file id is invalid")
            if file_id < 0 or file_id >= len(normalized):
                verifier.fail("coverage regeneration region file id exceeds filenames")
            if start <= region[0] <= region[2] <= end and region[4] > 0:
                positive_exact_region = True
        if positive_exact_region:
            candidates.append(name)
    if len(candidates) != 1:
        verifier.fail(
            f"coverage regeneration compiler identity is not unique: "
            f"{source_path}:{source_function}:candidates={candidates}"
        )
    return candidates[0]


def fresh_report(
    root: Path, spec: verifier.EvidenceSpec
) -> tuple[dict[str, object], str]:
    """Generate one run-owned report/target and return its report plus raw digest."""

    verifier.verify_evidence_source_closure(root, spec)
    with tempfile.TemporaryDirectory(
        prefix=f"druid-task6-regenerate-{spec.family.lower()}-"
    ) as temporary:
        run_directory = Path(temporary)
        report_path = run_directory / "coverage.json"
        environment = os.environ.copy()
        environment.pop(verifier.TRACE_CHALLENGE_ENV, None)
        environment["CARGO_TARGET_DIR"] = str(run_directory / "target")
        started_at_ns = time.time_ns()
        result = verifier.run_bounded_process(
            verifier.cargo_evidence_command(spec, report_path),
            cwd=root,
            timeout_seconds=verifier.DEFAULT_EVIDENCE_TIMEOUT_SECONDS,
            output_limit=verifier.DEFAULT_EVIDENCE_OUTPUT_LIMIT,
            env=environment,
        )
        if result.returncode != 0:
            verifier.fail(
                f"coverage regeneration exact run failed: {spec.family}:"
                f"{(result.stdout + result.stderr)[-20_000:]}"
            )
        raw = report_path.read_bytes() if report_path.is_file() else b""
        report = verifier.load_fresh_coverage_report(
            report_path,
            run_directory=run_directory,
            started_at_ns=started_at_ns,
        )
        verifier.verify_coverage_profile_artifacts(run_directory, started_at_ns)
        return report, hashlib.sha256(raw).hexdigest()


def snapshot_build_inputs(root: Path) -> dict[str, str]:
    """冻结源码、依赖及编译配置，排除生成缓存和证据草稿。"""

    result: dict[str, str] = {}
    excluded = {".git", ".codegraph", ".superpowers", "target", "node_modules"}
    names = {"Cargo.toml", "Cargo.lock", "rust-toolchain", "rust-toolchain.toml"}
    for directory, children, files in os.walk(root):
        children[:] = sorted(child for child in children if child not in excluded)
        for name in sorted(files):
            path = Path(directory) / name
            relative = path.relative_to(root).as_posix()
            if (path.suffix in {".rs", ".proto", ".sql"} or name in names
                    or relative in {".cargo/config", ".cargo/config.toml"}):
                result[relative] = hashlib.sha256(path.read_bytes()).hexdigest()
    return result


def require_unchanged_build_inputs(root: Path, snapshot: dict[str, str]) -> None:
    """新增、删除或修改构建输入均拒绝，不能为旧报告绑定新hash。"""

    current = snapshot_build_inputs(root)
    changed = sorted(path for path in set(snapshot) | set(current)
                     if snapshot.get(path) != current.get(path))
    if changed:
        verifier.fail(f"coverage regeneration build inputs changed: {changed}")


def build_manifest(
    root: Path,
) -> tuple[dict[str, object], dict[str, dict[str, object]], dict[str, str]]:
    """Run all families before constructing one in-memory canonical manifest."""

    input_snapshot = snapshot_build_inputs(root)
    adapters = verifier.task_adapter_rows(
        root / verifier.DEFAULT_ADAPTERS,
        root / verifier.DEFAULT_RUNTIME_EVIDENCE,
    )
    specs = tuple(
        sorted(
            (
                spec
                for spec in verifier.evidence_specs(adapters)
                if spec.category == verifier.RUNTIME_TRACE_CATEGORY
            ),
            key=lambda spec: spec.family,
        )
    )
    if tuple(spec.family for spec in specs) != verifier.COVERAGE_FAMILY_ORDER:
        verifier.fail("coverage regeneration family order/denominator is not exact")
    obligations = canonical_requirements(root)
    validate_canonical_source_requirements(root, obligations)
    known_paths = tuple(
        dict.fromkeys(
            path
            for family in verifier.COVERAGE_FAMILY_ORDER
            for _, path, _, _ in obligations[family]
        )
    )
    reports: dict[str, dict[str, object]] = {}
    report_hashes: dict[str, str] = {}
    for spec in specs:
        require_unchanged_build_inputs(root, input_snapshot)
        report, report_hash = fresh_report(root, spec)
        require_unchanged_build_inputs(root, input_snapshot)
        reports[spec.family] = report
        report_hashes[spec.family] = report_hash
    families: list[dict[str, object]] = []
    for spec in specs:
        functions = reports[spec.family]["data"][0]["functions"]
        if not isinstance(functions, list):
            verifier.fail("coverage regeneration report functions are missing")
        requirements: list[dict[str, object]] = []
        for requirement_id, obligation in zip(
            verifier.COVERAGE_REQUIREMENT_IDS,
            obligations[spec.family],
            strict=True,
        ):
            kind, source_path, source_function, anchor_id = obligation
            requirements.append(
                {
                    "requirement_id": requirement_id,
                    "requirement_kind": kind,
                    "source_path": source_path.as_posix(),
                    "source_sha256": input_snapshot[source_path.as_posix()],
                    "source_function": source_function,
                    "compiler_function_name": exact_compiler_name(
                        root,
                        functions,
                        known_paths,
                        source_path,
                        source_function,
                    ),
                    "anchor_id": anchor_id,
                    "minimum_count": 1,
                }
            )
        families.append(
            {
                "family": spec.family,
                "test_target": spec.test_target,
                "test_id": spec.test_id,
                "profile": spec.profile,
                "trace_version": spec.version,
                "trace_evidence_id": spec.evidence_id,
                "coverage_version": spec.coverage_version,
                "requirements": requirements,
            }
        )
    document: dict[str, object] = {
        "schema_version": verifier.COVERAGE_VERSION,
        "families": families,
    }
    require_unchanged_build_inputs(root, input_snapshot)
    return document, reports, report_hashes


def family_specs(
    document: dict[str, object]
) -> tuple[verifier.CoverageFamilySpec, ...]:
    """Convert an in-memory document through the production loader schema."""

    families = document["families"]
    if not isinstance(families, list):
        verifier.fail("regenerated coverage family array is missing")
    result: list[verifier.CoverageFamilySpec] = []
    for family in families:
        if not isinstance(family, dict):
            verifier.fail("regenerated coverage family is malformed")
        requirements = tuple(
            verifier.CoverageRequirement(
                requirement_id=row["requirement_id"],
                requirement_kind=row["requirement_kind"],
                source_path=Path(row["source_path"]),
                source_sha256=row["source_sha256"],
                source_function=row["source_function"],
                compiler_function_name=row["compiler_function_name"],
                anchor_id=row["anchor_id"],
                minimum_count=row["minimum_count"],
            )
            for row in family["requirements"]
        )
        result.append(
            verifier.CoverageFamilySpec(
                family=family["family"],
                test_target=family["test_target"],
                test_id=family["test_id"],
                profile=family["profile"],
                trace_version=family["trace_version"],
                trace_evidence_id=family["trace_evidence_id"],
                coverage_version=family["coverage_version"],
                requirements=requirements,
            )
        )
    return tuple(result)


def main() -> int:
    """Generate all reports, validate all 7x15 rows, then optionally write once."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root", type=Path, default=Path(__file__).resolve().parent.parent
    )
    parser.add_argument("--write", action="store_true")
    arguments = parser.parse_args()
    root = arguments.root.resolve()
    try:
        input_snapshot = snapshot_build_inputs(root)
        document, reports, report_hashes = build_manifest(root)
        specs = family_specs(document)
        verifier.validate_coverage_manifest(root, specs)
        adapter_specs = tuple(
            sorted(
                (
                    spec
                    for spec in verifier.evidence_specs(
                        verifier.task_adapter_rows(
                            root / verifier.DEFAULT_ADAPTERS,
                            root / verifier.DEFAULT_RUNTIME_EVIDENCE,
                        )
                    )
                    if spec.category == verifier.RUNTIME_TRACE_CATEGORY
                ),
                key=lambda spec: spec.family,
            )
        )
        for spec in adapter_specs:
            verifier.validate_compiler_coverage(root, spec, specs, reports[spec.family])
        payload = (
            json.dumps(document, ensure_ascii=False, indent=2).encode("utf-8") + b"\n"
        )
        require_unchanged_build_inputs(root, input_snapshot)
        if arguments.write:
            (root / verifier.DEFAULT_COVERAGE_MANIFEST).write_bytes(payload)
    except (OSError, UnicodeError, verifier.VerificationError) as error:
        print(f"TASK6_COVERAGE_REGENERATION_FAIL={error}")
        return 1
    print("TASK6_COVERAGE_REGENERATION_FAMILIES=7")
    print("TASK6_COVERAGE_REGENERATION_IDENTITIES=105")
    print(f"TASK6_COVERAGE_MANIFEST_SHA256={hashlib.sha256(payload).hexdigest()}")
    for family in verifier.COVERAGE_FAMILY_ORDER:
        print(f"TASK6_COVERAGE_REPORT_{family}_SHA256={report_hashes[family]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
