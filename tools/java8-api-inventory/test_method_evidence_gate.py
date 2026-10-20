"""Negative controls for method-level capability evidence requirements."""

import csv
import hashlib
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
        self.row.update(identity="javax.sql.PooledConnection#close()", owner="javax.sql.PooledConnection", name="close",
                        parameters="", rust_type_entry="druid_rdbc::connection::Connection",
                        rust_method_entry="PooledConnection::close", spec_scenario="rdbc-pooled-connection-events/spec.md#PE-02", batch="fixture",
                        backend="fixture", declaration="PASS", contract="PASS", adapter="NOT_RUN",
                        logical_proxy="NOT_RUN", real_db="NOT_RUN", capability="SUPPORTED",
                        positive_evidence="crates/druid-rdbc/tests/standard_event_contract_test.rs#connection_event_owns_the_same_source_after_callers_drop_it",
                        positive_evidence_sha256=hashlib.sha256(Path("crates/druid-rdbc/tests/standard_event_contract_test.rs").read_bytes()).hexdigest().upper(),
                        rust_source="crates/druid-rdbc/src/data_source/pooled_connection.rs",
                        rust_source_sha256=hashlib.sha256(Path("crates/druid-rdbc/src/data_source/pooled_connection.rs").read_bytes()).hexdigest().upper(),
                        unsupported_evidence="", unsupported_evidence_sha256="", unsupported_result="", not_run_reason="")
        self.inventory.write_text("# schema=jdbc-java-method-inventory-v1\tsource_commit=fixture-v1\nidentity\n"
                                  + self.row["identity"] + "\n", encoding="utf-8")

    def tearDown(self):
        self.temp.cleanup()

    @staticmethod
    def write(path, rows, fields):
        with path.open("w", encoding="utf-8", newline="") as target:
            writer = csv.DictWriter(target, fieldnames=fields, delimiter="\t", lineterminator="\n")
            writer.writeheader()
            writer.writerows(rows)

    def write_ledger(self):
        header = (f"# schema=java8-method-evidence-v1\tinventory_sha256="
                  f"{hashlib.sha256(self.inventory.read_bytes()).hexdigest().upper()}"
                  "\tsource_schema=jdbc-java-method-inventory-v1\tsource_commit=fixture-v1\n")
        with self.ledger.open("w", encoding="utf-8", newline="") as target:
            target.write(header)
            writer = csv.DictWriter(target, fieldnames=GATE.FIELDS, delimiter="\t", lineterminator="\n")
            writer.writeheader()
            writer.writerow(self.row)

    def check(self):
        return GATE.verify(self.ledger, self.inventory)

    def test_supported_capability_without_positive_evidence_fails_closed(self):
        self.row["positive_evidence"] = ""
        self.row["positive_evidence_sha256"] = ""
        self.write_ledger()
        with self.assertRaisesRegex(GATE.EvidenceError, "lacks a positive test/evidence"):
            self.check()

    def test_complete_supported_claim_passes_with_current_source_hashes(self):
        self.write_ledger()
        self.assertEqual("SUPPORTED", self.row["capability"])
        self.assertEqual(1, self.check()["linked"])

    def test_unsupported_capability_without_exact_error_evidence_fails_closed(self):
        self.row.update(capability="UNSUPPORTED", positive_evidence="", unsupported_evidence="",
                        unsupported_result="0A000", not_run_reason="")
        self.write_ledger()
        with self.assertRaisesRegex(GATE.EvidenceError, "lacks precise negative evidence"):
            self.check()

    def test_unsupported_capability_with_sqlstate_only_is_not_precise_enough(self):
        self.row.update(capability="UNSUPPORTED", positive_evidence="",
                        unsupported_evidence="crates/druid-rdbc/tests/standard_event_contract_test.rs#connection_event_owns_the_same_source_after_callers_drop_it",
                        unsupported_evidence_sha256=hashlib.sha256(Path("crates/druid-rdbc/tests/standard_event_contract_test.rs").read_bytes()).hexdigest().upper(),
                        positive_evidence_sha256="", unsupported_result="0A000", not_run_reason="")
        self.write_ledger()
        with self.assertRaisesRegex(GATE.EvidenceError, "exact observable error"):
            self.check()

    def test_inventory_denominator_and_explicit_unassessed_rows_pass(self):
        self.row.update({layer: "UNASSESSED" for layer in GATE.LAYERS})
        self.row.update(capability="UNASSESSED", positive_evidence="", unsupported_evidence="",
                        unsupported_result="", rust_method_entry="UNMAPPED", not_run_reason="Not assessed")
        self.row.update(positive_evidence_sha256="", unsupported_evidence_sha256="", rust_source="", rust_source_sha256="")
        self.write_ledger()
        result = self.check()
        self.assertEqual((1, 1, 0, 1), (result["inventory"], result["ledger"], result["linked"], result["unassessed"]))

    def test_changed_test_source_hash_invalidates_positive_evidence(self):
        self.row["positive_evidence_sha256"] = "0" * 64
        self.write_ledger()
        with self.assertRaisesRegex(GATE.EvidenceError, "stale evidence source hash"):
            self.check()

    def test_changed_rust_source_hash_invalidates_method_mapping(self):
        self.row["rust_source_sha256"] = "0" * 64
        self.write_ledger()
        with self.assertRaisesRegex(GATE.EvidenceError, "stale or missing Rust source hash"):
            self.check()

    def test_changed_inventory_version_invalidates_ledger(self):
        self.write_ledger()
        with self.inventory.open("a", encoding="utf-8") as target:
            target.write("java.sql.Statement#cancel()\n")
        with self.assertRaisesRegex(GATE.EvidenceError, "inventory hash or source version is stale"):
            self.check()

    def test_not_run_without_reason_is_rejected(self):
        self.row.update(capability="NOT_RUN", not_run_reason="", positive_evidence="",
                        positive_evidence_sha256="", rust_source="", rust_source_sha256="")
        self.write_ledger()
        with self.assertRaisesRegex(GATE.EvidenceError, "NOT_RUN requires a reason"):
            self.check()


if __name__ == "__main__":
    unittest.main()
