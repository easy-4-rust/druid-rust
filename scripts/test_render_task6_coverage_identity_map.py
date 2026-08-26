#!/usr/bin/env python3
"""Self-tests for the exact Task 6 compiler identity map renderer."""

from __future__ import annotations

import copy
import unittest

from render_task6_coverage_identity_map import (
    HEADER,
    VerificationError,
    migration_rows,
    render_rows,
)


def requirement(family: str, requirement_id: str, source_hash: str) -> dict[str, str]:
    return {
        "family": family,
        "requirement_id": requirement_id,
        "requirement_kind": "STAGE_ANCHOR_FUNCTION",
        "test_target": "test:family",
        "test_id": "family_test",
        "profile": "all-features",
        "trace_version": "task4-runtime-trace-v1",
        "coverage_version": "task4-compiler-coverage-v2",
        "source_path": "src/family.rs",
        "source_sha256": source_hash,
        "source_function": "anchor",
        "compiler_function_name": "_Ranchor",
        "anchor_id": "typed_stage.Anchor",
        "minimum_count": "1",
    }


class CoverageIdentityMapTest(unittest.TestCase):
    def test_hash_or_compiler_change_is_rendered_as_migration(self) -> None:
        base = requirement("SQLX", "STAGE", "a" * 64)
        current = copy.deepcopy(base)
        current["source_sha256"] = "b" * 64
        rows = migration_rows((base,), (current,))
        self.assertEqual(rows[0]["change"], "MIGRATED")
        self.assertEqual(tuple(rows[0]), HEADER)
        payload = render_rows(rows)
        self.assertTrue(payload.endswith(b"\n"))
        self.assertEqual(payload.count(b"\n"), 2)

    def test_reordered_requirement_identity_is_rejected(self) -> None:
        one = requirement("SQLX", "ONE", "a" * 64)
        two = requirement("SQLX", "TWO", "b" * 64)
        with self.assertRaisesRegex(VerificationError, "order or denominator"):
            migration_rows((one, two), (two, one))

    def test_tsv_control_character_is_rejected(self) -> None:
        row = requirement("SQLX", "ONE", "a" * 64)
        result = migration_rows((row,), (row,))
        malformed = dict(result[0])
        malformed["task6_source_path"] = "src/bad\tpath.rs"
        with self.assertRaisesRegex(VerificationError, "delimiter"):
            render_rows((malformed,))


if __name__ == "__main__":
    unittest.main()
