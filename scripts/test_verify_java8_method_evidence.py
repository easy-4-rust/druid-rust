"""Mutation checks for precise method evidence and resolved OpenSpec requirements."""
from pathlib import Path
import contextlib
import csv
import io
import shutil
import tempfile
import unittest
from unittest.mock import patch

import verify_java8_method_evidence as verifier


class Java8EvidenceTest(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.directory = Path("docs/verification/rdbc-first")
        self.ledger = self.root / self.directory / "java8-method-evidence.tsv"
        self.inventory = self.root / self.directory / "java8-public-method-inventory.tsv"
        original_rows = verifier.read_rows(verifier.ROOT / self.directory / self.ledger.name)
        files = {str(self.directory / self.ledger.name), str(self.directory / self.inventory.name)}
        for row in original_rows:
            for key in ("rust_source", "positive_evidence", "unsupported_evidence"):
                if row[key]:
                    files.add(row[key].partition("#")[0])
            if row["capability"] != "UNASSESSED":
                files.add("openspec/specs/" + row["spec_scenario"].partition("#")[0])
        for file in files:
            destination = self.root / file
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(verifier.ROOT / file, destination)
        self.addCleanup(patch.stopall)
        patch.object(verifier, "ROOT", self.root).start()

    def change_linked(self, **changes):
        lines = self.ledger.read_text(encoding="utf-8").splitlines()
        reader = csv.DictReader(lines[1:], delimiter="\t")
        rows = list(reader)
        row = next(row for row in rows if row["capability"] == "PARTIAL")
        row.update(changes)
        output = io.StringIO()
        output.write(lines[0] + "\n")
        writer = csv.DictWriter(output, fieldnames=reader.fieldnames, delimiter="\t", lineterminator="\n")
        writer.writeheader()
        writer.writerows(rows)
        self.ledger.write_text(output.getvalue(), encoding="utf-8")

    def verify(self):
        return verifier.verify(self.ledger, self.inventory)

    def test_actual_ledger_keeps_partial_evidence_and_full_denominator(self):
        result = self.verify()
        self.assertEqual((result["inventory"], result["ledger"], result["linked"], result["unassessed"]), (2001, 2001, 17, 1984))
        self.assertEqual(result["incomplete"], 2001)

    def test_missing_specification_is_rejected(self):
        self.change_linked(spec_scenario="missing/spec.md#DN-01")
        with self.assertRaisesRegex(verifier.EvidenceError, "OpenSpec requirement"):
            self.verify()

    def test_unknown_requirement_is_rejected(self):
        self.change_linked(spec_scenario="rdbc-driver-negotiation/spec.md#DN-999")
        with self.assertRaisesRegex(verifier.EvidenceError, "OpenSpec requirement"):
            self.verify()

    def test_escaping_specification_is_rejected(self):
        self.change_linked(spec_scenario="../spec.md#DN-01")
        with self.assertRaisesRegex(verifier.EvidenceError, "OpenSpec requirement"):
            self.verify()

    def test_duplicate_requirement_is_rejected(self):
        source = self.root / "openspec/specs/rdbc-driver-negotiation/spec.md"
        with source.open("a", encoding="utf-8") as stream:
            stream.write("\n### Requirement: DN-01 duplicate\n")
        with self.assertRaisesRegex(verifier.EvidenceError, "OpenSpec requirement"):
            self.verify()

    def test_partial_without_positive_evidence_is_rejected(self):
        self.change_linked(positive_evidence="", positive_evidence_sha256="")
        with self.assertRaisesRegex(verifier.EvidenceError, "positive test"):
            self.verify()

    def test_partial_without_method_mapping_is_rejected(self):
        self.change_linked(rust_method_entry="UNMAPPED")
        with self.assertRaisesRegex(verifier.EvidenceError, "Rust method mapping"):
            self.verify()

    def test_partial_without_passed_layer_is_rejected(self):
        self.change_linked(**{layer: "UNASSESSED" for layer in verifier.LAYERS})
        with self.assertRaisesRegex(verifier.EvidenceError, "passed evidence layer"):
            self.verify()

    def test_unassessed_capability_cannot_hide_passed_layers(self):
        self.change_linked(capability="UNASSESSED")
        with self.assertRaisesRegex(verifier.EvidenceError, "unassessed capability"):
            self.verify()

    def test_default_integrity_is_not_a_completion_claim(self):
        for strict, expected in ((False, 0), (True, 1)):
            arguments = ["verify", "--ledger", str(self.ledger), "--inventory", str(self.inventory)]
            if strict:
                arguments.append("--require-complete")
            with patch("sys.argv", arguments), contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(verifier.main(), expected)


if __name__ == "__main__":
    unittest.main()
