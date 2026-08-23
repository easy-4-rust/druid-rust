#!/usr/bin/env python3
"""Verify that the isolated ``druid-rdbc`` contract crate has no forbidden normal dependencies."""

from __future__ import annotations

import json
import subprocess
import sys
from collections.abc import Iterable
from pathlib import Path


FORBIDDEN = {
    "druid",
    "druid-wrapper",
    "druid-metrics",
    "druid-admin",
    "sqlx",
    "toasty",
    "toasty-core",
    "rbdc",
    "bb8",
    "deadpool",
    "axum",
    "tonic",
    "reqwest",
    "kube",
    "prometheus",
}


def workspace_root() -> Path:
    """Return the repository root regardless of the caller's directory."""

    return Path(__file__).resolve().parent.parent


def cargo_metadata(root: Path) -> dict[str, object]:
    """Load Cargo's resolved dependency graph for ``root``."""

    command = ["cargo", "metadata", "--format-version", "1"]
    completed = subprocess.run(
        command,
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        print(completed.stderr, file=sys.stderr, end="")
        raise RuntimeError("cargo metadata failed")
    return json.loads(completed.stdout)


def normal_dependency_ids(node: dict[str, object]) -> Iterable[str]:
    """Yield only dependency edges that Cargo classifies as normal dependencies."""

    dependencies = node.get("deps", [])
    if not isinstance(dependencies, list):
        return
    for dependency in dependencies:
        if not isinstance(dependency, dict):
            continue
        dep_kinds = dependency.get("dep_kinds", [])
        if not isinstance(dep_kinds, list):
            continue
        if any(
            isinstance(kind, dict) and kind.get("kind") is None
            for kind in dep_kinds
        ):
            package_id = dependency.get("pkg")
            if isinstance(package_id, str):
                yield package_id


def reachable_normal_dependencies(metadata: dict[str, object], root_id: str) -> set[str]:
    """Return all package ids reachable from ``root_id`` over normal dependency edges."""

    resolve = metadata.get("resolve")
    if not isinstance(resolve, dict):
        raise RuntimeError("cargo metadata did not include a resolve graph")
    nodes = resolve.get("nodes", [])
    if not isinstance(nodes, list):
        raise RuntimeError("cargo metadata resolve graph has invalid nodes")

    nodes_by_id = {
        node_id: node
        for node in nodes
        if isinstance(node, dict)
        if isinstance((node_id := node.get("id")), str)
    }
    reachable: set[str] = set()
    pending = [root_id]
    while pending:
        current_id = pending.pop()
        current = nodes_by_id.get(current_id)
        if current is None:
            continue
        for dependency_id in normal_dependency_ids(current):
            if dependency_id not in reachable:
                reachable.add(dependency_id)
                pending.append(dependency_id)
    return reachable


def main() -> int:
    """Check the resolved normal-dependency closure and report violations."""

    try:
        metadata = cargo_metadata(workspace_root())
    except RuntimeError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1

    packages = metadata.get("packages", [])
    if not isinstance(packages, list):
        print("ERROR: cargo metadata has invalid packages", file=sys.stderr)
        return 1
    package_by_id = {
        package_id: package
        for package in packages
        if isinstance(package, dict)
        if isinstance((package_id := package.get("id")), str)
    }
    rdbc_id = next(
        (
            package_id
            for package_id, package in package_by_id.items()
            if package.get("name") == "druid-rdbc"
        ),
        None,
    )
    if rdbc_id is None:
        print("ERROR: package druid-rdbc not found", file=sys.stderr)
        return 1

    try:
        dependency_ids = reachable_normal_dependencies(metadata, rdbc_id)
    except RuntimeError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    forbidden = sorted(
        str(package.get("name"))
        for dependency_id in dependency_ids
        if (package := package_by_id.get(dependency_id)) is not None
        if package.get("name") in FORBIDDEN
    )
    if forbidden:
        print(
            "ERROR: druid-rdbc reaches forbidden normal dependencies: "
            + ", ".join(forbidden),
            file=sys.stderr,
        )
        return 1

    print("PASS: druid-rdbc has no forbidden reachable normal dependencies")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
