#!/usr/bin/env python3
"""Freeze and verify the live single-standard RDBC migration denominator."""

from __future__ import annotations

import argparse
import hashlib
import subprocess
import sys
import unicodedata
from dataclasses import dataclass, fields as dataclass_fields
from pathlib import Path
from typing import NoReturn

from verify_rdbc_public_paths import VerificationError as RustLexError
from verify_rdbc_public_paths import tokenize
from verify_wrapper_rdbc_import_boundary import canonical_identifier, parse_use_tree


P03_BASE = "d25ef68d9d05c39eb75cb953e46f97ccff6a5908"
P03_MANIFEST = Path("docs/verification/rdbc-first/rdbc-physical-standardization.tsv")
DEFAULT_ADAPTERS = Path("docs/verification/rdbc-first/rdbc-adapter-implementations.tsv")
DEFAULT_CONSUMERS = Path("docs/verification/rdbc-first/rdbc-production-consumers.tsv")
PHYSICAL_HEADER = (
    "old_path\tclassification\ttarget_path\tcompatibility\tsource_file\t"
    "normalized_signature\ttarget_task"
)
LEGACY_PHYSICAL_HEADER = (
    "old_path\tclassification\ttarget_path\tcompatibility\tsource_file\t"
    "normalized_signature\tevidence"
)
ADAPTER_HEADER = (
    "source_path\tcurrent_trait\timplementation_type\towner\ttarget_trait\t"
    "target_task\tstate"
)
CONSUMER_HEADER = (
    "source_path\toccurrence_kind\tsymbol\told_path\ttarget_path\t"
    "target_task\tstate"
)
CLASSIFICATIONS = {"MERGE_STANDARD", "RDBC_EXTENSION", "DRUID_INTERNAL", "CONCRETE_IMPL"}
OCCURRENCE_KINDS = {"IMPORT", "TYPE", "TRAIT_OBJECT", "IMPL", "CALL"}
PRODUCTION_ROOTS = (
    Path("crates/druid-rdbc/src"),
    Path("crates/druid/src"),
    Path("crates/druid-wrapper/src"),
)


class VerificationError(RuntimeError):
    """Raised when source and checked-in denominator artifacts diverge."""


@dataclass(frozen=True, order=True)
class PhysicalRow:
    """One current public spi/resource item and its planned disposition."""

    old_path: str
    classification: str
    target_path: str
    compatibility: str
    source_file: str
    normalized_signature: str
    target_task: str


@dataclass(frozen=True, order=True)
class AdapterRow:
    """One real current trait implementation bound to a planned standard target."""

    source_path: str
    current_trait: str
    implementation_type: str
    owner: str
    target_trait: str
    target_task: str
    state: str


@dataclass(frozen=True, order=True)
class ConsumerRow:
    """One production token occurrence of a current standardization item."""

    source_path: str
    occurrence_kind: str
    symbol: str
    old_path: str
    target_path: str
    target_task: str
    state: str


@dataclass(frozen=True)
class LiveManifests:
    """All source-derived Task 2 denominator tables."""

    physical: tuple[PhysicalRow, ...]
    adapters: tuple[AdapterRow, ...]
    consumers: tuple[ConsumerRow, ...]


def fail(message: str) -> NoReturn:
    """Stop with one stable fail-closed diagnostic."""

    raise VerificationError(message)


def repository_root() -> Path:
    """Return the repository root independently of the command's CWD."""

    return Path(__file__).resolve().parent.parent


def run_git(root: Path, arguments: list[str]) -> bytes:
    """Run one read-only Git query used to anchor the P03 denominator."""

    completed = subprocess.run(
        ["git", *arguments],
        cwd=root,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )
    if completed.returncode != 0:
        diagnostic = completed.stderr.decode("utf-8", errors="replace").strip()
        fail(f"git {' '.join(arguments)} failed: {diagnostic}")
    return completed.stdout


def parse_physical_payload(payload: bytes, *, allow_legacy: bool) -> list[PhysicalRow]:
    """Parse one strict physical manifest; legacy evidence becomes a derived target task."""

    lines = canonical_tsv_lines(payload, "physical")
    if not lines:
        fail("physical manifest is empty")
    legacy = allow_legacy and lines[0] == LEGACY_PHYSICAL_HEADER
    if lines[0] != PHYSICAL_HEADER and not legacy:
        fail("physical manifest header drift")
    rows: list[PhysicalRow] = []
    seen: set[str] = set()
    for line_number, line in enumerate(lines[1:], start=2):
        fields = line.split("\t")
        if len(fields) != 7 or any(not field for field in fields):
            fail(f"physical row {line_number} must have seven non-empty fields")
        old_path, classification, target_path, compatibility, source_file, signature, last = fields
        if old_path in seen:
            fail(f"physical row {line_number} duplicates {old_path}")
        seen.add(old_path)
        if classification not in CLASSIFICATIONS:
            fail(f"physical row {line_number} has unknown classification {classification}")
        target_task = target_task_for(old_path, classification, target_path) if legacy else last
        rows.append(
            PhysicalRow(
                old_path,
                classification,
                target_path,
                compatibility,
                source_file,
                signature,
                target_task,
            )
        )
    if not legacy:
        require_sorted_unique_rows(rows, "physical")
        if payload != render_physical_rows(tuple(rows)):
            fail("physical manifest is not canonically serialized")
    return rows


def validate_tsv_field(value: str, label: str) -> None:
    """Reject delimiters and control data before one value reaches a TSV artifact."""

    if not value:
        fail(f"{label} is empty")
    if any(unicodedata.category(character) == "Cc" for character in value):
        fail(f"{label} contains a TSV delimiter or control character")


def canonical_tsv_lines(payload: bytes, label: str) -> list[str]:
    """Decode a canonical TSV payload without normalizing CRLF or blank records."""

    if not payload.endswith(b"\n") or payload.endswith(b"\n\n"):
        fail(f"{label} manifest must have exactly one final newline")
    if b"\r" in payload or any(
        (byte < 0x20 and byte not in {0x09, 0x0A}) or byte == 0x7F
        for byte in payload
    ):
        fail(f"{label} manifest contains a forbidden control byte")
    try:
        return payload.decode("utf-8").split("\n")[:-1]
    except UnicodeDecodeError as error:
        fail(f"{label} manifest is not UTF-8: {error}")


def require_sorted_unique_rows(rows: list[object], label: str) -> None:
    """Require one deterministic row order and reject exact duplicates."""

    if rows != sorted(rows):
        fail(f"{label} manifest rows are not sorted")
    if len(rows) != len(set(rows)):
        fail(f"{label} manifest contains duplicate rows")


def parse_dataclass_tsv(
    payload: bytes,
    header: str,
    row_type: type[object],
    label: str,
) -> list[object]:
    """Parse one exact schema into immutable rows before live comparison."""

    lines = canonical_tsv_lines(payload, label)
    if not lines or lines[0] != header:
        fail(f"{label} manifest header drift")
    expected_fields = len(dataclass_fields(row_type))
    rows: list[object] = []
    for line_number, line in enumerate(lines[1:], start=2):
        values = line.split("\t")
        if len(values) != expected_fields:
            fail(f"{label} row {line_number} must have {expected_fields} fields")
        for field_number, value in enumerate(values, start=1):
            validate_tsv_field(value, f"{label} row {line_number} field {field_number}")
        rows.append(row_type(*values))
    require_sorted_unique_rows(rows, label)
    return rows


def parse_adapter_payload(payload: bytes) -> list[AdapterRow]:
    """Parse and validate the adapter table independently of the live scanner."""

    rows = parse_dataclass_tsv(payload, ADAPTER_HEADER, AdapterRow, "adapter")
    for row in rows:
        if row.state != "CURRENT_IMPL_TARGET_PLANNED":
            fail(f"adapter row has unknown state: {row.state}")
    if payload != render_adapter_rows(tuple(rows)):
        fail("adapter manifest is not canonically serialized")
    return rows


def parse_consumer_payload(payload: bytes) -> list[ConsumerRow]:
    """Parse and validate the consumer table independently of the live scanner."""

    rows = parse_dataclass_tsv(payload, CONSUMER_HEADER, ConsumerRow, "consumer")
    for row in rows:
        if row.occurrence_kind not in OCCURRENCE_KINDS:
            fail(f"consumer row has unknown occurrence kind: {row.occurrence_kind}")
        if row.state != "CURRENT_REFERENCE_TARGET_PLANNED":
            fail(f"consumer row has unknown state: {row.state}")
    if payload != render_consumer_rows(tuple(rows)):
        fail("consumer manifest is not canonically serialized")
    return rows


def p03_baseline_rows(root: Path) -> dict[str, PhysicalRow]:
    """Load the immutable 48-row P03 denominator from the task's declared Git base."""

    payload = run_git(root, ["show", f"{P03_BASE}:{P03_MANIFEST.as_posix()}"])
    rows = parse_physical_payload(payload, allow_legacy=True)
    if len(rows) != 48:
        fail(f"P03 baseline row count drift: expected 48, found {len(rows)}")
    return {row.old_path: row for row in rows}


def target_task_for(old_path: str, classification: str, target_path: str) -> str:
    """Assign one approved downstream task without claiming its implementation is live."""

    if classification == "DRUID_INTERNAL":
        return "P04_TASK_5_DRUID_INTERNALIZATION"
    if classification == "CONCRETE_IMPL":
        if "BufferedRowSet" in target_path or "resource" in old_path:
            return "P04_TASK_7_SEMANTIC_HARDENING"
        return "P04_TASK_4_ADAPTER_CUTOVER"
    if "::resource::" in old_path:
        return "P04_TASK_7_SEMANTIC_HARDENING"
    return "P04_TASK_3_STANDARD_TRAITS"


def canonical_name(token: object) -> str:
    """Return a raw or ordinary Rust identifier's semantic spelling."""

    try:
        return canonical_identifier(token)
    except Exception as error:
        fail(f"cannot normalize Rust identifier: {error}")


def rust_tokens(source: str) -> list[object]:
    """Tokenize Rust while rejecting malformed comments/literals rather than guessing."""

    try:
        return tokenize(source)
    except RustLexError as error:
        fail(f"cannot tokenize Rust production source: {error}")


def cfg_test_indexes(tokens: list[object]) -> set[int]:
    """Return token indexes owned by exact ``#[cfg(test)]`` items."""

    excluded: set[int] = set()
    index = 0
    marker = ("#", "[", "cfg", "(", "test", ")", "]")
    while index + len(marker) <= len(tokens):
        if tuple(token.text for token in tokens[index:index + len(marker)]) != marker:
            index += 1
            continue
        cursor = index + len(marker)
        while cursor < len(tokens) and tokens[cursor].text not in {"{", ";"}:
            cursor += 1
        if cursor >= len(tokens):
            fail("cfg(test) item has no body or semicolon")
        end = cursor
        if tokens[cursor].text == "{":
            depth = 1
            end += 1
            while end < len(tokens) and depth:
                depth += tokens[end].text == "{"
                depth -= tokens[end].text == "}"
                end += 1
            if depth:
                fail("cfg(test) item has an unterminated body")
        else:
            end += 1
        excluded.update(range(index, end))
        index = end
    return excluded


def scan_public_items(root: Path) -> dict[str, tuple[str, str]]:
    """Enumerate every current public declaration in druid-rdbc spi and resource."""

    found: dict[str, tuple[str, str]] = {}
    for domain in ("spi", "resource"):
        source_root = root / "crates/druid-rdbc/src" / domain
        if not source_root.is_dir():
            fail(f"RDBC source domain not found: {source_root}")
        for source_file in sorted(source_root.rglob("*.rs")):
            relative = source_file.relative_to(root)
            tokens = rust_tokens(source_file.read_text(encoding="utf-8"))
            for index in range(len(tokens) - 2):
                if tokens[index].kind != "IDENT" or tokens[index].text != "pub":
                    continue
                kind_token = tokens[index + 1]
                name_token = tokens[index + 2]
                if kind_token.kind != "IDENT" or kind_token.text not in {"trait", "struct", "enum", "type"}:
                    continue
                if name_token.kind != "IDENT":
                    fail(f"public {kind_token.text} lacks a name in {relative}")
                name = canonical_name(name_token)
                old_path = f"druid_rdbc::{domain}::{name}"
                if old_path in found:
                    fail(f"duplicate current public item: {old_path}")
                found[old_path] = (
                    relative.as_posix(),
                    f"{kind_token.text} {name}",
                )
    if not found:
        fail("current spi/resource scan produced no public items")
    return found


STANDARD_RESOURCE_NAMES = {"Array", "Blob", "Clob", "NClob", "Ref", "SqlXml"}


def classify_new_resource(old_path: str, source_file: str, signature: str) -> PhysicalRow:
    """Classify a current resource item absent from the frozen 48-row P03 subset."""

    name = old_path.rsplit("::", 1)[1]
    kind = signature.split(" ", 1)[0]
    if name in STANDARD_RESOURCE_NAMES:
        classification = "MERGE_STANDARD"
        target = f"druid_rdbc::resource::{name}"
        compatibility = "REPLACE_VALUE_ALIAS_WITH_STANDARD_TRAIT_P04_TASK7"
    elif name.startswith("Physical"):
        classification = "RDBC_EXTENSION"
        base = name.removeprefix("Physical")
        target = f"druid_rdbc::extensions::{base}"
        compatibility = "MOVE_AND_RETAIN_UNTIL_P04_TASK6"
    elif kind == "enum" and (name.endswith("Length") or name.endswith("RepresentationType")):
        classification = "RDBC_EXTENSION"
        target = f"druid_rdbc::extensions::{name.removeprefix('Rdbc')}"
        compatibility = "MOVE_AND_RETAIN_UNTIL_P04_TASK6"
    elif name.startswith("Rdbc"):
        classification = "CONCRETE_IMPL"
        target = f"druid_rdbc::extensions::{name.removeprefix('Rdbc')}Handle"
        compatibility = "MOVE_AND_RETAIN_UNTIL_P04_TASK6"
    else:
        fail(f"unclassified live resource item: {old_path}")
    return PhysicalRow(
        old_path,
        classification,
        target,
        compatibility,
        source_file,
        signature,
        target_task_for(old_path, classification, target),
    )


def derive_physical_rows(root: Path) -> tuple[PhysicalRow, ...]:
    """Union the immutable P03 subset with every current live public declaration."""

    baseline = p03_baseline_rows(root)
    current = scan_public_items(root)
    missing_baseline = sorted(set(baseline) - set(current))
    if missing_baseline:
        fail(f"P03 physical denominator disappeared from source: {missing_baseline}")
    rows: list[PhysicalRow] = []
    for old_path, (source_file, signature) in current.items():
        if old_path in baseline:
            row = baseline[old_path]
            if row.source_file != source_file or row.normalized_signature != signature:
                fail(
                    f"P03 declaration drift for {old_path}: "
                    f"{source_file}/{signature}"
                )
            rows.append(row)
        else:
            if "::resource::" not in old_path:
                fail(f"new unreviewed spi item is outside the P03 denominator: {old_path}")
            rows.append(classify_new_resource(old_path, source_file, signature))
    return tuple(sorted(rows))


def module_prefixes(mapping: dict[str, str]) -> set[tuple[str, ...]]:
    """Return every standardized module prefix used to reject wildcard escape."""

    prefixes: set[tuple[str, ...]] = set()
    for old_path in mapping:
        parts = tuple(old_path.split("::"))
        for length in range(1, len(parts)):
            prefixes.add(parts[:length])
    return prefixes


def module_context(path: Path) -> tuple[str, ...]:
    """Derive the Rust module containing one production source file."""

    parts = path.parts
    try:
        crates_index = parts.index("crates")
        source_index = parts.index("src", crates_index + 2)
    except ValueError:
        fail(f"production path has no crates/<name>/src module context: {path}")
    crate_name = parts[crates_index + 1].replace("-", "_")
    relative = list(parts[source_index + 1:])
    if not relative:
        fail(f"production source path has no file name: {path}")
    file_name = relative.pop()
    if file_name not in {"lib.rs", "main.rs", "mod.rs"}:
        relative.append(Path(file_name).stem)
    return (crate_name, *relative)


def expanded_path(
    path: tuple[str, ...],
    aliases: dict[str, tuple[str, ...]],
    context: tuple[str, ...],
) -> tuple[str, ...]:
    """Expand aliases and Rust ``crate/self/super`` paths from one module context."""

    if path and path[0] in aliases:
        return aliases[path[0]] + path[1:]
    if path and path[0] == "crate":
        return (context[0], *path[1:])
    if path and path[0] == "self":
        return context + path[1:]
    if path and path[0] == "super":
        cursor = 0
        base = list(context)
        while cursor < len(path) and path[cursor] == "super":
            if len(base) <= 1:
                fail("super path escapes the crate root")
            base.pop()
            cursor += 1
        return (*base, *path[cursor:])
    return path


def proven_old_path(
    parts: tuple[str, ...],
    mapping: dict[str, str],
    proven_paths: dict[str, str],
) -> str | None:
    """Resolve only an old path or a source-proven facade/re-export prefix."""

    candidates: list[tuple[int, str]] = []
    for qualified, old_path in {**{old: old for old in mapping}, **proven_paths}.items():
        qualified_parts = tuple(qualified.split("::"))
        if qualified_parts == parts[: len(qualified_parts)]:
            candidates.append((len(qualified_parts), old_path))
    return max(candidates)[1] if candidates else None


def classify_occurrence(tokens: list[object], index: int) -> str:
    """Classify one bound identifier by its syntactic role."""

    previous = tokens[index - 1].text if index > 0 else ""
    following = tokens[index + 1].text if index + 1 < len(tokens) else ""
    if previous == "dyn":
        return "TRAIT_OBJECT"
    boundary = index - 1
    while boundary >= 0 and tokens[boundary].text not in {"impl", "{", "}", ";"}:
        boundary -= 1
    if boundary >= 0 and tokens[boundary].text == "impl":
        return "IMPL"
    if following == "::":
        cursor = index
        traversed = False
        while (
            cursor + 2 < len(tokens)
            and tokens[cursor + 1].text == "::"
            and tokens[cursor + 2].kind == "IDENT"
        ):
            traversed = True
            cursor += 2
        if traversed and cursor + 1 < len(tokens) and tokens[cursor + 1].text == "(":
            return "CALL"
    return "TYPE"


def declaration_name(tokens: list[object], index: int) -> bool:
    """Return whether an identifier is the name being declared, not a consumer."""

    return index > 0 and tokens[index - 1].text in {"trait", "struct", "enum", "type", "mod"}


def old_path_from_tokens(
    tokens: list[object],
    start: int,
    aliases: dict[str, tuple[str, ...]],
    bindings: dict[str, str],
    mapping: dict[str, str],
    context: tuple[str, ...],
    proven_paths: dict[str, str],
) -> tuple[str | None, int]:
    """Resolve one bare or qualified token path to a standardized old path."""

    if start >= len(tokens) or tokens[start].kind != "IDENT":
        return None, start
    name = canonical_name(tokens[start])
    if name in bindings:
        return bindings[name], start
    parts = (name,)
    cursor = start
    while cursor + 2 < len(tokens) and tokens[cursor + 1].text == "::" and tokens[cursor + 2].kind == "IDENT":
        parts += (canonical_name(tokens[cursor + 2]),)
        cursor += 2
    parts = expanded_path(parts, aliases, context)
    old_path = proven_old_path(parts, mapping, proven_paths)
    if old_path is None and parts and parts[0] not in {"druid", "druid_rdbc", "druid_wrapper"}:
        old_path = proven_old_path(context + parts, mapping, proven_paths)
    return old_path, cursor


def build_proven_reexports(
    root: Path,
    mapping: dict[str, str],
    declaration_paths: dict[str, str],
) -> dict[str, str]:
    """Build a source-proven qualified-path graph without same-leaf guessing."""

    proven: dict[str, str] = {old_path: old_path for old_path in mapping}
    for old_path, source_path in declaration_paths.items():
        context = module_context(Path(source_path))
        qualified = "::".join((*context, old_path.rsplit("::", 1)[1]))
        proven[qualified] = old_path

    sources = [
        (path, rust_tokens((root / path).read_text(encoding="utf-8")))
        for path in production_files(root)
    ]
    for _ in range(len(sources) + 1):
        changed = False
        for path, tokens in sources:
            context = module_context(path)
            excluded = cfg_test_indexes(tokens)
            aliases: dict[str, tuple[str, ...]] = {
                "druid_rdbc": ("druid_rdbc",),
                "druid": ("druid",),
            }
            index = 0
            while index < len(tokens):
                if (
                    index in excluded
                    or tokens[index].kind != "IDENT"
                    or tokens[index].text != "use"
                ):
                    index += 1
                    continue
                public = index > 0 and tokens[index - 1].text == "pub"
                try:
                    entries, end = parse_use_tree(tokens, index + 1)
                except Exception as error:
                    fail(f"cannot parse re-export use tree in {path}: {error}")
                if end >= len(tokens) or tokens[end].text != ";":
                    fail(f"re-export use item does not end in semicolon in {path}")
                index = end + 1
                for entry in entries:
                    expanded = expanded_path(entry.path, aliases, context)
                    if expanded and expanded[-1] == "*":
                        prefix = "::".join(expanded[:-1])
                        if public and any(key == prefix or key.startswith(prefix + "::") for key in proven):
                            fail(f"wildcard re-export can hide standardized symbols in {path}")
                        continue
                    old_path = proven_old_path(expanded, mapping, proven)
                    if old_path is None and expanded and expanded[0] not in {
                        "druid", "druid_rdbc", "druid_wrapper",
                    }:
                        old_path = proven_old_path(context + expanded, mapping, proven)
                    bound = entry.alias or (expanded[-1] if expanded else "")
                    if old_path is not None and public:
                        exported = "::".join((*context, bound))
                        existing = proven.get(exported)
                        if existing is not None and existing != old_path:
                            fail(f"conflicting RDBC re-export proof for {exported}")
                        if existing is None:
                            proven[exported] = old_path
                            changed = True
                    elif expanded and entry.alias is not None:
                        aliases[entry.alias] = expanded
        if not changed:
            return proven
    fail("RDBC re-export proof graph did not converge")


def derive_trait_methods(
    root: Path,
    physical: tuple[PhysicalRow, ...],
) -> dict[str, set[str]]:
    """Derive each standardized current trait's declared receiver-method names."""

    methods: dict[str, set[str]] = {}
    for row in physical:
        if not row.normalized_signature.startswith("trait "):
            continue
        name = row.old_path.rsplit("::", 1)[1]
        tokens = rust_tokens((root / row.source_file).read_text(encoding="utf-8"))
        trait_index = next(
            (
                index for index in range(len(tokens) - 1)
                if tokens[index].text == "trait"
                and tokens[index + 1].kind == "IDENT"
                and canonical_name(tokens[index + 1]) == name
            ),
            None,
        )
        if trait_index is None:
            fail(f"cannot find declared trait body for {row.old_path}")
        opening = trait_index + 2
        while opening < len(tokens) and tokens[opening].text != "{":
            opening += 1
        if opening >= len(tokens):
            fail(f"trait body has no opening brace for {row.old_path}")
        depth = 1
        cursor = opening + 1
        declared: set[str] = set()
        while cursor < len(tokens) and depth:
            if tokens[cursor].text == "{":
                depth += 1
            elif tokens[cursor].text == "}":
                depth -= 1
            elif (
                depth == 1
                and tokens[cursor].text == "fn"
                and cursor + 1 < len(tokens)
                and tokens[cursor + 1].kind == "IDENT"
            ):
                declared.add(canonical_name(tokens[cursor + 1]))
            cursor += 1
        if depth:
            fail(f"trait body has no closing brace for {row.old_path}")
        methods[row.old_path] = declared
    return methods


def signature_trait_paths(
    tokens: list[object],
    start: int,
    end: int,
    aliases: dict[str, tuple[str, ...]],
    bindings: dict[str, str],
    mapping: dict[str, str],
    context: tuple[str, ...],
    proven_paths: dict[str, str],
    trait_paths: set[str],
) -> set[str]:
    """Resolve every source-proven standardized trait mentioned in a signature slice."""

    found: set[str] = set()
    cursor = start
    while cursor < end:
        old_path, path_end = old_path_from_tokens(
            tokens, cursor, aliases, bindings, mapping, context, proven_paths
        )
        if old_path in trait_paths:
            found.add(old_path)
        cursor = max(cursor + 1, path_end + 1)
    return found


def typed_receiver_proofs(
    tokens: list[object],
    aliases: dict[str, tuple[str, ...]],
    bindings: dict[str, str],
    mapping: dict[str, str],
    context: tuple[str, ...],
    proven_paths: dict[str, str],
    trait_paths: set[str],
    excluded: set[int],
) -> tuple[dict[str, str], dict[str, str]]:
    """Derive typed variables/fields and accessor return types from explicit signatures."""

    typed_names: dict[str, str] = {}
    accessors: dict[str, str] = {}
    for index in range(len(tokens) - 2):
        if index in excluded:
            continue
        if tokens[index].kind == "IDENT" and tokens[index + 1].text == ":":
            end = index + 2
            while end < len(tokens) and tokens[end].text not in {",", ")", "=", ";", "{"}:
                end += 1
            resolved = signature_trait_paths(
                tokens, index + 2, end, aliases, bindings, mapping, context,
                proven_paths, trait_paths,
            )
            if len(resolved) == 1:
                typed_names[canonical_name(tokens[index])] = next(iter(resolved))
        if tokens[index].text != "fn" or tokens[index + 1].kind != "IDENT":
            continue
        arrow = index + 2
        while arrow + 1 < len(tokens) and tokens[arrow].text not in {"{", ";"}:
            if tokens[arrow].text == "-" and tokens[arrow + 1].text == ">":
                break
            arrow += 1
        if arrow + 1 >= len(tokens) or tokens[arrow].text != "-":
            continue
        end = arrow + 2
        while end < len(tokens) and tokens[end].text not in {"{", ";"}:
            end += 1
        resolved = signature_trait_paths(
            tokens, arrow + 2, end, aliases, bindings, mapping, context,
            proven_paths, trait_paths,
        )
        if len(resolved) == 1:
            accessors[canonical_name(tokens[index + 1])] = next(iter(resolved))
    return typed_names, accessors


def receiver_call_rows(
    path: Path,
    tokens: list[object],
    typed_names: dict[str, str],
    accessors: dict[str, str],
    trait_methods: dict[str, set[str]],
    mapping: dict[str, str],
    tasks: dict[str, str],
    excluded: set[int],
) -> list[ConsumerRow]:
    """Inventory method syntax only when an explicit receiver type proof exists."""

    rows: list[ConsumerRow] = []
    for method_index in range(1, len(tokens) - 1):
        if method_index in excluded:
            continue
        if (
            tokens[method_index].kind != "IDENT"
            or tokens[method_index - 1].text != "."
            or tokens[method_index + 1].text != "("
        ):
            continue
        trait_path: str | None = None
        receiver_index = method_index - 2
        if receiver_index >= 0 and tokens[receiver_index].kind == "IDENT":
            trait_path = typed_names.get(canonical_name(tokens[receiver_index]))
        cursor = method_index - 2
        while cursor >= 0 and tokens[cursor].text == "?":
            cursor -= 1
        if trait_path is None and cursor >= 0 and tokens[cursor].text == ")":
            depth = 1
            opening = cursor - 1
            while opening >= 0 and depth:
                depth += tokens[opening].text == ")"
                depth -= tokens[opening].text == "("
                opening -= 1
            accessor_index = opening
            if (
                depth == 0
                and accessor_index >= 0
                and tokens[accessor_index].kind == "IDENT"
            ):
                trait_path = accessors.get(canonical_name(tokens[accessor_index]))
        if trait_path is None:
            continue
        method = canonical_name(tokens[method_index])
        if method not in trait_methods.get(trait_path, set()):
            continue
        rows.append(
            ConsumerRow(
                path.as_posix(),
                "CALL",
                f"{method}@receiver={trait_path.rsplit('::', 1)[1]}@token={method_index}",
                trait_path,
                mapping[trait_path],
                tasks.get(trait_path, "PLANNED_TARGET"),
                "CURRENT_REFERENCE_TARGET_PLANNED",
            )
        )
    return rows


def implementation_rows(
    path: Path,
    tokens: list[object],
    aliases: dict[str, tuple[str, ...]],
    bindings: dict[str, str],
    mapping: dict[str, str],
    trait_paths: set[str],
    tasks: dict[str, str],
    context: tuple[str, ...],
    proven_paths: dict[str, str],
    excluded: set[int],
) -> list[AdapterRow]:
    """Enumerate real trait impl headers and bind them to planned canonical traits."""

    rows: list[AdapterRow] = []
    for impl_index, token in enumerate(tokens):
        if impl_index in excluded:
            continue
        if token.kind != "IDENT" or token.text != "impl":
            continue
        index = impl_index + 1
        if index < len(tokens) and tokens[index].text == "<":
            depth = 1
            index += 1
            while index < len(tokens) and depth:
                depth += tokens[index].text == "<"
                depth -= tokens[index].text == ">"
                index += 1
            if depth:
                fail(f"unterminated impl generic parameters in {path}")
        trait_path, trait_end = old_path_from_tokens(
            tokens, index, aliases, bindings, mapping, context, proven_paths
        )
        if trait_path is None or trait_path not in trait_paths:
            continue
        cursor = trait_end + 1
        while cursor < len(tokens) and tokens[cursor].text not in {"for", "{", ";"}:
            cursor += 1
        if cursor >= len(tokens) or tokens[cursor].text != "for":
            continue
        type_start = cursor + 1
        type_end = type_start
        angle = paren = bracket = 0
        while type_end < len(tokens):
            text = tokens[type_end].text
            if angle == paren == bracket == 0 and text in {"where", "{"}:
                break
            angle += text == "<"
            angle -= text == ">"
            paren += text == "("
            paren -= text == ")"
            bracket += text == "["
            bracket -= text == "]"
            type_end += 1
        implementation_type = "".join(token.text for token in tokens[type_start:type_end])
        if not implementation_type:
            fail(f"trait impl has no implementation type in {path}")
        owner = (
            "RDBC_CURRENT_IMPL" if path.parts[:2] == ("crates", "druid-rdbc")
            else "WRAPPER_ADAPTER" if path.parts[:2] == ("crates", "druid-wrapper")
            else "DRUID_LOGICAL_OR_PROXY"
        )
        target_task = (
            "P04_TASK_4_ADAPTER_CUTOVER" if owner == "WRAPPER_ADAPTER"
            else "P04_TASK_5_LOGICAL_CUTOVER" if owner == "DRUID_LOGICAL_OR_PROXY"
            else tasks[trait_path]
        )
        rows.append(
            AdapterRow(
                path.as_posix(),
                trait_path,
                implementation_type,
                owner,
                mapping[trait_path],
                target_task,
                "CURRENT_IMPL_TARGET_PLANNED",
            )
        )
    return rows


def scan_source_text(
    path: Path,
    source: str,
    mapping: dict[str, str],
    *,
    trait_paths: set[str] | None = None,
    tasks: dict[str, str] | None = None,
    proven_paths: dict[str, str] | None = None,
    declaration_paths: dict[str, str] | None = None,
    trait_methods: dict[str, set[str]] | None = None,
) -> tuple[list[ConsumerRow], list[AdapterRow]]:
    """Scan one Rust source without allowing literals, aliases, or use trees to hide rows."""

    tokens = rust_tokens(source)
    excluded = cfg_test_indexes(tokens)
    context = module_context(path)
    proven = proven_paths or {old_path: old_path for old_path in mapping}
    declarations = declaration_paths or {}
    current_trait_paths = trait_paths or set(mapping)
    current_tasks = tasks or {old: "PLANNED_TARGET" for old in mapping}
    aliases: dict[str, tuple[str, ...]] = {
        "druid_rdbc": ("druid_rdbc",),
        "druid": ("druid",),
    }
    bindings: dict[str, str] = {}
    by_name: dict[str, str] = {}
    for old_path in mapping:
        name = old_path.rsplit("::", 1)[1]
        if name in by_name:
            fail(f"ambiguous standardized symbol name: {name}")
        by_name[name] = old_path
    # Only the physical table's exact declaration file may bind a same-leaf local name.
    for old_path in mapping:
        if declarations.get(old_path) == path.as_posix():
            name = old_path.rsplit("::", 1)[1]
            if any(
                token.kind == "IDENT" and canonical_name(token) == name
                and declaration_name(tokens, index)
                for index, token in enumerate(tokens)
            ):
                bindings[name] = old_path

    covered: set[int] = set(excluded)
    occurrences: list[ConsumerRow] = []
    prefixes = module_prefixes(mapping)

    # Rust 2015-style crate aliases remain legal syntax and must not evade the scan.
    for index in range(len(tokens) - 4):
        if index in excluded:
            continue
        if (
            tokens[index].text == "extern" and tokens[index + 1].text == "crate"
            and tokens[index + 2].kind == "IDENT" and tokens[index + 3].text == "as"
            and tokens[index + 4].kind == "IDENT"
        ):
            root_name = canonical_name(tokens[index + 2])
            if root_name in {"druid", "druid_rdbc"}:
                aliases[canonical_name(tokens[index + 4])] = (root_name,)
                covered.update(range(index, min(index + 6, len(tokens))))

    index = 0
    while index < len(tokens):
        if (
            index in excluded
            or tokens[index].kind != "IDENT"
            or tokens[index].text != "use"
        ):
            index += 1
            continue
        start = index
        try:
            entries, index = parse_use_tree(tokens, index + 1)
        except Exception as error:
            fail(f"cannot parse use tree in {path}: {error}")
        if index >= len(tokens) or tokens[index].text != ";":
            fail(f"use item does not end in semicolon in {path}")
        index += 1
        covered.update(range(start, index))
        for entry in entries:
            expanded = expanded_path(entry.path, aliases, context)
            if expanded and expanded[-1] == "*":
                wildcard_prefix = expanded[:-1]
                if wildcard_prefix in prefixes or any(
                    tuple(old.split("::"))[: len(wildcard_prefix)] == wildcard_prefix
                    for old in mapping
                ):
                    fail(f"wildcard import can hide standardized symbols in {path}: {'::'.join(expanded)}")
                continue
            alias = entry.alias
            old_path = proven_old_path(expanded, mapping, proven)
            if old_path is None and expanded and expanded[0] not in {
                "druid", "druid_rdbc", "druid_wrapper",
            }:
                old_path = proven_old_path(context + expanded, mapping, proven)
            if old_path is not None:
                bound = alias or expanded[-1]
                bindings[bound] = old_path
                occurrences.append(
                    ConsumerRow(
                        path.as_posix(),
                        "IMPORT",
                        f"{bound}@token={start}:{len(occurrences)}",
                        old_path,
                        mapping[old_path],
                        (tasks or {}).get(old_path, "PLANNED_TARGET"),
                        "CURRENT_REFERENCE_TARGET_PLANNED",
                    )
                )
                continue
            is_module = any(
                tuple(qualified.split("::"))[: len(expanded)] == expanded
                for qualified in proven
            )
            if expanded and (alias is not None or is_module):
                aliases[alias or expanded[-1]] = expanded

    for token_index, token in enumerate(tokens):
        if token_index in covered or token.kind != "IDENT" or declaration_name(tokens, token_index):
            continue
        old_path, end = old_path_from_tokens(
            tokens, token_index, aliases, bindings, mapping, context, proven
        )
        if old_path is None:
            continue
        # Only the first identifier of a qualified path owns the occurrence.
        if token_index > 1 and tokens[token_index - 1].text == "::":
            continue
        kind = classify_occurrence(tokens, token_index)
        qualified = canonical_name(token) in {"self", "super", "crate"}
        symbol = old_path.rsplit("::", 1)[1] if qualified else canonical_name(token)
        anchor = "qualified-token" if qualified else "token"
        occurrences.append(
            ConsumerRow(
                path.as_posix(),
                kind,
                f"{symbol}@{anchor}={token_index}",
                old_path,
                mapping[old_path],
                (tasks or {}).get(old_path, "PLANNED_TARGET"),
                "CURRENT_REFERENCE_TARGET_PLANNED",
            )
        )
        covered.update(range(token_index + 1, end + 1))
    implementations = implementation_rows(
        path,
        tokens,
        aliases,
        bindings,
        mapping,
        current_trait_paths,
        current_tasks,
        context,
        proven,
        excluded,
    )
    typed_names, accessors = typed_receiver_proofs(
        tokens,
        aliases,
        bindings,
        mapping,
        context,
        proven,
        current_trait_paths,
        excluded,
    )
    occurrences.extend(
        receiver_call_rows(
            path,
            tokens,
            typed_names,
            accessors,
            trait_methods or {},
            mapping,
            current_tasks,
            excluded,
        )
    )
    return sorted(set(occurrences)), sorted(set(implementations))


def production_files(root: Path) -> list[Path]:
    """Enumerate every current production Rust file in the three declared crates."""

    files: list[Path] = []
    for source_root in PRODUCTION_ROOTS:
        directory = root / source_root
        if not directory.is_dir():
            fail(f"production source root not found: {source_root}")
        files.extend(path.relative_to(root) for path in directory.rglob("*.rs"))
    return sorted(files)


def derive_live_manifests(root: Path, legacy_physical: Path = P03_MANIFEST) -> LiveManifests:
    """Derive all three tables from the immutable P03 base and current production tokens."""

    # The argument is retained so the documented command names the artifact being refreshed.
    # The denominator itself comes from Git P03 plus current source, never from mutable TSV rows.
    _ = legacy_physical
    physical = derive_physical_rows(root)
    mapping = {row.old_path: row.target_path for row in physical}
    tasks = {row.old_path: row.target_task for row in physical}
    declaration_paths = {row.old_path: row.source_file for row in physical}
    trait_paths = {
        row.old_path for row in physical if row.normalized_signature.startswith("trait ")
    }
    proven_paths = build_proven_reexports(root, mapping, declaration_paths)
    trait_methods = derive_trait_methods(root, physical)
    consumers: list[ConsumerRow] = []
    adapters: list[AdapterRow] = []
    for path in production_files(root):
        source = (root / path).read_text(encoding="utf-8")
        file_consumers, file_adapters = scan_source_text(
            path,
            source,
            mapping,
            trait_paths=trait_paths,
            tasks=tasks,
            proven_paths=proven_paths,
            declaration_paths=declaration_paths,
            trait_methods=trait_methods,
        )
        consumers.extend(file_consumers)
        adapters.extend(file_adapters)
    if not consumers:
        fail("production scan produced no standardized consumer occurrences")
    if not adapters:
        fail("production scan produced no current standard trait implementations")
    return LiveManifests(
        physical=physical,
        adapters=tuple(sorted(set(adapters))),
        consumers=tuple(sorted(set(consumers))),
    )


def render_rows(header: str, rows: tuple[object, ...]) -> bytes:
    """Render dataclass rows in field order with stable sorting and final newline."""

    lines = [header]
    for row_number, row in enumerate(rows, start=1):
        values = [str(value) for value in row.__dict__.values()]
        for field_number, value in enumerate(values, start=1):
            validate_tsv_field(value, f"render row {row_number} field {field_number}")
        lines.append("\t".join(values))
    return ("\n".join(lines) + "\n").encode("utf-8")


def render_physical_rows(rows: tuple[PhysicalRow, ...]) -> bytes:
    """Render the physical disposition manifest."""

    return render_rows(PHYSICAL_HEADER, rows)


def render_adapter_rows(rows: tuple[AdapterRow, ...]) -> bytes:
    """Render the current implementation denominator."""

    return render_rows(ADAPTER_HEADER, rows)


def render_consumer_rows(rows: tuple[ConsumerRow, ...]) -> bytes:
    """Render every production occurrence in deterministic token order."""

    return render_rows(CONSUMER_HEADER, rows)


def require_checked_payload(
    path: Path,
    expected: bytes,
    expected_rows: tuple[object, ...],
    parser,
    label: str,
) -> None:
    """Reject a missing, reordered, duplicate, extra, or otherwise changed manifest."""

    if not path.is_file():
        fail(f"{label} manifest not found: {path}")
    payload = path.read_bytes()
    parsed = tuple(parser(payload))
    if parsed != expected_rows:
        fail(f"{label} manifest rows do not match the live denominator")
    if payload != expected:
        fail(
            f"{label} manifest does not match the live denominator: "
            f"checked={hashlib.sha256(payload).hexdigest()} "
            f"live={hashlib.sha256(expected).hexdigest()}"
        )


def verify_denominator(
    root: Path,
    physical_path: Path,
    adapter_path: Path,
    consumer_path: Path,
    *,
    live: LiveManifests | None = None,
) -> LiveManifests:
    """Compare all checked-in artifacts byte-for-byte with a fresh live derivation."""

    live = live or derive_live_manifests(root, physical_path)
    physical = physical_path if physical_path.is_absolute() else root / physical_path
    adapters = adapter_path if adapter_path.is_absolute() else root / adapter_path
    consumers = consumer_path if consumer_path.is_absolute() else root / consumer_path
    require_checked_payload(
        physical,
        render_physical_rows(live.physical),
        live.physical,
        lambda payload: parse_physical_payload(payload, allow_legacy=False),
        "physical",
    )
    require_checked_payload(
        adapters,
        render_adapter_rows(live.adapters),
        live.adapters,
        parse_adapter_payload,
        "adapter",
    )
    require_checked_payload(
        consumers,
        render_consumer_rows(live.consumers),
        live.consumers,
        parse_consumer_payload,
        "consumer",
    )
    return live


def emit_live(
    root: Path,
    physical_path: Path,
    adapter_path: Path,
    consumer_path: Path,
) -> LiveManifests:
    """Write the complete source-derived manifests after all classification succeeds."""

    live = derive_live_manifests(root, physical_path)
    outputs = (
        (physical_path, render_physical_rows(live.physical)),
        (adapter_path, render_adapter_rows(live.adapters)),
        (consumer_path, render_consumer_rows(live.consumers)),
    )
    for destination, payload in outputs:
        resolved = destination if destination.is_absolute() else root / destination
        resolved.parent.mkdir(parents=True, exist_ok=True)
        resolved.write_bytes(payload)
    return live


def print_summary(live: LiveManifests) -> None:
    """Print row counts and byte hashes for the task report."""

    payloads = {
        "PHYSICAL": render_physical_rows(live.physical),
        "ADAPTER": render_adapter_rows(live.adapters),
        "CONSUMER": render_consumer_rows(live.consumers),
    }
    for name, rows in (
        ("PHYSICAL", live.physical),
        ("ADAPTER", live.adapters),
        ("CONSUMER", live.consumers),
    ):
        print(f"{name}_ROWS={len(rows)}")
        print(f"{name}_SHA256={hashlib.sha256(payloads[name]).hexdigest()}")


def parse_arguments() -> argparse.Namespace:
    """Parse the documented emit-live and verification commands."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--emit-live", action="store_true")
    parser.add_argument("physical", nargs="?", type=Path, default=P03_MANIFEST)
    parser.add_argument("adapters", nargs="?", type=Path, default=DEFAULT_ADAPTERS)
    parser.add_argument("consumers", nargs="?", type=Path, default=DEFAULT_CONSUMERS)
    return parser.parse_args()


def main() -> int:
    """Emit or verify the Task 2 denominator."""

    arguments = parse_arguments()
    root = repository_root()
    try:
        if arguments.emit_live:
            live = emit_live(root, arguments.physical, arguments.adapters, arguments.consumers)
        else:
            live = verify_denominator(root, arguments.physical, arguments.adapters, arguments.consumers)
    except VerificationError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print_summary(live)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
