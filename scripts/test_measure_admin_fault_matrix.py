"""Parser contracts for real Admin fault-matrix release evidence."""

import json
from pathlib import Path
import tempfile
import unittest

from measure_admin_fault_matrix import (
    SCENARIOS, parse_limits, summarize_runs, verify_test_result, write_evidence,
)


class AdminFaultMatrixMeasurementTest(unittest.TestCase):
    def test_one_named_test_must_pass(self):
        name = SCENARIOS["duplicate_batch"][1]
        verify_test_result(f"running 1 test\ntest {name} ... ok\n"
                           "test result: ok. 1 passed; 0 failed; 0 ignored;\n", name)
        for output in ("test result: ok. 0 passed; 0 failed;", "test other ... ok\n"
                       "test result: ok. 1 passed; 0 failed;"):
            with self.subTest(output=output), self.assertRaises(ValueError):
                verify_test_result(output, name)

    def test_all_scenarios_are_required_and_percentiles_are_nearest_rank(self):
        runs = {name: [10, 20, 30, 40, 50] for name in SCENARIOS}
        measurement = summarize_runs(runs, 8192)
        self.assertEqual(measurement["latency_scope"],
                         "whole_scenario_including_fixture_setup")
        self.assertEqual(measurement["scenarios"]["duplicate_batch"]["p50_us"], 30)
        self.assertEqual(measurement["scenarios"]["duplicate_batch"]["p99_us"], 50)
        with self.assertRaises(ValueError):
            summarize_runs({"duplicate_batch": [1]}, 8192)

    def test_partial_or_repeated_limits_fail_closed(self):
        with self.assertRaises(ValueError):
            parse_limits(["duplicate_batch=10"])
        with self.assertRaises(ValueError):
            parse_limits(["unknown=10"])
        with self.assertRaises(ValueError):
            parse_limits(["duplicate_batch=10", "duplicate_batch=20"])

    def test_budget_violations_are_saved_before_failure(self):
        measurement = summarize_runs({name: [10, 20] for name in SCENARIOS}, 8192)
        limits = parse_limits([f"{name}=19" for name in SCENARIOS])
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "faults.json"
            with self.assertRaises(RuntimeError):
                write_evidence(measurement, limits, 8191, output)
            saved = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual(len(saved["budget"]["violations"]), len(SCENARIOS) + 1)


if __name__ == "__main__":
    unittest.main()
