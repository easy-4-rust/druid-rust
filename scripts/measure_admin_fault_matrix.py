"""Measure real Admin HTTP/gRPC fault scenarios for release evidence."""

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
import time


ROOT = Path(__file__).resolve().parents[1]
SCENARIOS = {
    "duplicate_batch": (
        "grpc_network_e2e_test", "grpc_duplicate_and_increasing_sequences_are_handled_over_network"
    ),
    "sequence_gap": (
        "grpc_network_e2e_test", "grpc_stream_enforces_identity_full_snapshot_resync_and_control_frames"
    ),
    "restart_pending": (
        "grpc_network_e2e_test",
        "admin_restart_with_pending_incremental_supersedes_it_with_monotonic_full_snapshot",
    ),
    "unavailable_shutdown": (
        "grpc_network_e2e_test", "unavailable_admin_shutdown_reports_exporter_and_client_work_as_unflushed"
    ),
    "remote_auth": (
        "admin_binary_test", "remote_http_and_grpc_use_tls_and_require_authentication"
    ),
    "pending_ack_sigterm": (
        "admin_binary_test", "sigterm_releases_pending_command_http_waiter_before_deadline"
    ),
}
PASS_SUMMARY = re.compile(r"test result: ok\. 1 passed; 0 failed;")
OBSERVATION_FIELDS = {
    "duplicate_batch": {"accepted_sequence": 11, "duplicate_preserved": 1},
    "sequence_gap": {"expected_sequence": 6, "identity_rejected": 1},
    "restart_pending": {"recovered_sequence": 3, "projected_active": 9},
    "unavailable_shutdown": {"unflushed": 1, "shutdown_us": None},
    "remote_auth": {"rejected_credentials": 5, "projected_identity": 901},
    "pending_ack_sigterm": {"pending_released": 1, "shutdown_us": None},
}


def test_executables(build_log: Path) -> dict[str, Path]:
    """Resolve the exact two integration-test executables from Cargo JSON."""
    required = {target for target, _ in SCENARIOS.values()}
    found = {target: [] for target in required}
    for line in build_log.read_text(encoding="utf-8").splitlines():
        if not line.startswith("{"):
            continue
        row = json.loads(line)
        target = row.get("target", {})
        name = target.get("name")
        if (row.get("reason") == "compiler-artifact" and name in required
                and "test" in target.get("kind", []) and row.get("executable")):
            found[name].append(Path(row["executable"]))
    if any(len(paths) != 1 or not paths[0].is_file() for paths in found.values()):
        raise ValueError("expected one built executable for each Admin fault test target")
    return {name: paths[0] for name, paths in found.items()}


def verify_test_result(output: str, test_name: str) -> None:
    """Reject a filtered-out test or a zero-test success before recording time."""
    if (f"test {test_name} ... ok" not in output
            or len(PASS_SUMMARY.findall(output)) != 1):
        raise ValueError(f"Admin fault test did not report one passing execution: {test_name}")


def parse_observation(output: str, scenario: str) -> dict[str, int]:
    """Require one complete, positive real-fault observation after assertions pass."""
    prefix = f"admin_fault_observation scenario={scenario} "
    lines = [line.split(prefix, 1)[1].strip() for line in output.splitlines() if prefix in line]
    if len(lines) != 1:
        raise ValueError(f"expected one Admin fault observation for {scenario}")
    fields = {}
    for item in lines[0].split():
        key, separator, raw_value = item.partition("=")
        if not separator or key in fields or not raw_value.isdecimal():
            raise ValueError(f"malformed Admin fault observation for {scenario}")
        fields[key] = int(raw_value)
    expected = OBSERVATION_FIELDS[scenario]
    if set(fields) != set(expected) or any(
        fields[key] <= 0 if value is None else fields[key] != value
        for key, value in expected.items()
    ):
        raise ValueError(f"incomplete or false Admin fault observation for {scenario}")
    return fields


def percentile(values: list[int], fraction: float) -> int:
    """Return nearest-rank microseconds for one scenario."""
    if not values or any(value <= 0 for value in values):
        raise ValueError("positive scenario measurements are required")
    ordered = sorted(values)
    return ordered[math.ceil(len(ordered) * fraction) - 1]


def summarize_runs(runs: dict[str, list[int]], observations: dict[str, list[dict[str, int]]],
                   peak_rss_kib: int) -> dict:
    """Require the whole matrix and retain every raw scenario duration."""
    if (set(runs) != set(SCENARIOS) or set(observations) != set(SCENARIOS)
            or peak_rss_kib <= 0 or any(
                len(observations[name]) != len(samples) for name, samples in runs.items()
            )):
        raise ValueError("all Admin scenarios and positive child peak RSS are required")
    return {
        "schema": "druid-admin-fault-matrix-v1",
        "commit": os.environ.get("GITHUB_SHA"),
        "platform": platform.platform(),
        "latency_scope": "whole_scenario_including_fixture_setup",
        "peak_rss_scope": "maximum_of_child_processes",
        "peak_rss_kib": peak_rss_kib,
        "scenarios": {
            name: {
                "target": SCENARIOS[name][0],
                "test": SCENARIOS[name][1],
                "runs_us": samples,
                "run_count": len(samples),
                "observations": observations[name],
                "p50_us": percentile(samples, 0.50),
                "p95_us": percentile(samples, 0.95),
                "p99_us": percentile(samples, 0.99),
            }
            for name, samples in runs.items()
        },
    }


def parse_limits(items: list[str]) -> dict[str, int]:
    """Require one positive limit for every scenario when limits are enabled."""
    if not items:
        return {}
    limits = {}
    for item in items:
        name, separator, raw_limit = item.partition("=")
        if not separator or name not in SCENARIOS or name in limits:
            raise ValueError(f"invalid or repeated scenario limit: {item}")
        limit = int(raw_limit)
        if limit <= 0:
            raise ValueError("scenario limits must be positive")
        limits[name] = limit
    if set(limits) != set(SCENARIOS):
        raise ValueError("numeric acceptance requires a limit for every Admin scenario")
    return limits


def write_evidence(measurement: dict, limits: dict[str, int], peak_limit: int | None,
                   output: Path) -> None:
    """Save the measured JSON before raising for a budget violation."""
    violations = [
        f"{name}.p99_us={measurement['scenarios'][name]['p99_us']} exceeds {limit}"
        for name, limit in limits.items()
        if measurement["scenarios"][name]["p99_us"] > limit
    ]
    if peak_limit is not None and measurement["peak_rss_kib"] > peak_limit:
        violations.append(f"peak_rss_kib={measurement['peak_rss_kib']} exceeds {peak_limit}")
    measurement["budget"] = {
        "status": "enforced" if limits else "baseline_only",
        "scenario_p99_us": limits,
        "peak_rss_kib": peak_limit,
        "violations": violations,
    }
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(measurement, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(measurement, sort_keys=True))
    if violations:
        raise RuntimeError("Admin fault budget exceeded: " + "; ".join(violations))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build-log", type=Path, required=True)
    parser.add_argument("--run-log", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--repetitions", type=int, default=10)
    parser.add_argument("--max-p99-us", action="append", default=[], metavar="SCENARIO=LIMIT")
    parser.add_argument("--max-peak-rss-kib", type=int)
    args = parser.parse_args()
    if not sys.platform.startswith("linux"):
        parser.error("child peak RSS measurement currently requires Linux")
    if args.repetitions < 2:
        parser.error("repetitions must be at least two")
    try:
        limits = parse_limits(args.max_p99_us)
    except ValueError as error:
        parser.error(str(error))
    if (limits and args.max_peak_rss_kib is None
            or args.max_peak_rss_kib is not None and (
                not limits or args.max_peak_rss_kib <= 0
            )):
        parser.error("accepted budgets require all scenario and positive RSS limits together")

    import resource  # Linux ru_maxrss is reported in KiB.

    runs = {name: [] for name in SCENARIOS}
    observations = {name: [] for name in SCENARIOS}
    args.run_log.parent.mkdir(parents=True, exist_ok=True)
    try:
        executables = test_executables(args.build_log)
        with args.run_log.open("w", encoding="utf-8") as log:
            for name, (target, test_name) in SCENARIOS.items():
                for index in range(args.repetitions):
                    started = time.perf_counter_ns()
                    try:
                        result = subprocess.run(
                            [str(executables[target]), "--exact", test_name, "--nocapture"],
                            cwd=ROOT, text=True, capture_output=True, check=False, timeout=45,
                        )
                    except subprocess.TimeoutExpired as error:
                        stdout = error.stdout.decode(errors="replace") if isinstance(error.stdout, bytes) else error.stdout or ""
                        stderr = error.stderr.decode(errors="replace") if isinstance(error.stderr, bytes) else error.stderr or ""
                        log.write(f"=== {name} run {index + 1} timeout ===\n{stdout}{stderr}\n")
                        log.flush()
                        raise
                    elapsed_us = (time.perf_counter_ns() - started) // 1000
                    output = result.stdout + result.stderr
                    log.write(f"=== {name} run {index + 1} exit={result.returncode} "
                              f"elapsed_us={elapsed_us} ===\n{output}\n")
                    log.flush()
                    if result.returncode != 0:
                        raise RuntimeError(f"Admin {name} run {index + 1} exited {result.returncode}")
                    verify_test_result(output, test_name)
                    observation = parse_observation(output, name)
                    runs[name].append(elapsed_us)
                    observations[name].append(observation)
        measurement = summarize_runs(
            runs, observations, resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss
        )
        measurement["run_log_sha256"] = hashlib.sha256(args.run_log.read_bytes()).hexdigest()
        write_evidence(measurement, limits, args.max_peak_rss_kib, args.output)
    except Exception as error:
        if not args.output.exists():
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_text(json.dumps({
                "schema": "druid-admin-fault-matrix-v1", "status": "failed",
                "completed_runs": {name: len(values) for name, values in runs.items()},
                "error": str(error),
                "run_log_sha256": hashlib.sha256(args.run_log.read_bytes()).hexdigest()
                if args.run_log.exists() else None,
            }, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        raise
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
