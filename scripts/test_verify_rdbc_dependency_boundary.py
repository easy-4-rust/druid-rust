#!/usr/bin/env python3
"""Adversarial process fixtures for the RDBC dependency-boundary verifier."""

from __future__ import annotations

import json
import os
import stat
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parent.parent
VERIFIER = REPOSITORY_ROOT / "scripts" / "verify_rdbc_dependency_boundary.py"
RDBC_MANIFEST = str(
    (REPOSITORY_ROOT / "crates" / "druid-rdbc" / "Cargo.toml").resolve()
)
RDBC_ID = f"path+file://{REPOSITORY_ROOT}/crates/druid-rdbc#0.0.0-design"


def package(package_id: str, name: str, manifest_path: str) -> dict[str, object]:
    """Build the metadata fields used by the verifier."""

    return {
        "id": package_id,
        "name": name,
        "manifest_path": manifest_path,
    }


def node(package_id: str, dependencies: list[dict[str, object]]) -> dict[str, object]:
    """Build one resolved graph node."""

    return {"id": package_id, "deps": dependencies}


def dependency(
    name: str, package_id: str, kind: str | None,
) -> dict[str, object]:
    """Build a dependency edge with Cargo's metadata kind representation."""

    return {"name": name, "pkg": package_id, "dep_kinds": [{"kind": kind}]}


class DependencyBoundaryVerifierTest(unittest.TestCase):
    """Run the command against controlled Cargo metadata without touching the workspace."""

    def run_verifier(self, metadata: dict[str, object]) -> subprocess.CompletedProcess[str]:
        """Provide one temporary fake Cargo executable that returns metadata."""

        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary_root = Path(temporary_directory)
            fake_cargo = temporary_root / "cargo"
            fake_cargo.write_text(
                "#!/bin/sh\nprintf '%s' \"$RDBC_TEST_METADATA\"\n",
                encoding="utf-8",
            )
            fake_cargo.chmod(fake_cargo.stat().st_mode | stat.S_IXUSR)
            environment = os.environ.copy()
            environment["PATH"] = f"{temporary_root}{os.pathsep}{environment['PATH']}"
            environment["RDBC_TEST_METADATA"] = json.dumps(metadata)
            return subprocess.run(
                [sys.executable, str(VERIFIER)],
                cwd=REPOSITORY_ROOT,
                capture_output=True,
                text=True,
                check=False,
                env=environment,
            )

    def test_rejects_registry_only_same_name_package(self) -> None:
        """A registry package named druid-rdbc is not the workspace contract crate."""

        registry_id = "registry+https://example.invalid/index#druid-rdbc@0.0.0"
        metadata = {
            "packages": [
                package(
                    registry_id,
                    "druid-rdbc",
                    "/registry/druid-rdbc-0.0.0/Cargo.toml",
                )
            ],
            "workspace_members": [],
            "resolve": {"nodes": [node(registry_id, [])]},
        }
        completed = self.run_verifier(metadata)
        self.assertEqual(1, completed.returncode, completed.stdout + completed.stderr)

    def test_rejects_multiple_matching_workspace_members(self) -> None:
        """Multiple matching workspace members cannot choose an arbitrary package id."""

        second_id = f"{RDBC_ID}-duplicate"
        metadata = {
            "packages": [
                package(RDBC_ID, "druid-rdbc", RDBC_MANIFEST),
                package(second_id, "druid-rdbc", RDBC_MANIFEST),
            ],
            "workspace_members": [RDBC_ID, second_id],
            "resolve": {"nodes": [node(RDBC_ID, []), node(second_id, [])]},
        }
        completed = self.run_verifier(metadata)
        self.assertEqual(1, completed.returncode, completed.stdout + completed.stderr)

    def test_accepts_renamed_normal_dependency_cycle(self) -> None:
        """Package-id traversal keeps renamed normal edges and terminates on a cycle."""

        helper_id = "registry+https://example.invalid/index#rdbc-helper@1.0.0"
        metadata = {
            "packages": [
                package(RDBC_ID, "druid-rdbc", RDBC_MANIFEST),
                package(helper_id, "rdbc-helper", "/registry/rdbc-helper/Cargo.toml"),
            ],
            "workspace_members": [RDBC_ID],
            "resolve": {
                "nodes": [
                    node(RDBC_ID, [dependency("renamed_helper", helper_id, None)]),
                    node(helper_id, [dependency("rdbc_contract", RDBC_ID, None)]),
                ]
            },
        }
        completed = self.run_verifier(metadata)
        self.assertEqual(0, completed.returncode, completed.stdout + completed.stderr)

    def test_excludes_dev_and_build_forbidden_dependencies(self) -> None:
        """Dev and build edges do not become forbidden normal-dependency violations."""

        dev_id = "registry+https://example.invalid/index#sqlx@0.8.0"
        build_id = "registry+https://example.invalid/index#toasty@0.9.0"
        metadata = {
            "packages": [
                package(RDBC_ID, "druid-rdbc", RDBC_MANIFEST),
                package(dev_id, "sqlx", "/registry/sqlx/Cargo.toml"),
                package(build_id, "toasty", "/registry/toasty/Cargo.toml"),
            ],
            "workspace_members": [RDBC_ID],
            "resolve": {
                "nodes": [
                    node(
                        RDBC_ID,
                        [
                            dependency("sqlx_test", dev_id, "dev"),
                            dependency("toasty_build", build_id, "build"),
                        ],
                    ),
                    node(dev_id, []),
                    node(build_id, []),
                ]
            },
        }
        completed = self.run_verifier(metadata)
        self.assertEqual(0, completed.returncode, completed.stdout + completed.stderr)


if __name__ == "__main__":
    unittest.main()
