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
MAPPING_HEADER = "old_path\tnew_path\tcompile_mode\told_module_file\tsource_mode"
COMPILE_MODES = {"OWNED", "DYN_TRAIT", "PATH_ONLY"}
SOURCE_MODES = {"PURE_SHIM", "FACADE_MEMBERSHIP"}
MINIMUM_LIVE_MAPPING_ROWS = 285
REQUIRED_SECONDARY_ROWS = (
    (
        "druid::core::connection::ExecResult",
        "druid_rdbc::spi::ExecResult",
        "OWNED",
        "crates/druid/src/core/connection.rs",
        "FACADE_MEMBERSHIP",
    ),
    (
        "druid::core::connection::PhysicalConnection",
        "druid_rdbc::spi::PhysicalConnection",
        "DYN_TRAIT",
        "crates/druid/src/core/connection.rs",
        "FACADE_MEMBERSHIP",
    ),
    (
        "druid::core::connection::Connection",
        "druid_rdbc::spi::PhysicalConnection",
        "DYN_TRAIT",
        "crates/druid/src/core/connection.rs",
        "FACADE_MEMBERSHIP",
    ),
    (
        "druid::core::connection_factory::PhysicalConnectionFactory",
        "druid_rdbc::spi::PhysicalConnectionFactory",
        "DYN_TRAIT",
        "crates/druid/src/core/connection_factory.rs",
        "FACADE_MEMBERSHIP",
    ),
    (
        "druid::core::connection_factory::ConnectionFactory",
        "druid_rdbc::spi::PhysicalConnectionFactory",
        "DYN_TRAIT",
        "crates/druid/src/core/connection_factory.rs",
        "FACADE_MEMBERSHIP",
    ),
    (
        "druid::sql::row_id::RowId",
        "druid_rdbc::types::RdbcRowId",
        "OWNED",
        "crates/druid/src/sql/row_id.rs",
        "FACADE_MEMBERSHIP",
    ),
    (
        "druid::sql::RowId",
        "druid_rdbc::types::RdbcRowId",
        "OWNED",
        "crates/druid/src/sql/mod.rs",
        "FACADE_MEMBERSHIP",
    ),
    (
        "druid::sql::row_id_lifetime::RowIdLifetime",
        "druid_rdbc::spi::DatabaseMetaDataRowIdLifetime",
        "OWNED",
        "crates/druid/src/sql/row_id_lifetime.rs",
        "FACADE_MEMBERSHIP",
    ),
    (
        "druid::sql::RowIdLifetime",
        "druid_rdbc::spi::DatabaseMetaDataRowIdLifetime",
        "OWNED",
        "crates/druid/src/sql/mod.rs",
        "FACADE_MEMBERSHIP",
    ),
    (
        "druid::sql::result_set_meta_data::ResultSetMetaData",
        "druid_rdbc::spi::ResultSetMetaData",
        "OWNED",
        "crates/druid/src/sql/result_set_meta_data.rs",
        "FACADE_MEMBERSHIP",
    ),
    (
        "druid::sql::ResultSetMetaData",
        "druid_rdbc::spi::ResultSetMetaData",
        "OWNED",
        "crates/druid/src/sql/mod.rs",
        "FACADE_MEMBERSHIP",
    ),
)
RUST_2021_STRICT_RESERVED_KEYWORDS = {
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else",
    "enum", "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop",
    "match", "mod", "move", "mut", "pub", "ref", "return", "self", "Self", "static",
    "struct", "super", "trait", "true", "type", "unsafe", "use", "where",
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
class TopLevelBinding:
    """One semantic name bound in a parent module's top-level namespace."""

    kind: str
    public: bool
    name: str
    target: tuple[str, ...] | None = None


@dataclass(frozen=True)
class ManifestShim:
    """One public path row normalized into a directed old-module re-export contract."""

    old_path: str
    old_module: Path
    target_facade: Path
    new_path: str
    compile_mode: str
    source_mode: str
    canonical_new_path: tuple[str, ...]
    reexport: str
    old_export: str


@dataclass(frozen=True)
class ShimGroup:
    """All public definitions owned by the same old module file."""

    old_module: Path
    source_mode: str
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


def consume_character_or_lifetime(source: str, start: int) -> tuple[Token, int]:
    """Consume a character literal or a Rust lifetime/label beginning with an apostrophe."""

    after_quote = start + 1
    if after_quote >= len(source):
        fail("unterminated Rust character or lifetime")
    character = source[after_quote]
    if character.isascii() and (character.isalpha() or character == "_"):
        end = after_quote + 1
        while end < len(source) and source[end].isascii() and (
            source[end].isalnum() or source[end] == "_"
        ):
            end += 1
        if end >= len(source) or source[end] != "'":
            return Token("LIFETIME", source[start:end]), end
    token, end = consume_quoted(source, after_quote, "'", "CHAR")
    return Token(token.kind, "'" + token.text), end


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
            token, index = consume_character_or_lifetime(source, index)
            tokens.append(token)
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
        tokens.append(Token("PUNCT", character))
        index += 1
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
        facade_items = parse_facade_items(facade.read_text(encoding="utf-8"))
        if any(item.kind == "mod" and item.public for item in facade_items):
            fail(
                "RDBC domain facades may declare implementation modules only privately"
            )


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
    if not components or any(
        not is_ascii_identifier_shape(component) or component == "_"
        for component in components
    ):
        fail(f"old_module_file has invalid module components: {old_module}")
    return components


def is_ascii_identifier_shape(identifier: str) -> bool:
    """Return whether text has the lexical shape of an ASCII Rust identifier."""

    return bool(identifier) and identifier.isascii() and (
        identifier[0].isalpha() or identifier[0] == "_"
    ) and all(character.isalnum() or character == "_" for character in identifier)


def normalize_manifest_identifier(identifier: str, line_number: int) -> str:
    """Validate one ASCII Rust 2021 identifier and remove an allowed raw prefix."""

    raw = identifier.startswith("r#")
    candidate = identifier[2:] if raw else identifier
    if not is_ascii_identifier_shape(candidate):
        fail(f"mapping row {line_number} has invalid Rust identifier: {identifier}")
    if raw:
        if candidate in RAW_IDENTIFIER_DISALLOWED:
            fail(f"mapping row {line_number} has invalid raw identifier: {identifier}")
        if candidate not in RUST_2021_STRICT_RESERVED_KEYWORDS:
            fail(
                f"mapping row {line_number} raw identifier must escape a Rust 2021 "
                f"keyword: {identifier}"
            )
        return candidate
    if candidate == "_":
        fail(f"mapping row {line_number} uses reserved identifier _: {identifier}")
    if candidate in RUST_2021_STRICT_RESERVED_KEYWORDS:
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

    old_path, new_path, compile_mode, old_module_file, source_mode = fields
    if any(not field or field != field.strip() for field in fields):
        fail(f"mapping row {line_number} has an empty required field")
    if compile_mode not in COMPILE_MODES:
        fail(f"mapping row {line_number} has invalid compile_mode: {compile_mode}")
    if source_mode not in SOURCE_MODES:
        fail(f"mapping row {line_number} has invalid source_mode: {source_mode}")
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
    if old_module.name == "mod.rs" and source_mode != "FACADE_MEMBERSHIP":
        fail(
            f"mapping row {line_number} parent mod.rs requires FACADE_MEMBERSHIP"
        )
    if tuple(normalized_old_segments[1:-1]) != old_module_components(old_module):
        fail(
            f"mapping row {line_number} old_path modules do not match old_module_file"
        )
    old_leaf = old_segments[-1]
    reexport = (
        new_path
        if normalized_old_segments[-1] == normalized_new_segments[-1]
        else f"{new_path} as {old_leaf}"
    )
    target_facade = Path("crates/druid-rdbc/src") / normalized_new_segments[1] / "mod.rs"
    return ManifestShim(
        old_path,
        old_module,
        target_facade,
        new_path,
        compile_mode,
        source_mode,
        tuple(normalized_new_segments),
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
    source_mode_by_module: dict[Path, str] = {}
    for line_number, line in enumerate(lines[1:], start=2):
        if not line:
            fail(f"mapping row {line_number} is empty")
        fields = line.split("\t")
        if len(fields) != 5:
            fail(f"mapping row {line_number} must have five tab-separated fields")
        mapping = mapping_from_fields(fields, line_number)
        normalized_old_path = "::".join(
            normalize_manifest_path(fields[0], "druid", line_number)[1]
        )
        if normalized_old_path in seen_old_paths:
            fail(f"mapping row {line_number} duplicates old_path: {fields[0]}")
        seen_old_paths.add(normalized_old_path)
        normalized_new_path = "::".join(mapping.canonical_new_path)
        previous_mode = mode_by_new_path.setdefault(normalized_new_path, fields[2])
        if previous_mode != fields[2]:
            fail(
                f"mapping row {line_number} has inconsistent compile_mode for "
                f"{normalized_new_path}"
            )
        previous_source_mode = source_mode_by_module.setdefault(
            mapping.old_module, mapping.source_mode
        )
        if previous_source_mode != mapping.source_mode:
            fail(
                f"mapping row {line_number} has inconsistent source_mode for "
                f"{mapping.old_module}"
            )
        mappings_by_module.setdefault(mapping.old_module, []).append(mapping)
    if not mappings_by_module and not allow_empty_scaffold:
        fail("mapping manifest has no rows; --allow-empty-scaffold is required")
    groups: list[ShimGroup] = []
    for old_module, mappings in mappings_by_module.items():
        reexports = [
            (mapping.canonical_new_path, mapping.old_export) for mapping in mappings
        ]
        if len(reexports) != len(set(reexports)):
            fail(f"mapping rows for {old_module} duplicate an expected re-export")
        groups.append(
            ShimGroup(old_module, source_mode_by_module[old_module], tuple(mappings))
        )
    return groups


def verify_required_secondary_denominator(
    mappings: list[ShimGroup], minimum_rows: int | None = None
) -> None:
    """Require every reviewed secondary facade path with its exact mapping contract."""

    observed = {
        (
            mapping.old_path,
            mapping.new_path,
            mapping.compile_mode,
            str(mapping.old_module),
            mapping.source_mode,
        )
        for group in mappings
        for mapping in group.mappings
    }
    missing = sorted(set(REQUIRED_SECONDARY_ROWS) - observed)
    if missing:
        fail(
            "mapping manifest is missing reviewed secondary public paths: "
            + ", ".join(row[0] for row in missing)
        )
    if minimum_rows is not None and len(observed) < minimum_rows:
        fail(
            f"mapping manifest has {len(observed)} rows; expected at least {minimum_rows}"
        )


def verify_target_facade(root: Path, mapping: ManifestShim) -> None:
    """Require the public domain facade to expose the mapped RDBC leaf by name."""

    target = root / mapping.target_facade
    if not target.is_file():
        fail(f"missing target facade: {mapping.target_facade}")
    items = parse_facade_items(target.read_text(encoding="utf-8"))
    expected_leaf = mapping.canonical_new_path[-1]
    if not any(
        item.kind == "use"
        and item.public
        and normalized_export_name(item) == expected_leaf
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
    if group.source_mode == "FACADE_MEMBERSHIP":
        verify_facade_membership(root, group)
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


DELIMITER_PAIRS = {"(": ")", "[": "]", "{": "}"}


def skip_delimited(tokens: list[Token], index: int, context: str) -> int:
    """Skip one balanced Rust delimiter tree and return the first following token."""

    if index >= len(tokens) or tokens[index].text not in DELIMITER_PAIRS:
        fail(f"malformed {context}")
    stack = [DELIMITER_PAIRS[tokens[index].text]]
    index += 1
    while index < len(tokens) and stack:
        token = tokens[index].text
        if token in DELIMITER_PAIRS:
            stack.append(DELIMITER_PAIRS[token])
        elif token in DELIMITER_PAIRS.values():
            if token != stack[-1]:
                fail(f"unbalanced delimiters in {context}")
            stack.pop()
        index += 1
    if stack:
        fail(f"unterminated delimiters in {context}")
    return index


def consume_outer_attributes(tokens: list[Token], index: int) -> int:
    """Consume arbitrary top-level outer or inner attributes before one Rust item."""

    while index < len(tokens) and tokens[index].text == "#":
        index += 1
        if index < len(tokens) and tokens[index].text == "!":
            index += 1
        index = skip_delimited(tokens, index, "top-level attribute")
    return index


def consume_visibility(tokens: list[Token], index: int) -> tuple[bool, int]:
    """Consume Rust visibility and report whether it is unrestricted public visibility."""

    if index >= len(tokens) or tokens[index].text != "pub":
        return False, index
    index += 1
    if index < len(tokens) and tokens[index].text == "(":
        return False, skip_delimited(tokens, index, "restricted visibility")
    return True, index


def canonical_source_identifier(
    token: Token, context: str, *, allow_path_special: bool = False
) -> str:
    """Normalize one source identifier while rejecting syntax rustc cannot bind."""

    if token.kind != "IDENT":
        fail(f"unsupported {context}")
    raw = token.text.startswith("r#")
    candidate = token.text[2:] if raw else token.text
    if not is_ascii_identifier_shape(candidate):
        fail(f"unsupported {context}")
    if raw:
        if candidate in RAW_IDENTIFIER_DISALLOWED:
            fail(f"invalid raw identifier in {context}: {token.text}")
        return candidate
    if candidate == "_":
        return candidate
    if candidate in RUST_2021_STRICT_RESERVED_KEYWORDS and not (
        allow_path_special and candidate in {"crate", "self", "super", "Self"}
    ):
        fail(f"unsupported {context}")
    return candidate


def parse_use_group(
    tokens: list[Token], index: int, prefix: tuple[str, ...], public: bool
) -> tuple[list[TopLevelBinding], int]:
    """Parse one grouped Rust use tree, including nested groups and self aliases."""

    if index >= len(tokens) or tokens[index].text != "{":
        fail("unsupported top-level use syntax")
    bindings: list[TopLevelBinding] = []
    index += 1
    while index < len(tokens) and tokens[index].text != "}":
        parsed, index = parse_use_tree(tokens, index, prefix, public)
        bindings.extend(parsed)
        if index < len(tokens) and tokens[index].text == ",":
            index += 1
            continue
        if index >= len(tokens) or tokens[index].text != "}":
            fail("unsupported top-level use syntax")
    if index >= len(tokens):
        fail("unsupported top-level use syntax")
    return bindings, index + 1


def parse_use_tree(
    tokens: list[Token], index: int, prefix: tuple[str, ...], public: bool
) -> tuple[list[TopLevelBinding], int]:
    """Parse a Rust 2021 use tree into every semantic top-level binding it creates."""

    if index < len(tokens) and tokens[index].text == "::":
        if prefix:
            fail("unsupported top-level use syntax")
        index += 1
    if index >= len(tokens):
        fail("unsupported top-level use syntax")
    if tokens[index].text == "{":
        return parse_use_group(tokens, index, prefix, public)
    if tokens[index].text == "*":
        fail("wildcard use is forbidden in parent facade")

    source_token = tokens[index]
    segment = canonical_source_identifier(
        source_token, "top-level use syntax", allow_path_special=True
    )
    if segment == "_":
        fail("unsupported top-level use syntax")
    index += 1
    path = prefix + (segment,)
    if index < len(tokens) and tokens[index].text == "::":
        index += 1
        if index < len(tokens) and tokens[index].text == "*":
            fail("wildcard use is forbidden in parent facade")
        if index < len(tokens) and tokens[index].text == "{":
            return parse_use_group(tokens, index, path, public)
        return parse_use_tree(tokens, index, path, public)

    target = prefix if source_token.text == "self" and prefix else path
    name = prefix[-1] if source_token.text == "self" and prefix else segment
    if index < len(tokens) and tokens[index].text == "as":
        index += 1
        if index >= len(tokens):
            fail("unsupported top-level use syntax")
        name = canonical_source_identifier(tokens[index], "top-level use alias")
        index += 1
    if name == "_":
        return [], index
    return [TopLevelBinding("use", public, name, target)], index


def parse_use_item(
    tokens: list[Token], index: int, public: bool
) -> tuple[list[TopLevelBinding], int]:
    """Parse one complete top-level use item and require its terminating semicolon."""

    bindings, index = parse_use_tree(tokens, index, (), public)
    if index >= len(tokens) or tokens[index].text != ";":
        fail("unsupported top-level use syntax")
    return bindings, index + 1


def skip_complete_item(
    tokens: list[Token], index: int, *, semicolon_terminated: bool = False
) -> int:
    """Skip one complete top-level item without interpreting its unrelated implementation."""

    stack: list[str] = []
    while index < len(tokens):
        token = tokens[index].text
        if token in DELIMITER_PAIRS:
            stack.append(DELIMITER_PAIRS[token])
        elif token in DELIMITER_PAIRS.values():
            if not stack or token != stack[-1]:
                fail("unbalanced delimiters in parent facade item")
            stack.pop()
            if not stack and token == "}" and not semicolon_terminated:
                return index + 1
        elif token == ";" and not stack:
            return index + 1
        index += 1
    fail("unterminated top-level item in parent facade")


def named_binding(
    tokens: list[Token], index: int, kind: str, public: bool
) -> TopLevelBinding | None:
    """Create a normalized item binding, ignoring anonymous const underscore items."""

    if index >= len(tokens):
        fail(f"missing name for top-level {kind} item")
    name = canonical_source_identifier(tokens[index], f"top-level {kind} item")
    if name == "_":
        return None
    return TopLevelBinding(kind, public, name)


def find_matching_brace(tokens: list[Token], index: int, context: str) -> tuple[int, int]:
    """Return the token bounds inside one balanced brace block."""

    if index >= len(tokens) or tokens[index].text != "{":
        fail(f"malformed {context}")
    after = skip_delimited(tokens, index, context)
    return index + 1, after - 1


def skip_union_macro_invocation(tokens: list[Token], index: int) -> int:
    """Skip one contextual union! item macro without creating a Rust binding."""

    if index + 1 >= len(tokens) or tokens[index + 1].text != "!":
        fail("malformed union macro invocation")
    after = skip_delimited(tokens, index + 2, "union macro invocation")
    if after < len(tokens) and tokens[after].text == ";":
        after += 1
    return after


def parse_extern_crate_item(
    tokens: list[Token], index: int, public: bool
) -> tuple[TopLevelBinding | None, int]:
    """Parse an Edition-2021 extern-crate item and return only its bound name."""

    if index >= len(tokens):
        fail("missing source for top-level extern crate item")
    source_token = tokens[index]
    if source_token.text == "self":
        source_name = "self"
    else:
        source_name = canonical_source_identifier(
            source_token, "top-level extern crate item"
        )
        if source_name == "_":
            fail("unsupported top-level extern crate item")
    index += 1

    alias: str | None = None
    if index < len(tokens) and tokens[index].text == "as":
        index += 1
        if index >= len(tokens):
            fail("missing top-level extern crate alias")
        alias = canonical_source_identifier(
            tokens[index], "top-level extern crate alias"
        )
        index += 1
    elif source_name == "self":
        fail("extern crate self requires an alias")

    if index >= len(tokens) or tokens[index].text != ";":
        fail("unsupported top-level extern crate item")
    bound_name = alias if alias is not None else source_name
    binding = (
        None
        if bound_name == "_"
        else TopLevelBinding("extern crate", public, bound_name)
    )
    return binding, index + 1


def scan_top_level_bindings_tokens(
    tokens: list[Token], start: int = 0, end: int | None = None
) -> list[TopLevelBinding]:
    """Scan direct Rust item bindings while safely skipping unrelated complete items."""

    limit = len(tokens) if end is None else end
    bindings: list[TopLevelBinding] = []
    index = start
    while index < limit:
        item_start = index
        index = consume_outer_attributes(tokens, index)
        if index >= limit:
            break
        visibility_start = index
        public, index = consume_visibility(tokens, index)
        has_visibility = index != visibility_start
        if index >= limit:
            fail("missing top-level item after visibility")

        keyword_index = index
        while index < limit and tokens[index].text in {
            "async",
            "auto",
            "default",
            "unsafe",
        }:
            index += 1
        if index >= limit:
            fail("missing top-level item after qualifier")
        keyword = tokens[index].text

        if keyword == "use" and keyword_index == index:
            parsed, index = parse_use_item(tokens, index + 1, public)
            bindings.extend(parsed)
            continue

        if keyword == "extern":
            cursor = index + 1
            if cursor < limit and tokens[cursor].text == "crate":
                binding, index = parse_extern_crate_item(tokens, cursor + 1, public)
                if binding is not None:
                    bindings.append(binding)
                continue
            if cursor < limit and tokens[cursor].kind in {"STRING", "BYTE_STRING"}:
                cursor += 1
            if cursor < limit and tokens[cursor].text == "{":
                inner_start, inner_end = find_matching_brace(
                    tokens, cursor, "extern block"
                )
                bindings.extend(
                    scan_top_level_bindings_tokens(tokens, inner_start, inner_end)
                )
                index = inner_end + 1
                if index < limit and tokens[index].text == ";":
                    index += 1
                continue
            if cursor < limit and tokens[cursor].text == "fn":
                binding = named_binding(tokens, cursor + 1, "fn", public)
                if binding is not None:
                    bindings.append(binding)
            index = skip_complete_item(tokens, item_start)
            continue

        if keyword == "const":
            cursor = index + 1
            while cursor < limit and tokens[cursor].text in {"async", "unsafe"}:
                cursor += 1
            if cursor < limit and tokens[cursor].text == "extern":
                cursor += 1
                if cursor < limit and tokens[cursor].kind == "STRING":
                    cursor += 1
            if cursor < limit and tokens[cursor].text == "fn":
                binding = named_binding(tokens, cursor + 1, "fn", public)
                semicolon_terminated = False
            else:
                binding = named_binding(tokens, index + 1, "const", public)
                semicolon_terminated = True
            if binding is not None:
                bindings.append(binding)
            index = skip_complete_item(
                tokens, item_start, semicolon_terminated=semicolon_terminated
            )
            continue

        if keyword == "static":
            cursor = index + 1
            if cursor < limit and tokens[cursor].text == "mut":
                cursor += 1
            binding = named_binding(tokens, cursor, "static", public)
            if binding is not None:
                bindings.append(binding)
            index = skip_complete_item(tokens, item_start, semicolon_terminated=True)
            continue

        if keyword == "macro_rules":
            if index + 2 >= limit or tokens[index + 1].text != "!":
                fail("unsupported top-level macro declaration")
            binding = named_binding(tokens, index + 2, "macro", public)
            if binding is not None:
                bindings.append(binding)
            index = skip_complete_item(tokens, item_start)
            continue

        if keyword == "macro":
            binding = named_binding(tokens, index + 1, "macro", public)
            if binding is not None:
                bindings.append(binding)
            index = skip_complete_item(tokens, item_start)
            continue

        if (
            keyword == "union"
            and keyword_index == index
            and index + 1 < limit
            and tokens[index + 1].text == "!"
        ):
            if has_visibility:
                fail("union macro invocation cannot have visibility")
            index = skip_union_macro_invocation(tokens, index)
            continue

        if keyword in {"mod", "struct", "enum", "trait", "type", "union", "fn"}:
            binding = named_binding(tokens, index + 1, keyword, public)
            if binding is not None:
                bindings.append(binding)
            index = skip_complete_item(
                tokens, item_start, semicolon_terminated=keyword == "type"
            )
            continue

        index = skip_complete_item(tokens, item_start)
    return bindings


def scan_top_level_bindings(source: str) -> list[TopLevelBinding]:
    """Tokenize one parent module and return all direct semantic bindings."""

    return scan_top_level_bindings_tokens(tokenize(source))


def verify_facade_membership(root: Path, group: ShimGroup) -> None:
    """Verify mapped aliases without constraining unrelated parent or secondary facade exports."""

    old_module = root / group.old_module
    if not old_module.is_file():
        fail(f"missing old module: {group.old_module}")
    expected_by_export: dict[str, tuple[str, ...]] = {}
    for mapping in group.mappings:
        if mapping.old_export in expected_by_export:
            fail(
                f"conflicting mapped export {mapping.old_export} in compatibility facade: "
                f"{group.old_module}"
            )
        expected_by_export[mapping.old_export] = mapping.canonical_new_path

    bindings = scan_top_level_bindings(old_module.read_text(encoding="utf-8"))
    for exported_name, expected_target in expected_by_export.items():
        matching = [binding for binding in bindings if binding.name == exported_name]
        if not matching:
            fail(
                f"compatibility facade missing mapped exports ['{exported_name}'] in "
                f"{group.old_module}"
            )
        if len(matching) == 1:
            binding = matching[0]
            if binding.kind == "use" and binding.public:
                if binding.target != expected_target:
                    fail(
                        f"compatibility facade mapped export {exported_name} has wrong target in "
                        f"{group.old_module}"
                    )
                continue
        fail(
            f"compatibility facade mapped export {exported_name} has conflicting binding in "
            f"{group.old_module}"
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

    def test_domain_facade_rejects_visible_implementation_modules(self) -> None:
        """Only lib.rs may publish domains; their implementation modules stay private."""

        facade = self.root / "crates" / "druid-rdbc" / "src" / "types" / "mod.rs"
        declarations = (
            "pub mod fixture;",
            "pub(crate) mod fixture;",
            "pub(super) mod fixture;",
        )
        for declaration in declarations:
            with self.subTest(declaration=declaration):
                facade.write_text(declaration + "\n", encoding="utf-8")
                with self.assertRaises(VerificationError):
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

    def test_requires_reviewed_secondary_denominator(self) -> None:
        """The eleven reviewed secondary facade paths are an executable denominator."""

        self.assertEqual(11, len(REQUIRED_SECONDARY_ROWS))
        mapping_file = self.write_manifest(list(REQUIRED_SECONDARY_ROWS))
        mappings = load_mapping(mapping_file, allow_empty_scaffold=False)
        verify_required_secondary_denominator(mappings)
        with self.assertRaisesRegex(VerificationError, "expected at least 285"):
            verify_required_secondary_denominator(
                mappings, minimum_rows=MINIMUM_LIVE_MAPPING_ROWS
            )
        with self.assertRaisesRegex(VerificationError, "secondary public paths"):
            verify_required_secondary_denominator(
                load_mapping(
                    self.write_manifest(list(REQUIRED_SECONDARY_ROWS[:-1])),
                    allow_empty_scaffold=False,
                )
            )

    def test_five_parameter_constants_are_legal_path_only_rows(self) -> None:
        """Module constants use PATH_ONLY without weakening owned or dyn identity modes."""

        constants = (
            "ASCII_INPUT_STREAM",
            "BINARY_INPUT_STREAM",
            "CHARACTER_INPUT_STREAM",
            "NCHARACTER_INPUT_STREAM",
            "URL",
        )
        rows = [
            (
                f"druid::core::rdbc_parameter::{constant}",
                f"druid_rdbc::types::{constant}",
                "PATH_ONLY",
                "crates/druid/src/core/rdbc_parameter.rs",
                "PURE_SHIM",
            )
            for constant in constants
        ]
        groups = load_mapping(self.write_manifest(rows), allow_empty_scaffold=False)
        self.assertEqual(
            {"PATH_ONLY"},
            {mapping.compile_mode for group in groups for mapping in group.mappings},
        )

    def write_manifest(self, rows: list[tuple[str, ...]]) -> Path:
        """Write an authoritative TSV fixture below this test's temporary repository."""

        mapping_file = self.root / "rdbc-public-path-map.tsv"
        normalized_rows = []
        for row in rows:
            if len(row) == 4:
                source_mode = (
                    "FACADE_MEMBERSHIP"
                    if Path(row[3]).name == "mod.rs"
                    else "PURE_SHIM"
                )
                row = (*row, source_mode)
            normalized_rows.append(row)
        contents = [MAPPING_HEADER, *("\t".join(row) for row in normalized_rows)]
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
            (
                "druid::core::legacy_fixture::FixtureType",
                "druid_rdbc::types::Value",
                "OWNED",
                "crates/druid/src/core/legacy_fixture.rs",
                "BANANA",
            ),
            (
                "druid::core::FixtureType",
                "druid_rdbc::types::Value",
                "OWNED",
                "crates/druid/src/core/mod.rs",
                "PURE_SHIM",
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

        mixed_source_modes = (
            (
                "druid::core::legacy_fixture::FixtureType",
                "druid_rdbc::types::Value",
                "OWNED",
                "crates/druid/src/core/legacy_fixture.rs",
                "PURE_SHIM",
            ),
            (
                "druid::core::legacy_fixture::OtherFixture",
                "druid_rdbc::types::OtherFixture",
                "OWNED",
                "crates/druid/src/core/legacy_fixture.rs",
                "FACADE_MEMBERSHIP",
            ),
        )
        with self.assertRaisesRegex(VerificationError, "inconsistent source_mode"):
            load_mapping(self.write_manifest(list(mixed_source_modes)), False)

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

    def test_secondary_facade_membership_keeps_unrelated_exports(self) -> None:
        """A non-mod.rs secondary facade validates one exact alias without becoming a shim."""

        self.prepare_direct_type_facade()
        facade = self.root / "crates" / "druid" / "src" / "core" / "connection.rs"
        facade.parent.mkdir(parents=True, exist_ok=True)
        facade.write_text(
            "pub use unrelated::Other;\n"
            "pub use druid_rdbc::types::Value;\n",
            encoding="utf-8",
        )
        mapping_file = self.write_manifest(
            [
                (
                    "druid::core::connection::Value",
                    "druid_rdbc::types::Value",
                    "OWNED",
                    "crates/druid/src/core/connection.rs",
                    "FACADE_MEMBERSHIP",
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

    def test_real_parent_facades_accept_mapped_grouped_reexport(self) -> None:
        """Current core/sql facades remain valid after one mapped re-export is replaced."""

        self.prepare_direct_type_facade()
        cases = (
            (
                "core",
                "pub use auto_load::AutoLoad;",
                "pub use druid_rdbc::types::{Value as AutoLoad};",
                "AutoLoad",
            ),
            (
                "sql",
                "pub use char_types::CharTypes;",
                "pub use druid_rdbc::types::{Value as CharTypes};",
                "CharTypes",
            ),
        )
        for module, old_reexport, new_reexport, old_export in cases:
            with self.subTest(module=module):
                source = (
                    repository_root()
                    / "crates"
                    / "druid"
                    / "src"
                    / module
                    / "mod.rs"
                ).read_text(encoding="utf-8")
                self.assertIn(old_reexport, source)
                source = source.replace(old_reexport, new_reexport, 1)
                source += (
                    "\n#[allow(dead_code)]\n"
                    "struct UnrelatedParentFixture;\n"
                    "impl UnrelatedParentFixture {\n"
                    "    fn helper(&self) -> &'static str { \"fixture\" }\n"
                    "}\n"
                )
                parent = (
                    self.root / "crates" / "druid" / "src" / module / "mod.rs"
                )
                parent.parent.mkdir(parents=True, exist_ok=True)
                parent.write_text(source, encoding="utf-8")
                mapping_file = self.write_manifest(
                    [
                        (
                            f"druid::{module}::{old_export}",
                            "druid_rdbc::types::Value",
                            "OWNED",
                            f"crates/druid/src/{module}/mod.rs",
                        )
                    ]
                )
                try:
                    verify_source_tree(self.root, load_mapping(mapping_file, False))
                except VerificationError as error:
                    self.fail(f"real {module}/mod.rs was rejected: {error}")

    def test_parent_facade_accepts_unrelated_top_level_syntax(self) -> None:
        """Unrelated visibility, use trees, attributes, functions, and impls are ignored."""

        self.prepare_direct_type_facade()
        parent = self.root / "crates" / "druid" / "src" / "core" / "mod.rs"
        parent.parent.mkdir(parents=True, exist_ok=True)
        parent.write_text(
            "#![allow(dead_code)]\n"
            "pub(crate) mod internal;\n"
            "#[cfg_attr(test, allow(unused_imports))]\n"
            "pub use unrelated::{Other, nested::{self as Nested, Thing as Alias}};\n"
            "pub use druid_rdbc::types::{self as RdbcTypes, Value};\n"
            "fn helper(value: &str) -> Option<&str> { Some(value) }\n"
            "struct Holder;\n"
            "impl Holder { fn value(&self) -> &'static str { r###\"ok\"### } }\n"
            "unrelated_macro! { baseline tokens }\n",
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
        try:
            verify_source_tree(self.root, load_mapping(mapping_file, False))
        except VerificationError as error:
            self.fail(f"unrelated parent syntax was rejected: {error}")

    def test_real_parent_facade_accepts_contextual_union_macro_invocations(self) -> None:
        """A real parent facade keeps mapped Value beside unrelated union! item macros."""

        self.prepare_direct_type_facade()
        source = (
            repository_root() / "crates" / "druid" / "src" / "core" / "mod.rs"
        ).read_text(encoding="utf-8")
        source = source.replace(
            "pub use value::Value;",
            "pub use druid_rdbc::types::Value;",
            1,
        )
        source += (
            "\nmacro_rules! union { ($($tokens:tt)*) => {} }\n"
            "#[allow(unused)]\n"
            "union! { brace tokens }\n"
            "union!(paren tokens);\n"
            "union![bracket tokens];\n"
        )
        parent = self.root / "crates" / "druid" / "src" / "core" / "mod.rs"
        parent.parent.mkdir(parents=True, exist_ok=True)
        parent.write_text(source, encoding="utf-8")
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

        try:
            verify_source_tree(self.root, load_mapping(mapping_file, False))
        except VerificationError as error:
            self.fail(f"real core/mod.rs union! invocation was rejected: {error}")

    def test_parent_facade_union_macro_delimiters_fail_closed(self) -> None:
        """Malformed contextual union! delimiters never bypass parent verification."""

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
        malformed_invocations = (
            "union!;",
            "union!({]",
            "union!{unterminated",
        )
        for invocation in malformed_invocations:
            with self.subTest(invocation=invocation):
                parent.write_text(
                    f"pub use druid_rdbc::types::Value;\n{invocation}\n",
                    encoding="utf-8",
                )
                with self.assertRaises(VerificationError):
                    verify_source_tree(self.root, load_mapping(mapping_file, False))

    def test_parent_facade_accepts_legal_extern_crate_self_aliases(self) -> None:
        """Crate-self aliases bind only their alias after attributes and visibility."""

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
        legal_forms = (
            "extern crate self as Alias;",
            "#[allow(unused_extern_crates)] pub extern crate self as Alias;",
            "extern crate self as _;",
            "extern crate core;",
            "extern crate core as Alias;",
            "extern crate core as _;",
        )
        for extern_item in legal_forms:
            with self.subTest(extern_item=extern_item):
                parent.write_text(
                    f"pub use druid_rdbc::types::Value;\n{extern_item}\n",
                    encoding="utf-8",
                )
                try:
                    verify_source_tree(self.root, load_mapping(mapping_file, False))
                except VerificationError as error:
                    self.fail(f"legal extern-crate item was rejected: {error}")

    def test_parent_facade_extern_crate_aliases_conflict_with_mapped_name(self) -> None:
        """Crate-self and ordinary extern-crate aliases cannot shadow mapped Value."""

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
        for extern_item in (
            "extern crate self as Value;",
            "pub extern crate self as Value;",
            "extern crate core as Value;",
        ):
            with self.subTest(extern_item=extern_item):
                parent.write_text(
                    f"pub use druid_rdbc::types::Value;\n{extern_item}\n",
                    encoding="utf-8",
                )
                with self.assertRaisesRegex(
                    VerificationError,
                    "mapped export Value has conflicting binding",
                ):
                    verify_source_tree(self.root, load_mapping(mapping_file, False))

    def test_parent_facade_rejects_invalid_union_macro_and_crate_self_forms(self) -> None:
        """Visibility-qualified union! and unaliased crate-self imports remain fail-closed."""

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
        for invalid_item in (
            "pub union!{}",
            "pub(crate) union!{}",
            "extern crate self;",
        ):
            with self.subTest(invalid_item=invalid_item):
                parent.write_text(
                    f"pub use druid_rdbc::types::Value;\n{invalid_item}\n",
                    encoding="utf-8",
                )
                with self.assertRaises(VerificationError):
                    verify_source_tree(self.root, load_mapping(mapping_file, False))

    def test_parent_facade_rejects_every_competing_mapped_binding(self) -> None:
        """Private uses, modules, and duplicate public targets cannot bind a mapped alias."""

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
        conflicts = (
            "pub use druid_rdbc::types::Value;\nuse unrelated::Value;\n",
            "pub use druid_rdbc::types::Value;\nmod Value;\n",
            "pub use druid_rdbc::types::Value;\n"
            "pub use druid_rdbc::types::Value;\n",
            "pub use druid_rdbc::types::Value;\n"
            "use unrelated::Other as r#Value;\n",
            "pub use druid_rdbc::types::Value;\nstruct Value;\n",
            "pub use druid_rdbc::types::Value;\nenum Value {}\n",
            "pub use druid_rdbc::types::Value;\ntrait Value {}\n",
            "pub use druid_rdbc::types::Value;\ntype Value = ();\n",
            "pub use druid_rdbc::types::Value;\nunion Value { byte: u8 }\n",
            "pub use druid_rdbc::types::Value;\nfn Value() {}\n",
            "pub use druid_rdbc::types::Value;\nconst Value: () = ();\n",
            "pub use druid_rdbc::types::Value;\nstatic Value: () = ();\n",
            "pub use druid_rdbc::types::Value;\nextern crate unrelated as Value;\n",
            "pub use druid_rdbc::types::Value;\n"
            "extern \"C\" { fn Value(); }\n",
            "pub use druid_rdbc::types::Value;\n"
            "macro_rules! Value { () => {} }\n",
            "pub use druid_rdbc::types::Value;\nmacro Value() {}\n",
        )
        for source in conflicts:
            with self.subTest(source=source):
                parent.write_text(source, encoding="utf-8")
                with self.assertRaisesRegex(
                    VerificationError,
                    "mapped export Value has conflicting binding",
                ):
                    verify_source_tree(self.root, load_mapping(mapping_file, False))

    def test_parent_facade_rejects_missing_wrong_and_wildcard_exports(self) -> None:
        """Missing, wrong-target, and wildcard parent exports fail with stable diagnostics."""

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
        cases = (
            ("pub use unrelated::Other;\n", "missing mapped exports"),
            (
                "pub use druid_rdbc::types::Other as Value;\n",
                "mapped export Value has wrong target",
            ),
            ("pub use druid_rdbc::types::*;\n", "wildcard use is forbidden"),
            (
                "pub use druid_rdbc::types::Value;\nuse unrelated::*;\n",
                "wildcard use is forbidden",
            ),
            ("use unrelated::_;\n", "unsupported top-level use syntax"),
        )
        for source, diagnostic in cases:
            with self.subTest(source=source):
                parent.write_text(source, encoding="utf-8")
                with self.assertRaisesRegex(VerificationError, diagnostic):
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

        for path, module_file in (
            ("druid::core::_::Value", "crates/druid/src/core/_.rs"),
            ("druid::core::value::_", "crates/druid/src/core/value.rs"),
        ):
            with self.subTest(plain_underscore=path):
                row = (
                    path,
                    "druid_rdbc::types::Value",
                    "OWNED",
                    module_file,
                )
                with self.assertRaisesRegex(VerificationError, "reserved identifier _"):
                    load_mapping(self.write_manifest([row]), False)

        union_row = (
            "druid::core::union::union",
            "druid_rdbc::types::union",
            "PATH_ONLY",
            "crates/druid/src/core/union.rs",
        )
        try:
            groups = load_mapping(self.write_manifest([union_row]), False)
        except VerificationError as error:
            self.fail(f"Rust 2021 contextual identifier union was rejected: {error}")
        self.assertEqual("union", groups[0].mappings[0].old_export)

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

    def test_raw_nonkeyword_cannot_bypass_semantic_duplicate_or_mode_keys(self) -> None:
        """Redundant raw nonkeywords are rejected before semantic key comparisons."""

        ordinary = (
            "druid::core::value::Value",
            "druid_rdbc::types::Value",
            "OWNED",
            "crates/druid/src/core/value.rs",
        )
        duplicate_old = (
            "druid::core::value::r#Value",
            "druid_rdbc::types::Value",
            "OWNED",
            "crates/druid/src/core/value.rs",
        )
        mode_bypass = (
            "druid::core::other::Other",
            "druid_rdbc::types::r#Value",
            "PATH_ONLY",
            "crates/druid/src/core/other.rs",
        )
        for rows in ((ordinary, duplicate_old), (ordinary, mode_bypass)):
            with self.subTest(rows=rows):
                with self.assertRaisesRegex(
                    VerificationError,
                    "raw identifier must escape a Rust 2021 keyword",
                ):
                    load_mapping(self.write_manifest(list(rows)), False)


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
        if not arguments.allow_empty_scaffold:
            verify_required_secondary_denominator(
                mappings, minimum_rows=MINIMUM_LIVE_MAPPING_ROWS
            )
        verify_source_tree(arguments.root.resolve(), mappings)
    except VerificationError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(
        "PASS: RDBC public-path source verification "
        f"({len(RDBC_MODULES)} declared modules, {len(mappings)} source groups)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
