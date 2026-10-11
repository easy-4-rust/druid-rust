"""Verify the current six-crate RDBC-first workspace dependency direction."""

import json
from pathlib import Path
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
EXPECTED = {
    "druid-rdbc",
    "druid",
    "druid-wrapper",
    "druid-metrics",
    "druid-admin",
    "druid-bench",
}
FORBIDDEN = {
    ("druid-rdbc", "druid"),
    ("druid", "druid-wrapper"),
    ("druid", "druid-metrics"),
    ("druid-metrics", "druid-wrapper"),
    ("druid-admin", "druid"),
    ("druid-admin", "druid-wrapper"),
}


def main() -> int:
    result = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1", "--offline", "--locked"],
        cwd=ROOT,
        capture_output=True,
        text=True,
        encoding="utf-8",
        check=True,
    )
    metadata = json.loads(result.stdout)
    members = set(metadata["workspace_members"])
    packages = {
        package["name"]: package
        for package in metadata["packages"]
        if package["id"] in members
    }
    errors = []
    if set(packages) != EXPECTED:
        errors.append(f"workspace packages {sorted(packages)} != {sorted(EXPECTED)}")

    edges = set()
    for name, package in packages.items():
        for dependency in package["dependencies"]:
            if dependency["name"] not in packages:
                continue
            if dependency["kind"] is None:
                edge = (name, dependency["name"])
                edges.add(edge)
                if edge in FORBIDDEN:
                    errors.append(f"forbidden normal dependency: {name} -> {dependency['name']}")

    adjacency = {name: set() for name in packages}
    for source, target in edges:
        adjacency[source].add(target)
    visiting = set()
    visited = set()

    def visit(name: str) -> None:
        if name in visiting:
            errors.append(f"cycle through {name}")
            return
        if name in visited:
            return
        visiting.add(name)
        for target in sorted(adjacency[name]):
            visit(target)
        visiting.remove(name)
        visited.add(name)

    for name in sorted(packages):
        visit(name)

    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print(f"workspace DAG verified: {len(packages)} packages, {len(edges)} normal edges")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
