#!/usr/bin/env python3
"""Fail if the documented RSA advisory waiver outlives or exceeds its scope."""

import json
from pathlib import Path
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
EXPECTED_RSA_VERSION = "0.9.10"
EXPECTED_CONSUMERS = {"druid", "sqlx-mysql"}


def waiver_scope_errors(metadata):
    """Check the resolved, locked graph rather than declared optional features."""
    packages = {package["id"]: package for package in metadata["packages"]}
    rsa_ids = {
        package_id
        for package_id, package in packages.items()
        if package["name"] == "rsa"
    }
    errors = []
    versions = {packages[package_id]["version"] for package_id in rsa_ids}
    if len(rsa_ids) != 1 or versions != {EXPECTED_RSA_VERSION}:
        errors.append(
            f"waived rsa package changed: expected one rsa@{EXPECTED_RSA_VERSION}, "
            f"found {sorted(versions)}"
        )

    consumers = {
        packages[node["id"]]["name"]
        for node in metadata["resolve"]["nodes"]
        if node["id"] in packages
        for dependency in node["deps"]
        if dependency["pkg"] in rsa_ids
    }
    if consumers != EXPECTED_CONSUMERS:
        errors.append(
            f"rsa direct consumers changed: expected {sorted(EXPECTED_CONSUMERS)}, "
            f"found {sorted(consumers)}"
        )
    return errors


def main():
    result = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked"],
        cwd=ROOT,
        text=True,
        encoding="utf-8",
        capture_output=True,
        check=True,
    )
    errors = waiver_scope_errors(json.loads(result.stdout))
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("RSA advisory waiver scope verified: rsa@0.9.10 -> druid, sqlx-mysql")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
