"""Contract checks for the complete microbenchmark evidence gate."""

import json
from pathlib import Path
import tempfile
import unittest

from measure_microbenchmark_matrix import CASES, collect_case_samples, sample_path, write_evidence


class MicrobenchmarkMatrixEvidenceTest(unittest.TestCase):
    def test_fresh_samples_are_required_for_every_case(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for case in CASES["pool"]:
                path = sample_path(root, "pool", case)
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(
                    json.dumps({"iters": [1] * 20, "times": [1000] * 20}),
                    encoding="utf-8",
                )
            previous = {f"pool/{case}": None for case in CASES["pool"]}
            observed = collect_case_samples(root, "pool", previous)
            self.assertEqual(set(observed), set(previous))
            self.assertEqual(observed["pool/single_borrow_return"]["p99_us"], 1.0)
            case = "parallel_16"
            previous[f"pool/{case}"] = sample_path(root, "pool", case).stat().st_mtime_ns
            with self.assertRaisesRegex(ValueError, "fresh Criterion sample"):
                collect_case_samples(root, "pool", previous)

    def test_exceeded_budget_preserves_all_case_evidence(self):
        names = {f"{target}/{case}" for target, cases in CASES.items() for case in cases}
        measurement = {"cases": {name: {"p99_us": 2.0} for name in names}, "peak_rss_kib": 42}
        limits = {name: 3.0 for name in names}
        limits["wall/check_2_safe"] = 1.0
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "matrix.json"
            with self.assertRaisesRegex(RuntimeError, "wall/check_2_safe"):
                write_evidence(measurement, limits, output, 64)
            saved = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual(saved["budget"]["status"], "ENFORCED")
            self.assertEqual(len(saved["cases"]), len(names))
            self.assertEqual(len(saved["budget"]["violations"]), 1)

    def test_missing_case_limit_is_rejected(self):
        names = {f"{target}/{case}" for target, cases in CASES.items() for case in cases}
        measurement = {"cases": {name: {"p99_us": 2.0} for name in names}, "peak_rss_kib": 42}
        limits = {name: 3.0 for name in names}
        limits.pop("adapter/single_borrow")
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, "frozen matrix"):
                write_evidence(measurement, limits, Path(directory) / "matrix.json", 64)

    def test_peak_rss_budget_is_enforced(self):
        names = {f"{target}/{case}" for target, cases in CASES.items() for case in cases}
        measurement = {"cases": {name: {"p99_us": 2.0} for name in names}, "peak_rss_kib": 65}
        limits = {name: 3.0 for name in names}
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "matrix.json"
            with self.assertRaisesRegex(RuntimeError, "peak_rss_kib"):
                write_evidence(measurement, limits, output, 64)
            self.assertTrue(output.is_file())


if __name__ == "__main__":
    unittest.main()
