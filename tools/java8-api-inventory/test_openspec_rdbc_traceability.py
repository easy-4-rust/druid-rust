"""Bounds and validates the OpenSpec capability/scenario crosswalk."""

import csv
import importlib.util
from pathlib import Path
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[2] / "scripts/audit_openspec_rdbc_traceability.py"
SPEC = importlib.util.spec_from_file_location("audit_openspec_rdbc_traceability", SCRIPT)
AUDIT = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(AUDIT)


class OpenSpecTraceabilityTest(unittest.TestCase):
    def test_all_scenarios_trace_to_existing_tasks_and_evidence_without_expanding_scope(self):
        tasks = AUDIT.TASKS.read_bytes()
        with tempfile.TemporaryDirectory(prefix="rdbc-traceability-") as temporary:
            output = Path(temporary)
            scenarios, scope_count = AUDIT.audit(output)
            self.assertEqual((96, 6), (scenarios, scope_count))
            with (output / "spec-scenario-traceability.tsv").open(encoding="utf-8") as source:
                trace = list(csv.DictReader(source, delimiter="\t"))
            self.assertEqual(96, len(trace))
            self.assertTrue(all(row["requirement"] and row["tasks"] and row["evidence"] for row in trace))
            with (output / "global-remainder.tsv").open(encoding="utf-8") as source:
                remainder = list(csv.DictReader(source, delimiter="\t"))
            methods = next(row for row in remainder if row["remainder"] == "METHOD_LEVEL_API_COVERAGE")
            root = Path(__file__).resolve().parents[2] / "docs/verification/rdbc-first"
            with (root / "java8-public-method-inventory.tsv").open(encoding="utf-8") as source:
                inventory = csv.DictReader((line for line in source if not line.startswith("#")), delimiter="\t")
                method_ids = {row["identity"] for row in inventory if row["kind"] in {"method", "inherited_method"}}
            with (root / "java8-method-evidence.tsv").open(encoding="utf-8") as source:
                ledger = csv.DictReader((line for line in source if not line.startswith("#")), delimiter="\t")
                unassessed = sum(row["identity"] in method_ids and row["capability"] == "UNASSESSED" for row in ledger)
            self.assertEqual(1573, len(method_ids), "the frozen method denominator must not expand")
            self.assertEqual(f"{unassessed} of 1573 method identities remain unassessed", methods["observed"])
            self.assertTrue(all(row["added_to_tasks"] in {"NO", "NO_NEW_TASKS"} for row in remainder))
        self.assertEqual(tasks, AUDIT.TASKS.read_bytes(), "report generation must not add or edit implementation tasks")


if __name__ == "__main__":
    unittest.main()
