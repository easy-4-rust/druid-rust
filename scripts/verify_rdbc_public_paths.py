#!/usr/bin/env python3
"""Validate source-level RDBC module facades and directed compatibility shims."""

from __future__ import annotations

import argparse
import re
import sys
import tempfile
import unittest
from dataclasses import dataclass
from pathlib import Path
from typing import NoReturn


RDBC_MODULES = {"types", "error", "spi", "resource", "compat"}
PUBLIC_DEFINITION = re.compile(
    r"^\s*pub\s+(?:struct|trait|enum|type)\s+[A-Za-z_][A-Za-z0-9_]*",
    re.MULTILINE,
)
WILDCARD_REEXPORT = re.compile(r"^\s*pub\s+use\s+[^;]*\*\s*;", re.MULTILINE)


class VerificationError(RuntimeError):
    """Raised when a source-only public-path invariant is violated."""


@dataclass(frozen=True)
class ShimMapping:
    """A directed old-module shim and the exact public re-export it must contain."""

    old_module: Path
    target_module: Path
    reexport: str


def repository_root() -> Path:
    """Return the repository root regardless of the command's current directory."""

    return Path(__file__).resolve().parent.parent


def fail(message: str) -> NoReturn:
    """Raise a verification error with a stable diagnostic."""

    raise VerificationError(message)


def verify_rdbc_module_facades(root: Path) -> None:
    """Check the five declared RDBC module facades without compiling the workspace."""

    source_root = root / "crates" / "druid-rdbc" / "src"
    lib_rs = source_root / "lib.rs"
    if not lib_rs.is_file():
        fail(f"missing RDBC crate root: {lib_rs.relative_to(root)}")

    declared = set(
        re.findall(
            r"^\s*pub\s+mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;",
            lib_rs.read_text(encoding="utf-8"),
            flags=re.MULTILINE,
        )
    )
    if declared != RDBC_MODULES:
        fail(
            "RDBC crate must declare exactly "
            f"{sorted(RDBC_MODULES)}, found {sorted(declared)}"
        )

    for module in sorted(RDBC_MODULES):
        facade = source_root / module / "mod.rs"
        if not facade.is_file():
            fail(f"missing target module: {facade.relative_to(root)}")
        if PUBLIC_DEFINITION.search(facade.read_text(encoding="utf-8")):
            fail(f"module facade defines a public type: {facade.relative_to(root)}")


def verify_shim(root: Path, mapping: ShimMapping) -> None:
    """Check that one old module is a thin, exact, directed public re-export."""

    target = root / mapping.target_module
    if not target.is_file():
        fail(f"missing target module: {mapping.target_module}")

    old_module = root / mapping.old_module
    if not old_module.is_file():
        fail(f"missing old module: {mapping.old_module}")
    source = old_module.read_text(encoding="utf-8")
    if WILDCARD_REEXPORT.search(source):
        fail(f"wildcard re-export is forbidden: {mapping.old_module}")
    if PUBLIC_DEFINITION.search(source):
        fail(f"second public definition is forbidden: {mapping.old_module}")

    expected = f"pub use {mapping.reexport};"
    reexports = [
        line.strip()
        for line in source.splitlines()
        if line.strip().startswith("pub use ")
    ]
    if reexports != [expected]:
        fail(
            f"shim target mismatch in {mapping.old_module}: "
            f"expected exactly {expected}"
        )


def verify_source_tree(root: Path, mappings: list[ShimMapping]) -> None:
    """Run the source-only facade and compatibility-shim verification."""

    verify_rdbc_module_facades(root)
    for mapping in mappings:
        verify_shim(root, mapping)


def parse_mapping(value: str) -> ShimMapping:
    """Parse OLD|TARGET|REEXPORT for explicit source-only compatibility checks."""

    parts = value.split("|", maxsplit=2)
    if len(parts) != 3 or any(not part for part in parts):
        raise argparse.ArgumentTypeError(
            "shim mapping must be OLD_MODULE|TARGET_MODULE|EXACT_REEXPORT"
        )
    return ShimMapping(Path(parts[0]), Path(parts[1]), parts[2])


def write_module_facades(root: Path) -> None:
    """Create a temporary valid module-facade tree for verifier unit fixtures."""

    source_root = root / "crates" / "druid-rdbc" / "src"
    source_root.mkdir(parents=True)
    declarations = "\n".join(f"pub mod {module};" for module in sorted(RDBC_MODULES))
    (source_root / "lib.rs").write_text(declarations + "\n", encoding="utf-8")
    for module in RDBC_MODULES:
        facade = source_root / module / "mod.rs"
        facade.parent.mkdir(parents=True, exist_ok=True)
        facade.write_text("", encoding="utf-8")


def write_fixture_target(root: Path) -> Path:
    """Create an isolated fixture target; it is never part of the product crate."""

    target = root / "crates" / "druid-rdbc" / "src" / "types" / "fixture_type.rs"
    target.write_text("pub struct FixtureType;\n", encoding="utf-8")
    return target.relative_to(root)


class PublicPathVerifierTest(unittest.TestCase):
    """Exercise all source-only failure modes using a unique temporary directory."""

    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        write_module_facades(self.root)
        self.target = write_fixture_target(self.root)
        self.old = Path("crates/druid/src/legacy_fixture.rs")
        self.mapping = ShimMapping(
            self.old,
            self.target,
            "druid_rdbc::types::fixture_type::FixtureType",
        )

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def write_old_shim(self, contents: str) -> None:
        """Write one disposable old-module fixture."""

        old_path = self.root / self.old
        old_path.parent.mkdir(parents=True, exist_ok=True)
        old_path.write_text(contents, encoding="utf-8")

    def test_rejects_missing_target_module(self) -> None:
        """A missing new module cannot silently satisfy an old public path."""

        self.write_old_shim("pub use druid_rdbc::types::fixture_type::FixtureType;\n")
        missing_target = Path("crates/druid-rdbc/src/types/missing_fixture.rs")
        mapping = ShimMapping(self.old, missing_target, self.mapping.reexport)
        with self.assertRaisesRegex(VerificationError, "missing target module"):
            verify_source_tree(self.root, [mapping])

    def test_rejects_missing_old_module(self) -> None:
        """A missing legacy module is a compatibility break."""

        with self.assertRaisesRegex(VerificationError, "missing old module"):
            verify_source_tree(self.root, [self.mapping])

    def test_rejects_wildcard_reexport(self) -> None:
        """Wildcard shims cannot define the maintained public-path contract."""

        self.write_old_shim("pub use druid_rdbc::types::fixture_type::*;\n")
        with self.assertRaisesRegex(VerificationError, "wildcard re-export"):
            verify_source_tree(self.root, [self.mapping])

    def test_rejects_second_public_definition(self) -> None:
        """An old shim may not introduce another public type definition."""

        self.write_old_shim(
            "pub use druid_rdbc::types::fixture_type::FixtureType;\n"
            "pub struct DuplicateFixtureType;\n"
        )
        with self.assertRaisesRegex(VerificationError, "second public definition"):
            verify_source_tree(self.root, [self.mapping])

    def test_accepts_valid_thin_shim(self) -> None:
        """A single exact named re-export is a valid thin shim."""

        self.write_old_shim("pub use druid_rdbc::types::fixture_type::FixtureType;\n")
        verify_source_tree(self.root, [self.mapping])


def main() -> int:
    """Run the requested source-only verifier mode or its isolated unit fixtures."""

    parser = argparse.ArgumentParser()
    parser.add_argument("--source-only", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--root", type=Path, default=repository_root())
    parser.add_argument("--shim", action="append", type=parse_mapping, default=[])
    arguments = parser.parse_args()

    if arguments.self_test:
        result = unittest.main(argv=[sys.argv[0]], exit=False)
        return 0 if result.result.wasSuccessful() else 1
    if not arguments.source_only:
        parser.error("--source-only is required unless --self-test is selected")
    try:
        verify_source_tree(arguments.root.resolve(), arguments.shim)
    except VerificationError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(
        "PASS: RDBC public-path source verification "
        f"({len(RDBC_MODULES)} declared modules, {len(arguments.shim)} shim mappings)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
