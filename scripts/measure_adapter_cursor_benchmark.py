"""Measure real SQLx SQLite cursor latency and retain auditable Criterion evidence."""

import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import subprocess
import sys

from measure_runtime_benchmark import ROOT, benchmark_executable


def sample_percentiles_us(sample_path: Path) -> dict[str, int | float]:
    """Use per-iteration timings and nearest-rank quantiles from a real sample."""
    sample = json.loads(sample_path.read_text(encoding="utf-8"))
    iterations = sample["iters"]
    times = sample["times"]
    if len(iterations) < 20 or len(iterations) != len(times):
        raise ValueError("cursor sample requires at least 20 matching observations")
    if any(not isinstance(value, (int, float)) or value <= 0 for value in iterations):
        raise ValueError("cursor sample iterations must be positive")
    microseconds = sorted(time / iteration / 1000 for time, iteration in zip(times, iterations))
    if any(not math.isfinite(value) or value <= 0 for value in microseconds):
        raise ValueError("cursor sample contains non-positive or non-finite timings")

    def quantile(fraction: float) -> float:
        return round(microseconds[math.ceil(fraction * len(microseconds)) - 1], 3)

    return {
        "samples": len(microseconds),
        "p50_us": quantile(0.50),
        "p95_us": quantile(0.95),
        "p99_us": quantile(0.99),
        "sample_sha256": hashlib.sha256(sample_path.read_bytes()).hexdigest(),
    }


def write_evidence(measurement: dict, limits: dict[str, int | float], output: Path) -> None:
    """Save measured results before failing a configured numeric release budget."""
    violations = [
        f"{metric}={measurement[metric]} exceeds {limit}"
        for metric, limit in limits.items()
        if measurement[metric] > limit
    ]
    measurement["budget"] = {
        "status": "ENFORCED" if limits else "BASELINE_ONLY",
        "limits": limits,
        "violations": violations,
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(measurement, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(measurement, sort_keys=True))
    if violations:
        raise RuntimeError("adapter cursor budget exceeded: " + "; ".join(violations))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build-log", type=Path, required=True)
    parser.add_argument("--first-row-sample", type=Path, required=True)
    parser.add_argument("--full-consume-sample", type=Path, required=True)
    parser.add_argument("--run-log", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--max-first-p99-us", type=float)
    parser.add_argument("--max-full-p99-us", type=float)
    parser.add_argument("--max-peak-rss-kib", type=int)
    args = parser.parse_args()
    if not sys.platform.startswith("linux"):
        parser.error("peak RSS measurement currently requires a Linux runner")

    import resource  # Linux ru_maxrss uses KiB.

    executable = benchmark_executable(args.build_log, "adapter")
    if not executable.is_file():
        raise ValueError(f"adapter benchmark executable is missing: {executable}")
    samples = {
        "first_row": args.first_row_sample,
        "full_consume": args.full_consume_sample,
    }
    previous_mtimes = {
        name: path.stat().st_mtime_ns if path.exists() else None
        for name, path in samples.items()
    }
    result = subprocess.run(
        [str(executable), "--bench", "--noplot", "adapter/sqlite_cursor"],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    args.run_log.parent.mkdir(parents=True, exist_ok=True)
    args.run_log.write_text(result.stdout + result.stderr, encoding="utf-8")
    if result.returncode != 0:
        raise RuntimeError(f"adapter cursor benchmark exited {result.returncode}; see {args.run_log}")
    for name, path in samples.items():
        if not path.is_file() or (
            previous_mtimes[name] is not None and path.stat().st_mtime_ns <= previous_mtimes[name]
        ):
            raise ValueError(f"adapter cursor benchmark did not produce a fresh {name} sample")

    first = sample_percentiles_us(args.first_row_sample)
    full = sample_percentiles_us(args.full_consume_sample)
    measurement = {
        "schema": "druid-adapter-cursor-benchmark-v1",
        "commit": os.environ.get("GITHUB_SHA"),
        "platform": platform.platform(),
        "peak_rss_kib": resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss,
        "first_row": first,
        "full_consume": full,
        "full_consume_rows_per_second": round(1_000_000_000 / full["p50_us"], 1),
    }
    limits = {
        metric: value
        for metric, value in {
            "first_row_p99_us": args.max_first_p99_us,
            "full_consume_p99_us": args.max_full_p99_us,
            "peak_rss_kib": args.max_peak_rss_kib,
        }.items()
        if value is not None
    }
    if limits and len(limits) != 3:
        parser.error("cursor budgets must configure all three limits together")
    if any(not math.isfinite(value) or value <= 0 for value in limits.values()):
        parser.error("cursor budgets must be finite positive values")
    measurement["first_row_p99_us"] = first["p99_us"]
    measurement["full_consume_p99_us"] = full["p99_us"]
    write_evidence(measurement, limits, args.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
