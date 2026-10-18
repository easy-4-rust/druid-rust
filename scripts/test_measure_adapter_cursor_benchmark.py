"""Failure-boundary checks for the real SQLite cursor benchmark collector."""

from contextlib import redirect_stdout
from io import StringIO
import json
from pathlib import Path
import tempfile
import unittest

from measure_adapter_cursor_benchmark import sample_percentiles_us, write_evidence
from measure_runtime_benchmark import benchmark_executable


class AdapterCursorBenchmarkEvidenceTest(unittest.TestCase):
    def test_selects_adapter_executable_without_using_runtime_target(self):
        with tempfile.TemporaryDirectory() as directory:
            build_log = Path(directory) / "build.jsonl"
            build_log.write_text(
                "\n".join(
                    json.dumps(row)
                    for row in (
                        {"reason": "compiler-artifact", "target": {"name": "runtime", "kind": ["bench"]}, "executable": "/tmp/runtime"},
                        {"reason": "compiler-artifact", "target": {"name": "adapter", "kind": ["bench"]}, "executable": "/tmp/adapter"},
                    )
                ),
                encoding="utf-8",
            )
            self.assertEqual(benchmark_executable(build_log, "adapter"), Path("/tmp/adapter"))

    def test_per_iteration_quantiles_and_sample_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            sample = Path(directory) / "sample.json"
            sample.write_text(
                json.dumps({"iters": [1] * 19 + [2], "times": [1000] * 19 + [4000]}),
                encoding="utf-8",
            )
            evidence = sample_percentiles_us(sample)
            self.assertEqual(evidence["samples"], 20)
            self.assertEqual(evidence["p50_us"], 1.0)
            self.assertEqual(evidence["p95_us"], 1.0)
            self.assertEqual(evidence["p99_us"], 2.0)
            self.assertEqual(len(evidence["sample_sha256"]), 64)

    def test_rejects_truncated_or_invalid_samples(self):
        with tempfile.TemporaryDirectory() as directory:
            sample = Path(directory) / "sample.json"
            sample.write_text(json.dumps({"iters": [1] * 19, "times": [1000] * 19}), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "at least 20"):
                sample_percentiles_us(sample)
            sample.write_text(json.dumps({"iters": [1] * 20, "times": [1000] * 19}), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "matching"):
                sample_percentiles_us(sample)

    def test_exceeded_budget_saves_artifact_before_failing(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "cursor.json"
            measurement = {"first_row_p99_us": 251.0, "full_consume_p99_us": 5000.0, "peak_rss_kib": 20000}
            limits = {"first_row_p99_us": 250.0, "full_consume_p99_us": 5000.0, "peak_rss_kib": 65536}
            with redirect_stdout(StringIO()):
                with self.assertRaisesRegex(RuntimeError, "first_row_p99_us"):
                    write_evidence(measurement, limits, output)
            saved = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual(saved["budget"]["status"], "ENFORCED")
            self.assertEqual(len(saved["budget"]["violations"]), 1)

    def test_baseline_is_explicitly_not_a_release_gate(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "cursor.json"
            with redirect_stdout(StringIO()):
                write_evidence({"first_row_p99_us": 251.0}, {}, output)
            saved = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual(saved["budget"], {"status": "BASELINE_ONLY", "limits": {}, "violations": []})


if __name__ == "__main__":
    unittest.main()
