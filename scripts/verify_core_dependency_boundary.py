#!/usr/bin/env python3
"""Verify that druid-core's dependency graph contains no forbidden packages.

Uses `cargo tree -e normal` which excludes dev-dependencies, giving an
accurate picture of what druid-core actually ships to downstream consumers.

Exit 0 on success, 1 if any forbidden dependency is found.
"""

import re
import subprocess
import sys

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
        ["cargo", "tree", "-p", "druid-core", "-e", "normal", "--depth", "999"],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        print(f"ERROR: cargo tree failed:\n{result.stderr}", file=sys.stderr)
        return 1

    violations: list[str] = []
    seen: set[str] = set()

    for line in result.stdout.splitlines():
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
            f"\nFAIL: druid-core depends on forbidden packages: {sorted(set(violations))}",
            file=sys.stderr,
        )
        return 1

    print("\nPASS: druid-core dependency graph is clean.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
