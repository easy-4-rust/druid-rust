"""Negative controls for method-level capability evidence requirements."""

import csv
import importlib.util
from pathlib import Path
import tempfile
import unittest

SCRIPT = Path(__file__).resolve().parents[2] / "scripts/verify_java8_method_evidence.py"
SPEC = importlib.util.spec_from_file_location("verify_java8_method_evidence", SCRIPT)
GATE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(GATE)


class MethodEvidenceGateTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.inventory = self.root / "inventory.tsv"
        self.ledger = self.root / "ledger.tsv"
        self.row = {field: "" for field in GATE.FIELDS}
        self.row.update(identity="java.sql.Connection#commit()", owner="java.sql.Connection", name="commit",
                        parameters="", rust_type_entry="druid_rdbc::connection::Connection",
                        rust_method_entry="Connection::commit", spec_scenario="fixture", batch="fixture",
                        backend="fixture", declaration="PASS", contract="PASS", adapter="NOT_RUN",
                        logical_proxy="NOT_RUN", real_db="NOT_RUN", capability="SUPPORTED",
                        positive_evidence="crates/druid-rdbc/tests/standard_event_contract_test.rs#connection_event_owns_the_same_source_after_callers_drop_it",
                        unsupported_evidence="", unsupported_result="", not_run_reason="")
        self.write(self.inventory, [dict(identity=self.row["identity"])], ["identity"])

    def tearDown(self):
        self.temp.cleanup()

    @staticmethod
    def write(path, rows, fields):
        with path.open("w", encoding="utf-8", newline="") as target:
            writer = csv.DictWriter(target, fieldnames=fields, delimiter="\t", lineterminator="\n")
            writer.writeheader()
            writer.writerows(rows)

    def check(self):
        return GATE.verify(self.ledger, self.inventory)

    def test_supported_capability_without_positive_evidence_fails_closed(self):
        self.row["positive_evidence"] = ""
        self.write(self.ledger, [self.row], list(GATE.FIELDS))
        with self.assertRaisesRegex(GATE.EvidenceError, "lacks a positive test/evidence"):
            self.check()

    def test_unsupported_capability_without_exact_error_evidence_fails_closed(self):
        self.row.update(capability="UNSUPPORTED", positive_evidence="", unsupported_evidence="",
                        unsupported_result="0A000", not_run_reason="")
        self.write(self.ledger, [self.row], list(GATE.FIELDS))
        with self.assertRaisesRegex(GATE.EvidenceError, "lacks precise negative evidence"):
            self.check()

    def test_unsupported_capability_with_sqlstate_only_is_not_precise_enough(self):
        self.row.update(capability="UNSUPPORTED", positive_evidence="",
                        unsupported_evidence="crates/druid-rdbc/tests/standard_event_contract_test.rs#connection_event_owns_the_same_source_after_callers_drop_it",
                        unsupported_result="0A000", not_run_reason="")
        self.write(self.ledger, [self.row], list(GATE.FIELDS))
        with self.assertRaisesRegex(GATE.EvidenceError, "exact observable error"):
            self.check()

    def test_inventory_denominator_and_explicit_unassessed_rows_pass(self):
        self.row.update(capability="UNASSESSED", positive_evidence="", unsupported_evidence="",
                        unsupported_result="", rust_method_entry="UNMAPPED", not_run_reason="Not assessed")
        self.write(self.ledger, [self.row], list(GATE.FIELDS))
        self.assertEqual({"inventory": 1, "ledger": 1, "linked": 0}, self.check())


if __name__ == "__main__":
    unittest.main()
