"""Mutation checks: a reduced denominator or falsely promoted evidence must fail."""
from pathlib import Path
import shutil
import tempfile
import unittest

import verify_java_baseline_case_pairs as verifier


class FrozenCasePairsTest(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        source = verifier.ROOT / verifier.DIRECTORY
        destination = self.root / verifier.DIRECTORY
        destination.mkdir(parents=True)
        shutil.copyfile(source / verifier.MANIFEST.name, destination / verifier.MANIFEST.name)
        _, rows = verifier.records(source / verifier.MANIFEST.name)
        for reference in {row["comparison"] for row in rows if row["comparison"] != "-"}:
            target = destination / reference
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source / reference, target)


    def test_actual_evidence_keeps_full_denominator_and_approved_counts(self):
        counts = verifier.verify(self.root)
        self.assertEqual(dict(counts), {"MATCH": 47, "APPROVED_STANDARD_DIFFERENCE": 12})

    def test_removed_case_is_rejected(self):
        path = self.root / verifier.MANIFEST
        lines = path.read_text(encoding="utf-8").splitlines()
        path.write_text("\n".join(lines[:-1]) + "\n", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "59 cases"):
            verifier.verify(self.root)

    def test_falsely_promoted_approved_difference_is_rejected(self):
        path = self.root / verifier.MANIFEST
        path.write_text(path.read_text(encoding="utf-8").replace("\tAPPROVED_STANDARD_DIFFERENCE\t", "\tMATCH\t", 1), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "actual paired output"):
            verifier.verify(self.root)

    def test_duplicate_comparison_identity_is_rejected(self):
        path = self.root / verifier.DIRECTORY / "wall-multi-20261020/comparison.tsv"
        lines = path.read_text(encoding="utf-8").splitlines()
        path.write_text("\n".join(lines + [lines[1]]) + "\n", encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "duplicate comparison"):
            verifier.verify(self.root)

    def test_unpaired_case_cannot_claim_a_comparison(self):
        path = self.root / verifier.MANIFEST
        path.write_text(path.read_text(encoding="utf-8").replace("\tMATCH\t", "\tNOT_PAIRED\t", 1), encoding="utf-8")
        with self.assertRaisesRegex(ValueError, "unpaired case"):
            verifier.verify(self.root)


if __name__ == "__main__":
    unittest.main()
