"""Measure repeated saturated Metrics shutdown and enforce release budgets."""

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
SUMMARY = re.compile(
    r"^metrics_saturation_shutdown coalesced=(\d+) shutdown_us=(\d+) "
    r"active_tasks=(\d+) unflushed=(\d+)$",
    re.MULTILINE,
)
TEST_NAME = "saturated_runtime_shutdown_drains_and_joins_every_owned_task"


def test_executable(build_log: Path) -> Path:
    """Locate the queue-semantics integration-test executable in Cargo JSON."""
    matches = []
    for line in build_log.read_text(encoding="utf-8").splitlines():
        if not line.startswith("{"):
            continue
        row = json.loads(line)
        target = row.get("target", {})
        if (row.get("reason") == "compiler-artifact"
                and target.get("name") == "queue_semantics_test"
                and "test" in target.get("kind", [])
                and row.get("executable")):
            matches.append(Path(row["executable"]))
    if len(matches) != 1 or not matches[0].is_file():
        raise ValueError("expected exactly one built queue_semantics_test executable")
    return matches[0]


def parse_summary(output: str) -> dict[str, int]:
    """Require one actual saturation result with no owned work left behind."""
    matches = SUMMARY.findall(output)
    if len(matches) != 1:
        raise ValueError("expected exactly one metrics_saturation_shutdown summary")
    coalesced, shutdown_us, active_tasks, unflushed = map(int, matches[0])
    if coalesced == 0:
        raise ValueError("saturated Metrics run had no coalesced snapshots")
    if active_tasks or unflushed:
        raise ValueError("saturated Metrics run retained owned work")
    return {
        "coalesced": coalesced,
        "shutdown_us": shutdown_us,
        "active_tasks": active_tasks,
        "unflushed": unflushed,
    }


def summarize_runs(runs: list[dict[str, int]], peak_rss_kib: int) -> dict:
    """Calculate nearest-rank shutdown quantiles from successful executions."""
    if not runs or peak_rss_kib <= 0:
        raise ValueError("successful runs and positive peak RSS are required")
    values = sorted(run["shutdown_us"] for run in runs)

    def quantile(fraction: float) -> int:
        return values[math.ceil(fraction * len(values)) - 1]

    return {
        "schema": "druid-metrics-saturation-v1",
        "commit": os.environ.get("GITHUB_SHA"),
        "platform": platform.platform(),
        "runs": runs,
        "run_count": len(runs),
        "coalesced_min": min(run["coalesced"] for run in runs),
        "p50_shutdown_us": quantile(0.50),
        "p95_shutdown_us": quantile(0.95),
        "p99_shutdown_us": quantile(0.99),
        "peak_rss_kib": peak_rss_kib,
    }


def write_evidence(measurement: dict, limits: dict[str, int], output: Path) -> None:
    """Persist evidence before rejecting any exceeded limit."""
    violations = [
        f"{name}={measurement[name]} exceeds {limit}"
        for name, limit in limits.items()
        if measurement[name] > limit
    ]
    measurement["budget"] = {"limits": limits, "violations": violations}
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(measurement, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(measurement, sort_keys=True))
    if violations:
        raise RuntimeError("Metrics saturation budget exceeded: " + "; ".join(violations))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build-log", type=Path, required=True)
    parser.add_argument("--run-log", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repetitions", type=int, default=20)
    parser.add_argument("--max-p99-shutdown-us", type=int, required=True)
    parser.add_argument("--max-peak-rss-kib", type=int, required=True)
    args = parser.parse_args()
    if not sys.platform.startswith("linux"):
        parser.error("peak RSS measurement currently requires Linux")
    if args.repetitions < 2 or args.max_p99_shutdown_us <= 0 or args.max_peak_rss_kib <= 0:
        parser.error("repetitions must be at least two and budgets must be positive")

    import resource  # Linux ru_maxrss is measured in KiB.

    executable = test_executable(args.build_log)
    args.run_log.parent.mkdir(parents=True, exist_ok=True)
    runs = []
    try:
        with args.run_log.open("w", encoding="utf-8") as log:
            for index in range(args.repetitions):
                result = subprocess.run(
                    [str(executable), "--exact", TEST_NAME, "--nocapture"],
                    cwd=ROOT, text=True, capture_output=True, check=False, timeout=15,
                )
                output = result.stdout + result.stderr
                log.write(f"=== run {index + 1} exit={result.returncode} ===\n{output}\n")
                log.flush()
                if result.returncode != 0:
                    raise RuntimeError(f"Metrics saturation run {index + 1} failed: exit {result.returncode}")
                runs.append(parse_summary(output))
        measurement = summarize_runs(runs, resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss)
        measurement["run_log_sha256"] = hashlib.sha256(args.run_log.read_bytes()).hexdigest()
        write_evidence(measurement, {
            "p99_shutdown_us": args.max_p99_shutdown_us,
            "peak_rss_kib": args.max_peak_rss_kib,
        }, args.output)
    except Exception as error:
        if not args.output.exists():
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(json.dumps({
                "schema": "druid-metrics-saturation-v1", "status": "failed",
                "completed_runs": len(runs), "error": str(error),
            }, indent=2) + "\n", encoding="utf-8")
        raise
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
