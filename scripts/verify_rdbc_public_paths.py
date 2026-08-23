#!/usr/bin/env python3
"""Validate source-level RDBC module facades and directed compatibility shims."""

from __future__ import annotations

import argparse
import sys
import tempfile
import unittest
from dataclasses import dataclass
from pathlib import Path
from typing import NoReturn


RDBC_MODULES = {"types", "error", "spi", "resource", "compat"}
MAPPING_HEADER = "old_path\tnew_path\tcompile_mode\told_module_file"
COMPILE_MODES = {"OWNED", "DYN_TRAIT", "PATH_ONLY"}
RUST_2021_KEYWORDS = {
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else",
    "enum", "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop",
    "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static",
    "struct", "super", "trait", "true", "type", "union", "unsafe", "use", "where",
    "while", "abstract", "become", "box", "do", "final", "macro", "override", "priv",
    "typeof", "unsized", "virtual", "yield", "try",
}
RAW_IDENTIFIER_DISALLOWED = {"crate", "self", "super", "Self", "_"}


class VerificationError(RuntimeError):
    """Raised when a source-only public-path invariant is violated."""


@dataclass(frozen=True)
class ShimMapping:
    """A directed old-module shim and the exact public re-export it must contain."""

    old_module: Path
    target_module: Path
    reexport: str


@dataclass(frozen=True)
class Token:
    """One lexical Rust token retained by the fail-closed source verifier."""

    kind: str
    text: str


@dataclass(frozen=True)
class FacadeItem:
    """One allowed facade declaration after comments and documentation are removed."""

    kind: str
    public: bool
    value: str
    alias: str | None = None


@dataclass(frozen=True)
class ManifestShim:
    """One public path row normalized into a directed old-module re-export contract."""

    old_module: Path
    target_facade: Path
    new_path: str
    reexport: str
    old_export: str


@dataclass(frozen=True)
class ShimGroup:
    """All public definitions owned by the same old module file."""

    old_module: Path
    mappings: tuple[ManifestShim, ...]


def repository_root() -> Path:
    """Return the repository root regardless of the command's current directory."""

    return Path(__file__).resolve().parent.parent


def fail(message: str) -> NoReturn:
    """Raise a verification error with a stable diagnostic."""

    raise VerificationError(message)


def consume_quoted(source: str, start: int, quote: str, kind: str) -> tuple[Token, int]:
    """Consume one normal string, byte string, or character literal."""

    index = start
    escaped = False
    while index < len(source):
        character = source[index]
        if escaped:
            escaped = False
        elif character == "\\":
            escaped = True
        elif character == quote:
            return Token(kind, source[start:index + 1]), index + 1
        index += 1
    fail("unterminated Rust literal")


def consume_raw_string(source: str, start: int, byte: bool) -> tuple[Token, int] | None:
    """Consume a raw string when the current offset starts one, otherwise return None."""

    prefix_length = 2 if byte else 1
    if not source.startswith("br" if byte else "r", start):
        return None
    index = start + prefix_length
    hashes = 0
    while index < len(source) and source[index] == "#":
        hashes += 1
        index += 1
    if index >= len(source) or source[index] != '"':
        return None
    terminator = '"' + "#" * hashes
    end = source.find(terminator, index + 1)
    if end == -1:
        fail("unterminated raw Rust string")
    kind = "BYTE_STRING" if byte else "STRING"
    return Token(kind, source[start:end + len(terminator)]), end + len(terminator)


def tokenize(source: str) -> list[Token]:
    """Lex Rust 2021 source while discarding comments and retaining literal boundaries."""

    tokens: list[Token] = []
    index = 0
    block_depth = 0
    while index < len(source):
        if source.startswith("//", index):
            newline = source.find("\n", index + 2)
            index = len(source) if newline == -1 else newline + 1
            continue
        if source.startswith("/*", index):
            block_depth = 1
            index += 2
            while index < len(source) and block_depth:
                if source.startswith("/*", index):
                    block_depth += 1
                    index += 2
                elif source.startswith("*/", index):
                    block_depth -= 1
                    index += 2
                else:
                    index += 1
            if block_depth:
                fail("unterminated nested block comment")
            continue

        character = source[index]
        if character.isspace():
            index += 1
            continue
        raw = consume_raw_string(source, index, byte=True)
        if raw is None:
            raw = consume_raw_string(source, index, byte=False)
        if raw is not None:
            token, index = raw
            tokens.append(token)
            continue
        if source.startswith("r#", index):
            end = index + 2
            if (
                end >= len(source)
                or not source[end].isascii()
                or not (source[end].isalpha() or source[end] == "_")
            ):
                fail(f"invalid raw Rust identifier at byte offset {index}")
            end += 1
            while end < len(source) and source[end].isascii() and (
                source[end].isalnum() or source[end] == "_"
            ):
                end += 1
            tokens.append(Token("IDENT", source[index:end]))
            index = end
            continue
        if source.startswith('b"', index):
            token, index = consume_quoted(source, index + 2, '"', "BYTE_STRING")
            tokens.append(Token(token.kind, "b" + token.text))
            continue
        if character == '"':
            token, index = consume_quoted(source, index + 1, '"', "STRING")
            tokens.append(Token(token.kind, '"' + token.text))
            continue
        if character == "'":
            token, index = consume_quoted(source, index + 1, "'", "CHAR")
            tokens.append(Token(token.kind, "'" + token.text))
            continue
        if character.isascii() and (character.isalpha() or character == "_"):
            end = index + 1
            while end < len(source) and source[end].isascii() and (
                source[end].isalnum() or source[end] == "_"
            ):
                end += 1
            tokens.append(Token("IDENT", source[index:end]))
            index = end
            continue
        if source.startswith("::", index):
            tokens.append(Token("PUNCT", "::"))
            index += 2
            continue
        if character in "#![];*=(),{}":
            tokens.append(Token("PUNCT", character))
            index += 1
            continue
        fail(f"unrecognized Rust syntax at byte offset {index}")
    return tokens


def consume_docs(tokens: list[Token], index: int) -> int:
    """Consume only doc attributes; other attributes remain fail-closed syntax."""

    while index < len(tokens) and tokens[index].text == "#":
        start = index
        index += 1
        if index < len(tokens) and tokens[index].text == "!":
            index += 1
        if (
            index + 4 >= len(tokens)
            or tokens[index].text != "["
            or tokens[index + 1] != Token("IDENT", "doc")
            or tokens[index + 2].text != "="
            or tokens[index + 3].kind != "STRING"
            or tokens[index + 4].text != "]"
        ):
            return start
        index += 5
    return index


def parse_path(tokens: list[Token], index: int) -> tuple[list[str], int]:
    """Parse a named Rust use path and stop before a semicolon or explicit alias."""

    path: list[str] = []
    expect_identifier = True
    while index < len(tokens) and tokens[index].text not in {";", "as"}:
        token = tokens[index]
        if token.text == "*":
            fail("wildcard re-export is forbidden")
        if expect_identifier:
            if token.kind != "IDENT":
                fail("extra Rust syntax in public-path facade")
            path.append(token.text)
            expect_identifier = False
        else:
            if token.text != "::":
                fail("extra Rust syntax in public-path facade")
            path.append(token.text)
            expect_identifier = True
        index += 1
    if index >= len(tokens) or expect_identifier:
        fail("extra Rust syntax in public-path facade")
    return path, index


def parse_reexport(
    tokens: list[Token], index: int
) -> tuple[list[str], str | None, int]:
    """Parse one named public use item with an optional explicit alias."""

    path, index = parse_path(tokens, index)
    alias: str | None = None
    if tokens[index].text == "as":
        if index + 1 >= len(tokens) or tokens[index + 1].kind != "IDENT":
            fail("extra Rust syntax in public-path facade")
        alias = tokens[index + 1].text
        index += 2
    if index >= len(tokens) or tokens[index].text != ";":
        fail("extra Rust syntax in public-path facade")
    return path, alias, index + 1


def render_reexport(path: list[str], alias: str | None) -> str:
    """Render one normalized named use path for deterministic exact-set comparison."""

    rendered = "".join(path)
    return rendered if alias is None else f"{rendered} as {alias}"


def parse_facade_items(source: str) -> list[FacadeItem]:
    """Allow only module declarations and targeted public re-exports in a facade."""

    tokens = tokenize(source)
    items: list[FacadeItem] = []
    index = 0
    while index < len(tokens):
        index = consume_docs(tokens, index)
        if index >= len(tokens):
            break
        public = False
        if tokens[index] == Token("IDENT", "pub"):
            public = True
            index += 1
        if index >= len(tokens):
            fail("extra Rust syntax in public-path facade")
        if tokens[index] == Token("IDENT", "mod"):
            if (
                index + 2 >= len(tokens)
                or tokens[index + 1].kind != "IDENT"
                or tokens[index + 2].text != ";"
            ):
                fail("extra Rust syntax in public-path facade")
            items.append(FacadeItem("mod", public, tokens[index + 1].text))
            index += 3
            continue
        if tokens[index] == Token("IDENT", "use"):
            path, alias, index = parse_reexport(tokens, index + 1)
            items.append(FacadeItem("use", public, "".join(path), alias))
            continue
        fail("extra Rust syntax in public-path facade")
    return items


def exact_reexport(path: str) -> str:
    """Tokenize an expected named re-export and reject non-path syntax."""

    tokens = tokenize(path)
    parsed, alias, index = parse_reexport(tokens + [Token("PUNCT", ";")], 0)
    if index != len(tokens) + 1:
        fail("invalid expected re-export path")
    return render_reexport(parsed, alias)


def verify_rdbc_module_facades(root: Path) -> None:
    """Check the five declared RDBC module facades without compiling the workspace."""

    source_root = root / "crates" / "druid-rdbc" / "src"
    lib_rs = source_root / "lib.rs"
    if not lib_rs.is_file():
        fail(f"missing RDBC crate root: {lib_rs.relative_to(root)}")
    items = parse_facade_items(lib_rs.read_text(encoding="utf-8"))
    if any(item.kind != "mod" or not item.public for item in items):
        fail("RDBC crate root may contain only public module declarations")
    declared = {item.value for item in items}
    if declared != RDBC_MODULES:
        fail(
            "RDBC crate must declare exactly "
            f"{sorted(RDBC_MODULES)}, found {sorted(declared)}"
        )

    for module in sorted(RDBC_MODULES):
        facade = source_root / module / "mod.rs"
        if not facade.is_file():
            fail(f"missing target module: {facade.relative_to(root)}")
        parse_facade_items(facade.read_text(encoding="utf-8"))


def verify_shim(root: Path, mapping: ShimMapping) -> None:
    """Check that one old module has exactly one named directed public re-export."""

    target = root / mapping.target_module
    if not target.is_file():
        fail(f"missing target module: {mapping.target_module}")
    old_module = root / mapping.old_module
    if not old_module.is_file():
        fail(f"missing old module: {mapping.old_module}")

    tokens = tokenize(old_module.read_text(encoding="utf-8"))
    index = consume_docs(tokens, 0)
    if index == len(tokens):
        fail(f"shim target mismatch in {mapping.old_module}: expected exact re-export")
    if (
        index + 1 >= len(tokens)
        or tokens[index] != Token("IDENT", "pub")
        or tokens[index + 1] != Token("IDENT", "use")
    ):
        fail(f"extra Rust syntax in shim: {mapping.old_module}")
    actual_path, actual_alias, index = parse_reexport(tokens, index + 2)
    actual = render_reexport(actual_path, actual_alias)
    expected = exact_reexport(mapping.reexport)
    if actual != expected:
        fail(f"shim target mismatch in {mapping.old_module}: expected {mapping.reexport}")
    if index != len(tokens):
        fail(f"extra Rust syntax in shim: {mapping.old_module}")


def old_module_components(old_module: Path) -> tuple[str, ...]:
    """Derive a public module path from a Rust source file, including mod.rs facades."""

    source_root = Path("crates/druid/src")
    try:
        relative = old_module.relative_to(source_root)
    except ValueError:
        fail(f"old_module_file must be below {source_root}: {old_module}")
    if relative.name == "mod.rs":
        components = relative.parts[:-1]
    else:
        components = relative.parts[:-1] + (relative.stem,)
    if not components or any(not component.isidentifier() for component in components):
        fail(f"old_module_file has invalid module components: {old_module}")
    return components


def normalize_manifest_identifier(identifier: str, line_number: int) -> str:
    """Validate one ASCII Rust 2021 identifier and remove an allowed raw prefix."""

    raw = identifier.startswith("r#")
    candidate = identifier[2:] if raw else identifier
    if (
        not candidate
        or not candidate.isascii()
        or not (candidate[0].isalpha() or candidate[0] == "_")
        or any(not (character.isalnum() or character == "_") for character in candidate)
    ):
        fail(f"mapping row {line_number} has invalid Rust identifier: {identifier}")
    if raw:
        if candidate in RAW_IDENTIFIER_DISALLOWED:
            fail(f"mapping row {line_number} has invalid raw identifier: {identifier}")
        return candidate
    if candidate in RUST_2021_KEYWORDS:
        fail(f"mapping row {line_number} uses keyword without raw prefix: {identifier}")
    return candidate


def normalize_manifest_path(
    path: str, crate_name: str, line_number: int
) -> tuple[list[str], list[str]]:
    """Validate an absolute manifest path and return spelling plus normalized identifiers."""

    segments = path.split("::")
    if len(segments) < 2 or segments[0] != crate_name:
        fail(f"mapping row {line_number} has invalid path prefix: {path}")
    normalized = [segments[0]]
    for segment in segments[1:]:
        normalized.append(normalize_manifest_identifier(segment, line_number))
    return segments, normalized


def mapping_from_fields(fields: list[str], line_number: int) -> ManifestShim:
    """Convert one authoritative TSV row into a facade-targeted public path contract."""

    old_path, new_path, compile_mode, old_module_file = fields
    if any(not field or field != field.strip() for field in fields):
        fail(f"mapping row {line_number} has an empty required field")
    if compile_mode not in COMPILE_MODES:
        fail(f"mapping row {line_number} has invalid compile_mode: {compile_mode}")
    old_segments, normalized_old_segments = normalize_manifest_path(
        old_path, "druid", line_number
    )
    if len(old_segments) < 3:
        fail(f"mapping row {line_number} has invalid old_path: {old_path}")
    new_segments, normalized_new_segments = normalize_manifest_path(
        new_path, "druid_rdbc", line_number
    )
    if (
        len(new_segments) != 3
        or normalized_new_segments[1] not in RDBC_MODULES
    ):
        fail(f"mapping row {line_number} has invalid new_path: {new_path}")
    old_module = Path(old_module_file)
    if old_module.is_absolute() or old_module.suffix != ".rs" or ".." in old_module.parts:
        fail(f"mapping row {line_number} has invalid old_module_file: {old_module_file}")
    if tuple(normalized_old_segments[1:-1]) != old_module_components(old_module):
        fail(
            f"mapping row {line_number} old_path modules do not match old_module_file"
        )
    old_leaf = old_segments[-1]
    new_leaf = new_segments[-1]
    reexport = (
        new_path
        if normalized_old_segments[-1] == normalized_new_segments[-1]
        else f"{new_path} as {old_leaf}"
    )
    target_facade = Path("crates/druid-rdbc/src") / normalized_new_segments[1] / "mod.rs"
    return ManifestShim(
        old_module,
        target_facade,
        new_path,
        reexport,
        normalized_old_segments[-1],
    )


def load_mapping(mapping_path: Path, allow_empty_scaffold: bool) -> list[ShimGroup]:
    """Read the tracked public-path denominator and reject missing, malformed, or empty standard maps."""

    if not mapping_path.is_file():
        fail(f"mapping manifest not found: {mapping_path}")
    lines = mapping_path.read_text(encoding="utf-8").splitlines()
    if not lines or lines[0] != MAPPING_HEADER:
        fail(f"mapping manifest has invalid header: {mapping_path}")
    mappings_by_module: dict[Path, list[ManifestShim]] = {}
    seen_old_paths: set[str] = set()
    mode_by_new_path: dict[str, str] = {}
    for line_number, line in enumerate(lines[1:], start=2):
        if not line:
            fail(f"mapping row {line_number} is empty")
        fields = line.split("\t")
        if len(fields) != 4:
            fail(f"mapping row {line_number} must have four tab-separated fields")
        old_path = fields[0]
        if old_path in seen_old_paths:
            fail(f"mapping row {line_number} duplicates old_path: {old_path}")
        mapping = mapping_from_fields(fields, line_number)
        seen_old_paths.add(old_path)
        previous_mode = mode_by_new_path.setdefault(fields[1], fields[2])
        if previous_mode != fields[2]:
            fail(
                f"mapping row {line_number} has inconsistent compile_mode for {fields[1]}"
            )
        mappings_by_module.setdefault(mapping.old_module, []).append(mapping)
    if not mappings_by_module and not allow_empty_scaffold:
        fail("mapping manifest has no rows; --allow-empty-scaffold is required")
    groups: list[ShimGroup] = []
    for old_module, mappings in mappings_by_module.items():
        reexports = [mapping.reexport for mapping in mappings]
        if len(reexports) != len(set(reexports)):
            fail(f"mapping rows for {old_module} duplicate an expected re-export")
        groups.append(ShimGroup(old_module, tuple(mappings)))
    return groups


def verify_target_facade(root: Path, mapping: ManifestShim) -> None:
    """Require the public domain facade to expose the mapped RDBC leaf by name."""

    target = root / mapping.target_facade
    if not target.is_file():
        fail(f"missing target facade: {mapping.target_facade}")
    items = parse_facade_items(target.read_text(encoding="utf-8"))
    expected_leaf = mapping.new_path.rsplit("::", maxsplit=1)[1]
    if not any(
        item.kind == "use"
        and item.public
        and (item.alias or item.value.rsplit("::", maxsplit=1)[-1]) == expected_leaf
        for item in items
    ):
        fail(
            f"target facade does not expose {mapping.new_path}: "
            f"{mapping.target_facade}"
        )


def verify_shim_group(root: Path, group: ShimGroup) -> None:
    """Check one old module's exact set of normalized named compatibility re-exports."""

    for mapping in group.mappings:
        verify_target_facade(root, mapping)
    if group.old_module.name == "mod.rs":
        verify_parent_facade(root, group)
        return
    old_module = root / group.old_module
    if not old_module.is_file():
        fail(f"missing old module: {group.old_module}")
    tokens = tokenize(old_module.read_text(encoding="utf-8"))
    index = 0
    actual_reexports: list[str] = []
    while index < len(tokens):
        index = consume_docs(tokens, index)
        if index >= len(tokens):
            break
        if (
            index + 1 >= len(tokens)
            or tokens[index] != Token("IDENT", "pub")
            or tokens[index + 1] != Token("IDENT", "use")
        ):
            fail(f"extra Rust syntax in shim: {group.old_module}")
        path, alias, index = parse_reexport(tokens, index + 2)
        actual_reexports.append(render_reexport(path, alias))
    expected_reexports = {mapping.reexport for mapping in group.mappings}
    if len(actual_reexports) != len(set(actual_reexports)):
        fail(f"duplicate named re-export in shim: {group.old_module}")
    if set(actual_reexports) != expected_reexports:
        fail(f"shim target mismatch in {group.old_module}: expected exact re-export set")


def normalized_export_name(item: FacadeItem) -> str:
    """Return a source export's semantic leaf name while preserving raw-identifier identity."""

    name = item.alias or item.value.rsplit("::", maxsplit=1)[-1]
    return name[2:] if name.startswith("r#") else name


def verify_parent_facade(root: Path, group: ShimGroup) -> None:
    """Verify mapped aliases inside a parent facade without constraining unrelated baseline exports."""

    old_module = root / group.old_module
    if not old_module.is_file():
        fail(f"missing old module: {group.old_module}")
    expected_by_export: dict[str, str] = {}
    for mapping in group.mappings:
        if mapping.old_export in expected_by_export:
            fail(
                f"conflicting mapped export {mapping.old_export} in parent facade: "
                f"{group.old_module}"
            )
        expected_by_export[mapping.old_export] = mapping.reexport

    seen_exports: set[str] = set()
    for item in parse_facade_items(old_module.read_text(encoding="utf-8")):
        if item.kind != "use" or not item.public:
            continue
        exported_name = normalized_export_name(item)
        expected = expected_by_export.get(exported_name)
        if expected is None:
            continue
        actual = (
            item.value if item.alias is None else f"{item.value} as {item.alias}"
        )
        if actual != expected:
            fail(
                f"parent facade export {exported_name} has wrong target in "
                f"{group.old_module}"
            )
        if exported_name in seen_exports:
            fail(
                f"duplicate mapped export {exported_name} in parent facade: "
                f"{group.old_module}"
            )
        seen_exports.add(exported_name)
    missing = sorted(set(expected_by_export) - seen_exports)
    if missing:
        fail(
            f"parent facade missing mapped exports {missing} in {group.old_module}"
        )


def verify_source_tree(root: Path, mappings: list[ShimMapping | ShimGroup]) -> None:
    """Run the source-only facade and compatibility-shim verification."""

    verify_rdbc_module_facades(root)
    for mapping in mappings:
        if isinstance(mapping, ShimGroup):
            verify_shim_group(root, mapping)
        else:
            verify_shim(root, mapping)


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
        with self.assertRaisesRegex(VerificationError, "extra Rust syntax"):
            verify_source_tree(self.root, [self.mapping])

    def test_accepts_valid_thin_shim(self) -> None:
        """A single exact named re-export is a valid thin shim."""

        self.write_old_shim("pub use druid_rdbc::types::fixture_type::FixtureType;\n")
        verify_source_tree(self.root, [self.mapping])

    def test_rejects_block_commented_only_reexport(self) -> None:
        """Comment text cannot impersonate the only compatibility re-export."""

        self.write_old_shim(
            "/*\n"
            "pub use druid_rdbc::types::fixture_type::FixtureType;\n"
            "*/\n"
        )
        with self.assertRaisesRegex(VerificationError, "shim target mismatch"):
            verify_source_tree(self.root, [self.mapping])

    def test_rejects_public_unsafe_trait(self) -> None:
        """A shim may not hide a public unsafe trait behind the named re-export."""

        self.write_old_shim(
            "pub use druid_rdbc::types::fixture_type::FixtureType;\n"
            "pub unsafe trait UnsafeFixture {}\n"
        )
        with self.assertRaisesRegex(VerificationError, "extra Rust syntax"):
            verify_source_tree(self.root, [self.mapping])

    def test_accepts_commented_public_definition(self) -> None:
        """A definition-shaped comment is not a second public definition."""

        self.write_old_shim(
            "pub use druid_rdbc::types::fixture_type::FixtureType;\n"
            "/*\n"
            "pub struct CommentOnlyFixture;\n"
            "*/\n"
        )
        verify_source_tree(self.root, [self.mapping])

    def test_accepts_doc_strings_with_fake_items(self) -> None:
        """Normal and raw doc strings cannot be parsed as shim items."""

        self.write_old_shim(
            "/// This documentation says pub struct CommentOnlyFixture.\n"
            "#[doc = \"pub unsafe trait CommentOnlyUnsafe {}\"]\n"
            "#[doc = r###\"pub enum CommentOnlyEnum {}\"###]\n"
            "pub use druid_rdbc::types::fixture_type::FixtureType;\n"
        )
        verify_source_tree(self.root, [self.mapping])

    def test_accepts_nested_comments_with_fake_items(self) -> None:
        """Nested block comments cannot contribute false Rust definitions."""

        self.write_old_shim(
            "/* outer comment\n"
            "   /* nested pub struct CommentOnlyFixture; */\n"
            "*/\n"
            "pub use druid_rdbc::types::fixture_type::FixtureType;\n"
        )
        verify_source_tree(self.root, [self.mapping])

    def test_rejects_extra_char_literal_syntax(self) -> None:
        """Unrecognized expression syntax fails closed instead of becoming a false green."""

        self.write_old_shim(
            "pub use druid_rdbc::types::fixture_type::FixtureType;\n"
            "'x';\n"
        )
        with self.assertRaisesRegex(VerificationError, "extra Rust syntax"):
            verify_source_tree(self.root, [self.mapping])

    def test_rejects_extra_byte_string_syntax(self) -> None:
        """A byte string is tokenized but cannot become ignored extra shim syntax."""

        self.write_old_shim(
            "pub use druid_rdbc::types::fixture_type::FixtureType;\n"
            "b\"pub unsafe trait Hidden {}\";\n"
        )
        with self.assertRaisesRegex(VerificationError, "extra Rust syntax"):
            verify_source_tree(self.root, [self.mapping])

    def test_facade_allows_private_module_and_named_reexport(self) -> None:
        """A facade may route one private module through a named public re-export."""

        facade = self.root / "crates" / "druid-rdbc" / "src" / "types" / "mod.rs"
        facade.write_text(
            "mod fixture_type;\npub use fixture_type::FixtureType;\n",
            encoding="utf-8",
        )
        verify_source_tree(self.root, [])

    def test_facade_rejects_wildcards_and_public_definitions(self) -> None:
        """Facade parsing applies the same no-wildcard and no-definition boundary."""

        facade = self.root / "crates" / "druid-rdbc" / "src" / "types" / "mod.rs"
        facade.write_text(
            "pub use fixture_type::*;\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "wildcard re-export"):
            verify_source_tree(self.root, [])
        facade.write_text("pub unsafe trait HiddenFacade {}\n", encoding="utf-8")
        with self.assertRaisesRegex(VerificationError, "extra Rust syntax"):
            verify_source_tree(self.root, [])

    def test_empty_mapping_requires_explicit_scaffold_waiver(self) -> None:
        """The tracked zero-row scaffold map cannot pass default verifier mode."""

        mapping_file = self.root / "rdbc-public-path-map.tsv"
        mapping_file.write_text(MAPPING_HEADER + "\n", encoding="utf-8")
        with self.assertRaisesRegex(VerificationError, "no rows"):
            load_mapping(mapping_file, allow_empty_scaffold=False)
        self.assertEqual([], load_mapping(mapping_file, allow_empty_scaffold=True))

    def write_manifest(self, rows: list[tuple[str, str, str, str]]) -> Path:
        """Write an authoritative TSV fixture below this test's temporary repository."""

        mapping_file = self.root / "rdbc-public-path-map.tsv"
        contents = [MAPPING_HEADER, *("\t".join(row) for row in rows)]
        mapping_file.write_text("\n".join(contents) + "\n", encoding="utf-8")
        return mapping_file

    def prepare_direct_type_facade(self) -> None:
        """Create the direct public types::Value facade expected by complete mappings."""

        types_directory = self.root / "crates" / "druid-rdbc" / "src" / "types"
        (types_directory / "value.rs").write_text(
            "pub struct Value;\n",
            encoding="utf-8",
        )
        (types_directory / "mod.rs").write_text(
            "mod value;\npub use value::Value;\n",
            encoding="utf-8",
        )

    def test_maps_direct_type_to_domain_facade(self) -> None:
        """A direct druid_rdbc::types::Value mapping targets types/mod.rs, not types.rs."""

        self.prepare_direct_type_facade()
        old_module = self.root / "crates" / "druid" / "src" / "core" / "legacy_fixture.rs"
        old_module.parent.mkdir(parents=True, exist_ok=True)
        old_module.write_text("pub use druid_rdbc::types::Value;\n", encoding="utf-8")
        mapping_file = self.write_manifest(
            [
                (
                    "druid::core::legacy_fixture::Value",
                    "druid_rdbc::types::Value",
                    "OWNED",
                    "crates/druid/src/core/legacy_fixture.rs",
                )
            ]
        )
        verify_source_tree(self.root, load_mapping(mapping_file, False))

    def test_allows_multiple_public_paths_in_one_old_module(self) -> None:
        """Three rows can require three exact named re-exports from sql_exception.rs."""

        error_directory = self.root / "crates" / "druid-rdbc" / "src" / "error"
        (error_directory / "sql_exception.rs").write_text("", encoding="utf-8")
        (error_directory / "mod.rs").write_text(
            "mod sql_exception;\n"
            "pub use sql_exception::SqlException;\n"
            "pub use sql_exception::SqlWarning;\n"
            "pub use sql_exception::SqlExceptionKind;\n",
            encoding="utf-8",
        )
        old_module = self.root / "crates" / "druid" / "src" / "core" / "sql_exception.rs"
        old_module.parent.mkdir(parents=True, exist_ok=True)
        old_module.write_text(
            "pub use druid_rdbc::error::SqlException;\n"
            "pub use druid_rdbc::error::SqlWarning;\n"
            "pub use druid_rdbc::error::SqlExceptionKind;\n",
            encoding="utf-8",
        )
        mapping_file = self.write_manifest(
            [
                (
                    "druid::core::sql_exception::SqlException",
                    "druid_rdbc::error::SqlException",
                    "OWNED",
                    "crates/druid/src/core/sql_exception.rs",
                ),
                (
                    "druid::core::sql_exception::SqlWarning",
                    "druid_rdbc::error::SqlWarning",
                    "OWNED",
                    "crates/druid/src/core/sql_exception.rs",
                ),
                (
                    "druid::core::sql_exception::SqlExceptionKind",
                    "druid_rdbc::error::SqlExceptionKind",
                    "OWNED",
                    "crates/druid/src/core/sql_exception.rs",
                ),
            ]
        )
        verify_source_tree(self.root, load_mapping(mapping_file, False))

    def test_alias_row_requires_old_leaf_alias(self) -> None:
        """An old SQLException name is preserved with an explicit alias to SqlException."""

        error_directory = self.root / "crates" / "druid-rdbc" / "src" / "error"
        (error_directory / "sql_exception.rs").write_text("", encoding="utf-8")
        (error_directory / "mod.rs").write_text(
            "mod sql_exception;\npub use sql_exception::SqlException;\n",
            encoding="utf-8",
        )
        old_module = self.root / "crates" / "druid" / "src" / "sql" / "exceptions.rs"
        old_module.parent.mkdir(parents=True, exist_ok=True)
        old_module.write_text(
            "pub use druid_rdbc::error::SqlException as SQLException;\n",
            encoding="utf-8",
        )
        mapping_file = self.write_manifest(
            [
                (
                    "druid::sql::exceptions::SQLException",
                    "druid_rdbc::error::SqlException",
                    "DYN_TRAIT",
                    "crates/druid/src/sql/exceptions.rs",
                )
            ]
        )
        verify_source_tree(self.root, load_mapping(mapping_file, False))

    def test_rejects_invalid_manifest_rows(self) -> None:
        """Modes, crate prefixes, module paths, duplicate old paths, and modes are schema-bound."""

        invalid_rows = [
            (
                "druid::core::legacy_fixture::FixtureType",
                "druid_rdbc::types::Value",
                "BANANA",
                "crates/druid/src/core/legacy_fixture.rs",
            ),
            (
                "other::core::legacy_fixture::FixtureType",
                "druid_rdbc::types::Value",
                "OWNED",
                "crates/druid/src/core/legacy_fixture.rs",
            ),
            (
                "druid::core::wrong_module::FixtureType",
                "druid_rdbc::types::Value",
                "OWNED",
                "crates/druid/src/core/legacy_fixture.rs",
            ),
        ]
        for row in invalid_rows:
            with self.subTest(row=row):
                with self.assertRaises(VerificationError):
                    load_mapping(self.write_manifest([row]), False)

        duplicate_old = (
            "druid::core::legacy_fixture::FixtureType",
            "druid_rdbc::types::Value",
            "OWNED",
            "crates/druid/src/core/other_fixture.rs",
        )
        first_old = (
            "druid::core::legacy_fixture::FixtureType",
            "druid_rdbc::types::Value",
            "OWNED",
            "crates/druid/src/core/legacy_fixture.rs",
        )
        with self.assertRaisesRegex(VerificationError, "old_path"):
            load_mapping(self.write_manifest([first_old, duplicate_old]), False)

        inconsistent_mode = (
            "druid::core::other_fixture::OtherFixture",
            "druid_rdbc::types::Value",
            "PATH_ONLY",
            "crates/druid/src/core/other_fixture.rs",
        )
        first_mode = (
            "druid::core::legacy_fixture::FixtureType",
            "druid_rdbc::types::Value",
            "OWNED",
            "crates/druid/src/core/legacy_fixture.rs",
        )
        with self.assertRaisesRegex(VerificationError, "inconsistent compile_mode"):
            load_mapping(self.write_manifest([first_mode, inconsistent_mode]), False)

    def test_parent_facade_keeps_unrelated_exports(self) -> None:
        """A mapped parent mod.rs can retain unrelated baseline modules and uses."""

        self.prepare_direct_type_facade()
        parent = self.root / "crates" / "druid" / "src" / "core" / "mod.rs"
        parent.parent.mkdir(parents=True, exist_ok=True)
        parent.write_text(
            "pub mod untouched;\n"
            "use private_baseline::Hidden;\n"
            "pub use unrelated_baseline::Other;\n"
            "pub use druid_rdbc::types::Value;\n",
            encoding="utf-8",
        )
        mapping_file = self.write_manifest(
            [
                (
                    "druid::core::Value",
                    "druid_rdbc::types::Value",
                    "OWNED",
                    "crates/druid/src/core/mod.rs",
                )
            ]
        )
        verify_source_tree(self.root, load_mapping(mapping_file, False))

    def test_parent_facade_rejects_bad_mapped_exports(self) -> None:
        """Parent rows reject wrong, missing, duplicate, and wildcard mapped exports."""

        self.prepare_direct_type_facade()
        parent = self.root / "crates" / "druid" / "src" / "core" / "mod.rs"
        parent.parent.mkdir(parents=True, exist_ok=True)
        mapping_file = self.write_manifest(
            [
                (
                    "druid::core::Value",
                    "druid_rdbc::types::Value",
                    "OWNED",
                    "crates/druid/src/core/mod.rs",
                )
            ]
        )
        for source in (
            "pub use druid_rdbc::types::Other as Value;\n",
            "pub use unrelated_baseline::Other;\n",
            "pub use druid_rdbc::types::Value;\n"
            "pub use unrelated_baseline::Other as Value;\n",
            "pub use druid_rdbc::types::*;\n",
        ):
            with self.subTest(source=source):
                parent.write_text(source, encoding="utf-8")
                with self.assertRaises(VerificationError):
                    verify_source_tree(self.root, load_mapping(mapping_file, False))

    def test_manifest_uses_rust_2021_identifier_policy(self) -> None:
        """Manifest paths distinguish forbidden plain keywords from valid and invalid raw identifiers."""

        for keyword in ("type", "match", "async", "crate", "self"):
            with self.subTest(plain_old_keyword=keyword):
                row = (
                    f"druid::core::{keyword}::Value",
                    "druid_rdbc::types::Value",
                    "OWNED",
                    f"crates/druid/src/core/{keyword}.rs",
                )
                with self.assertRaises(VerificationError):
                    load_mapping(self.write_manifest([row]), False)
            with self.subTest(plain_new_keyword=keyword):
                row = (
                    "druid::core::value::Value",
                    f"druid_rdbc::types::{keyword}",
                    "OWNED",
                    "crates/druid/src/core/value.rs",
                )
                with self.assertRaises(VerificationError):
                    load_mapping(self.write_manifest([row]), False)

        raw_row = (
            "druid::core::r#type::r#match",
            "druid_rdbc::types::r#type",
            "OWNED",
            "crates/druid/src/core/type.rs",
        )
        groups = load_mapping(self.write_manifest([raw_row]), False)
        self.assertEqual(
            "druid_rdbc::types::r#type as r#match",
            groups[0].mappings[0].reexport,
        )

        for special in ("crate", "self", "super", "Self", "_"):
            with self.subTest(special_raw_identifier=special):
                row = (
                    f"druid::core::r#{special}::Value",
                    "druid_rdbc::types::Value",
                    "OWNED",
                    f"crates/druid/src/core/{special}.rs",
                )
                with self.assertRaises(VerificationError):
                    load_mapping(self.write_manifest([row]), False)


def main() -> int:
    """Run the requested source-only verifier mode or its isolated unit fixtures."""

    parser = argparse.ArgumentParser()
    parser.add_argument("--source-only", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--root", type=Path, default=repository_root())
    parser.add_argument(
        "--mapping",
        type=Path,
        default=Path("docs/verification/rdbc-first/rdbc-public-path-map.tsv"),
    )
    parser.add_argument("--allow-empty-scaffold", action="store_true")
    parser.add_argument("--compile-all-pairs", action="store_true")
    parser.add_argument("--baseline", type=Path)
    parser.add_argument("--after", type=Path)
    parser.add_argument("--allowlist", type=Path)
    parser.add_argument("--api-diff", type=Path)
    arguments = parser.parse_args()

    if arguments.self_test:
        if any(
            (
                arguments.source_only,
                arguments.allow_empty_scaffold,
                arguments.compile_all_pairs,
                arguments.baseline,
                arguments.after,
                arguments.allowlist,
                arguments.api_diff,
            )
        ):
            parser.error("--self-test cannot be combined with verifier modes")
        result = unittest.main(argv=[sys.argv[0]], exit=False)
        return 0 if result.result.wasSuccessful() else 1
    api_mode = any(
        (arguments.baseline, arguments.after, arguments.allowlist, arguments.api_diff)
    )
    if arguments.allow_empty_scaffold and (
        arguments.compile_all_pairs or api_mode or not arguments.source_only
    ):
        parser.error("--allow-empty-scaffold is only valid with --source-only")
    if arguments.compile_all_pairs or api_mode:
        parser.error("compile/API public-path modes are not implemented by Task 1")
    if not arguments.source_only:
        parser.error("--source-only is required unless --self-test is selected")
    try:
        mapping_path = arguments.mapping
        if not mapping_path.is_absolute():
            mapping_path = repository_root() / mapping_path
        mappings = load_mapping(mapping_path, arguments.allow_empty_scaffold)
        verify_source_tree(arguments.root.resolve(), mappings)
    except VerificationError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(
        "PASS: RDBC public-path source verification "
        f"({len(RDBC_MODULES)} declared modules, {len(mappings)} shim mappings)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
