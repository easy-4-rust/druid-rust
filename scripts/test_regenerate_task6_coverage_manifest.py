#!/usr/bin/env python3
"""Adversarial tests for atomic Task 6 coverage-manifest regeneration."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

import verify_wrapper_standard_impls as verifier
import regenerate_task6_coverage_manifest as regeneration
from regenerate_task6_coverage_manifest import (
    exact_compiler_name,
    validate_source_function_unique,
)


def compiler_function(name: str) -> dict[str, object]:
    return {
        "name": name,
        "count": 1,
        "filenames": ["/temporary/repository/src/family.rs"],
        "regions": [[1, 1, 1, 16, 1, 0, 0, 0]],
        "branches": [],
        "mcdc_records": [],
    }


class CoverageRegenerationTest(unittest.TestCase):
    def test_unchanged_inputs_allow_all_seven_families_and_frozen_hashes(self) -> None:
        """一致输入可生成完整七family清单，不能用一律拒绝替代漂移检查。"""

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "src").mkdir()
            (root / "src/family.rs").write_bytes(b"fn anchor() {}\r\n")
            frozen = regeneration.snapshot_build_inputs(root)
            canonical_hash = verifier.canonical_source_sha256(
                (root / "src/family.rs").read_bytes()
            )
            self.assertNotEqual(frozen["src/family.rs"], canonical_hash)
            specs = tuple(SimpleNamespace(
                family=family, category=verifier.RUNTIME_TRACE_CATEGORY,
                test_target="lib", test_id="anchor", profile="all-features",
                version=verifier.TRACE_VERSION, evidence_id="fixture",
                coverage_version=verifier.COVERAGE_VERSION,
            ) for family in verifier.COVERAGE_FAMILY_ORDER)
            obligations = {family: tuple(
                ("TEST_IDENTITY_FUNCTION", Path("src/family.rs"), "anchor", "fixture")
                for _ in verifier.COVERAGE_REQUIREMENT_IDS
            ) for family in verifier.COVERAGE_FAMILY_ORDER}
            with patch.object(verifier, "task_adapter_rows", return_value=()), \
                 patch.object(verifier, "evidence_specs", return_value=specs), \
                 patch.object(regeneration, "canonical_requirements", return_value=obligations), \
                 patch.object(regeneration, "validate_canonical_source_requirements"), \
                 patch.object(regeneration, "exact_compiler_name", return_value="_RNv6anchor"), \
                 patch.object(regeneration, "fresh_report", return_value=(
                     {"data": [{"functions": []}]}, "0" * 64
                 )) as run:
                document, reports, hashes = regeneration.build_manifest(
                    root, timeout_seconds=1800.0
                )
            self.assertEqual(run.call_count, 7)
            self.assertTrue(all(call.kwargs == {"timeout_seconds": 1800.0} for call in run.call_args_list))
            self.assertEqual(tuple(reports), verifier.COVERAGE_FAMILY_ORDER)
            self.assertEqual(tuple(hashes), verifier.COVERAGE_FAMILY_ORDER)
            rows = [row for family in document["families"] for row in family["requirements"]]
            self.assertEqual(len(rows), 105)
            self.assertTrue(all(row["source_sha256"] == canonical_hash for row in rows))

    def test_fresh_report_passes_explicit_bounded_timeout_to_runner(self) -> None:
        spec = verifier.EvidenceSpec(
            test_target="test:duckdb_connection_adapter_test",
            test_id="duckdb_standard_factory_emits_typed_runtime_trace",
            profile="all-features",
            category=verifier.RUNTIME_TRACE_CATEGORY,
            family="DUCKDB",
            version=verifier.TRACE_VERSION,
            evidence_id="fixture",
        )
        with tempfile.TemporaryDirectory() as temporary, \
             patch.object(verifier, "verify_evidence_source_closure"), \
             patch.object(verifier, "run_bounded_process", side_effect=RuntimeError("stop")) as run:
            with self.assertRaisesRegex(RuntimeError, "stop"):
                regeneration.fresh_report(Path(temporary), spec, timeout_seconds=1800.0)
        self.assertEqual(run.call_args.kwargs["timeout_seconds"], 1800.0)

    def test_drift_during_final_validation_preserves_existing_manifest(self) -> None:
        """全部报告返回后仍须检查输入，验证期间变化不能覆盖正式清单。"""

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "input.rs"
            source.write_text("fn anchor() {}\n", encoding="utf-8")
            manifest = root / verifier.DEFAULT_COVERAGE_MANIFEST
            manifest.parent.mkdir(parents=True)
            manifest.write_bytes(b"original-manifest\n")
            def drift(*_):
                source.write_text("fn changed() {}\n", encoding="utf-8")
            with patch("sys.argv", ["regenerate", "--root", str(root), "--write"]), \
                 patch.object(regeneration, "build_manifest", return_value=({"families": []}, {}, {})), \
                 patch.object(regeneration, "family_specs", return_value=()), \
                 patch.object(verifier, "validate_coverage_manifest", side_effect=drift), \
                 patch.object(verifier, "task_adapter_rows", return_value=()), \
                 patch.object(verifier, "evidence_specs", return_value=()), \
                 patch("builtins.print") as output:
                self.assertEqual(regeneration.main(), 1)
            self.assertIn("build inputs changed", str(output.call_args))
            self.assertEqual(manifest.read_bytes(), b"original-manifest\n")

    def test_input_drift_after_one_family_cannot_produce_a_manifest(self) -> None:
        """第一family之后的源码/依赖变化必须停止，不能拼接不同输入的报告。"""

        for mutation in ("source", "removed", "new-source", "lock"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                (root / "src").mkdir()
                source = root / "src/family.rs"
                source.write_text("fn anchor() {}\n", encoding="utf-8")
                (root / "Cargo.lock").write_text("version = 4\n", encoding="utf-8")
                specs = tuple(SimpleNamespace(
                    family=family, category=verifier.RUNTIME_TRACE_CATEGORY,
                    test_target="lib", test_id="anchor", profile="all-features",
                    version=verifier.TRACE_VERSION, evidence_id="fixture",
                    coverage_version=verifier.COVERAGE_VERSION,
                ) for family in verifier.COVERAGE_FAMILY_ORDER)
                obligations = {family: tuple(
                    ("TEST_IDENTITY_FUNCTION", Path("src/family.rs"), "anchor", "fixture")
                    for _ in verifier.COVERAGE_REQUIREMENT_IDS
                ) for family in verifier.COVERAGE_FAMILY_ORDER}

                def changed_report(*_, **__):
                    if mutation == "source":
                        source.write_text("fn anchor() { changed(); }\n", encoding="utf-8")
                    elif mutation == "removed":
                        source.unlink()
                    elif mutation == "new-source":
                        (root / "src/new.rs").write_text("fn added() {}\n", encoding="utf-8")
                    else:
                        (root / "Cargo.lock").write_text("version = 3\n", encoding="utf-8")
                    return {"data": [{"functions": []}]}, "0" * 64

                with patch.object(verifier, "task_adapter_rows", return_value=()), \
                     patch.object(verifier, "evidence_specs", return_value=specs), \
                     patch.object(regeneration, "canonical_requirements", return_value=obligations), \
                     patch.object(regeneration, "validate_canonical_source_requirements"), \
                     patch.object(regeneration, "exact_compiler_name", return_value="_RNv6anchor"), \
                     patch.object(regeneration, "fresh_report", side_effect=changed_report) as run:
                    with self.assertRaisesRegex(verifier.VerificationError, "build inputs changed"):
                        regeneration.build_manifest(root)
                    self.assertEqual(run.call_count, 1)

    def test_exact_positive_source_function_selects_one_compiler_identity(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "src").mkdir()
            (root / "src/family.rs").write_text("fn anchor() {}\n", encoding="utf-8")
            name = exact_compiler_name(
                root,
                [compiler_function("_RNv6anchor")],
                (Path("src/family.rs"),),
                Path("src/family.rs"),
                "anchor",
            )
            self.assertEqual(name, "_RNv6anchor")

    def test_async_nested_coroutine_is_not_a_top_level_test_identity(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "src").mkdir()
            (root / "src/family.rs").write_text("fn anchor() {}\n", encoding="utf-8")
            top_level = "_RNvCsExample_6anchor"
            nested = "_RNCNvCsExample_6anchor0B3_"
            name = exact_compiler_name(
                root,
                [compiler_function(nested), compiler_function(top_level)],
                (Path("src/family.rs"),),
                Path("src/family.rs"),
                "anchor",
            )
            self.assertEqual(name, top_level)

    def test_duplicate_or_zero_identity_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "src").mkdir()
            (root / "src/family.rs").write_text("fn anchor() {}\n", encoding="utf-8")
            functions = [
                compiler_function("_RNvCsFirst_6anchor"),
                compiler_function("_RNvCsSecond_6anchor"),
            ]
            with self.assertRaisesRegex(verifier.VerificationError, "not unique"):
                exact_compiler_name(
                    root,
                    functions,
                    (Path("src/family.rs"),),
                    Path("src/family.rs"),
                    "anchor",
                )

    def test_missing_or_ambiguous_obligation_source_fails_before_runtime(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "src").mkdir()
            source_path = Path("src/family.rs")
            (root / source_path).write_text("fn anchor() {}\n", encoding="utf-8")
            validate_source_function_unique(root, source_path, "anchor", "TEST:ANCHOR")
            with self.assertRaisesRegex(verifier.VerificationError, "preflight"):
                validate_source_function_unique(
                    root, source_path, "missing", "TEST:MISSING"
                )


if __name__ == "__main__":
    unittest.main()
