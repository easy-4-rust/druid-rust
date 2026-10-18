"""Parser contracts for the Pool slow-close release measurement."""

import json
from pathlib import Path
import tempfile
import unittest

from measure_pool_slow_close import parse_summary, summarize_runs, write_evidence


SUMMARY = (
    "pool_slow_close_budget queue_capacity=4 queue_peak=4 queue_remaining=0 "
    "rejected=20 timeouts=0 physical_closed=24 storm_us=12000 "
    "shutdown_us=3000 worker_terminated=true"
)


class PoolSlowCloseMeasurementTest(unittest.TestCase):
    def test_valid_saturated_and_drained_run(self):
        row = parse_summary(SUMMARY)
        self.assertEqual(row["queue_peak"], 4)
        self.assertEqual(row["rejected"], 20)
        self.assertTrue(row["worker_terminated"])

    def test_missing_duplicate_and_unverified_faults_fail_closed(self):
        invalid = [
            "", SUMMARY + "\n" + SUMMARY,
            SUMMARY.replace("rejected=20", "rejected=0"),
            SUMMARY.replace("queue_peak=4", "queue_peak=5"),
            SUMMARY.replace("queue_remaining=0", "queue_remaining=1"),
            SUMMARY.replace("worker_terminated=true", "worker_terminated=false"),
        ]
        for output in invalid:
            with self.subTest(output=output), self.assertRaises(ValueError):
                parse_summary(output)

    def test_nearest_rank_percentiles_and_budget_failures(self):
        runs = []
        for latency in (10, 20, 30, 40, 50):
            runs.append(parse_summary(
                SUMMARY.replace("storm_us=12000", f"storm_us={latency}")
                .replace("shutdown_us=3000", f"shutdown_us={latency * 2}")
            ))
        measurement = summarize_runs(runs, 8192)
        self.assertEqual((measurement["p50_storm_us"], measurement["p95_storm_us"],
                          measurement["p99_storm_us"]), (30, 50, 50))
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "measurement.json"
            with self.assertRaises(RuntimeError):
                write_evidence(measurement, {"p99_storm_us": 49}, output)
            saved = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual(saved["budget"]["violations"], ["p99_storm_us=50 exceeds 49"])


if __name__ == "__main__":
    unittest.main()
