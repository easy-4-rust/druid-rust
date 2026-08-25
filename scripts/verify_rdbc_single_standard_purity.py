#!/usr/bin/env python3
"""Verify that canonical RDBC standard modules remain product-independent."""

from __future__ import annotations

import argparse
from dataclasses import dataclass, field
from pathlib import Path
from typing import NoReturn

from verify_rdbc_public_paths import Token, tokenize
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
ALLOWED_CRATE_ITEMS = {"impl_standard_wrapper"}
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
    children: dict[str, tuple[str, ...]] = field(default_factory=dict)
    child_visibility: dict[str, bool] = field(default_factory=dict)
    bindings: dict[str, UseBinding] = field(default_factory=dict)
    local_items: dict[str, LocalItem] = field(default_factory=dict)
    use_ranges: list[tuple[int, int]] = field(default_factory=list)
    macro_edges: list[ModuleMacroEdge] = field(default_factory=list)


@dataclass
class SourceUnit:
    """Tokenized Rust source plus delimiter and reachable-scope indexes."""

    relative: Path
    tokens: list[Token]
    matching: dict[int, int]
    reverse_matching: dict[int, int]
    scopes: list[ModuleScope] = field(default_factory=list)


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
                and canonical_identifier(tokens[cursor]) == "path"
                and cursor + 1 < end
                and tokens[cursor + 1].text == "="
            ):
                fail(f"#[path] escapes reachable module discovery in {unit.relative}")
        ranges.append((index, end + 1))
        index = end + 1
    return tuple(ranges)


class ModuleScopeIndex:
    """Two-pass reachable Rust module/binding index rooted at src/lib.rs."""

    def __init__(self, root: Path) -> None:
        self.root = root
        self.sources: dict[Path, SourceUnit] = {}
        self.scopes: dict[tuple[str, ...], ModuleScope] = {}
        self.binding_cache: dict[tuple[tuple[str, ...], str, bool], ResolvedTarget] = {}
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
        for scope in self.scopes.values():
            if scope.identity and scope.identity[0] in self.canonical_roots:
                for name in scope.bindings:
                    self.resolve_binding(scope, name, canonical=True, resolving=set())
                self.verify_macro_edges(scope)

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
    ) -> ModuleScope:
        """Register one unique reachable module identity and source range."""

        if identity in self.scopes:
            fail(f"ambiguous module identity: {'::'.join(identity) or 'crate'}")
        scope = ModuleScope(identity, source, start, end, parent, public)
        self.scopes[identity] = scope
        self.source(source).scopes.append(scope)
        return scope

    def public_before(self, unit: SourceUnit, index: int, scope: ModuleScope) -> bool:
        """Return whether the item keyword has any pub visibility qualifier."""

        previous = index - 1
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
                        identity, relative, 0, len(child_unit.tokens), scope.identity, public
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
                        identity, scope.source, index + 3, end, scope.identity, public
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
                    name = entry.alias or path[-1]
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
                if token.text == "fn" and index > scope.start and tokens[index - 1].text == "const":
                    pass
                elif tokens[index + 1].kind == "IDENT":
                    name = canonical_identifier(tokens[index + 1])
                    if name in scope.children or name in scope.bindings or name in scope.local_items:
                        fail(f"ambiguous local item binding in {scope.source}: {name}")
                    scope.local_items[name] = LocalItem(self.public_before(unit, index, scope))
            elif token.text in {"const", "static"}:
                cursor = index + 1
                if cursor < scope.end and tokens[cursor].text == "mut":
                    cursor += 1
                if cursor < scope.end and tokens[cursor].kind == "IDENT" and tokens[cursor].text != "fn":
                    name = canonical_identifier(tokens[cursor])
                    if name != "_":
                        scope.local_items.setdefault(
                            name, LocalItem(self.public_before(unit, index, scope))
                        )
            elif token.text == "macro_rules" and index + 2 < scope.end:
                if tokens[index + 1].text == "!" and tokens[index + 2].kind == "IDENT":
                    name = canonical_identifier(tokens[index + 2])
                    scope.local_items.setdefault(name, LocalItem(False))

            macro = self.module_macro_path(tokens, index, scope.end)
            if macro is not None and token.text != "macro_rules":
                path, bang = macro
                scope.macro_edges.append(ModuleMacroEdge(path, scope.source, index))
                index = bang + 1
                continue
            index += 1

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
        if module.identity == () and name in ALLOWED_CRATE_ITEMS:
            return ResolvedTarget("item", ())
        fail(f"absent child/binding/re-export: crate::{'::'.join(module.identity + (name,))}")

    def resolve_path(
        self,
        scope: ModuleScope | None,
        path: tuple[str, ...],
        *,
        absolute: bool,
        canonical: bool,
        resolving: set[tuple[tuple[str, ...], str]] | None = None,
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

    def verify_macro_edges(self, scope: ModuleScope) -> None:
        """Allow only the reviewed macro that emits Wrapper impls at module scope."""

        for edge in scope.macro_edges:
            if edge.path[-1] == "include":
                fail(f"include! escapes reachable module discovery in {edge.source}")
            target = self.resolve_path(
                scope,
                edge.path,
                absolute=False,
                canonical=True,
            )
            if (
                edge.path[-1] in ALLOWED_CRATE_ITEMS
                and target.kind == "item"
                and target.module == ()
            ):
                continue
            fail(
                "unknown macro-generated module edge in "
                f"{edge.source}: {'::'.join(edge.path)}!"
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
    tokens = unit.tokens
    for cursor, token in enumerate(tokens):
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

    covered: set[int] = set()
    cursor = 0
    while cursor < len(tokens):
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
            index.resolve_path(scope, path, absolute=absolute, canonical=True)
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
        index.resolve_path(scope, path, absolute=absolute, canonical=True)
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
