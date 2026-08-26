#!/usr/bin/env python3
"""Adversarial tests for atomic Task 6 coverage-manifest regeneration."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

import verify_wrapper_standard_impls as verifier
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
