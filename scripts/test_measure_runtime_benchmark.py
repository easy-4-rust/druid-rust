"""Contract checks for the runtime benchmark evidence parser."""

import json
from pathlib import Path
import tempfile
import unittest

from measure_runtime_benchmark import (
    benchmark_executable,
    benchmark_summary,
    runtime_budget_violations,
    sample_percentiles_ms,
)


class RuntimeBenchmarkEvidenceTest(unittest.TestCase):
    def test_selects_runtime_bench_executable(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "build.jsonl"
            path.write_text(
                "\n".join(
                    json.dumps(row)
                    for row in (
                        {"reason": "compiler-artifact", "target": {"name": "pool", "kind": ["bench"]}, "executable": "/tmp/pool"},
                        {"reason": "compiler-artifact", "target": {"name": "runtime", "kind": ["bench"]}, "executable": "/tmp/runtime"},
                    )
                ),
                encoding="utf-8",
            )
            self.assertEqual(benchmark_executable(path), Path("/tmp/runtime"))

    def test_percentiles_use_per_iteration_sample_times(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "sample.json"
            path.write_text(
                json.dumps({"iters": [1, 2, 1, 1], "times": [1_000_000, 4_000_000, 3_000_000, 4_000_000]}),
                encoding="utf-8",
            )
            self.assertEqual(sample_percentiles_ms(path), {"samples": 4, "p50_ms": 2.0, "p95_ms": 4.0, "p99_ms": 4.0})

    def test_missing_runtime_summary_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "runtime_benchmark summary"):
            benchmark_summary("Benchmarking runtime/pool_burst_64")

    def test_runtime_budget_accepts_exact_limits(self):
        measurement = {"p99_ms": 100.0, "peak_rss_kib": 65536, "shutdown_ms": 250,
                       "max_wait_queue_depth": 56, "active_peak": 8}
        limits = dict(measurement)
        self.assertEqual(runtime_budget_violations(measurement, limits), [])

    def test_runtime_budget_reports_every_exceeded_limit(self):
        measurement = {"p99_ms": 101.0, "peak_rss_kib": 65537, "shutdown_ms": 251,
                       "max_wait_queue_depth": 57, "active_peak": 9}
        limits = {"p99_ms": 100.0, "peak_rss_kib": 65536, "shutdown_ms": 250,
                  "max_wait_queue_depth": 56, "active_peak": 8}
        violations = runtime_budget_violations(measurement, limits)
        self.assertEqual(len(violations), 5)
        for metric in limits:
            self.assertTrue(any(metric in violation for violation in violations))


if __name__ == "__main__":
    unittest.main()
