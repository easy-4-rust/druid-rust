#!/usr/bin/env python3
"""Generate a deterministic public API inventory from pinned OpenJDK 8u Java source."""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT / "scripts"))
from generate_jdbc42_semantic_inventory import (  # noqa: E402
    ARCHIVE_SHA256,
    ORACLE_COMMIT,
    download_archive,
    verify_archive,
)

TOOL_SOURCE = REPO_ROOT / "tools/java8-api-inventory/JdbcApiInventoryDoclet.java"
DEFAULT_OUTPUT = REPO_ROOT / "docs/verification/rdbc-first/java8-public-method-inventory.tsv"
JAVA_SOURCE_PREFIX = f"jdk/src/share/classes/"
API_SOURCE_PREFIXES = ("jdk/src/share/classes/java/sql/", "jdk/src/share/classes/javax/sql/")


def java8_tools() -> tuple[Path, Path, Path]:
    """Resolve an installed JDK 8 toolchain without installing or changing it."""
    root = Path(os.environ.get("JAVA8_HOME", r"D:\Java\jdk1.8.0_321"))
    javac = root / "bin" / "javac.exe"
    javadoc = root / "bin" / "javadoc.exe"
    tools_jar = root / "lib" / "tools.jar"
    missing = [path for path in (javac, javadoc, tools_jar) if not path.is_file()]
    if missing:
        raise RuntimeError(
            "JDK 8 javac, javadoc, and tools.jar are required; set JAVA8_HOME. Missing: "
            + ", ".join(str(path) for path in missing)
        )
    return javac, javadoc, tools_jar


def selected_sources(archive: Path, destination: Path) -> list[Path]:
    """Extract only java.sql/javax.sql source files from the verified source archive."""
    files: list[Path] = []
    root = destination.resolve()
    with tarfile.open(archive, "r:gz") as bundle:
        for member in sorted(bundle.getmembers(), key=lambda entry: entry.name):
            if not member.isfile() or not member.name.endswith(".java"):
                continue
            source_offset = member.name.find(JAVA_SOURCE_PREFIX)
            if source_offset < 0:
                continue
            source_path = member.name[source_offset:]
            if not source_path.startswith(API_SOURCE_PREFIXES):
                continue
            relative = Path(source_path)
            if len(relative.parts) != 7:
                continue
            target = (root / relative).resolve()
            if root not in target.parents:
                raise RuntimeError(f"source archive path escapes extraction root: {member.name}")
            source = bundle.extractfile(member)
            if source is None:
                raise RuntimeError(f"unable to read source archive member: {member.name}")
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(source.read())
            files.append(target)
    if len(files) != 69:
        raise RuntimeError(f"pinned JDBC source file count changed: expected 69, found {len(files)}")
    return files


def canonical_inventory_payload(payload: bytes) -> bytes:
    """Keep doclet fields intact while publishing canonical LF bytes on every platform."""
    return payload.replace(b"\r\n", b"\n")


def generate(archive: Path, output: Path) -> None:
    """Compile and invoke the JDK 8 doclet, then atomically publish its text inventory."""
    javac, javadoc, tools_jar = java8_tools()
    verify_archive(archive, ARCHIVE_SHA256)
    output = output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="jdbc-java8-method-inventory-") as temporary:
        temporary_root = Path(temporary)
        source_root = temporary_root / "openjdk"
        class_root = temporary_root / "doclet-classes"
        class_root.mkdir()
        sources = selected_sources(archive, source_root)
        subprocess.run(
            [str(javac), "-encoding", "UTF-8", "-cp", str(tools_jar), "-d", str(class_root), str(TOOL_SOURCE)],
            check=True,
            cwd=REPO_ROOT,
        )
        generated = temporary_root / "inventory.tsv"
        command = [
            str(javadoc), "-quiet", "-source", "1.8", "-doclet", "JdbcApiInventoryDoclet",
            "-docletpath", str(class_root),
            "-inventory-output", str(generated),
            "-inventory-source-root", str(source_root),
            "-inventory-source-commit", f"openjdk/jdk8u@{ORACLE_COMMIT}",
            *map(str, sources),
        ]
        subprocess.run(command, check=True, cwd=REPO_ROOT)
        payload = canonical_inventory_payload(generated.read_bytes())
        rows = payload.decode("utf-8").splitlines()
        if not rows or rows[0] != f"# schema=jdbc-java-method-inventory-v1\tsource_commit=openjdk/jdk8u@{ORACLE_COMMIT}":
            raise RuntimeError("doclet output does not identify the pinned source commit")
        if len(rows) < 2 or not any("java.sql.Connection#TYPE" in row for row in rows):
            raise RuntimeError("doclet output is missing the java.sql source API")
        temporary_output = output.with_name(output.name + ".tmp")
        temporary_output.write_bytes(payload)
        temporary_output.replace(output)
    print(f"SOURCE_COMMIT=openjdk/jdk8u@{ORACLE_COMMIT}")
    print(f"SOURCE_ARCHIVE_SHA256={ARCHIVE_SHA256}")
    print(f"OUTPUT={output}")
    print(f"ROWS={len(rows) - 2}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, help="verified pinned OpenJDK source archive; downloads if omitted")
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    arguments = parser.parse_args()
    try:
        if arguments.archive is not None:
            generate(arguments.archive, arguments.output)
        else:
            with tempfile.TemporaryDirectory(prefix="jdbc-java8-openjdk-archive-") as temporary:
                archive = Path(temporary) / f"jdk8u-{ORACLE_COMMIT}.tar.gz"
                download_archive(archive)
                generate(archive, arguments.output)
    except (OSError, RuntimeError, subprocess.CalledProcessError, tarfile.TarError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
