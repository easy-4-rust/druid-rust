"""Contract checks for the runtime benchmark evidence parser."""

import json
from pathlib import Path
import tempfile
import unittest

from measure_runtime_benchmark import (
    benchmark_executable,
    benchmark_summary,
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


if __name__ == "__main__":
    unittest.main()
