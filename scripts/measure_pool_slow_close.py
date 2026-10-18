"""Measure repeated bounded physical-close faults and retain release evidence."""

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
TEST_NAME = "slow_physical_close_has_bounded_queue_and_shutdown_joins_worker"
SUMMARY = re.compile(
    r"^pool_slow_close_budget queue_capacity=(\d+) queue_peak=(\d+) "
    r"queue_remaining=(\d+) rejected=(\d+) timeouts=(\d+) "
    r"physical_closed=(\d+) storm_us=(\d+) shutdown_us=(\d+) "
    r"worker_terminated=(true|false)$",
    re.MULTILINE,
)


def test_executable(build_log: Path) -> Path:
    """Locate the one Cargo-built Pool close integration-test executable."""
    matches = []
    for line in build_log.read_text(encoding="utf-8").splitlines():
        if not line.startswith("{"):
            continue
        row = json.loads(line)
        target = row.get("target", {})
        if (row.get("reason") == "compiler-artifact"
                and target.get("name") == "pool_close_backpressure_test"
                and "test" in target.get("kind", [])
                and row.get("executable")):
            matches.append(Path(row["executable"]))
    if len(matches) != 1 or not matches[0].is_file():
        raise ValueError("expected one built pool_close_backpressure_test executable")
    return matches[0]


def parse_summary(output: str) -> dict[str, int]:
    """Reject missing, duplicate, unsaturated or unjoined fault observations."""
    matches = SUMMARY.findall(output)
    if len(matches) != 1:
        raise ValueError("expected exactly one pool_slow_close_budget summary")
    values = list(map(int, matches[0][:-1]))
    result = dict(zip((
        "queue_capacity", "queue_peak", "queue_remaining", "rejected",
        "timeouts", "physical_closed", "storm_us", "shutdown_us",
    ), values, strict=True))
    result["worker_terminated"] = matches[0][-1] == "true"
    if result["queue_capacity"] <= 0 or result["queue_peak"] <= 0:
        raise ValueError("close queue capacity and observed peak must be positive")
    if result["queue_peak"] > result["queue_capacity"]:
        raise ValueError("close queue exceeded configured capacity")
    if result["rejected"] <= 0:
        raise ValueError("fault injection did not saturate the close queue")
    if result["queue_remaining"] or not result["worker_terminated"]:
        raise ValueError("close worker retained owned work after shutdown")
    if result["physical_closed"] <= 0 or result["storm_us"] <= 0 or result["shutdown_us"] <= 0:
        raise ValueError("fault run is missing physical close or elapsed time")
    return result


def summarize_runs(runs: list[dict[str, int]], peak_rss_kib: int) -> dict:
    """Calculate nearest-rank latency quantiles from successful real runs."""
    if not runs or peak_rss_kib <= 0:
        raise ValueError("successful runs and positive peak RSS are required")

    def quantile(field: str, fraction: float) -> int:
        values = sorted(run[field] for run in runs)
        return values[math.ceil(fraction * len(values)) - 1]

    return {
        "schema": "druid-pool-slow-close-v1",
        "commit": os.environ.get("GITHUB_SHA"),
        "platform": platform.platform(),
        "runs": runs,
        "run_count": len(runs),
        "queue_capacity": runs[0]["queue_capacity"],
        "queue_peak_max": max(run["queue_peak"] for run in runs),
        "rejected_min": min(run["rejected"] for run in runs),
        "timeouts_max": max(run["timeouts"] for run in runs),
        "physical_closed_min": min(run["physical_closed"] for run in runs),
        "p50_storm_us": quantile("storm_us", 0.50),
        "p95_storm_us": quantile("storm_us", 0.95),
        "p99_storm_us": quantile("storm_us", 0.99),
        "p50_shutdown_us": quantile("shutdown_us", 0.50),
        "p95_shutdown_us": quantile("shutdown_us", 0.95),
        "p99_shutdown_us": quantile("shutdown_us", 0.99),
        "peak_rss_kib": peak_rss_kib,
    }


def write_evidence(measurement: dict, limits: dict[str, int], output: Path) -> None:
    """Persist the observation before rejecting any accepted numeric ceiling."""
    violations = [
        f"{name}={measurement[name]} exceeds {limit}"
        for name, limit in limits.items()
        if measurement[name] > limit
    ]
    measurement["budget"] = {
        "status": "enforced" if limits else "baseline_only",
        "limits": limits,
        "violations": violations,
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(measurement, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(measurement, sort_keys=True))
    if violations:
        raise RuntimeError("Pool slow-close budget exceeded: " + "; ".join(violations))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build-log", type=Path, required=True)
    parser.add_argument("--run-log", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repetitions", type=int, default=20)
    parser.add_argument("--max-p99-storm-us", type=int)
    parser.add_argument("--max-p99-shutdown-us", type=int)
    parser.add_argument("--max-peak-rss-kib", type=int)
    args = parser.parse_args()
    if not sys.platform.startswith("linux"):
        parser.error("peak RSS measurement currently requires Linux")
    if args.repetitions < 2:
        parser.error("repetitions must be at least two")
    limits = {
        name: limit for name, limit in (
            ("p99_storm_us", args.max_p99_storm_us),
            ("p99_shutdown_us", args.max_p99_shutdown_us),
            ("peak_rss_kib", args.max_peak_rss_kib),
        ) if limit is not None
    }
    if any(limit <= 0 for limit in limits.values()):
        parser.error("numeric budgets must be positive")
    if limits and len(limits) != 3:
        parser.error("accepted Pool budgets require storm, shutdown, and RSS limits together")

    import resource  # Linux ru_maxrss is reported in KiB.

    runs = []
    args.run_log.parent.mkdir(parents=True, exist_ok=True)
    try:
        executable = test_executable(args.build_log)
        with args.run_log.open("w", encoding="utf-8") as log:
            for index in range(args.repetitions):
                try:
                    result = subprocess.run(
                        [str(executable), "--exact", TEST_NAME, "--nocapture"],
                        cwd=ROOT, text=True, capture_output=True, check=False, timeout=20,
                    )
                except subprocess.TimeoutExpired as error:
                    stdout = error.stdout.decode(errors="replace") if isinstance(error.stdout, bytes) else error.stdout or ""
                    stderr = error.stderr.decode(errors="replace") if isinstance(error.stderr, bytes) else error.stderr or ""
                    log.write(f"=== run {index + 1} timeout ===\n{stdout}{stderr}\n")
                    log.flush()
                    raise
                output = result.stdout + result.stderr
                log.write(f"=== run {index + 1} exit={result.returncode} ===\n{output}\n")
                log.flush()
                if result.returncode != 0:
                    raise RuntimeError(f"Pool slow-close run {index + 1} exited {result.returncode}")
                run = parse_summary(output)
                if runs and run["queue_capacity"] != runs[0]["queue_capacity"]:
                    raise ValueError("close queue capacity changed between runs")
                runs.append(run)
        measurement = summarize_runs(runs, resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss)
        measurement["run_log_sha256"] = hashlib.sha256(args.run_log.read_bytes()).hexdigest()
        write_evidence(measurement, limits, args.output)
    except Exception as error:
        if not args.output.exists():
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(json.dumps({
                "schema": "druid-pool-slow-close-v1", "status": "failed",
                "completed_runs": len(runs), "error": str(error),
                "run_log_sha256": hashlib.sha256(args.run_log.read_bytes()).hexdigest()
                if args.run_log.exists() else None,
            }, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        raise
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
