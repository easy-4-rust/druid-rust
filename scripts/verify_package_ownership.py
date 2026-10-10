"""Verify workspace publication policy and CI binary ownership."""

import json
from pathlib import Path
import re
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
WORKFLOWS = {
    ".github/workflows/driver-matrix.yml": (
        "aggregate-evidence sqlite sqlx target/contract-evidence/sqlite-runs target/contract-evidence/sqlite-evidence.json",
        "aggregate-evidence duckdb native target/contract-evidence/duckdb-runs target/contract-evidence/duckdb-evidence.json",
    ),
    ".github/workflows/database-profile-certification.yml": (
        'aggregate-evidence "${{ inputs.profile }}" "${{ inputs.runtime_mode }}" target/contract-evidence/runs target/contract-evidence/${{ inputs.profile }}-evidence.json',
    ),
}
DRIVER_PREFIX = "cargo run -p druid-wrapper --features managed-driver-install --bin druid-driver -- "
LIVE_TEST_TARGET = "task6_database_profile_live_contract_test_parity"
LIVE_TEST_WORKFLOWS = {
    ".github/workflows/driver-matrix.yml": 2,
    ".github/workflows/database-profile-certification.yml": 1,
}


def main() -> int:
    cargo = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1", "--offline", "--locked"],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=True,
    )
    metadata = json.loads(cargo.stdout)
    members = set(metadata["workspace_members"])
    packages = {package["name"]: package for package in metadata["packages"] if package["id"] in members}
    errors = []

    for name, package in sorted(packages.items()):
        if package.get("publish") != []:
            errors.append(f"{name}: publish must be disabled")

    for binary, owner in (("druid-driver", "druid-wrapper"), ("druid-admin", "druid-admin")):
        owners = [
            name
            for name, package in packages.items()
            if any(binary == target["name"] and "bin" in target["kind"] for target in package["targets"])
        ]
        if owners != [owner]:
            errors.append(f"{binary}: expected owner {owner}, found {owners}")

    for relative, commands in WORKFLOWS.items():
        source = (ROOT / relative).read_text(encoding="utf-8")
        for command in commands:
            if DRIVER_PREFIX + command not in source:
                errors.append(f"{relative}: missing owned invocation: {command}")

    wrapper_targets = {
        target["name"]
        for target in packages["druid-wrapper"]["targets"]
        if "test" in target["kind"]
    }
    if LIVE_TEST_TARGET not in wrapper_targets:
        errors.append(f"druid-wrapper: missing live contract test target {LIVE_TEST_TARGET}")
    for relative, expected_count in LIVE_TEST_WORKFLOWS.items():
        source = (ROOT / relative).read_text(encoding="utf-8")
        actual_count = source.count(f"--test {LIVE_TEST_TARGET}")
        if actual_count != expected_count:
            errors.append(
                f"{relative}: expected {expected_count} live contract invocations "
                f"for {LIVE_TEST_TARGET}, found {actual_count}"
            )

    for workflow in sorted((ROOT / ".github/workflows").glob("*.y*ml")):
        for number, line in enumerate(workflow.read_text(encoding="utf-8").splitlines(), 1):
            if "--bin druid-driver" in line and DRIVER_PREFIX not in line:
                errors.append(f"{workflow.relative_to(ROOT)}:{number}: druid-driver must use druid-wrapper")
            for package_name, target_name in re.findall(
                r"cargo\s+test\s+-p\s+([\w-]+).*?--test\s+([\w-]+)", line
            ):
                package = packages.get(package_name)
                test_targets = {
                    target["name"]
                    for target in package["targets"]
                    if "test" in target["kind"]
                } if package else set()
                if target_name not in test_targets:
                    errors.append(
                        f"{workflow.relative_to(ROOT)}:{number}: "
                        f"{package_name} has no test target {target_name}"
                    )

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"package ownership verified: {len(packages)} workspace packages, 2 binaries, 3 driver and 3 live contract CI invocations")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
