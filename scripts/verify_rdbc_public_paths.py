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
    """Parse a named Rust use path and reject wildcard, grouped, aliased, or unknown forms."""

    path: list[str] = []
    expect_identifier = True
    while index < len(tokens) and tokens[index].text != ";":
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
    return path, index + 1


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
        if public and tokens[index] == Token("IDENT", "use"):
            path, index = parse_path(tokens, index + 1)
            items.append(FacadeItem("use", True, "".join(path)))
            continue
        fail("extra Rust syntax in public-path facade")
    return items


def exact_path_tokens(path: str) -> list[str]:
    """Tokenize an expected named re-export path and reject non-path syntax."""

    tokens = tokenize(path)
    parsed, index = parse_path(tokens + [Token("PUNCT", ";")], 0)
    if index != len(tokens) + 1:
        fail("invalid expected re-export path")
    return parsed


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
    actual, index = parse_path(tokens, index + 2)
    expected = exact_path_tokens(mapping.reexport)
    if actual != expected:
        fail(f"shim target mismatch in {mapping.old_module}: expected {mapping.reexport}")
    if index != len(tokens):
        fail(f"extra Rust syntax in shim: {mapping.old_module}")


def mapping_from_fields(fields: list[str], line_number: int) -> ShimMapping:
    """Convert one authoritative TSV row into the source paths required by the verifier."""

    old_path, new_path, _compile_mode, old_module_file = fields
    if not old_path or not new_path or not old_module_file:
        fail(f"mapping row {line_number} has an empty required field")
    new_segments = new_path.split("::")
    if (
        len(new_segments) < 3
        or new_segments[0] != "druid_rdbc"
        or any(not segment.isidentifier() for segment in new_segments)
    ):
        fail(f"mapping row {line_number} has invalid new_path: {new_path}")
    old_module = Path(old_module_file)
    if old_module.is_absolute() or old_module.suffix != ".rs" or ".." in old_module.parts:
        fail(f"mapping row {line_number} has invalid old_module_file: {old_module_file}")
    target_module = Path("crates/druid-rdbc/src").joinpath(*new_segments[1:-1]).with_suffix(".rs")
    return ShimMapping(old_module, target_module, new_path)


def load_mapping(mapping_path: Path, allow_empty_scaffold: bool) -> list[ShimMapping]:
    """Read the tracked public-path denominator and reject missing, malformed, or empty standard maps."""

    if not mapping_path.is_file():
        fail(f"mapping manifest not found: {mapping_path}")
    lines = mapping_path.read_text(encoding="utf-8").splitlines()
    if not lines or lines[0] != MAPPING_HEADER:
        fail(f"mapping manifest has invalid header: {mapping_path}")
    mappings: list[ShimMapping] = []
    seen_old_paths: set[Path] = set()
    for line_number, line in enumerate(lines[1:], start=2):
        if not line:
            continue
        fields = line.split("\t")
        if len(fields) != 4:
            fail(f"mapping row {line_number} must have four tab-separated fields")
        mapping = mapping_from_fields(fields, line_number)
        if mapping.old_module in seen_old_paths:
            fail(f"mapping row {line_number} duplicates old module: {mapping.old_module}")
        seen_old_paths.add(mapping.old_module)
        mappings.append(mapping)
    if not mappings and not allow_empty_scaffold:
        fail("mapping manifest has no rows; --allow-empty-scaffold is required")
    return mappings


def verify_source_tree(root: Path, mappings: list[ShimMapping]) -> None:
    """Run the source-only facade and compatibility-shim verification."""

    verify_rdbc_module_facades(root)
    for mapping in mappings:
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
