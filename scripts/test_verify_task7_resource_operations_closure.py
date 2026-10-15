"""Fail-closed tests for the 108 historical resource capability consumers."""

from pathlib import Path
import tempfile
import unittest

from verify_task7_resource_operations_closure import (
    CLOSURE_HEADER,
    TARGET,
    verify_closure_rows,
    verify_source_tree,
)


class ResourceOperationsClosureTest(unittest.TestCase):
    def setUp(self):
        self.historical = [
            {"source_path": "crates/druid-rdbc/src/resource/array.rs", "occurrence_kind": "TYPE", "symbol": f"RdbcResourceCapabilities@token={index}", "old_path": "druid_rdbc::resource::RdbcResourceCapabilities", "target_path": "druid::core::resource_runtime_capabilities", "target_task": "P04_TASK_5_DRUID_INTERNALIZATION", "state": "TASK7_RESOURCE_OPERATIONS_PENDING"}
            for index in range(108)
        ]
        self.closure = [tuple(row[key] for key in CLOSURE_HEADER[:-1]) + (TARGET,) for row in self.historical]

    def test_exact_historical_multiset_closes(self):
        self.assertEqual(verify_closure_rows(self.historical, self.closure), 108)

    def test_omitted_duplicate_and_fabricated_rows_fail(self):
        for mutated in (
            self.closure[:-1],
            self.closure[:-1] + [self.closure[0]],
            self.closure[:-1] + [(*self.closure[-1][:-1], "druid::core::resource_runtime_capabilities")],
        ):
            with self.subTest(mutated=mutated[-1]):
                with self.assertRaises(ValueError):
                    verify_closure_rows(self.historical, mutated)

    def test_retired_production_symbol_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            resource = root / "crates/druid-rdbc/src/resource"
            resource.mkdir(parents=True)
            (resource / "resource_operations.rs").write_text("pub struct ResourceOperations;\n", encoding="utf-8")
            (resource / "mod.rs").write_text("mod resource_operations;\npub use resource_operations::ResourceOperations;\n", encoding="utf-8")
            verify_source_tree(root)
            (resource / "array.rs").write_text("pub use RdbcResourceCapabilities;\n", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "retired"):
                verify_source_tree(root)


if __name__ == "__main__":
    unittest.main()
