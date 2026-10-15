"""Tests for the Metrics saturation release-evidence parser."""

import tempfile
import unittest
from pathlib import Path
import json

from measure_metrics_saturation import parse_summary, summarize_runs, test_executable, write_evidence


class MetricsSaturationEvidenceTest(unittest.TestCase):
    def test_cargo_artifact_locator_requires_one_test_executable(self):
        with tempfile.TemporaryDirectory() as directory:
            executable = Path(directory) / "queue_semantics_test"
            executable.touch()
            build_log = Path(directory) / "build.jsonl"
            artifact = {"reason": "compiler-artifact", "target": {"name": "queue_semantics_test", "kind": ["test"]}, "executable": str(executable)}
            build_log.write_text(json.dumps(artifact) + "\n", encoding="utf-8")
            self.assertEqual(test_executable(build_log), executable)
            build_log.write_text(json.dumps(artifact) + "\n" + json.dumps(artifact) + "\n", encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "exactly one"):
                test_executable(build_log)

    def test_missing_or_duplicate_summary_is_rejected(self):
        line = "metrics_saturation_shutdown coalesced=3 shutdown_us=42 active_tasks=0 unflushed=0"
        with self.assertRaisesRegex(ValueError, "exactly one"):
            parse_summary("unrelated output")
        with self.assertRaisesRegex(ValueError, "exactly one"):
            parse_summary(line + "\n" + line)

    def test_saturated_run_must_drain(self):
        with self.assertRaisesRegex(ValueError, "coalesced"):
            parse_summary("metrics_saturation_shutdown coalesced=0 shutdown_us=42 active_tasks=0 unflushed=0")
        with self.assertRaisesRegex(ValueError, "owned work"):
            parse_summary("metrics_saturation_shutdown coalesced=3 shutdown_us=42 active_tasks=1 unflushed=0")

    def test_quantiles_and_budget_failure_preserve_json(self):
        runs = [
            parse_summary(f"metrics_saturation_shutdown coalesced=3 shutdown_us={value} active_tasks=0 unflushed=0")
            for value in (10, 20, 30, 40, 50)
        ]
        measurement = summarize_runs(runs, 2048)
        self.assertEqual((measurement["p50_shutdown_us"], measurement["p95_shutdown_us"], measurement["p99_shutdown_us"]), (30, 50, 50))
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "evidence.json"
            with self.assertRaisesRegex(RuntimeError, "budget exceeded"):
                write_evidence(measurement, {"p99_shutdown_us": 49, "peak_rss_kib": 1024}, output)
            self.assertTrue(output.is_file())
            self.assertIn("p99_shutdown_us", output.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
