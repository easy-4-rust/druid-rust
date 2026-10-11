"""Run the built runtime benchmark and emit machine-readable release evidence."""

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
    r"runtime_benchmark active_peak=(\d+) max_wait_queue_depth=(\d+) "
    r"waits=(\d+) shutdown_ms=(\d+)"
)


def benchmark_executable(build_log: Path) -> Path:
    """Return the executable Cargo emitted for the runtime bench target."""
    executable = None
    for line in build_log.read_text(encoding="utf-8").splitlines():
        if not line.startswith("{"):
            continue
        row = json.loads(line)
        target = row.get("target", {})
        if (
            row.get("reason") == "compiler-artifact"
            and target.get("name") == "runtime"
            and "bench" in target.get("kind", [])
            and row.get("executable")
        ):
            executable = Path(row["executable"])
    if executable is None:
        raise ValueError("Cargo did not emit the runtime benchmark executable")
    return executable


def sample_percentiles_ms(sample_path: Path) -> dict[str, int | float]:
    """Calculate nearest-rank quantiles from Criterion's per-iteration samples."""
    sample = json.loads(sample_path.read_text(encoding="utf-8"))
    iterations = sample["iters"]
    times = sample["times"]
    if not iterations or len(iterations) != len(times) or any(value <= 0 for value in iterations):
        raise ValueError("Criterion sample must have matching positive iterations and times")
    milliseconds = sorted(time / iteration / 1_000_000 for time, iteration in zip(times, iterations))
    if any(not math.isfinite(value) or value <= 0 for value in milliseconds):
        raise ValueError("Criterion sample contains non-positive or non-finite timings")

    def quantile(fraction: float) -> float:
        return round(milliseconds[math.ceil(fraction * len(milliseconds)) - 1], 3)

    return {
        "samples": len(milliseconds),
        "p50_ms": quantile(0.50),
        "p95_ms": quantile(0.95),
        "p99_ms": quantile(0.99),
    }


def benchmark_summary(output: str) -> dict[str, int]:
    """Require the pool queue and shutdown counters from the real bench run."""
    matches = SUMMARY.findall(output)
    if len(matches) != 1:
        raise ValueError("expected exactly one runtime_benchmark summary")
    active_peak, queue_peak, waits, shutdown_ms = (int(value) for value in matches[0])
    return {
        "active_peak": active_peak,
        "max_wait_queue_depth": queue_peak,
        "waits": waits,
        "shutdown_ms": shutdown_ms,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build-log", type=Path, required=True)
    parser.add_argument("--sample", type=Path, required=True)
    parser.add_argument("--run-log", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if not sys.platform.startswith("linux"):
        parser.error("peak RSS measurement currently requires a Linux runner")

    import resource  # Linux ru_maxrss is measured in KiB.

    executable = benchmark_executable(args.build_log)
    if not executable.is_file():
        raise ValueError(f"runtime benchmark executable is missing: {executable}")
    previous_mtime = args.sample.stat().st_mtime_ns if args.sample.exists() else None
    result = subprocess.run(
        [str(executable), "--bench", "--noplot"],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    output = result.stdout + result.stderr
    args.run_log.parent.mkdir(parents=True, exist_ok=True)
    args.run_log.write_text(output, encoding="utf-8")
    if result.returncode != 0:
        raise RuntimeError(f"runtime benchmark exited {result.returncode}; see {args.run_log}")
    if not args.sample.is_file() or (
        previous_mtime is not None and args.sample.stat().st_mtime_ns <= previous_mtime
    ):
        raise ValueError("runtime benchmark did not produce a fresh Criterion sample")

    payload = {
        "schema": "druid-runtime-benchmark-v1",
        "commit": os.environ.get("GITHUB_SHA"),
        "platform": platform.platform(),
        "peak_rss_kib": resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss,
        "sample_sha256": hashlib.sha256(args.sample.read_bytes()).hexdigest(),
        **sample_percentiles_ms(args.sample),
        **benchmark_summary(output),
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(payload, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
