#!/usr/bin/env python3
"""Verify that the current druid core's default dependency graph is clean.

Uses `cargo tree -e normal` which excludes dev-dependencies, giving an
accurate picture of what druid ships by default to downstream consumers.
Optional feature matrices are verified separately; this is not an all-features gate.

Exit 0 on success, 1 if any forbidden dependency is found.
"""

import re
import subprocess
import sys
from pathlib import Path

FORBIDDEN = {
    "toasty",
    "toasty-core",
    "sqlx",
    "rbdc",
    "duckdb",
    "libsql",
    "bb8",
    "deadpool",
    "prometheus",
    "reqwest",
    "tonic",
    "axum",
    "topcoat",
}

# Pattern to extract package name from cargo tree output lines like:
#   ├── toasty v0.9.0
#   │   └── toasty-core v0.9.0
# Unicode box-drawing chars: ─ (U+2500), │ (U+2502), ├ (U+251C), └ (U+2514)
TREE_LINE_RE = re.compile(r"[├└│\u2500\u2502 ]+ (\S+) v\S+")


def main() -> int:
    result = subprocess.run(
        ["cargo", "tree", "--locked", "-p", "druid", "-e", "normal", "--depth", "999"],
        cwd=Path(__file__).resolve().parent.parent,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        print(f"ERROR: cargo tree failed:\n{result.stderr}", file=sys.stderr)
        return 1

    lines = [line for line in result.stdout.splitlines() if line.strip()]
    if not lines or re.match(r"^druid v\S+(?:\s|$)", lines[0]) is None:
        print("ERROR: cargo tree did not return the druid root package", file=sys.stderr)
        return 1

    violations: list[str] = []
    seen: set[str] = set()

    for line in lines:
        match = TREE_LINE_RE.search(line)
        if not match:
            continue
        pkg_name = match.group(1)
        if pkg_name in seen:
            continue
        seen.add(pkg_name)
        print(f"  traversed: {pkg_name}")
        if pkg_name in FORBIDDEN:
            violations.append(pkg_name)

    if violations:
        print(
            f"\nFAIL: druid depends on forbidden packages: {sorted(set(violations))}",
            file=sys.stderr,
        )
        return 1

    print("\nPASS: druid default normal-dependency graph is clean.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
