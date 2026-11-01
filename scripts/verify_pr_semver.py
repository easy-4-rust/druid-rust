#!/usr/bin/env python3
"""Reject new SemVer breaks against the immutable pull request base commit."""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from typing import Callable

CRATES = ("druid-rdbc", "druid", "druid-wrapper")


def check_pr_semver(
    baseline: str,
    runner: Callable[..., subprocess.CompletedProcess[str]] = subprocess.run,
) -> int:
    if re.fullmatch(r"[0-9a-fA-F]{40}", baseline) is None:
        print("PR SemVer baseline must be a commit SHA", file=sys.stderr)
        return 1
    failed = False
    for crate in CRATES:
        command = [
            "cargo", "semver-checks", "--package", crate,
            "--baseline-rev", baseline, "--release-type", "patch",
            "--all-features", "--verbose",
        ]
        try:
            result = runner(command, check=False)
        except OSError as error:
            print(f"PR SemVer scan could not start for {crate}: {error}", file=sys.stderr)
            failed = True
            continue
        if result.returncode != 0:
            print(f"PR SemVer scan failed for {crate}: exit {result.returncode}", file=sys.stderr)
            failed = True
    return int(failed)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", required=True)
    return check_pr_semver(parser.parse_args().baseline)


if __name__ == "__main__":
    raise SystemExit(main())
