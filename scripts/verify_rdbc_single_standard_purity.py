#!/usr/bin/env python3
"""Verify that canonical RDBC standard modules remain product-independent."""

from __future__ import annotations

import argparse
from dataclasses import dataclass, field
from pathlib import Path
from typing import NoReturn

from verify_rdbc_public_paths import RUST_2021_STRICT_RESERVED_KEYWORDS, Token, tokenize
from verify_wrapper_rdbc_import_boundary import parse_use_tree


REQUIRED_CANONICAL_MODULES = (
    "connection",
    "data_source",
    "driver",
    "extensions",
    "metadata",
    "result_set",
    "statement",
    "wrapper",
)
TRANSITIONAL_ROOT_MODULES = {"compat", "error", "resource", "spi", "types"}
ALLOWED_EXTERNAL_IMPORT_ROOTS = {
    "alloc", "async_trait", "bool", "char", "clippy", "core", "f32", "f64", "i8",
    "i16", "i32", "i64", "i128", "isize", "str", "std", "u8", "u16", "u32",
    "u64", "u128", "usize",
}
ALLOWED_BARE_PRELUDE_MACROS = {"format"}
ALLOWED_QUALIFIED_MACRO_ROOTS = {"alloc", "core", "std"}
CANONICAL_CRATE_MACRO_SCOPES = {
    "impl_standard_wrapper": ("wrapper", "wrapper"),
}
CANONICAL_CRATE_MACRO_SOURCES = {
    "impl_standard_wrapper": Path("wrapper/wrapper.rs"),
}
FORBIDDEN_TRANSITIONAL_PATH_ROOTS = {"compat", "resource", "spi"}
RUST_PRELUDE_ROOTS = {
    "Box", "Clone", "Copy", "Default", "DoubleEndedIterator", "Drop", "Eq", "Err",
    "ExactSizeIterator", "Extend", "Fn", "FnMut", "FnOnce", "From", "Into",
    "IntoIterator", "Iterator", "None", "Ok", "Option", "Ord", "PartialEq",
    "PartialOrd", "Result", "Send", "Sized", "Some", "String", "Sync", "ToOwned",
    "ToString", "TryFrom", "TryInto", "Unpin", "Vec",
}
DELIMITER_PAIRS = {"(": ")", "[": "]", "{": "}"}


class VerificationError(RuntimeError):
    """Raised when the canonical standard surface is missing or polluted."""


def fail(message: str) -> NoReturn:
    """Stop with one stable fail-closed diagnostic."""

    raise VerificationError(message)


def forbidden_identity(identifier: str) -> bool:
    """Return whether one Rust identifier belongs to transitional/product runtime."""

    normalized = identifier.removeprefix("r#").replace("_", "").lower()
    if any(
        fragment in normalized
        for fragment in (
            "physical",
            "druid",
            "holder",
            "lease",
            "cache",
            "filter",
            "resourceowner",
        )
    ):
        return True
    if normalized.startswith("pooledconnection") or normalized == "connectionpooldatasource":
        return False
    if identifier in {"spi", "compat"}:
        return True
    return "pool" in normalized


def verify_module_root(path: Path, token_texts: list[str]) -> None:
    """Require module roots to contain declarations and re-exports only."""

    forbidden_items = {"trait", "struct", "enum", "union", "impl", "fn", "const", "static", "type"}
    present = sorted(forbidden_items.intersection(token_texts))
    if present:
        fail(f"module root defines items instead of declaring/re-exporting modules: {path}: {present}")


@dataclass(frozen=True)
class LocalItem:
    """One named item declared directly in a Rust module scope."""

    public: bool
    declaration_kind: str
    namespace: str


@dataclass(frozen=True)
class UseBinding:
    """One explicit use binding retained for second-pass resolution."""

    name: str
    path: tuple[str, ...]
    public: bool
    absolute: bool
    source: Path
    token_index: int


@dataclass(frozen=True)
class ModuleMacroEdge:
    """One module-scope macro invocation that may generate item/module edges."""

    path: tuple[str, ...]
    source: Path
    token_index: int


@dataclass
class ModuleScope:
    """One reachable external or inline Rust module with a token-index source range."""

    identity: tuple[str, ...]
    source: Path
    start: int
    end: int
    parent: tuple[str, ...] | None
    public: bool
    declared_at_source: Path | None = None
    declared_at_index: int | None = None
    children: dict[str, tuple[str, ...]] = field(default_factory=dict)
    child_visibility: dict[str, bool] = field(default_factory=dict)
    bindings: dict[str, UseBinding] = field(default_factory=dict)
    local_items: dict[str, LocalItem] = field(default_factory=dict)
    use_ranges: list[tuple[int, int]] = field(default_factory=list)
    macro_edges: list[ModuleMacroEdge] = field(default_factory=list)


@dataclass(frozen=True)
class LexicalGenericScope:
    """Type/const/lifetime generic bindings valid only in one declaration range."""

    declaration_kind: str
    source: Path
    start: int
    end: int
    type_bindings: frozenset[str]
    const_bindings: frozenset[str]
    lifetime_bindings: frozenset[str]


@dataclass(frozen=True)
class DeclarationScope:
    """One item declaration range that can stop enclosing function-generic capture."""

    declaration_kind: str
    start: int
    end: int
    name: str | None


@dataclass(frozen=True)
class LexicalItemScope:
    """One nested function/block item binding and its exact containing block range."""

    name: str
    declaration_kind: str
    namespace: str
    source: Path
    start: int
    end: int


@dataclass(frozen=True)
class GenericBoundSite:
    """One bound root that must resolve under the normal canonical path policy."""

    source: Path
    token_index: int
    path: tuple[str, ...]
    absolute: bool


@dataclass(frozen=True)
class MacroInvocationSite:
    """One macro invocation outside every macro_rules definition token range."""

    source: Path
    token_index: int
    end: int
    path: tuple[str, ...]
    absolute: bool
    raw: bool


@dataclass(frozen=True)
class MacroDefinition:
    """One macro_rules binding with exact textual and path-export provenance."""

    name: str
    source: Path
    start: int
    end: int
    scope_identity: tuple[str, ...] | None
    active_start: int
    active_end: int
    module_scoped: bool
    exported: bool
    raw: bool


@dataclass(frozen=True)
class LexicalUseBinding:
    """One use/import binding active throughout its exact enclosing block."""

    name: str
    source: Path
    token_index: int
    start: int
    end: int
    path: tuple[str, ...]
    absolute: bool


@dataclass
class SourceUnit:
    """Tokenized Rust source plus delimiter and reachable-scope indexes."""

    relative: Path
    tokens: list[Token]
    matching: dict[int, int]
    reverse_matching: dict[int, int]
    scopes: list[ModuleScope] = field(default_factory=list)
    lexical_generic_scopes: list[LexicalGenericScope] = field(default_factory=list)
    declaration_scopes: list[DeclarationScope] = field(default_factory=list)
    lexical_item_scopes: list[LexicalItemScope] = field(default_factory=list)
    generic_bound_sites: list[GenericBoundSite] = field(default_factory=list)
    macro_invocations: list[MacroInvocationSite] = field(default_factory=list)
    macro_invocations_indexed: bool = False
    lexical_use_bindings: list[LexicalUseBinding] = field(default_factory=list)
    macro_provenance_indexed: bool = False


@dataclass(frozen=True)
class ResolvedTarget:
    """A resolved module, item, or external/prelude path root."""

    kind: str
    module: tuple[str, ...] | None = None
    external_root: str | None = None


def canonical_identifier(token: Token) -> str:
    """Return an identifier without raw-identifier syntax."""

    if token.kind != "IDENT":
        fail(f"expected Rust identifier, found {token.kind}:{token.text}")
    return token.text.removeprefix("r#")


def delimiter_index(tokens: list[Token], source: Path) -> tuple[dict[int, int], dict[int, int]]:
    """Build exact delimiter pairs for one tokenized source file."""

    matching: dict[int, int] = {}
    reverse: dict[int, int] = {}
    stack: list[tuple[str, int]] = []
    closers = {value: key for key, value in DELIMITER_PAIRS.items()}
    for index, token in enumerate(tokens):
        if token.text in DELIMITER_PAIRS:
            stack.append((token.text, index))
        elif token.text in closers:
            if not stack or stack[-1][0] != closers[token.text]:
                fail(f"unbalanced Rust delimiters in {source} at token {index}")
            _, start = stack.pop()
            matching[start] = index
            reverse[index] = start
    if stack:
        fail(f"unterminated Rust delimiter in {source} at token {stack[-1][1]}")
    return matching, reverse


def attribute_ranges(unit: SourceUnit) -> tuple[tuple[int, int], ...]:
    """Return complete #[...] token ranges and reject path only inside those ranges."""

    ranges: list[tuple[int, int]] = []
    tokens = unit.tokens
    index = 0
    while index < len(tokens):
        if tokens[index].text != "#":
            index += 1
            continue
        bracket = index + 1
        if bracket < len(tokens) and tokens[bracket].text == "!":
            bracket += 1
        if bracket >= len(tokens) or tokens[bracket].text != "[":
            fail(f"unknown attribute edge in {unit.relative} at token {index}")
        end = unit.matching.get(bracket)
        if end is None:
            fail(f"unterminated attribute in {unit.relative} at token {index}")
        for cursor in range(bracket + 1, end):
            if (
                tokens[cursor].kind == "IDENT"
                and canonical_identifier(tokens[cursor]) == "macro_use"
            ):
                fail(
                    "#[macro_use] creates untracked macro provenance in "
                    f"{unit.relative}"
                )
            if (
                tokens[cursor].kind == "IDENT"
                and canonical_identifier(tokens[cursor]) == "path"
                and cursor + 1 < end
                and tokens[cursor + 1].text == "="
            ):
                fail(f"#[path] escapes reachable module discovery in {unit.relative}")
        ranges.append((index, end + 1))
        index = end + 1
    return tuple(ranges)


def macro_definition_ranges(unit: SourceUnit) -> tuple[tuple[int, int], ...]:
    """Return macro_rules definition ranges whose generated declarations are not lexical source."""

    ranges: list[tuple[int, int]] = []
    tokens = unit.tokens
    index = 0
    while index + 3 < len(tokens):
        if (
            tokens[index].text != "macro_rules"
            or tokens[index + 1].text != "!"
            or tokens[index + 2].kind != "IDENT"
            or tokens[index + 3].text not in DELIMITER_PAIRS
        ):
            index += 1
            continue
        end = unit.matching.get(index + 3)
        if end is None:
            fail(f"unterminated macro_rules definition in {unit.relative}")
        ranges.append((index, end + 1))
        index = end + 1
    return tuple(ranges)


def macro_invocation_sites(unit: SourceUnit) -> tuple[MacroInvocationSite, ...]:
    """Index every named macro invocation outside complete macro_rules definitions."""

    tokens = unit.tokens
    excluded = {
        cursor
        for start, end in macro_definition_ranges(unit)
        for cursor in range(start, end)
    }
    sites: list[MacroInvocationSite] = []
    index = 0
    while index < len(tokens):
        if index in excluded:
            index += 1
            continue
        start = index
        absolute = tokens[index].text == "::"
        cursor = index + int(absolute)
        if cursor >= len(tokens) or tokens[cursor].kind != "IDENT":
            index += 1
            continue
        path = [canonical_identifier(tokens[cursor])]
        raw = tokens[cursor].text.startswith("r#")
        cursor += 1
        while (
            cursor + 1 < len(tokens)
            and cursor not in excluded
            and tokens[cursor].text == "::"
            and tokens[cursor + 1].kind == "IDENT"
        ):
            raw = raw or tokens[cursor + 1].text.startswith("r#")
            path.append(canonical_identifier(tokens[cursor + 1]))
            cursor += 2
        if cursor < len(tokens) and cursor not in excluded and tokens[cursor].text == "!":
            root_token = tokens[start + int(absolute)]
            if (
                len(path) == 1
                and root_token.text in RUST_2021_STRICT_RESERVED_KEYWORDS
            ):
                index = cursor + 1
                continue
            sites.append(
                MacroInvocationSite(
                    unit.relative,
                    start,
                    cursor + 1,
                    tuple(path),
                    absolute,
                    raw,
                )
            )
            index = cursor + 1
            continue
        index += 1
    return tuple(sites)


def angle_close(unit: SourceUnit, opening: int, limit: int) -> int:
    """Find a declaration generic list's closing angle without treating function arrows as closes."""

    tokens = unit.tokens
    depth = 1
    cursor = opening + 1
    while cursor < limit:
        token = tokens[cursor].text
        if token in DELIMITER_PAIRS:
            end = unit.matching.get(cursor)
            if end is None or end >= limit:
                fail(f"generic delimiter escapes declaration in {unit.relative}")
            cursor = end + 1
            continue
        if token == "<":
            depth += 1
        elif token == ">" and (cursor == 0 or tokens[cursor - 1].text != "-"):
            depth -= 1
            if depth == 0:
                return cursor
        cursor += 1
    fail(f"unterminated generic parameter list in {unit.relative}")


def declaration_generic_open(tokens: list[Token], index: int, limit: int) -> int | None:
    """Return the `<` that declares generics for one supported Rust item keyword."""

    keyword = tokens[index].text
    if keyword == "impl":
        candidate = index + 1
    elif keyword in {"enum", "fn", "struct", "trait", "type", "union"}:
        if index + 1 >= limit or tokens[index + 1].kind != "IDENT":
            return None
        candidate = index + 2
    else:
        return None
    if candidate < limit and tokens[candidate].text == "<":
        return candidate
    return None


def declaration_end(unit: SourceUnit, index: int, limit: int) -> int:
    """Return one Rust item's half-open lexical range ending at its body or semicolon."""

    tokens = unit.tokens
    cursor = index + 1
    angle_depth = 0
    while cursor < limit:
        token = tokens[cursor].text
        if token in {"(", "["}:
            end = unit.matching.get(cursor)
            if end is None or end >= limit:
                fail(f"item delimiter escapes source range in {unit.relative}")
            cursor = end + 1
            continue
        if token == "<":
            angle_depth += 1
        elif token == ">" and (cursor == 0 or tokens[cursor - 1].text != "-"):
            angle_depth = max(0, angle_depth - 1)
        elif token == "{" and angle_depth == 0:
            end = unit.matching.get(cursor)
            if end is None or end >= limit:
                fail(f"item body escapes source range in {unit.relative}")
            return end + 1
        elif token == ";" and angle_depth == 0:
            return cursor + 1
        cursor += 1
    fail(f"unterminated declaration in {unit.relative} at token {index}")


def split_top_level_ranges(
    unit: SourceUnit, start: int, end: int, separator: str
) -> tuple[tuple[int, int], ...]:
    """Split a token range at one separator outside delimiters and nested angle arguments."""

    tokens = unit.tokens
    ranges: list[tuple[int, int]] = []
    segment_start = start
    cursor = start
    angle_depth = 0
    while cursor < end:
        token = tokens[cursor].text
        if token in DELIMITER_PAIRS:
            closing = unit.matching.get(cursor)
            if closing is None or closing >= end:
                fail(f"delimiter escapes bounded syntax in {unit.relative}")
            cursor = closing + 1
            continue
        if token == "<":
            angle_depth += 1
        elif token == ">" and (cursor == 0 or tokens[cursor - 1].text != "-"):
            angle_depth = max(0, angle_depth - 1)
        elif token == separator and angle_depth == 0:
            ranges.append((segment_start, cursor))
            segment_start = cursor + 1
        cursor += 1
    ranges.append((segment_start, end))
    return tuple(ranges)


def generic_bound_sites_in_predicate(
    unit: SourceUnit, start: int, end: int
) -> tuple[GenericBoundSite, ...]:
    """Extract each top-level trait/type root after a generic predicate colon."""

    tokens = unit.tokens
    colon: int | None = None
    cursor = start
    angle_depth = 0
    while cursor < end:
        token = tokens[cursor].text
        if token in DELIMITER_PAIRS:
            closing = unit.matching.get(cursor)
            if closing is None or closing >= end:
                fail(f"generic predicate delimiter escapes range in {unit.relative}")
            cursor = closing + 1
            continue
        if token == "<":
            angle_depth += 1
        elif token == ">" and (cursor == 0 or tokens[cursor - 1].text != "-"):
            angle_depth = max(0, angle_depth - 1)
        elif token == ":" and angle_depth == 0:
            colon = cursor
            break
        cursor += 1
    if colon is None:
        return ()

    bound_end = end
    cursor = colon + 1
    angle_depth = 0
    while cursor < end:
        token = tokens[cursor].text
        if token in DELIMITER_PAIRS:
            closing = unit.matching.get(cursor)
            if closing is None or closing >= end:
                fail(f"generic bound delimiter escapes range in {unit.relative}")
            cursor = closing + 1
            continue
        if token == "<":
            angle_depth += 1
        elif token == ">" and (cursor == 0 or tokens[cursor - 1].text != "-"):
            angle_depth = max(0, angle_depth - 1)
        elif token == "=" and angle_depth == 0:
            bound_end = cursor
            break
        cursor += 1

    sites: list[GenericBoundSite] = []
    for bound_start, bound_stop in split_top_level_ranges(
        unit, colon + 1, bound_end, "+"
    ):
        cursor = bound_start
        while cursor < bound_stop and tokens[cursor].text in {"?", "~", "const"}:
            cursor += 1
        if cursor < bound_stop and tokens[cursor].text == "for":
            cursor += 1
            if cursor >= bound_stop or tokens[cursor].text != "<":
                fail(f"for bound lacks lifetime list in {unit.relative}")
            cursor = angle_close(unit, cursor, bound_stop) + 1
        if cursor >= bound_stop or tokens[cursor].kind == "LIFETIME":
            continue
        absolute = tokens[cursor].text == "::"
        if absolute:
            cursor += 1
        if cursor >= bound_stop or tokens[cursor].kind != "IDENT":
            fail(f"unknown generic bound syntax in {unit.relative} at token {cursor}")
        path_start = cursor
        path = [canonical_identifier(tokens[cursor])]
        cursor += 1
        while (
            cursor + 1 < bound_stop
            and tokens[cursor].text == "::"
            and tokens[cursor + 1].kind == "IDENT"
        ):
            path.append(canonical_identifier(tokens[cursor + 1]))
            cursor += 2
        sites.append(
            GenericBoundSite(unit.relative, path_start, tuple(path), absolute)
        )
    return tuple(sites)


def where_bound_sites(
    unit: SourceUnit, generic_closing: int, declaration_stop: int
) -> tuple[GenericBoundSite, ...]:
    """Extract bound roots from a declaration's top-level where predicates."""

    tokens = unit.tokens
    cursor = generic_closing + 1
    where_index: int | None = None
    header_end = declaration_stop
    angle_depth = 0
    while cursor < declaration_stop:
        token = tokens[cursor].text
        if token in {"(", "["}:
            closing = unit.matching.get(cursor)
            if closing is None or closing >= declaration_stop:
                fail(f"where header delimiter escapes declaration in {unit.relative}")
            cursor = closing + 1
            continue
        if token == "<":
            angle_depth += 1
        elif token == ">" and (cursor == 0 or tokens[cursor - 1].text != "-"):
            angle_depth = max(0, angle_depth - 1)
        elif token == "where" and angle_depth == 0 and where_index is None:
            where_index = cursor
        elif token in {"{", ";"} and angle_depth == 0:
            header_end = cursor
            break
        cursor += 1
    if where_index is None:
        return ()
    sites: list[GenericBoundSite] = []
    for predicate_start, predicate_end in split_top_level_ranges(
        unit, where_index + 1, header_end, ","
    ):
        sites.extend(
            generic_bound_sites_in_predicate(unit, predicate_start, predicate_end)
        )
    return tuple(sites)


def generic_parameter_bindings(
    unit: SourceUnit, opening: int, closing: int
) -> tuple[
    frozenset[str],
    frozenset[str],
    frozenset[str],
    tuple[GenericBoundSite, ...],
]:
    """Classify top-level type, const, and lifetime parameters from one generic list."""

    tokens = unit.tokens
    segments: list[tuple[int, int]] = []
    start = opening + 1
    cursor = start
    nested_angles = 0
    while cursor < closing:
        token = tokens[cursor].text
        if token in DELIMITER_PAIRS:
            end = unit.matching.get(cursor)
            if end is None or end > closing:
                fail(f"generic parameter delimiter escapes list in {unit.relative}")
            cursor = end + 1
            continue
        if token == "<":
            nested_angles += 1
        elif token == ">" and (cursor == 0 or tokens[cursor - 1].text != "-"):
            nested_angles = max(0, nested_angles - 1)
        elif token == "," and nested_angles == 0:
            segments.append((start, cursor))
            start = cursor + 1
        cursor += 1
    segments.append((start, closing))

    type_bindings: set[str] = set()
    const_bindings: set[str] = set()
    lifetime_bindings: set[str] = set()
    bound_sites: list[GenericBoundSite] = []
    for segment_start, segment_end in segments:
        cursor = segment_start
        while cursor < segment_end and tokens[cursor].text == "#":
            bracket = cursor + 1 + int(
                cursor + 1 < segment_end and tokens[cursor + 1].text == "!"
            )
            end = unit.matching.get(bracket)
            if end is None or end >= segment_end:
                fail(f"generic parameter attribute escapes segment in {unit.relative}")
            cursor = end + 1
        if cursor >= segment_end:
            continue
        if tokens[cursor].kind == "LIFETIME":
            lifetime_bindings.add(tokens[cursor].text)
            bound_sites.extend(
                generic_bound_sites_in_predicate(unit, segment_start, segment_end)
            )
            continue
        if tokens[cursor].text == "const":
            if cursor + 1 >= segment_end or tokens[cursor + 1].kind != "IDENT":
                fail(f"const generic lacks a name in {unit.relative}")
            const_bindings.add(canonical_identifier(tokens[cursor + 1]))
            bound_sites.extend(
                generic_bound_sites_in_predicate(unit, segment_start, segment_end)
            )
            continue
        if tokens[cursor].kind == "IDENT":
            type_bindings.add(canonical_identifier(tokens[cursor]))
            bound_sites.extend(
                generic_bound_sites_in_predicate(unit, segment_start, segment_end)
            )
            continue
        fail(f"unknown generic parameter syntax in {unit.relative} at token {cursor}")
    duplicate = (type_bindings & const_bindings) or (type_bindings & lifetime_bindings)
    if duplicate:
        fail(f"ambiguous generic binding in {unit.relative}: {sorted(duplicate)}")
    return (
        frozenset(type_bindings),
        frozenset(const_bindings),
        frozenset(lifetime_bindings),
        tuple(bound_sites),
    )


class ModuleScopeIndex:
    """Two-pass reachable Rust module/binding index rooted at src/lib.rs."""

    def __init__(self, root: Path) -> None:
        self.root = root
        self.sources: dict[Path, SourceUnit] = {}
        self.scopes: dict[tuple[str, ...], ModuleScope] = {}
        self.binding_cache: dict[tuple[tuple[str, ...], str, bool], ResolvedTarget] = {}
        self.macro_definitions: list[MacroDefinition] = []
        self.exported_macros: dict[str, MacroDefinition] = {}
        crate_root = Path("lib.rs")
        if not (root / crate_root).is_file():
            fail("standard scaffold is incomplete: ['lib.rs']")
        unit = self.source(crate_root)
        root_scope = self.register_scope((), crate_root, 0, len(unit.tokens), None, True)
        self.scan_scope(root_scope)
        root_names = {
            name
            for name, identity in root_scope.children.items()
            if root_scope.child_visibility[name] and identity and name not in TRANSITIONAL_ROOT_MODULES
        }
        missing = sorted(set(REQUIRED_CANONICAL_MODULES).difference(root_names))
        if missing:
            fail(f"standard scaffold is incomplete or unreachable from lib.rs: {missing}")
        self.canonical_roots = tuple(sorted(root_names))
        self.index_lexical_generic_scopes()
        for unit in tuple(self.sources.values()):
            self.ensure_macro_provenance_index(unit)
        self.index_and_verify_macro_invocations()
        self.verify_generic_bound_sites()
        for scope in self.scopes.values():
            if scope.identity and scope.identity[0] in self.canonical_roots:
                for name in scope.bindings:
                    self.resolve_binding(scope, name, canonical=True, resolving=set())

    def source(self, relative: Path) -> SourceUnit:
        """Load and tokenize one UTF-8 source exactly once."""

        if relative in self.sources:
            return self.sources[relative]
        path = self.root / relative
        try:
            source = path.read_text(encoding="utf-8")
        except UnicodeDecodeError as error:
            fail(f"standard source is not UTF-8: {relative}: {error}")
        tokens = tokenize(source)
        matching, reverse = delimiter_index(tokens, relative)
        unit = SourceUnit(relative, tokens, matching, reverse)
        attribute_ranges(unit)
        self.sources[relative] = unit
        return unit

    def register_scope(
        self,
        identity: tuple[str, ...],
        source: Path,
        start: int,
        end: int,
        parent: tuple[str, ...] | None,
        public: bool,
        declared_at_source: Path | None = None,
        declared_at_index: int | None = None,
    ) -> ModuleScope:
        """Register one unique reachable module identity and source range."""

        if identity in self.scopes:
            fail(f"ambiguous module identity: {'::'.join(identity) or 'crate'}")
        scope = ModuleScope(
            identity,
            source,
            start,
            end,
            parent,
            public,
            declared_at_source,
            declared_at_index,
        )
        self.scopes[identity] = scope
        self.source(source).scopes.append(scope)
        return scope

    def public_before(self, unit: SourceUnit, index: int, scope: ModuleScope) -> bool:
        """Return whether the item keyword has any pub visibility qualifier."""

        previous = index - 1
        while previous >= scope.start and unit.tokens[previous].text in {
            "async",
            "const",
            "default",
            "unsafe",
        }:
            previous -= 1
        if previous < scope.start:
            return False
        if unit.tokens[previous].text == "pub":
            return True
        if unit.tokens[previous].text == ")":
            opening = unit.reverse_matching.get(previous)
            return bool(opening is not None and opening - 1 >= scope.start and unit.tokens[opening - 1].text == "pub")
        return False

    def external_module_source(self, identity: tuple[str, ...]) -> Path:
        """Resolve exactly one conventional foo.rs or foo/mod.rs module source."""

        base = self.root.joinpath(*identity)
        rs_candidate = base.parent / f"{base.name}.rs"
        mod_candidate = base / "mod.rs"
        candidates = [path for path in (rs_candidate, mod_candidate) if path.is_file()]
        if not candidates:
            fail(f"missing module source for crate::{'::'.join(identity)}")
        if len(candidates) != 1:
            fail(
                "ambiguous module source for "
                f"crate::{'::'.join(identity)}: both {rs_candidate.relative_to(self.root)} "
                f"and {mod_candidate.relative_to(self.root)} exist"
            )
        return candidates[0].relative_to(self.root)

    def add_child(
        self, scope: ModuleScope, name: str, child: ModuleScope, public: bool
    ) -> None:
        """Bind one declared child, rejecting duplicate/ambiguous namespace entries."""

        if name in scope.children or name in scope.bindings or name in scope.local_items:
            fail(f"ambiguous module child binding in {scope.source}: {name}")
        scope.children[name] = child.identity
        scope.child_visibility[name] = public

    def add_binding(self, scope: ModuleScope, binding: UseBinding) -> None:
        """Bind one explicit use/re-export without last-write-wins ambiguity."""

        if (
            binding.name in scope.bindings
            or binding.name in scope.children
            or binding.name in scope.local_items
        ):
            fail(f"ambiguous or duplicate binding in {binding.source}: {binding.name}")
        scope.bindings[binding.name] = binding

    def scan_scope(self, scope: ModuleScope) -> None:
        """First pass: discover children, ranges, use bindings, local items, and macro edges."""

        unit = self.source(scope.source)
        tokens = unit.tokens
        index = scope.start
        item_keywords = {"enum", "fn", "struct", "trait", "type", "union"}
        while index < scope.end:
            token = tokens[index]
            if token.text == "#":
                bracket = index + 1 + int(index + 1 < len(tokens) and tokens[index + 1].text == "!")
                end = unit.matching.get(bracket)
                if end is None or end >= scope.end:
                    fail(f"attribute escapes module source range in {scope.source}")
                index = end + 1
                continue
            if token.text in DELIMITER_PAIRS:
                end = unit.matching.get(index)
                if end is None or end >= scope.end:
                    fail(f"delimiter escapes module source range in {scope.source}")
                index = end + 1
                continue
            if token.text == "mod" and index + 2 < scope.end and tokens[index + 1].kind == "IDENT":
                name = canonical_identifier(tokens[index + 1])
                public = self.public_before(unit, index, scope)
                delimiter = tokens[index + 2].text
                identity = scope.identity + (name,)
                if delimiter == ";":
                    relative = self.external_module_source(identity)
                    child_unit = self.source(relative)
                    child = self.register_scope(
                        identity,
                        relative,
                        0,
                        len(child_unit.tokens),
                        scope.identity,
                        public,
                        scope.source,
                        index,
                    )
                    self.add_child(scope, name, child, public)
                    self.scan_scope(child)
                    index += 3
                    continue
                if delimiter == "{":
                    end = unit.matching.get(index + 2)
                    if end is None or end >= scope.end:
                        fail(f"unterminated inline module {name} in {scope.source}")
                    child = self.register_scope(
                        identity,
                        scope.source,
                        index + 3,
                        end,
                        scope.identity,
                        public,
                        scope.source,
                        index,
                    )
                    self.add_child(scope, name, child, public)
                    self.scan_scope(child)
                    index = end + 1
                    continue
                fail(f"unknown or macro-generated module edge in {scope.source}: mod {name}")
            if token.text == "use":
                try:
                    entries, end = parse_use_tree(tokens, index + 1)
                except Exception as error:
                    fail(f"cannot parse canonical import in {scope.source}: {error}")
                if end >= scope.end or tokens[end].text != ";":
                    fail(f"use item is not terminated in {scope.source}")
                public = self.public_before(unit, index, scope)
                absolute = index + 1 < scope.end and tokens[index + 1].text == "::"
                for entry in entries:
                    path = tuple(part.removeprefix("r#") for part in entry.path)
                    if not path:
                        fail(f"empty canonical import in {scope.source}")
                    if path[-1] == "*":
                        fail(f"wildcard import is forbidden in canonical source: {scope.source}")
                    name = (entry.alias or path[-1]).removeprefix("r#")
                    if name == "_":
                        continue
                    self.add_binding(
                        scope,
                        UseBinding(name, path, public, absolute, scope.source, index),
                    )
                scope.use_ranges.append((index, end + 1))
                index = end + 1
                continue
            if token.text in item_keywords and index + 1 < scope.end:
                if tokens[index + 1].kind == "IDENT":
                    name = canonical_identifier(tokens[index + 1])
                    if name in scope.children or name in scope.bindings or name in scope.local_items:
                        fail(f"ambiguous local item binding in {scope.source}: {name}")
                    scope.local_items[name] = LocalItem(
                        self.public_before(unit, index, scope),
                        token.text,
                        "value" if token.text == "fn" else "type",
                    )
                index = declaration_end(unit, index, scope.end)
                continue
            if token.text == "impl":
                index = declaration_end(unit, index, scope.end)
                continue
            elif token.text in {"const", "static"}:
                if token.text == "const":
                    function_cursor = index + 1
                    while function_cursor < scope.end and tokens[function_cursor].text in {
                        "async",
                        "unsafe",
                    }:
                        function_cursor += 1
                    if function_cursor < scope.end and tokens[function_cursor].text == "extern":
                        function_cursor += 1
                        if (
                            function_cursor < scope.end
                            and tokens[function_cursor].kind == "STRING"
                        ):
                            function_cursor += 1
                    if function_cursor < scope.end and tokens[function_cursor].text == "fn":
                        index += 1
                        continue
                cursor = index + 1
                if cursor < scope.end and tokens[cursor].text == "mut":
                    cursor += 1
                if cursor < scope.end and tokens[cursor].kind == "IDENT" and tokens[cursor].text != "fn":
                    name = canonical_identifier(tokens[cursor])
                    if name != "_":
                        scope.local_items.setdefault(
                            name,
                            LocalItem(
                                self.public_before(unit, index, scope),
                                token.text,
                                "value",
                            ),
                        )
                index = declaration_end(unit, index, scope.end)
                continue
            elif token.text == "macro_rules" and index + 2 < scope.end:
                if tokens[index + 1].text == "!" and tokens[index + 2].kind == "IDENT":
                    name = canonical_identifier(tokens[index + 2])
                    scope.local_items.setdefault(
                        name,
                        LocalItem(False, "macro_rules", "macro"),
                    )
                    opening = index + 3
                    if opening >= scope.end or tokens[opening].text not in DELIMITER_PAIRS:
                        fail(f"unknown macro_rules declaration in {scope.source}")
                    end = unit.matching.get(opening)
                    if end is None or end >= scope.end:
                        fail(f"macro_rules body escapes module range in {scope.source}")
                    index = end + 1
                    continue

            macro = self.module_macro_path(tokens, index, scope.end)
            if macro is not None and token.text != "macro_rules":
                path, bang = macro
                scope.macro_edges.append(ModuleMacroEdge(path, scope.source, index))
                index = bang + 1
                continue
            index += 1

    def index_lexical_generic_scopes(self) -> None:
        """Index generic bindings and exact declaration ranges in reachable canonical sources."""

        for unit in self.sources.values():
            if not any(
                scope.identity and scope.identity[0] in self.canonical_roots
                for scope in unit.scopes
            ):
                continue
            excluded = sorted((*macro_definition_ranges(unit), *attribute_ranges(unit)))
            excluded_cursor = 0
            index = 0
            while index < len(unit.tokens):
                while (
                    excluded_cursor < len(excluded)
                    and excluded[excluded_cursor][1] <= index
                ):
                    excluded_cursor += 1
                if (
                    excluded_cursor < len(excluded)
                    and excluded[excluded_cursor][0] <= index < excluded[excluded_cursor][1]
                ):
                    index = excluded[excluded_cursor][1]
                    continue
                declaration_kind = unit.tokens[index].text
                if declaration_kind not in {
                    "enum",
                    "fn",
                    "impl",
                    "mod",
                    "struct",
                    "trait",
                    "type",
                    "union",
                }:
                    index += 1
                    continue
                named_declaration = declaration_kind != "impl" and (
                    index + 1 < len(unit.tokens)
                    and unit.tokens[index + 1].kind == "IDENT"
                )
                opening = declaration_generic_open(unit.tokens, index, len(unit.tokens))
                if not named_declaration and opening is None:
                    index += 1
                    continue
                end = declaration_end(unit, index, len(unit.tokens))
                declaration_name = (
                    canonical_identifier(unit.tokens[index + 1])
                    if named_declaration
                    else None
                )
                unit.declaration_scopes.append(
                    DeclarationScope(
                        declaration_kind,
                        index,
                        end,
                        declaration_name,
                    )
                )
                if opening is None:
                    index += 1
                    continue
                closing = angle_close(unit, opening, len(unit.tokens))
                (
                    type_bindings,
                    const_bindings,
                    lifetime_bindings,
                    inline_bound_sites,
                ) = generic_parameter_bindings(unit, opening, closing)
                unit.generic_bound_sites.extend(inline_bound_sites)
                unit.generic_bound_sites.extend(
                    where_bound_sites(unit, closing, end)
                )
                if type_bindings or const_bindings or lifetime_bindings:
                    unit.lexical_generic_scopes.append(
                        LexicalGenericScope(
                            unit.tokens[index].text,
                            unit.relative,
                            index,
                            end,
                            type_bindings,
                            const_bindings,
                            lifetime_bindings,
                        )
                    )
                index += 1
            self.index_lexical_item_scopes(unit)

    @staticmethod
    def index_lexical_item_scopes(unit: SourceUnit) -> None:
        """Bind nested items throughout their nearest enclosing function/block range."""

        bindable_kinds = {"enum", "fn", "mod", "struct", "trait", "type", "union"}
        for declaration in unit.declaration_scopes:
            if (
                declaration.name is None
                or declaration.declaration_kind not in bindable_kinds
            ):
                continue
            parents = [
                parent
                for parent in unit.declaration_scopes
                if parent.start < declaration.start
                and declaration.end <= parent.end
            ]
            if not parents:
                continue
            parent = min(parents, key=lambda item: item.end - item.start)
            if parent.declaration_kind != "fn":
                continue
            blocks = [
                (opening, closing)
                for opening, closing in unit.matching.items()
                if unit.tokens[opening].text == "{"
                and parent.start < opening < declaration.start
                and declaration.end <= closing < parent.end
            ]
            if not blocks:
                fail(
                    f"nested item has no containing function/block range in {unit.relative}: "
                    f"{declaration.name}"
                )
            opening, closing = max(blocks, key=lambda pair: pair[0])
            lexical_scope = LexicalItemScope(
                declaration.name,
                declaration.declaration_kind,
                (
                    "value"
                    if declaration.declaration_kind == "fn"
                    else "module"
                    if declaration.declaration_kind == "mod"
                    else "type"
                ),
                unit.relative,
                opening + 1,
                closing,
            )
            if any(
                existing.name == lexical_scope.name
                and existing.start == lexical_scope.start
                and existing.end == lexical_scope.end
                for existing in unit.lexical_item_scopes
            ):
                fail(
                    f"ambiguous nested item binding in {unit.relative}: "
                    f"{lexical_scope.name}"
                )
            unit.lexical_item_scopes.append(lexical_scope)

    def verify_generic_bound_sites(self) -> None:
        """Resolve inline/where bound roots without weakening the canonical path policy."""

        for unit in self.sources.values():
            for site in unit.generic_bound_sites:
                self.resolve_path(
                    self.scope_for(site.source, site.token_index),
                    site.path,
                    absolute=site.absolute,
                    canonical=True,
                    source=site.source,
                    token_index=site.token_index,
                )

    def index_and_verify_macro_invocations(self) -> None:
        """Resolve every canonical macro invocation to trusted binding provenance."""

        for unit in self.sources.values():
            if not any(
                not scope.identity
                or scope.identity[0] in self.canonical_roots
                for scope in unit.scopes
            ):
                continue
            self.verify_macro_invocations(unit)

    def ensure_macro_provenance_index(self, unit: SourceUnit) -> None:
        """Index macro definitions/import ranges before resolving an invocation."""

        if unit.macro_provenance_indexed:
            return
        self.verify_extern_crate_provenance(unit)
        self.index_macro_definitions(unit)
        self.index_lexical_use_bindings(unit)
        unit.macro_provenance_indexed = True

    @staticmethod
    def has_direct_attribute(unit: SourceUnit, item_start: int, name: str) -> bool:
        """Return whether a consecutive attribute attached to one item names `name`."""

        by_end = {end: (start, end) for start, end in attribute_ranges(unit)}
        cursor = item_start
        found = False
        while cursor in by_end:
            start, end = by_end[cursor]
            bracket = start + 1
            if bracket < end and unit.tokens[bracket].text == "!":
                bracket += 1
            attribute_name = bracket + 1
            found = found or bool(
                attribute_name < end
                and unit.tokens[attribute_name].kind == "IDENT"
                and canonical_identifier(unit.tokens[attribute_name]) == name
            )
            cursor = start
        return found

    def index_macro_definitions(self, unit: SourceUnit) -> None:
        """Record textual ranges and exact macro_export origins for one source."""

        module_boundaries = {
            (scope.start - 1, scope.end)
            for scope in unit.scopes
            if scope.start > 0
            and unit.tokens[scope.start - 1].text == "{"
        }
        for start, end in macro_definition_ranges(unit):
            name_token = unit.tokens[start + 2]
            name = canonical_identifier(name_token)
            scope = self.scope_for(unit.relative, start)
            containing_blocks = [
                (opening, closing)
                for opening, closing in unit.matching.items()
                if unit.tokens[opening].text == "{"
                and opening < start
                and end <= closing
            ]
            block = max(containing_blocks, key=lambda pair: pair[0], default=None)
            module_scoped = block is None or block in module_boundaries
            active_end = (
                scope.end
                if module_scoped and scope is not None
                else len(unit.tokens)
                if module_scoped
                else block[1]
            )
            definition = MacroDefinition(
                name,
                unit.relative,
                start,
                end,
                scope.identity if scope is not None else None,
                end,
                active_end,
                module_scoped,
                self.has_direct_attribute(unit, start, "macro_export"),
                name_token.text.startswith("r#"),
            )
            self.macro_definitions.append(definition)
            if not definition.exported:
                continue
            if name in self.exported_macros:
                fail(f"ambiguous exported macro provenance: {name}")
            self.exported_macros[name] = definition

    @staticmethod
    def verify_extern_crate_provenance(unit: SourceUnit) -> None:
        """Allow only exact, unaliased trusted standard-library extern declarations."""

        excluded = {
            cursor
            for start, end in (*macro_definition_ranges(unit), *attribute_ranges(unit))
            for cursor in range(start, end)
        }
        tokens = unit.tokens
        index = 0
        while index + 2 < len(tokens):
            if index in excluded or tokens[index].text != "extern" or tokens[index + 1].text != "crate":
                index += 1
                continue
            crate_token = tokens[index + 2]
            if crate_token.kind != "IDENT":
                fail(f"extern crate lacks a provable root in {unit.relative}")
            crate_name = canonical_identifier(crate_token)
            cursor = index + 3
            if cursor < len(tokens) and tokens[cursor].text == "as":
                fail(
                    "extern crate aliases cannot prove standard macro provenance in "
                    f"{unit.relative}: {crate_name}"
                )
            if cursor >= len(tokens) or tokens[cursor].text != ";":
                fail(f"extern crate is not terminated in {unit.relative}: {crate_name}")
            if crate_token.text.startswith("r#") or crate_name not in ALLOWED_QUALIFIED_MACRO_ROOTS:
                fail(f"extern crate is forbidden in {unit.relative}: {crate_name}")
            index = cursor + 1

    def index_lexical_use_bindings(self, unit: SourceUnit) -> None:
        """Record use aliases inside functions/blocks with exact block ranges."""

        module_use_starts = {
            start
            for scope in unit.scopes
            for start, _ in scope.use_ranges
        }
        excluded = {
            cursor
            for start, end in (*macro_definition_ranges(unit), *attribute_ranges(unit))
            for cursor in range(start, end)
        }
        module_boundaries = {
            (scope.start - 1, scope.end)
            for scope in unit.scopes
            if scope.start > 0 and unit.tokens[scope.start - 1].text == "{"
        }
        tokens = unit.tokens
        index = 0
        while index < len(tokens):
            if index in excluded or tokens[index].text != "use" or index in module_use_starts:
                index += 1
                continue
            try:
                entries, end = parse_use_tree(tokens, index + 1)
            except Exception as error:
                fail(f"cannot parse lexical macro import in {unit.relative}: {error}")
            if end >= len(tokens) or tokens[end].text != ";":
                fail(f"lexical use item is not terminated in {unit.relative}")
            blocks = [
                (opening, closing)
                for opening, closing in unit.matching.items()
                if tokens[opening].text == "{"
                and opening < index < closing
                and (opening, closing) not in module_boundaries
            ]
            block = max(blocks, key=lambda pair: pair[0], default=None)
            if block is None:
                fail(f"lexical use has no provable block scope in {unit.relative}")
            absolute = index + 1 < len(tokens) and tokens[index + 1].text == "::"
            for entry in entries:
                path = tuple(part.removeprefix("r#") for part in entry.path)
                if not path or path[-1] == "*":
                    fail(f"unprovable lexical macro import in {unit.relative}")
                name = (entry.alias or path[-1]).removeprefix("r#")
                if name == "_":
                    continue
                binding = LexicalUseBinding(
                    name,
                    unit.relative,
                    index,
                    block[0] + 1,
                    block[1],
                    path,
                    absolute,
                )
                if any(
                    current.name == binding.name
                    and current.start == binding.start
                    and current.end == binding.end
                    for current in unit.lexical_use_bindings
                ):
                    fail(
                        f"ambiguous lexical macro binding in {unit.relative}: {name}"
                    )
                unit.lexical_use_bindings.append(binding)
            index = end + 1

    def macro_definition_active(
        self, definition: MacroDefinition, site: MacroInvocationSite
    ) -> bool:
        """Apply macro_rules textual order, block range, and child-module propagation."""

        if not definition.module_scoped:
            return (
                definition.source == site.source
                and definition.active_start <= site.token_index < definition.active_end
            )
        invocation_scope = self.scope_for(site.source, site.token_index)
        if invocation_scope is None or definition.scope_identity is None:
            return (
                definition.source == site.source
                and definition.active_start <= site.token_index < definition.active_end
            )
        if invocation_scope.identity == definition.scope_identity:
            return (
                definition.source == site.source
                and definition.active_start <= site.token_index < definition.active_end
            )
        if invocation_scope.identity[: len(definition.scope_identity)] != definition.scope_identity:
            return False
        child_identity = invocation_scope.identity[: len(definition.scope_identity) + 1]
        child = self.scopes.get(child_identity)
        return bool(
            child is not None
            and child.declared_at_source == definition.source
            and child.declared_at_index is not None
            and definition.end <= child.declared_at_index
        )

    def lexical_use_shadow(
        self, site: MacroInvocationSite, name: str
    ) -> LexicalUseBinding | None:
        """Return the innermost active lexical import with the requested name."""

        unit = self.sources.get(site.source)
        if unit is None:
            return None
        candidates = [
            binding
            for binding in unit.lexical_use_bindings
            if binding.name == name and binding.start <= site.token_index < binding.end
        ]
        return min(candidates, key=lambda binding: binding.end - binding.start, default=None)

    def module_path_shadow(
        self, site: MacroInvocationSite, name: str
    ) -> str | None:
        """Return a module/type/import provenance that shadows one relative path root."""

        scope = self.scope_for(site.source, site.token_index)
        if scope is not None:
            identity = scope.identity
            while True:
                candidate = self.scopes[identity]
                if name in candidate.bindings:
                    return f"use binding in {candidate.source}"
                if name in candidate.children:
                    return f"module binding in {candidate.source}"
                item = candidate.local_items.get(name)
                if item is not None and item.namespace == "type":
                    return f"type binding in {candidate.source}"
                if not identity:
                    break
                identity = identity[:-1]
        lexical_use = self.lexical_use_shadow(site, name)
        if lexical_use is not None:
            return f"lexical use binding in {lexical_use.source}"
        lexical_item = self.lexical_item_binding(site.source, site.token_index, name)
        if lexical_item is not None and lexical_item.namespace in {"module", "type"}:
            return f"lexical {lexical_item.namespace} binding in {lexical_item.source}"
        return None

    def bare_macro_shadow(
        self, site: MacroInvocationSite, name: str
    ) -> str | None:
        """Return textual/path-based provenance shadowing a standard-prelude macro."""

        for definition in self.macro_definitions:
            if definition.name == name and self.macro_definition_active(definition, site):
                return f"textual macro definition in {definition.source}"
        scope = self.scope_for(site.source, site.token_index)
        if scope is not None:
            identity = scope.identity
            while True:
                candidate = self.scopes[identity]
                if name in candidate.bindings:
                    return f"macro-capable use binding in {candidate.source}"
                if not identity:
                    break
                identity = identity[:-1]
        lexical_use = self.lexical_use_shadow(site, name)
        if lexical_use is not None:
            return f"lexical macro-capable use binding in {lexical_use.source}"
        if name in self.exported_macros:
            return f"crate-root exported macro in {self.exported_macros[name].source}"
        return None

    def prove_canonical_crate_macro(self, site: MacroInvocationSite, name: str) -> None:
        """Require the exact checked macro_export definition, never a same-name re-export."""

        expected_scope = CANONICAL_CRATE_MACRO_SCOPES.get(name)
        expected_source = CANONICAL_CRATE_MACRO_SOURCES.get(name)
        definition = self.exported_macros.get(name)
        root_scope = self.scopes[()]
        if (
            expected_scope is None
            or expected_source is None
            or definition is None
            or definition.scope_identity != expected_scope
            or definition.source != expected_source
            or not definition.module_scoped
            or definition.raw
            or name in root_scope.bindings
        ):
            fail(
                "crate macro lacks canonical definition provenance in "
                f"{site.source}: {name}!"
            )

    def verify_macro_invocation(self, site: MacroInvocationSite) -> None:
        """Approve a macro only after resolving raw/textual/path binding provenance."""

        rendered = "::".join(site.path)
        if site.raw:
            fail(f"raw macro identifier has unprovable provenance in {site.source}: {rendered}!")
        if len(site.path) == 1:
            name = site.path[0]
            if name == "impl_standard_wrapper":
                fail(
                    "bare impl_standard_wrapper macro provenance is ambiguous; "
                    f"use crate::impl_standard_wrapper! in {site.source}"
                )
            if name in ALLOWED_BARE_PRELUDE_MACROS:
                shadow = self.bare_macro_shadow(site, name)
                if shadow is not None:
                    fail(
                        f"bare macro is shadowed in {site.source}: {name}! ({shadow})"
                    )
                return
        if not site.absolute and site.path == ("crate", "impl_standard_wrapper"):
            self.prove_canonical_crate_macro(site, "impl_standard_wrapper")
            return
        if len(site.path) >= 2 and site.path[0] in ALLOWED_QUALIFIED_MACRO_ROOTS:
            if not site.absolute:
                shadow = self.module_path_shadow(site, site.path[0])
                if shadow is not None:
                    fail(
                        "qualified standard macro root is shadowed in "
                        f"{site.source}: {rendered}! ({shadow})"
                    )
            return
        fail(
            f"macro invocation has no canonical binding provenance in {site.source}: "
            f"{rendered}!"
        )

    def verify_macro_invocations(self, unit: SourceUnit) -> None:
        """Reject every invocation whose resolved binding is not explicitly trusted."""

        self.ensure_macro_provenance_index(unit)
        if not unit.macro_invocations_indexed:
            unit.macro_invocations.extend(macro_invocation_sites(unit))
            unit.macro_invocations_indexed = True
        for site in unit.macro_invocations:
            self.verify_macro_invocation(site)

    def lexical_binding_kind(
        self, source: Path | None, token_index: int | None, name: str
    ) -> str | None:
        """Resolve the innermost range-scoped generic binding at one source token."""

        if source is None or token_index is None:
            return None
        unit = self.sources.get(source)
        if unit is None:
            return None
        candidates = [
            scope
            for scope in unit.lexical_generic_scopes
            if scope.start <= token_index < scope.end
            and (
                name in scope.type_bindings
                or name in scope.const_bindings
            )
        ]
        for lexical_scope in sorted(
            candidates, key=lambda scope: (scope.end - scope.start, -scope.start)
        ):
            if lexical_scope.declaration_kind == "fn" and any(
                lexical_scope.start < declaration.start <= token_index < declaration.end
                for declaration in unit.declaration_scopes
            ):
                continue
            if name in lexical_scope.type_bindings:
                return "type"
            if name in lexical_scope.const_bindings:
                return "const"
        return None

    def lexical_item_binding(
        self, source: Path | None, token_index: int | None, name: str
    ) -> LexicalItemScope | None:
        """Return the innermost nested item binding visible at one source token."""

        if source is None or token_index is None:
            return None
        unit = self.sources.get(source)
        if unit is None:
            return None
        candidates = [
            scope
            for scope in unit.lexical_item_scopes
            if scope.name == name and scope.start <= token_index < scope.end
        ]
        return min(candidates, key=lambda scope: scope.end - scope.start, default=None)

    @staticmethod
    def module_macro_path(
        tokens: list[Token], index: int, end: int
    ) -> tuple[tuple[str, ...], int] | None:
        """Parse a module-scope named macro invocation starting at index."""

        if index >= end or tokens[index].kind != "IDENT":
            return None
        path = [canonical_identifier(tokens[index])]
        cursor = index + 1
        while cursor + 1 < end and tokens[cursor].text == "::" and tokens[cursor + 1].kind == "IDENT":
            path.append(canonical_identifier(tokens[cursor + 1]))
            cursor += 2
        if cursor < end and tokens[cursor].text == "!":
            return tuple(path), cursor
        return None

    def scope_for(self, relative: Path, token_index: int) -> ModuleScope | None:
        """Return the deepest reachable inline/external module containing one token."""

        unit = self.sources.get(relative)
        if unit is None:
            return None
        candidates = [scope for scope in unit.scopes if scope.start <= token_index < scope.end]
        return max(candidates, key=lambda scope: len(scope.identity), default=None)

    def resolve_binding(
        self,
        scope: ModuleScope,
        name: str,
        *,
        canonical: bool,
        resolving: set[tuple[tuple[str, ...], str]],
    ) -> ResolvedTarget:
        """Second pass: resolve one explicit binding and reject cycles/unknown targets."""

        cache_key = (scope.identity, name, canonical)
        if cache_key in self.binding_cache:
            return self.binding_cache[cache_key]
        recursion_key = (scope.identity, name)
        if recursion_key in resolving:
            fail(f"cyclic re-export binding at crate::{'::'.join(scope.identity + (name,))}")
        binding = scope.bindings[name]
        resolving.add(recursion_key)
        target = self.resolve_path(
            scope,
            binding.path,
            absolute=binding.absolute,
            canonical=canonical,
            resolving=resolving,
            source=binding.source,
            token_index=binding.token_index,
        )
        resolving.remove(recursion_key)
        self.binding_cache[cache_key] = target
        return target

    def member(
        self,
        module: ModuleScope,
        name: str,
        requester: ModuleScope,
        *,
        canonical: bool,
        resolving: set[tuple[tuple[str, ...], str]],
    ) -> ResolvedTarget:
        """Resolve one child/binding/item from an already resolved module."""

        requester_is_descendant = (
            requester.identity[: len(module.identity)] == module.identity
        )
        require_public = not requester_is_descendant
        if name in module.children:
            if require_public and not module.child_visibility[name]:
                fail(f"module child is not reachable: crate::{'::'.join(module.identity + (name,))}")
            return ResolvedTarget("module", module.children[name])
        if name in module.bindings:
            binding = module.bindings[name]
            if require_public and not binding.public:
                fail(f"binding is not reachable: crate::{'::'.join(module.identity + (name,))}")
            return self.resolve_binding(
                module, name, canonical=canonical, resolving=resolving
            )
        if name in module.local_items:
            item = module.local_items[name]
            if require_public and not item.public:
                fail(f"item is not reachable: crate::{'::'.join(module.identity + (name,))}")
            return ResolvedTarget("item", module.identity)
        fail(f"absent child/binding/re-export: crate::{'::'.join(module.identity + (name,))}")

    def resolve_path(
        self,
        scope: ModuleScope | None,
        path: tuple[str, ...],
        *,
        absolute: bool,
        canonical: bool,
        resolving: set[tuple[tuple[str, ...], str]] | None = None,
        source: Path | None = None,
        token_index: int | None = None,
    ) -> ResolvedTarget:
        """Resolve crate/self/N-super/bare paths uniformly for use and non-use sites."""

        if not path:
            fail("cannot resolve an empty Rust path")
        resolving = resolving if resolving is not None else set()
        root_scope = self.scopes[()]
        cursor = 0
        first = path[0]
        if absolute:
            if first not in ALLOWED_EXTERNAL_IMPORT_ROOTS and canonical:
                fail(f"unknown absolute/external root: {first}")
            return ResolvedTarget("external", external_root=first)
        if first == "crate":
            target = ResolvedTarget("module", ())
            cursor = 1
        elif first == "self":
            if scope is None:
                fail("self path has no reachable module scope")
            target = ResolvedTarget("module", scope.identity)
            cursor = 1
        elif first == "super":
            if scope is None:
                fail("super path has no reachable module scope")
            identity = scope.identity
            while cursor < len(path) and path[cursor] == "super":
                if not identity:
                    fail(f"super traversal above crate root in {scope.source}")
                identity = identity[:-1]
                cursor += 1
            target = ResolvedTarget("module", identity)
        elif first == "Self":
            return ResolvedTarget("item", scope.identity if scope else ())
        elif first in ALLOWED_EXTERNAL_IMPORT_ROOTS or first in RUST_PRELUDE_ROOTS:
            return ResolvedTarget("external", external_root=first)
        else:
            lexical_kind = self.lexical_binding_kind(source, token_index, first)
            if lexical_kind == "type":
                return ResolvedTarget("generic_type", scope.identity if scope else ())
            if lexical_kind == "const":
                fail(f"const generic cannot be an associated path root in {source}: {first}")
            lexical_item = self.lexical_item_binding(source, token_index, first)
            if lexical_item is not None:
                if lexical_item.namespace == "value" and len(path) != 1:
                    fail(
                        f"function binding cannot be a type/module namespace in {source}: "
                        f"{first}"
                    )
                return ResolvedTarget(
                    f"lexical_{lexical_item.namespace}",
                    scope.identity if scope else (),
                )
            if scope is None:
                if canonical:
                    fail(f"unknown path root without reachable module scope: {first}")
                return ResolvedTarget("external", external_root=first)
            try:
                target = self.member(
                    scope, first, scope, canonical=canonical, resolving=resolving
                )
            except VerificationError:
                if canonical:
                    raise VerificationError(f"unknown path root in {scope.source}: {first}") from None
                return ResolvedTarget("external", external_root=first)
            cursor = 1

        requester = scope or root_scope
        while cursor < len(path):
            if target.kind != "module" or target.module is None:
                break
            module = self.scopes.get(target.module)
            if module is None:
                fail(f"resolved module is absent from reachable index: {'::'.join(target.module)}")
            target = self.member(
                module,
                path[cursor],
                requester,
                canonical=canonical,
                resolving=resolving,
            )
            cursor += 1
        self.validate_target(target, path, canonical=canonical)
        return target

    @staticmethod
    def validate_target(
        target: ResolvedTarget, path: tuple[str, ...], *, canonical: bool
    ) -> None:
        """Enforce canonical external and transitional-root allowlists after resolution."""

        if not canonical:
            return
        if target.kind == "external":
            root = target.external_root or path[0]
            if root not in ALLOWED_EXTERNAL_IMPORT_ROOTS and root not in RUST_PRELUDE_ROOTS:
                fail(f"backend/external root is forbidden: {root}")
        if target.module and target.module[0] in FORBIDDEN_TRANSITIONAL_PATH_ROOTS:
            fail(
                "canonical path resolves into transitional module: "
                f"{'::'.join(target.module)} via {'::'.join(path)}"
            )

def discover_standard_files(index: ModuleScopeIndex) -> tuple[Path, ...]:
    """Keep every physical Rust file under canonical roots in the pollution denominator."""

    files: list[Path] = []
    missing: list[str] = []
    for module in index.canonical_roots:
        directory = index.root / module
        module_root = directory / "mod.rs"
        if not module_root.is_file():
            missing.append(module_root.relative_to(index.root).as_posix())
            continue
        discovered = sorted(path for path in directory.rglob("*.rs") if path.is_file())
        if len(discovered) < 2:
            missing.append(f"{module}/<canonical-object>.rs")
            continue
        files.extend(discovered)
    if missing:
        fail(f"standard scaffold is incomplete: {missing}")
    return tuple(sorted(path.relative_to(index.root) for path in files))


def parse_candidate_path(
    tokens: list[Token], index: int, covered: set[int]
) -> tuple[tuple[str, ...], bool, int] | None:
    """Parse one non-use qualified path, including ::root and $crate forms."""

    if index in covered:
        return None
    if (
        tokens[index].kind == "LIFETIME"
        and index + 1 < len(tokens)
        and tokens[index + 1].text == "::"
    ):
        fail(f"lifetime cannot be an associated path root: {tokens[index].text}")
    absolute = False
    if tokens[index].text == "::":
        if index > 0 and tokens[index - 1].text in {">", ")", "]"}:
            return None
        absolute = True
        index += 1
    elif (
        tokens[index].text == "$"
        and index + 2 < len(tokens)
        and tokens[index + 1].text == "crate"
        and tokens[index + 2].text == "::"
    ):
        start = index
        path = ["crate"]
        cursor = index + 2
        while cursor + 1 < len(tokens) and tokens[cursor].text == "::" and tokens[cursor + 1].kind == "IDENT":
            path.append(canonical_identifier(tokens[cursor + 1]))
            cursor += 2
        return tuple(path), False, max(cursor, start + 1)
    elif tokens[index].kind != "IDENT":
        return None
    if index >= len(tokens) or tokens[index].kind != "IDENT":
        return None
    if index > 0 and tokens[index - 1].text in {"::", ".", "$"}:
        return None
    if index + 1 >= len(tokens) or tokens[index + 1].text != "::":
        return None
    path = [canonical_identifier(tokens[index])]
    cursor = index + 1
    while cursor + 1 < len(tokens) and tokens[cursor].text == "::" and tokens[cursor + 1].kind == "IDENT":
        path.append(canonical_identifier(tokens[cursor + 1]))
        cursor += 2
    return tuple(path), absolute, cursor


def verify_file_paths(index: ModuleScopeIndex, relative: Path) -> None:
    """Resolve every import/non-use/macro path against the reachable module index."""

    unit = index.sources.get(relative)
    if unit is None:
        path = index.root / relative
        try:
            source = path.read_text(encoding="utf-8")
        except UnicodeDecodeError as error:
            fail(f"standard source is not UTF-8: {relative}: {error}")
        tokens = tokenize(source)
        matching, reverse = delimiter_index(tokens, relative)
        unit = SourceUnit(relative, tokens, matching, reverse)
    attribute_ranges(unit)
    macro_ranges = macro_definition_ranges(unit)
    macro_indexes = {
        cursor
        for start, end in macro_ranges
        for cursor in range(start, end)
    }
    tokens = unit.tokens
    for cursor, token in enumerate(tokens):
        if cursor in macro_indexes:
            continue
        name = token.text.removeprefix("r#")
        if name == "include" and cursor + 1 < len(tokens) and tokens[cursor + 1].text == "!":
            fail(f"include! escapes reachable module discovery in {relative}")
        if (
            name == "extern"
            and cursor + 2 < len(tokens)
            and tokens[cursor + 1].text == "crate"
            and tokens[cursor + 2].kind == "IDENT"
        ):
            crate_name = canonical_identifier(tokens[cursor + 2])
            if crate_name not in ALLOWED_EXTERNAL_IMPORT_ROOTS:
                fail(f"extern crate is forbidden in {relative}: {crate_name}")

    index.verify_macro_invocations(unit)

    covered: set[int] = set(macro_indexes)
    for site in unit.macro_invocations:
        covered.update(range(site.token_index, site.end))
    cursor = 0
    while cursor < len(tokens):
        if cursor in macro_indexes:
            cursor += 1
            continue
        if tokens[cursor].text != "use":
            cursor += 1
            continue
        try:
            entries, end = parse_use_tree(tokens, cursor + 1)
        except Exception as error:
            fail(f"cannot parse canonical import in {relative}: {error}")
        if end >= len(tokens) or tokens[end].text != ";":
            fail(f"use item is not terminated in {relative}")
        scope = index.scope_for(relative, cursor)
        absolute = cursor + 1 < len(tokens) and tokens[cursor + 1].text == "::"
        for entry in entries:
            path = tuple(part.removeprefix("r#") for part in entry.path)
            if not path:
                fail(f"empty canonical import in {relative}")
            if path[-1] == "*":
                fail(f"wildcard import is forbidden in canonical source: {relative}")
            index.resolve_path(
                scope,
                path,
                absolute=absolute,
                canonical=True,
                source=relative,
                token_index=cursor,
            )
        covered.update(range(cursor, end + 1))
        cursor = end + 1

    cursor = 0
    while cursor < len(tokens):
        parsed = parse_candidate_path(tokens, cursor, covered)
        if parsed is None:
            cursor += 1
            continue
        path, absolute, end = parsed
        scope = index.scope_for(relative, cursor)
        index.resolve_path(
            scope,
            path,
            absolute=absolute,
            canonical=True,
            source=relative,
            token_index=cursor,
        )
        cursor = max(cursor + 1, end)


def verify_tree(root: Path, *, phase: str) -> tuple[Path, ...]:
    """Verify one source tree for the requested migration phase."""

    if phase not in {"scaffold", "transitional"}:
        fail(f"unknown purity phase: {phase}")
    module_index = ModuleScopeIndex(root)
    standard_files = discover_standard_files(module_index)
    for relative in standard_files:
        path = root / relative
        try:
            source = path.read_text(encoding="utf-8")
        except UnicodeDecodeError as error:
            fail(f"standard source is not UTF-8: {relative}: {error}")
        tokens = tokenize(source)
        token_texts = [token.text for token in tokens]
        polluted = sorted(
            {
                token.text
                for token in tokens
                if token.kind == "IDENT" and forbidden_identity(token.text)
            }
        )
        if polluted:
            fail(f"standard source contains product/runtime identities: {relative}: {polluted}")
        verify_file_paths(module_index, relative)
        if relative.name == "mod.rs":
            verify_module_root(relative, token_texts)
    return standard_files


def main() -> int:
    """Run the repository purity gate."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--phase", choices=("scaffold", "transitional"), required=True)
    arguments = parser.parse_args()
    root = Path(__file__).resolve().parent.parent / "crates/druid-rdbc/src"
    try:
        standard_files = verify_tree(root, phase=arguments.phase)
    except VerificationError as error:
        print(f"ERROR: {error}")
        return 1
    print(f"STANDARD_FILES={len(standard_files)}")
    print(f"PURITY_PHASE={arguments.phase}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
