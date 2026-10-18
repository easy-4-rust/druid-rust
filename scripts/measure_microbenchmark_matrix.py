"""Measure the non-runtime Criterion cases and enforce per-case p99 budgets."""

import argparse
import json
import math
import os
from pathlib import Path
import platform
import subprocess
import sys

from measure_adapter_cursor_benchmark import sample_percentiles_us
from measure_runtime_benchmark import ROOT, benchmark_executable


CASES = {
    "pool": ("single_borrow_return", "parallel_16", "parallel_64"),
    "filter_chain": ("empty_build", "5_adapter_build", "20_adapter_build"),
    "wall": ("check_2_safe", "check_all_categories"),
    "adapter": ("single_borrow", "parallel_8_borrow", "parallel_64_borrow"),
}


def sample_path(criterion_root: Path, target: str, case: str) -> Path:
    """Return Criterion's flattened sample path for one named case."""
    return criterion_root / f"{target}_{case}" / "new" / "sample.json"


def collect_case_samples(
    criterion_root: Path, target: str, previous: dict[str, int | None]
) -> dict[str, dict]:
    """Require a fresh, valid Criterion sample for every case in a target."""
    measurements = {}
    for case in CASES[target]:
        name = f"{target}/{case}"
        path = sample_path(criterion_root, target, case)
        if not path.is_file() or (
            previous[name] is not None and path.stat().st_mtime_ns <= previous[name]
        ):
            raise ValueError(f"missing fresh Criterion sample: {name}")
        measurements[name] = sample_percentiles_us(path)
    return measurements


def write_evidence(
    measurement: dict, limits: dict[str, float], output: Path, max_peak_rss_kib: int
) -> None:
    """Persist every observation and violation before returning a failing gate."""
    cases = measurement["cases"]
    expected = {f"{target}/{case}" for target, names in CASES.items() for case in names}
    if set(cases) != expected or set(limits) != expected:
        raise ValueError("microbenchmark cases and p99 limits must match the frozen matrix")
    if any(not math.isfinite(limit) or limit <= 0 for limit in limits.values()) or max_peak_rss_kib <= 0:
        raise ValueError("microbenchmark limits must be finite and positive")
    violations = [
        f"{name} p99_us={cases[name]['p99_us']} exceeds {limit}"
        for name, limit in limits.items()
        if cases[name]["p99_us"] > limit
    ]
    if measurement["peak_rss_kib"] > max_peak_rss_kib:
        violations.append(
            f"peak_rss_kib={measurement['peak_rss_kib']} exceeds {max_peak_rss_kib}"
        )
    measurement["budget"] = {
        "status": "ENFORCED",
        "p99_us_limits": limits,
        "max_peak_rss_kib": max_peak_rss_kib,
        "violations": violations,
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(measurement, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(measurement, sort_keys=True))
    if violations:
        raise RuntimeError("microbenchmark budget exceeded: " + "; ".join(violations))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build-log-dir", type=Path, required=True)
    parser.add_argument("--criterion-root", type=Path, required=True)
    parser.add_argument("--run-log-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--max-p99-us", action="append", default=[], metavar="CASE=LIMIT")
    parser.add_argument("--max-peak-rss-kib", type=int, required=True)
    args = parser.parse_args()
    if not sys.platform.startswith("linux"):
        parser.error("peak RSS measurement currently requires a Linux runner")

    import resource  # Linux ru_maxrss uses KiB.

    limits = {}
    for item in args.max_p99_us:
        try:
            name, raw_limit = item.split("=", 1)
            if name in limits:
                raise ValueError("duplicate case")
            limits[name] = float(raw_limit)
        except ValueError:
            parser.error(f"invalid or duplicate --max-p99-us value: {item}")

    previous = {
        f"{target}/{case}": (
            path.stat().st_mtime_ns if path.is_file() else None
        )
        for target, names in CASES.items()
        for case in names
        for path in [sample_path(args.criterion_root, target, case)]
    }
    cases = {}
    for target in CASES:
        executable = benchmark_executable(args.build_log_dir / f"{target}-build.jsonl", target)
        if not executable.is_file():
            raise ValueError(f"missing {target} benchmark executable: {executable}")
        command = [str(executable), "--bench", "--noplot"]
        if target == "adapter":
            command.append("adapter/(single_borrow|parallel_8_borrow|parallel_64_borrow)$")
        result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True, check=False)
        args.run_log_dir.mkdir(parents=True, exist_ok=True)
        (args.run_log_dir / f"{target}-run.log").write_text(
            result.stdout + result.stderr, encoding="utf-8"
        )
        if result.returncode != 0:
            raise RuntimeError(f"{target} benchmark exited {result.returncode}")
        target_cases = collect_case_samples(args.criterion_root, target, previous)
        for value in target_cases.values():
            value["operations_per_second"] = round(1_000_000 / value["p50_us"], 1)
        cases.update(target_cases)

    measurement = {
        "schema": "druid-microbenchmark-matrix-v1",
        "commit": os.environ.get("GITHUB_SHA"),
        "platform": platform.platform(),
        "cases": cases,
        "peak_rss_kib": resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss,
    }
    write_evidence(measurement, limits, args.output, args.max_peak_rss_kib)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
