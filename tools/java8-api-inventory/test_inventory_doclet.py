#!/usr/bin/env python3
"""Behavioral tests for the Java 8 source-derived JDBC API inventory doclet."""

from __future__ import annotations

import csv
import os
import subprocess
import tempfile
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[2]
TOOL_ROOT = Path(__file__).resolve().parent
DOCLET_SOURCE = TOOL_ROOT / "JdbcApiInventoryDoclet.java"
FIXTURE_ROOT = TOOL_ROOT / "fixtures"
JDK8 = Path(os.environ.get("JAVA8_HOME", r"D:\Java\jdk1.8.0_321"))
JAVAC = JDK8 / "bin" / "javac.exe"
JAVADOC = JDK8 / "bin" / "javadoc.exe"
TOOLS_JAR = JDK8 / "lib" / "tools.jar"


def run_doclet(output: Path) -> None:
    """Compile and run the doclet using only Java source inputs."""
    with tempfile.TemporaryDirectory(prefix="jdbc-api-doclet-") as temporary:
        class_dir = Path(temporary) / "classes"
        class_dir.mkdir()
        subprocess.run(
            [str(JAVAC), "-encoding", "UTF-8", "-cp", str(TOOLS_JAR), "-d", str(class_dir), str(DOCLET_SOURCE)],
            check=True,
            cwd=REPO_ROOT,
        )
        subprocess.run(
            [
                str(JAVADOC), "-quiet", "-source", "1.8", "-doclet", "JdbcApiInventoryDoclet",
                "-docletpath", str(class_dir), "-inventory-output", str(output),
                "-inventory-source-root", str(FIXTURE_ROOT), "-inventory-source-commit", "fixture-source-v1",
                "-sourcepath", str(FIXTURE_ROOT), "-subpackages", "fixture.api",
            ],
            check=True,
            cwd=REPO_ROOT,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )


class InventoryDocletTest(unittest.TestCase):
    def test_listener_overloads_constructors_constants_and_inheritance_are_independent_and_deterministic(self) -> None:
        with tempfile.TemporaryDirectory(prefix="jdbc-api-inventory-test-") as temporary:
            root = Path(temporary)
            first = root / "first.tsv"
            second = root / "second.tsv"
            run_doclet(first)
            run_doclet(second)
            first_bytes = first.read_bytes()
            self.assertEqual(first_bytes, second.read_bytes(), "same Java input produces byte-identical output")

            lines = first_bytes.decode("utf-8").splitlines()
            self.assertIn("# schema=jdbc-java-method-inventory-v1\tsource_commit=fixture-source-v1", lines[0])
            records = list(csv.DictReader(lines[1:], delimiter="\t"))
            signatures = {(row["kind"], row["identity"], row["declared_in"], row["is_default"]) for row in records}

            listener = "fixture.api.ConnectionEventListener"
            self.assertIn(("type", f"{listener}#TYPE", listener, "false"), signatures)
            self.assertIn(("method", f"{listener}#connectionClosed()", listener, "false"), signatures)
            self.assertIn(("method", f"{listener}#connectionErrorOccurred(java.lang.String)", listener, "false"), signatures)
            self.assertIn(("method", f"{listener}#classify()", listener, "false"), signatures)
            self.assertIn(("method", f"{listener}#classify(java.lang.String)", listener, "false"), signatures)

            owner = "fixture.api.PooledOwner"
            self.assertIn(("constructor", f"{owner}#<init>()", owner, "false"), signatures)
            self.assertIn(("constructor", f"{owner}#<init>(int)", owner, "false"), signatures)
            self.assertIn(("constant", f"{owner}#STANDARD_NAME", owner, "false"), signatures)
            self.assertIn(
                (
                    "constructor",
                    "fixture.api.ImplicitConstructorApi#<init>()",
                    "fixture.api.ImplicitConstructorApi",
                    "false",
                ),
                signatures,
                "source classes with an implicit public constructor remain in the API denominator",
            )
            self.assertIn(("inherited_method", f"{owner}#ownerName()", listener, "true"), signatures)
            self.assertIn(("inherited_method", f"{owner}#closeOwner()", "fixture.api.BaseOwner", "false"), signatures)
            self.assertIn(("inherited_constant", f"{owner}#CONTRACT_VERSION", "fixture.api.InheritedContract", "false"), signatures)
            self.assertFalse(any("internalOnly" in row["identity"] for row in records))
            self.assertFalse(any("crates/druid" in row["source"] or "druid-rdbc" in row["source"] for row in records))


if __name__ == "__main__":
    unittest.main()
