#!/usr/bin/env python3
"""Freeze and verify the live single-standard RDBC migration denominator."""

from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import re
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
TASK4_PRE_CUTOVER_COMMIT = "8252a20f882fd3ade9a99d202c0e697a35307e45"
TASK4_CLOSED_HISTORY_SHA256 = (
    "bcc50e12e484a78bff19e578e362f25e4a382259be1ec53aadef53f7f01eca14"
)
TASK5_PRE_CUTOVER_COMMIT = "d1ccc8e3005a3a71638766ee701af99594fc43ed"
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
    "target_task\tstandard_implementation_type\tstandard_source_path\t"
    "standard_core_evidence\tbehavior_evidence\tstate"
)
CONSUMER_HEADER = (
    "source_path\toccurrence_kind\tsymbol\told_path\ttarget_path\t" "target_task\tstate"
)
CLASSIFICATIONS = {
    "MERGE_STANDARD",
    "RDBC_EXTENSION",
    "DRUID_INTERNAL",
    "CONCRETE_IMPL",
}
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
    standard_implementation_type: str
    standard_source_path: str
    standard_core_evidence: str
    behavior_evidence: str
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


@dataclass(frozen=True)
class ReceiverBinding:
    """One lexical receiver declaration, including unrelated shadow declarations."""

    name: str
    trait_path: str | None
    declaration_index: int
    scope_start: int
    scope_end: int
    identity: str


@dataclass(frozen=True)
class ImplScope:
    """One impl body and the concrete/inherent self type that owns it."""

    type_name: str
    base_name: str
    start: int
    end: int


@dataclass(frozen=True)
class FunctionScope:
    """One function/method body with parameter and owning impl boundaries."""

    name: str
    params_start: int
    params_end: int
    body_start: int
    body_end: int
    impl_type: str | None


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
        (
            old_path,
            classification,
            target_path,
            compatibility,
            source_file,
            signature,
            last,
        ) = fields
        if old_path in seen:
            fail(f"physical row {line_number} duplicates {old_path}")
        seen.add(old_path)
        if classification not in CLASSIFICATIONS:
            fail(
                f"physical row {line_number} has unknown classification {classification}"
            )
        target_task = (
            target_task_for(old_path, classification, target_path) if legacy else last
        )
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
        (byte < 0x20 and byte not in {0x09, 0x0A}) or byte == 0x7F for byte in payload
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
        if row.state == "STANDARD_IMPL_LIVE":
            if (
                row.target_task != "P04_TASK_4_ADAPTER_CUTOVER"
                or row.owner != "WRAPPER_ADAPTER"
            ):
                fail("adapter LIVE state is only valid for Task 4 wrapper rows")
            if any(
                value == "-"
                for value in (
                    row.standard_implementation_type,
                    row.standard_source_path,
                    row.standard_core_evidence,
                    row.behavior_evidence,
                )
            ):
                fail(
                    "adapter LIVE row lacks canonical implementation/core/behavior evidence"
                )
        elif row.state != "CURRENT_IMPL_TARGET_PLANNED":
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
        if row.state == "STANDARD_REFERENCE_LIVE":
            if (
                row.target_task != "P04_TASK_4_ADAPTER_CUTOVER"
                or not row.source_path.startswith("crates/druid-wrapper/")
            ):
                fail("consumer LIVE state is only valid for Task 4 wrapper rows")
        elif row.state == "STANDARD_REFERENCE_CLOSED":
            if (
                row.target_task != "P04_TASK_4_ADAPTER_CUTOVER"
                or not row.source_path.startswith("crates/druid-wrapper/")
            ):
                fail("consumer CLOSED state is only valid for Task 4 wrapper rows")
        elif row.state in {
            "DRUID_INTERNAL_REFERENCE_CLOSED",
            "TASK6_COMPATIBILITY_PENDING",
            "TASK7_RESOURCE_OPERATIONS_PENDING",
        }:
            if row.target_task != "P04_TASK_5_DRUID_INTERNALIZATION":
                fail("Druid Task 5 disposition is only valid for Task 5 rows")
        elif row.state != "CURRENT_REFERENCE_TARGET_PLANNED":
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


CfgExpr = tuple[object, ...]


def parse_cfg_expression(
    tokens: list[object],
    start: int,
    end: int,
) -> tuple[CfgExpr, int]:
    """Parse one cfg predicate into a small boolean AST."""

    if start >= end or tokens[start].kind != "IDENT":
        return ("atom", f"unknown@{start}"), min(start + 1, end)
    name = canonical_name(tokens[start])
    cursor = start + 1
    if cursor < end and tokens[cursor].text == "=":
        value = tokens[cursor + 1].text if cursor + 1 < end else "<missing>"
        return ("atom", f"{name}={value}"), min(cursor + 2, end)
    if cursor >= end or tokens[cursor].text != "(":
        return (("test",) if name == "test" else ("atom", name)), cursor
    cursor += 1
    children: list[CfgExpr] = []
    while cursor < end and tokens[cursor].text != ")":
        child, cursor = parse_cfg_expression(tokens, cursor, end)
        children.append(child)
        if cursor < end and tokens[cursor].text == ",":
            cursor += 1
    if cursor < end and tokens[cursor].text == ")":
        cursor += 1
    if name in {"all", "any"}:
        return (name, tuple(children)), cursor
    if name == "not" and len(children) == 1:
        return ("not", children[0]), cursor
    return ("atom", f"unknown-call:{name}@{start}"), cursor


def cfg_atoms(expression: CfgExpr) -> set[str]:
    """Collect non-test atoms used by one cfg expression."""

    kind = expression[0]
    if kind == "atom":
        return {str(expression[1])}
    if kind == "test":
        return set()
    if kind == "not":
        return cfg_atoms(expression[1])
    return set().union(*(cfg_atoms(child) for child in expression[1]))


def evaluate_cfg(
    expression: CfgExpr,
    assignment: dict[str, bool],
    *,
    test_value: bool = False,
) -> bool | None:
    """Evaluate a cfg AST with an explicit test profile and partial atom assignments."""

    kind = expression[0]
    if kind == "test":
        return test_value
    if kind == "atom":
        return assignment.get(str(expression[1]))
    if kind == "not":
        value = evaluate_cfg(expression[1], assignment, test_value=test_value)
        return None if value is None else not value
    values = [
        evaluate_cfg(child, assignment, test_value=test_value)
        for child in expression[1]
    ]
    if kind == "all":
        if any(value is False for value in values):
            return False
        return True if all(value is True for value in values) else None
    if any(value is True for value in values):
        return True
    return False if all(value is False for value in values) else None


def cfg_satisfiable_without_test(expression: CfgExpr) -> bool:
    """Use a small DPLL search to decide whether an item can exist with test=false."""

    atoms = sorted(cfg_atoms(expression))

    def search(assignment: dict[str, bool]) -> bool:
        value = evaluate_cfg(expression, assignment, test_value=False)
        if value is not None:
            return value
        atom = next(atom for atom in atoms if atom not in assignment)
        return search({**assignment, atom: False}) or search({**assignment, atom: True})

    return search({})


def cfg_profile_excluded_indexes(
    tokens: list[object],
    assignment: dict[str, bool],
    *,
    test_value: bool,
) -> set[int]:
    """Return cfg-disabled node ranges for one fully proven Cargo/rustc profile."""

    excluded: set[int] = set()
    index = 0
    while index + 1 < len(tokens):
        if tokens[index].text != "#" or tokens[index + 1].text != "[":
            index += 1
            continue
        group_start = index
        constraints: list[CfgExpr] = []
        while (
            index + 1 < len(tokens)
            and tokens[index].text == "#"
            and tokens[index + 1].text == "["
        ):
            depth = 1
            bracket_end = index + 2
            while bracket_end < len(tokens) and depth:
                depth += tokens[bracket_end].text == "["
                depth -= tokens[bracket_end].text == "]"
                bracket_end += 1
            if depth:
                fail("outer attribute is not closed")
            constraints.extend(parse_cfg_attribute(tokens, index, bracket_end - 1))
            index = bracket_end
        attributes_end = index
        node_end = next_rust_node_end(tokens, attributes_end)
        excluded_node = False
        if constraints:
            conjunction: CfgExpr = ("all", tuple(constraints))
            enabled = evaluate_cfg(
                conjunction,
                assignment,
                test_value=test_value,
            )
            if enabled is None:
                unresolved = sorted(
                    atom for atom in cfg_atoms(conjunction) if atom not in assignment
                )
                fail(f"cfg profile cannot prove atoms: {unresolved}")
            if not enabled:
                excluded.update(range(group_start, node_end))
                excluded_node = True
        index = node_end if excluded_node else attributes_end
    return excluded


def parse_cfg_attribute(
    tokens: list[object],
    start: int,
    bracket_end: int,
) -> list[CfgExpr]:
    """Return effective constraints contributed by cfg/cfg_attr."""

    return parse_cfg_meta(tokens, start + 2, bracket_end)


def parse_cfg_meta(
    tokens: list[object],
    start: int,
    end: int,
) -> list[CfgExpr]:
    """Recursively convert one cfg/cfg_attr meta item into effective constraints."""

    if start + 2 >= end or tokens[start].kind != "IDENT":
        return []
    name = canonical_name(tokens[start])
    if tokens[start + 1].text != "(" or tokens[end - 1].text != ")":
        return []
    if name == "cfg":
        expression, _ = parse_cfg_expression(tokens, start + 2, end - 1)
        return [expression]
    if name != "cfg_attr":
        return []
    arguments = split_top_level_ranges(tokens, start + 2, end - 1)
    if not arguments:
        return []
    condition, _ = parse_cfg_expression(tokens, *arguments[0])
    constraints: list[CfgExpr] = []
    for nested_start, nested_end in arguments[1:]:
        for nested in parse_cfg_meta(tokens, nested_start, nested_end):
            constraints.append(("any", (("not", condition), nested)))
    return constraints


def generic_angle_closing(tokens: list[object], start: int) -> int | None:
    """Return a plausible generic ``>``; comparisons are not generic delimiters."""

    depth = 1
    paren = bracket = brace = 0
    cursor = start + 1
    while cursor < len(tokens):
        text = tokens[cursor].text
        if text == "(":
            paren += 1
        elif text == ")":
            if paren == 0:
                return None
            paren -= 1
        elif text == "[":
            bracket += 1
        elif text == "]":
            if bracket == 0:
                return None
            bracket -= 1
        elif text == "{":
            brace += 1
        elif text == "}":
            if brace == 0:
                return None
            brace -= 1
        elif paren == bracket == brace == 0 and text == "<":
            depth += 1
        elif paren == bracket == brace == 0 and text == ">":
            depth -= 1
            if depth == 0:
                follower = (
                    tokens[cursor + 1].text if cursor + 1 < len(tokens) else "<eof>"
                )
                if follower in {
                    "::",
                    "(",
                    ")",
                    "[",
                    "]",
                    "{",
                    "}",
                    ",",
                    ";",
                    "+",
                    ":",
                    "=",
                    "where",
                    "for",
                    "as",
                    "<eof>",
                }:
                    return cursor
                return None
        elif paren == bracket == brace == 0 and text == ";":
            return None
        cursor += 1
    return None


def next_rust_node_end(tokens: list[object], start: int) -> int:
    """Return the exact comma/semicolon/body boundary of the next Rust node."""

    paren = bracket = angle = 0
    cursor = start
    while cursor < len(tokens):
        text = tokens[cursor].text
        if text == "(":
            paren += 1
        elif text == ")":
            paren = max(0, paren - 1)
        elif text == "[":
            bracket += 1
        elif text == "]":
            bracket = max(0, bracket - 1)
        elif text == "<" and paren == bracket == 0:
            if angle > 0 or generic_angle_closing(tokens, cursor) is not None:
                angle += 1
        elif text == ">" and angle > 0:
            angle -= 1
        elif paren == bracket == angle == 0 and text in {",", ";"}:
            return cursor + 1
        elif paren == bracket == angle == 0 and text == "{":
            depth = 1
            cursor += 1
            while cursor < len(tokens) and depth:
                depth += tokens[cursor].text == "{"
                depth -= tokens[cursor].text == "}"
                cursor += 1
            if depth:
                fail("attribute target has an unterminated body")
            if cursor < len(tokens) and tokens[cursor].text in {",", ";"}:
                cursor += 1
            return cursor
        cursor += 1
    fail("attribute has no following Rust node boundary")


def production_excluded_indexes(tokens: list[object]) -> set[int]:
    """Return exact next-node ranges unsatisfiable when ``test=false``."""

    excluded: set[int] = set()
    index = 0
    while index + 1 < len(tokens):
        if tokens[index].text != "#" or tokens[index + 1].text != "[":
            index += 1
            continue
        group_start = index
        constraints: list[CfgExpr] = []
        while (
            index + 1 < len(tokens)
            and tokens[index].text == "#"
            and tokens[index + 1].text == "["
        ):
            depth = 1
            bracket_end = index + 2
            while bracket_end < len(tokens) and depth:
                depth += tokens[bracket_end].text == "["
                depth -= tokens[bracket_end].text == "]"
                bracket_end += 1
            if depth:
                fail("outer attribute is not closed")
            constraints.extend(parse_cfg_attribute(tokens, index, bracket_end - 1))
            index = bracket_end
        attributes_end = index
        node_end = next_rust_node_end(tokens, attributes_end)
        excluded_node = False
        if constraints:
            conjunction: CfgExpr = ("all", tuple(constraints))
            if not cfg_satisfiable_without_test(conjunction):
                excluded.update(range(group_start, node_end))
                excluded_node = True
        # A live attributed parent may contain nested cfg fields/variants/statements.
        index = node_end if excluded_node else attributes_end
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
            excluded = production_excluded_indexes(tokens)
            for index in range(len(tokens) - 2):
                if index in excluded:
                    continue
                if tokens[index].kind != "IDENT" or tokens[index].text != "pub":
                    continue
                kind_token = tokens[index + 1]
                name_token = tokens[index + 2]
                if kind_token.kind != "IDENT" or kind_token.text not in {
                    "trait",
                    "struct",
                    "enum",
                    "type",
                }:
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


def classify_new_resource(
    old_path: str, source_file: str, signature: str
) -> PhysicalRow:
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
    elif kind == "enum" and (
        name.endswith("Length") or name.endswith("RepresentationType")
    ):
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


def derive_physical_rows(
    root: Path,
    *,
    allow_task5_internalized: bool = False,
) -> tuple[PhysicalRow, ...]:
    """Union the immutable P03 subset with every current live public declaration."""

    baseline = p03_baseline_rows(root)
    current = scan_public_items(root)
    missing_baseline = sorted(set(baseline) - set(current))
    allowed_missing = {
        old_path
        for old_path, row in baseline.items()
        if row.classification == "DRUID_INTERNAL"
        and row.target_task == "P04_TASK_5_DRUID_INTERNALIZATION"
    }
    if missing_baseline and not (
        allow_task5_internalized and set(missing_baseline) <= allowed_missing
    ):
        fail(f"P03 physical denominator disappeared from source: {missing_baseline}")
    rows: list[PhysicalRow] = []
    if allow_task5_internalized:
        rows.extend(baseline[old_path] for old_path in missing_baseline)
    task7_replacements = {
        "druid_rdbc::resource::ResourceOperations",
        "druid_rdbc::resource::ResourceHandleFactory",
    }
    for old_path, (source_file, signature) in current.items():
        if allow_task5_internalized and old_path in task7_replacements:
            continue
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
                fail(
                    f"new unreviewed spi item is outside the P03 denominator: {old_path}"
                )
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
    relative = list(parts[source_index + 1 :])
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
    for qualified, old_path in {
        **{old: old for old in mapping},
        **proven_paths,
    }.items():
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

    return index > 0 and tokens[index - 1].text in {
        "trait",
        "struct",
        "enum",
        "type",
        "mod",
    }


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
    while (
        cursor + 2 < len(tokens)
        and tokens[cursor + 1].text == "::"
        and tokens[cursor + 2].kind == "IDENT"
    ):
        parts += (canonical_name(tokens[cursor + 2]),)
        cursor += 2
    parts = expanded_path(parts, aliases, context)
    old_path = proven_old_path(parts, mapping, proven_paths)
    if (
        old_path is None
        and parts
        and parts[0] not in {"druid", "druid_rdbc", "druid_wrapper"}
    ):
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
            excluded = production_excluded_indexes(tokens)
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
                        if public and any(
                            key == prefix or key.startswith(prefix + "::")
                            for key in proven
                        ):
                            fail(
                                f"wildcard re-export can hide standardized symbols in {path}"
                            )
                        continue
                    old_path = proven_old_path(expanded, mapping, proven)
                    if (
                        old_path is None
                        and expanded
                        and expanded[0]
                        not in {
                            "druid",
                            "druid_rdbc",
                            "druid_wrapper",
                        }
                    ):
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
        source = root / row.source_file
        if (
            not source.is_file()
            and row.target_task == "P04_TASK_5_DRUID_INTERNALIZATION"
        ):
            methods[row.old_path] = set()
            continue
        name = row.old_path.rsplit("::", 1)[1]
        tokens = rust_tokens(source.read_text(encoding="utf-8"))
        excluded = production_excluded_indexes(tokens)
        trait_index = next(
            (
                index
                for index in range(len(tokens) - 1)
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
                and cursor not in excluded
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


def delimiter_pairs(tokens: list[object], opening: str, closing: str) -> dict[int, int]:
    """Return exact opening-to-closing delimiter indexes."""

    stack: list[int] = []
    pairs: dict[int, int] = {}
    for index, token in enumerate(tokens):
        if token.text == opening:
            stack.append(index)
        elif token.text == closing:
            if not stack:
                fail(f"unmatched {closing} in Rust source")
            pairs[stack.pop()] = index
    if stack:
        fail(f"unmatched {opening} in Rust source")
    return pairs


def first_identifier(tokens: list[object], start: int, end: int) -> str | None:
    """Return the first non-keyword identifier in a type/pattern slice."""

    ignored = {"mut", "ref", "dyn", "where", "for", "impl", "pub", "crate", "self"}
    for token in tokens[start:end]:
        if token.kind == "IDENT" and canonical_name(token) not in ignored:
            return canonical_name(token)
    return None


def generic_parameter_map(
    tokens: list[object],
    start: int,
    end: int,
) -> dict[str, str]:
    """Normalize declared type/lifetime/const parameter names by position."""

    mapping: dict[str, str] = {}
    if start >= end or tokens[start].text != "<":
        return mapping
    depth = 0
    slot = 0
    cursor = start + 1
    segment_start = cursor
    while cursor < end:
        text = tokens[cursor].text
        depth += text in {"<", "(", "["}
        depth -= text in {">", ")", "]"}
        if (text == "," and depth == 0) or (text == ">" and depth < 0):
            segment = tokens[segment_start:cursor]
            candidate = next(
                (
                    token.text
                    for token in segment
                    if token.kind in {"IDENT", "LIFETIME"}
                    and token.text not in {"const", "where"}
                ),
                None,
            )
            if candidate is not None:
                mapping[candidate] = f"_G{slot}"
                slot += 1
            segment_start = cursor + 1
        if text == ">" and depth < 0:
            break
        cursor += 1
    return mapping


def normalize_type_identity(
    tokens: list[object],
    start: int,
    end: int,
    generics: dict[str, str],
    context: tuple[str, ...],
    imports: dict[str, tuple[str, ...]],
    local_types: set[str],
    local_modules: set[str],
) -> tuple[str, str] | None:
    """Normalize a self type to an absolute or explicitly unresolved identity."""

    identity_start = start
    if identity_start >= end:
        return None

    base_search_start = identity_start
    base_search_end = end
    if tokens[identity_start].text == "<":
        depth = 1
        associated_end = identity_start + 1
        while associated_end < end and depth:
            depth += tokens[associated_end].text == "<"
            depth -= tokens[associated_end].text == ">"
            associated_end += 1
        if depth:
            return None
        if associated_end < end:
            base_search_start = associated_end
    else:
        angle_start = next(
            (
                index
                for index in range(identity_start, end)
                if tokens[index].text == "<"
            ),
            None,
        )
        if angle_start is not None:
            base_search_end = angle_start
    base_index = next(
        (
            index
            for index in range(base_search_end - 1, base_search_start - 1, -1)
            if tokens[index].kind == "IDENT"
            and canonical_name(tokens[index])
            not in {"mut", "dyn", "crate", "self", "super"}
        ),
        None,
    )
    if base_index is None:
        return None
    base_name = canonical_name(tokens[base_index])
    pieces: list[str] = []
    cursor = identity_start
    while cursor < end:
        token = tokens[cursor]
        path_start = (
            token.kind == "IDENT"
            and cursor + 1 < end
            and tokens[cursor + 1].text == "::"
            and (cursor == identity_start or tokens[cursor - 1].text != "::")
        )
        leading_bare_type = (
            cursor == identity_start
            and token.kind == "IDENT"
            and canonical_name(token) not in generics
            and canonical_name(token) not in {"dyn", "impl", "mut"}
        )
        if path_start or leading_bare_type:
            path = [canonical_name(token)]
            path_end = cursor
            while (
                path_end + 2 < end
                and tokens[path_end + 1].text == "::"
                and tokens[path_end + 2].kind == "IDENT"
            ):
                path.append(canonical_name(tokens[path_end + 2]))
                path_end += 2
            raw_path = tuple(path)
            if raw_path[0] in {"crate", "self", "super"}:
                expanded = expanded_path(raw_path, {}, context)
            elif raw_path[0] in imports:
                expanded = imports[raw_path[0]] + raw_path[1:]
            elif raw_path[0] in local_types and len(raw_path) == 1:
                expanded = context + raw_path
            elif raw_path[0] in local_modules:
                expanded = context + raw_path
            elif raw_path[0] in {"druid", "druid_rdbc", "druid_wrapper"}:
                expanded = raw_path
            else:
                # Rust's extern prelude makes a bare path ambiguous without source
                # proof. Keep it in a disjoint namespace so it cannot impersonate
                # a repository-relative `crate::...` type.
                expanded = ("__unresolved__", *raw_path)
            pieces.append("::".join(expanded))
            cursor = path_end + 1
            continue
        text = token.text
        if token.kind == "IDENT":
            text = canonical_name(token)
        text = generics.get(text, text)
        pieces.append(text)
        cursor += 1
    return "".join(pieces), base_name


def derive_impl_scopes(
    tokens: list[object],
    brace_pairs: dict[int, int],
    excluded: set[int],
    context: tuple[str, ...],
    imports: dict[str, tuple[str, ...]],
) -> list[ImplScope]:
    """Derive impl ownership so same-named fields/accessors cannot cross impls."""

    brace_depth = 0
    local_types: set[str] = set()
    local_modules: set[str] = set()
    for index in range(len(tokens) - 1):
        if tokens[index].text == "}":
            brace_depth = max(0, brace_depth - 1)
        if (
            brace_depth == 0
            and index not in excluded
            and tokens[index + 1].kind == "IDENT"
        ):
            name = canonical_name(tokens[index + 1])
            if tokens[index].text in {"struct", "enum", "union", "type"}:
                local_types.add(name)
            elif tokens[index].text == "mod":
                local_modules.add(name)
        if tokens[index].text == "{":
            brace_depth += 1

    scopes: list[ImplScope] = []
    for impl_index, token in enumerate(tokens):
        if impl_index in excluded or token.text != "impl":
            continue
        opening = impl_index + 1
        while opening < len(tokens) and tokens[opening].text not in {"{", ";"}:
            opening += 1
        if opening >= len(tokens) or tokens[opening].text != "{":
            continue
        cursor = impl_index + 1
        generics: dict[str, str] = {}
        if cursor < opening and tokens[cursor].text == "<":
            depth = 1
            generic_end = cursor + 1
            while generic_end < opening and depth:
                depth += tokens[generic_end].text == "<"
                depth -= tokens[generic_end].text == ">"
                generic_end += 1
            if depth:
                fail("impl generic declaration is not closed")
            generics = generic_parameter_map(tokens, cursor, generic_end)
            cursor = generic_end
        depth = 0
        for_index: int | None = None
        scan = cursor
        while scan < opening:
            depth += tokens[scan].text in {"<", "(", "["}
            depth -= tokens[scan].text in {">", ")", "]"}
            if depth == 0 and tokens[scan].text == "for":
                for_index = scan
                break
            scan += 1
        type_start = for_index + 1 if for_index is not None else cursor
        type_end = type_start
        depth = 0
        while type_end < opening:
            text = tokens[type_end].text
            if depth == 0 and text == "where":
                break
            depth += text in {"<", "(", "["}
            depth -= text in {">", ")", "]"}
            type_end += 1
        identity = normalize_type_identity(
            tokens,
            type_start,
            type_end,
            generics,
            context,
            imports,
            local_types,
            local_modules,
        )
        if identity is None:
            continue
        type_name, base_name = identity
        scopes.append(
            ImplScope(type_name, base_name, opening + 1, brace_pairs[opening])
        )
    return scopes


def enclosing_impl(index: int, impl_scopes: list[ImplScope]) -> ImplScope | None:
    """Return the narrowest impl containing one token index."""

    candidates = [scope for scope in impl_scopes if scope.start <= index < scope.end]
    return (
        min(candidates, key=lambda scope: scope.end - scope.start)
        if candidates
        else None
    )


def derive_function_scopes(
    tokens: list[object],
    paren_pairs: dict[int, int],
    brace_pairs: dict[int, int],
    impl_scopes: list[ImplScope],
    excluded: set[int],
) -> list[FunctionScope]:
    """Derive each function body and its owning impl/method identity."""

    scopes: list[FunctionScope] = []
    for fn_index in range(len(tokens) - 1):
        if (
            fn_index in excluded
            or tokens[fn_index].text != "fn"
            or tokens[fn_index + 1].kind != "IDENT"
        ):
            continue
        params_start = fn_index + 2
        while params_start < len(tokens) and tokens[params_start].text != "(":
            params_start += 1
        if params_start not in paren_pairs:
            continue
        params_end = paren_pairs[params_start]
        body_start = params_end + 1
        while body_start < len(tokens) and tokens[body_start].text not in {"{", ";"}:
            body_start += 1
        if body_start not in brace_pairs:
            continue
        owner = enclosing_impl(fn_index, impl_scopes)
        scopes.append(
            FunctionScope(
                canonical_name(tokens[fn_index + 1]),
                params_start + 1,
                params_end,
                body_start + 1,
                brace_pairs[body_start],
                owner.type_name if owner else None,
            )
        )
    return scopes


def innermost_brace_scope(
    index: int,
    brace_pairs: dict[int, int],
    fallback_end: int,
) -> tuple[int, int]:
    """Return the smallest lexical block containing one declaration."""

    candidates = [
        (opening + 1, closing)
        for opening, closing in brace_pairs.items()
        if opening < index < closing
    ]
    return (
        min(candidates, key=lambda pair: pair[1] - pair[0])
        if candidates
        else (index, fallback_end)
    )


def unique_trait_or_none(paths: set[str]) -> str | None:
    """Return a unique trait proof; ambiguity deliberately yields no proof."""

    return next(iter(paths)) if len(paths) == 1 else None


def split_top_level_ranges(
    tokens: list[object],
    start: int,
    end: int,
) -> list[tuple[int, int]]:
    """Split a comma-separated token slice without crossing nested delimiters."""

    ranges: list[tuple[int, int]] = []
    segment_start = start
    paren = bracket = brace = angle = 0
    for index in range(start, end):
        text = tokens[index].text
        if text == "(":
            paren += 1
        elif text == ")":
            paren -= 1
        elif text == "[":
            bracket += 1
        elif text == "]":
            bracket -= 1
        elif text == "{":
            brace += 1
        elif text == "}":
            brace -= 1
        elif text == "<":
            angle += 1
        elif text == ">":
            angle -= 1
        elif text == "," and paren == bracket == brace == angle == 0:
            ranges.append((segment_start, index))
            segment_start = index + 1
    if segment_start < end:
        ranges.append((segment_start, end))
    return [(left, right) for left, right in ranges if left < right]


def top_level_token_index(
    tokens: list[object],
    start: int,
    end: int,
    target: str,
) -> int | None:
    """Find one token outside nested (), [], {}, and <> delimiters."""

    paren = bracket = brace = angle = 0
    for index in range(start, end):
        text = tokens[index].text
        if text == "(":
            paren += 1
        elif text == ")":
            paren -= 1
        elif text == "[":
            bracket += 1
        elif text == "]":
            bracket -= 1
        elif text == "{":
            brace += 1
        elif text == "}":
            brace -= 1
        elif text == "<":
            angle += 1
        elif text == ">":
            angle -= 1
        elif text == target and paren == bracket == brace == angle == 0:
            return index
    return None


def expression_scope_end(tokens: list[object], start: int, end: int) -> int:
    """Find a comma/semicolon or parent close outside nested expression delimiters."""

    paren = bracket = brace = angle = 0
    for index in range(start, end):
        text = tokens[index].text
        if text == "(":
            paren += 1
        elif text == ")":
            if paren == 0:
                return index
            paren -= 1
        elif text == "[":
            bracket += 1
        elif text == "]":
            if bracket == 0:
                return index
            bracket -= 1
        elif text == "{":
            brace += 1
        elif text == "}":
            if brace == 0:
                return index
            brace -= 1
        elif text == "<" and paren == bracket == brace == 0:
            if angle > 0 or generic_angle_closing(tokens, index) is not None:
                angle += 1
        elif text == ">" and paren == bracket == brace == 0 and angle > 0:
            angle -= 1
        elif text in {",", ";"} and paren == bracket == brace == angle == 0:
            return index
    return end


def match_arm_arrow(tokens: list[object], start: int, end: int) -> int | None:
    """Find a match arm arrow without confusing delimiters inside its guard."""

    paren = bracket = brace = 0
    index = start
    while index + 1 < end:
        text = tokens[index].text
        if paren == bracket == brace == 0:
            if text == "=" and tokens[index + 1].text == ">":
                return index
            if text in {",", "}"}:
                return None
        if text == "(":
            paren += 1
        elif text == ")":
            paren = max(0, paren - 1)
        elif text == "[":
            bracket += 1
        elif text == "]":
            bracket = max(0, bracket - 1)
        elif text == "{":
            brace += 1
        elif text == "}":
            brace = max(0, brace - 1)
        index += 1
    return None


def outer_pair(
    tokens: list[object],
    start: int,
    end: int,
    pairs: dict[int, int],
    opening: str,
) -> tuple[int, int] | None:
    """Strip an outer delimiter only when it encloses the complete slice."""

    if start < end and tokens[start].text == opening and pairs.get(start) == end - 1:
        return start + 1, end - 1
    return None


def pattern_identifiers(
    tokens: list[object], start: int, end: int
) -> list[tuple[int, str]]:
    """Return binding identifiers from a simple/tuple pattern, excluding paths and keywords."""

    ignored = {"mut", "ref", "self", "Self", "crate", "super", "_"}
    result: list[tuple[int, str]] = []
    for index in range(start, end):
        if tokens[index].kind != "IDENT":
            continue
        name = canonical_name(tokens[index])
        if name in ignored:
            continue
        if index + 1 < end and tokens[index + 1].text == "::":
            continue
        if index > start and tokens[index - 1].text == "::":
            continue
        result.append((index, name))
    return result


def typed_pattern_bindings(
    tokens: list[object],
    pattern_start: int,
    pattern_end: int,
    type_start: int,
    type_end: int,
    paren_pairs: dict[int, int],
    bracket_pairs: dict[int, int],
    aliases: dict[str, tuple[str, ...]],
    imports: dict[str, str],
    mapping: dict[str, str],
    context: tuple[str, ...],
    proven_paths: dict[str, str],
    trait_paths: set[str],
) -> list[tuple[int, str, str | None]]:
    """Map simple or tuple-destructured pattern leaves to their matching type leaves."""

    pattern_outer = outer_pair(tokens, pattern_start, pattern_end, paren_pairs, "(")
    type_outer = outer_pair(tokens, type_start, type_end, paren_pairs, "(")
    if pattern_outer and type_outer:
        patterns = split_top_level_ranges(tokens, *pattern_outer)
        types = split_top_level_ranges(tokens, *type_outer)
        if len(patterns) == len(types):
            result: list[tuple[int, str, str | None]] = []
            for pattern_range, type_range in zip(patterns, types, strict=True):
                result.extend(
                    typed_pattern_bindings(
                        tokens,
                        *pattern_range,
                        *type_range,
                        paren_pairs,
                        bracket_pairs,
                        aliases,
                        imports,
                        mapping,
                        context,
                        proven_paths,
                        trait_paths,
                    )
                )
            return result
    pattern_outer = outer_pair(tokens, pattern_start, pattern_end, bracket_pairs, "[")
    type_outer = outer_pair(tokens, type_start, type_end, bracket_pairs, "[")
    if pattern_outer and type_outer:
        patterns = split_top_level_ranges(tokens, *pattern_outer)
        types = split_top_level_ranges(tokens, *type_outer)
        if len(patterns) == len(types):
            result = []
            for pattern_range, type_range in zip(patterns, types, strict=True):
                result.extend(
                    typed_pattern_bindings(
                        tokens,
                        *pattern_range,
                        *type_range,
                        paren_pairs,
                        bracket_pairs,
                        aliases,
                        imports,
                        mapping,
                        context,
                        proven_paths,
                        trait_paths,
                    )
                )
            return result
    trait_path = unique_trait_or_none(
        signature_trait_paths(
            tokens,
            type_start,
            type_end,
            aliases,
            imports,
            mapping,
            context,
            proven_paths,
            trait_paths,
        )
    )
    return [
        (index, name, trait_path)
        for index, name in pattern_identifiers(tokens, pattern_start, pattern_end)
    ]


def derive_receiver_proofs(
    tokens: list[object],
    aliases: dict[str, tuple[str, ...]],
    imports: dict[str, str],
    mapping: dict[str, str],
    context: tuple[str, ...],
    proven_paths: dict[str, str],
    trait_paths: set[str],
    excluded: set[int],
    identity_imports: dict[str, tuple[str, ...]],
) -> tuple[list[ReceiverBinding], dict[tuple[str | None, str], str], list[ImplScope]]:
    """Derive lexical field/parameter/local/pattern bindings and scoped accessors."""

    brace_pairs = delimiter_pairs(tokens, "{", "}")
    paren_pairs = delimiter_pairs(tokens, "(", ")")
    bracket_pairs = delimiter_pairs(tokens, "[", "]")
    impl_scopes = derive_impl_scopes(
        tokens, brace_pairs, excluded, context, identity_imports
    )
    functions = derive_function_scopes(
        tokens, paren_pairs, brace_pairs, impl_scopes, excluded
    )
    bindings: list[ReceiverBinding] = []
    accessors: dict[tuple[str | None, str], str] = {}

    # Struct fields are visible only in impls of that exact self type.
    struct_fields: dict[str, list[tuple[str, str | None, int]]] = {}
    for index in range(len(tokens) - 1):
        if (
            index in excluded
            or tokens[index].text != "struct"
            or tokens[index + 1].kind != "IDENT"
        ):
            continue
        name = canonical_name(tokens[index + 1])
        opening = index + 2
        while opening < len(tokens) and tokens[opening].text not in {"{", ";", "("}:
            opening += 1
        if opening not in brace_pairs:
            continue
        closing = brace_pairs[opening]
        depth = 1
        cursor = opening + 1
        fields: list[tuple[str, str | None, int]] = []
        while cursor < closing:
            depth += tokens[cursor].text == "{"
            depth -= tokens[cursor].text == "}"
            if (
                depth == 1
                and tokens[cursor].kind == "IDENT"
                and cursor + 1 < closing
                and tokens[cursor + 1].text == ":"
            ):
                end = cursor + 2
                while end < closing and tokens[end].text != ",":
                    end += 1
                resolved = signature_trait_paths(
                    tokens,
                    cursor + 2,
                    end,
                    aliases,
                    imports,
                    mapping,
                    context,
                    proven_paths,
                    trait_paths,
                )
                fields.append(
                    (
                        canonical_name(tokens[cursor]),
                        unique_trait_or_none(resolved),
                        cursor,
                    )
                )
                cursor = end
            cursor += 1
        struct_fields[name] = fields
    for scope in impl_scopes:
        for field_name, trait_path, declaration in struct_fields.get(
            scope.base_name, []
        ):
            bindings.append(
                ReceiverBinding(
                    field_name,
                    trait_path,
                    declaration,
                    scope.start,
                    scope.end,
                    f"field:{scope.type_name}:{field_name}",
                )
            )

    # Parameters, accessor return types, and local shadows are function-scoped.
    for function in functions:
        for param_start, param_end in split_top_level_ranges(
            tokens, function.params_start, function.params_end
        ):
            colon = top_level_token_index(tokens, param_start, param_end, ":")
            if colon is None:
                continue
            for declaration, name, trait_path in typed_pattern_bindings(
                tokens,
                param_start,
                colon,
                colon + 1,
                param_end,
                paren_pairs,
                bracket_pairs,
                aliases,
                imports,
                mapping,
                context,
                proven_paths,
                trait_paths,
            ):
                bindings.append(
                    ReceiverBinding(
                        name,
                        trait_path,
                        declaration,
                        function.body_start,
                        function.body_end,
                        f"parameter:{function.name}:{name}:{declaration}",
                    )
                )

        # Resolve this method's return type into an impl-qualified accessor identity.
        arrow = function.params_end + 1
        while arrow + 1 < function.body_start and not (
            tokens[arrow].text == "-" and tokens[arrow + 1].text == ">"
        ):
            arrow += 1
        if arrow + 1 < function.body_start:
            resolved = signature_trait_paths(
                tokens,
                arrow + 2,
                function.body_start - 1,
                aliases,
                imports,
                mapping,
                context,
                proven_paths,
                trait_paths,
            )
            trait_path = unique_trait_or_none(resolved)
            if trait_path is not None:
                key = (function.impl_type, function.name)
                previous = accessors.get(key)
                if previous is not None and previous != trait_path:
                    accessors.pop(key, None)
                elif previous is None:
                    accessors[key] = trait_path

        cursor = function.body_start
        while cursor < function.body_end:
            if cursor in excluded or tokens[cursor].text != "let":
                cursor += 1
                continue
            if cursor > function.body_start and tokens[cursor - 1].text in {
                "if",
                "while",
            }:
                cursor += 1
                continue
            pattern_start = cursor + 1
            equals = pattern_start
            colon: int | None = None
            while equals < function.body_end and tokens[equals].text not in {"=", ";"}:
                if tokens[equals].text == ":" and colon is None:
                    colon = equals
                equals += 1
            pattern_end = colon if colon is not None else equals
            local_bindings: list[tuple[int, str, str | None]]
            if colon is not None:
                local_bindings = typed_pattern_bindings(
                    tokens,
                    pattern_start,
                    colon,
                    colon + 1,
                    equals,
                    paren_pairs,
                    bracket_pairs,
                    aliases,
                    imports,
                    mapping,
                    context,
                    proven_paths,
                    trait_paths,
                )
            else:
                local_bindings = [
                    (index, name, None)
                    for index, name in pattern_identifiers(
                        tokens, pattern_start, pattern_end
                    )
                ]
            inferred_trait: str | None = None
            if (
                colon is None
                and equals < function.body_end
                and tokens[equals].text == "="
            ):
                rhs_end = equals + 1
                while rhs_end < function.body_end and tokens[rhs_end].text != ";":
                    rhs_end += 1
                for rhs_index in range(equals + 1, rhs_end - 1):
                    if (
                        tokens[rhs_index].kind != "IDENT"
                        or tokens[rhs_index + 1].text != "("
                    ):
                        continue
                    key = (function.impl_type, canonical_name(tokens[rhs_index]))
                    if key in accessors:
                        inferred_trait = accessors[key]
                        break
            scope_start, scope_end = innermost_brace_scope(
                cursor, brace_pairs, function.body_end
            )
            for declaration, name, trait_path in local_bindings:
                bindings.append(
                    ReceiverBinding(
                        name,
                        trait_path or inferred_trait,
                        declaration,
                        max(scope_start, declaration),
                        scope_end,
                        f"local:{function.name}:{name}:{declaration}",
                    )
                )
            cursor = max(cursor + 1, equals + 1)

        # Closure parameters own their expression/body scope and may shadow method bindings.
        cursor = function.body_start
        while cursor < function.body_end:
            if tokens[cursor].text != "|" or cursor in excluded:
                cursor += 1
                continue
            previous = tokens[cursor - 1].text if cursor > function.body_start else "{"
            if previous not in {"=", "(", ",", "{", "move", "return", ">"}:
                cursor += 1
                continue
            closing_pipe = cursor + 1
            while closing_pipe < function.body_end and tokens[closing_pipe].text != "|":
                closing_pipe += 1
            if closing_pipe >= function.body_end:
                break
            body_token = closing_pipe + 1
            if body_token < function.body_end and tokens[body_token].text == "{":
                closure_start = body_token + 1
                closure_end = brace_pairs.get(body_token, function.body_end)
            else:
                closure_start = body_token
                closure_end = expression_scope_end(
                    tokens, body_token, function.body_end
                )
            for param_start, param_end in split_top_level_ranges(
                tokens, cursor + 1, closing_pipe
            ):
                colon = top_level_token_index(tokens, param_start, param_end, ":")
                closure_bindings = (
                    typed_pattern_bindings(
                        tokens,
                        param_start,
                        colon,
                        colon + 1,
                        param_end,
                        paren_pairs,
                        bracket_pairs,
                        aliases,
                        imports,
                        mapping,
                        context,
                        proven_paths,
                        trait_paths,
                    )
                    if colon is not None
                    else [
                        (index, name, None)
                        for index, name in pattern_identifiers(
                            tokens, param_start, param_end
                        )
                    ]
                )
                for declaration, name, trait_path in closure_bindings:
                    bindings.append(
                        ReceiverBinding(
                            name,
                            trait_path,
                            declaration,
                            closure_start,
                            closure_end,
                            f"closure:{function.name}:{name}:{declaration}",
                        )
                    )
            cursor = max(closing_pipe + 1, closure_end)

    # Enum variant patterns carry their payload trait into one match arm.
    variant_traits: dict[tuple[str, str], str] = {}
    for index in range(len(tokens) - 2):
        if (
            index in excluded
            or tokens[index].text != "enum"
            or tokens[index + 1].kind != "IDENT"
        ):
            continue
        enum_name = canonical_name(tokens[index + 1])
        opening = index + 2
        while opening < len(tokens) and tokens[opening].text != "{":
            opening += 1
        if opening not in brace_pairs:
            continue
        closing = brace_pairs[opening]
        cursor = opening + 1
        while cursor + 1 < closing:
            if (
                tokens[cursor].kind == "IDENT"
                and tokens[cursor + 1].text == "("
                and cursor + 1 in paren_pairs
            ):
                end = paren_pairs[cursor + 1]
                resolved = signature_trait_paths(
                    tokens,
                    cursor + 2,
                    end,
                    aliases,
                    imports,
                    mapping,
                    context,
                    proven_paths,
                    trait_paths,
                )
                trait_path = unique_trait_or_none(resolved)
                if trait_path is not None:
                    variant_traits[(enum_name, canonical_name(tokens[cursor]))] = (
                        trait_path
                    )
                cursor = end
            cursor += 1
    for index in range(len(tokens) - 5):
        if (
            tokens[index].kind != "IDENT"
            or tokens[index + 1].text != "::"
            or tokens[index + 2].kind != "IDENT"
            or tokens[index + 3].text != "("
        ):
            continue
        key = (canonical_name(tokens[index]), canonical_name(tokens[index + 2]))
        trait_path = variant_traits.get(key)
        if index + 3 not in paren_pairs:
            continue
        closing = paren_pairs[index + 3]
        pattern_names = pattern_identifiers(tokens, index + 4, closing)
        if (
            index >= 2
            and tokens[index - 1].text == "let"
            and tokens[index - 2].text in {"if", "while"}
        ):
            body = closing + 1
            while body < len(tokens) and tokens[body].text != "{":
                if tokens[body].text == ";":
                    break
                body += 1
            if body in brace_pairs:
                for binding_index, name in pattern_names:
                    bindings.append(
                        ReceiverBinding(
                            name,
                            trait_path,
                            binding_index,
                            body + 1,
                            brace_pairs[body],
                            f"control-pattern:{tokens[index - 2].text}:{key[0]}::{key[1]}"
                            f":{name}:{binding_index}",
                        )
                    )
            continue
        arrow = match_arm_arrow(tokens, closing + 1, len(tokens))
        if arrow is None:
            continue
        arm_end = expression_scope_end(tokens, arrow + 2, len(tokens))
        for binding_index, name in pattern_names:
            bindings.append(
                ReceiverBinding(
                    name,
                    trait_path,
                    binding_index,
                    closing + 1,
                    arm_end,
                    f"pattern:{key[0]}::{key[1]}:{name}:{binding_index}",
                )
            )
    return bindings, accessors, impl_scopes


def receiver_binding_at(
    name: str,
    call_index: int,
    bindings: list[ReceiverBinding],
    *,
    field_owner: str | None = None,
) -> ReceiverBinding | None:
    """Resolve the nearest lexical declaration; unrelated shadow blocks outer proofs."""

    candidates = [
        binding
        for binding in bindings
        if binding.name == name
        and binding.declaration_index < call_index
        and binding.scope_start <= call_index < binding.scope_end
        and (
            binding.identity.startswith(f"field:{field_owner}:")
            if field_owner is not None
            else not binding.identity.startswith("field:")
        )
    ]
    if not candidates:
        return None
    candidates.sort(
        key=lambda binding: (
            binding.scope_end - binding.scope_start,
            -binding.declaration_index,
        )
    )
    best = candidates[0]
    best_key = (best.scope_end - best.scope_start, best.declaration_index)
    tied = [
        binding
        for binding in candidates
        if (binding.scope_end - binding.scope_start, binding.declaration_index)
        == best_key
    ]
    if len({binding.trait_path for binding in tied}) > 1:
        return None
    return best


def receiver_call_rows(
    path: Path,
    tokens: list[object],
    lexical_bindings: list[ReceiverBinding],
    accessors: dict[tuple[str | None, str], str],
    impl_scopes: list[ImplScope],
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
        proof_identity: str | None = None
        receiver_index = method_index - 2
        if receiver_index >= 0 and tokens[receiver_index].kind == "IDENT":
            owner = enclosing_impl(method_index, impl_scopes)
            is_self_field = (
                receiver_index >= 2
                and tokens[receiver_index - 1].text == "."
                and tokens[receiver_index - 2].text == "self"
            )
            is_bare_receiver = (
                receiver_index == 0 or tokens[receiver_index - 1].text != "."
            )
            binding = None
            if is_self_field and owner is not None:
                binding = receiver_binding_at(
                    canonical_name(tokens[receiver_index]),
                    method_index,
                    lexical_bindings,
                    field_owner=owner.type_name,
                )
            elif is_bare_receiver:
                binding = receiver_binding_at(
                    canonical_name(tokens[receiver_index]),
                    method_index,
                    lexical_bindings,
                )
            if binding is not None:
                trait_path = binding.trait_path
                proof_identity = binding.identity
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
                and accessor_index >= 2
                and tokens[accessor_index - 1].text == "."
                and tokens[accessor_index - 2].text == "self"
            ):
                owner = enclosing_impl(method_index, impl_scopes)
                owner_name = owner.type_name if owner else None
                accessor_name = canonical_name(tokens[accessor_index])
                trait_path = accessors.get((owner_name, accessor_name))
                if trait_path is not None:
                    proof_identity = (
                        f"accessor:{owner_name or '<module>'}:{accessor_name}"
                    )
        if trait_path is None:
            continue
        method = canonical_name(tokens[method_index])
        if method not in trait_methods.get(trait_path, set()):
            continue
        rows.append(
            ConsumerRow(
                path.as_posix(),
                "CALL",
                f"{method}@receiver={trait_path.rsplit('::', 1)[1]}"
                f"@binding={proof_identity}@token={method_index}",
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
        implementation_type = "".join(
            token.text for token in tokens[type_start:type_end]
        )
        if not implementation_type:
            fail(f"trait impl has no implementation type in {path}")
        owner = (
            "RDBC_CURRENT_IMPL"
            if path.parts[:2] == ("crates", "druid-rdbc")
            else (
                "WRAPPER_ADAPTER"
                if path.parts[:2] == ("crates", "druid-wrapper")
                else "DRUID_LOGICAL_OR_PROXY"
            )
        )
        target_task = (
            "P04_TASK_4_ADAPTER_CUTOVER"
            if owner == "WRAPPER_ADAPTER"
            else (
                "P04_TASK_5_LOGICAL_CUTOVER"
                if owner == "DRUID_LOGICAL_OR_PROXY"
                else tasks[trait_path]
            )
        )
        target_trait = mapping[trait_path]
        standard_type = implementation_type
        standard_source = path.as_posix()
        standard_core = "-"
        behavior = "-"
        if target_task == "P04_TASK_4_ADAPTER_CUTOVER":
            family_behaviors = {
                "Sqlx": "statement_executor_ownership_test::sqlx_standard_factory_emits_typed_runtime_trace",
                "Toasty": "toasty_connection_adapter_test::toasty_standard_factory_emits_typed_runtime_trace",
                "Rbdc": "rbdc_connection_adapter_test::rbdc_standard_factory_emits_typed_runtime_trace",
                "DuckDb": "duckdb_connection_adapter_test::duckdb_standard_factory_emits_typed_runtime_trace",
                "HttpSql": "http_sql::http_sql_connection_factory::tests::http_sql_standard_factory_uses_deterministic_transport_owned_core",
                "LibSql": "libsql::libsql_connection_factory::tests::libsql_standard_factory_uses_deterministic_transport_owned_core",
                "JdbcAgent": "jdbc_agent::jdbc_agent_connection_factory::tests::jdbc_agent_standard_factory_uses_deterministic_runtime_owned_core",
                "DruidRdbcDataSource": "standard_adapter_conformance_test::catalog_data_sources_implement_canonical_wrapper_identity",
                "DynamicRdbcDataSource": "standard_adapter_conformance_test::catalog_data_sources_implement_canonical_wrapper_identity",
            }
            behavior = next(
                (
                    evidence
                    for prefix, evidence in family_behaviors.items()
                    if implementation_type.startswith(prefix)
                ),
                "-",
            )
            prepared_families = {
                "DuckDbPreparedStatement": (
                    "DuckDbConnectionAdapter::standard_executor_core",
                    "duckdb_connection_adapter_test::duckdb_standard_factory_emits_typed_runtime_trace",
                ),
                "RbdcPreparedStatement": (
                    "RbdcConnectionAdapter::standard_executor_core",
                    "rbdc_connection_adapter_test::rbdc_standard_factory_emits_typed_runtime_trace",
                ),
                "ToastyPreparedStatement": (
                    "ToastyConnectionAdapter::standard_executor_core",
                    "toasty_connection_adapter_test::toasty_standard_factory_emits_typed_runtime_trace",
                ),
                "HttpSqlPreparedStatement": (
                    "HttpSqlConnectionAdapter::standard_executor_core",
                    "http_sql::http_sql_connection_factory::tests::http_sql_standard_factory_uses_deterministic_transport_owned_core",
                ),
                "LibSqlPreparedStatement": (
                    "LibSqlConnectionAdapter::standard_executor_core",
                    "libsql::libsql_connection_factory::tests::libsql_standard_factory_uses_deterministic_transport_owned_core",
                ),
                "JdbcAgentPreparedStatement": (
                    "JdbcAgentConnection::standard_executor_core",
                    "jdbc_agent::jdbc_agent_connection_factory::tests::jdbc_agent_standard_factory_uses_deterministic_runtime_owned_core",
                ),
            }
            if implementation_type in prepared_families:
                standard_type = "StandardAdapterPreparedStatement"
                standard_source = (
                    "crates/druid-wrapper/src/standard_adapter_prepared_statement.rs"
                )
                standard_core, behavior = prepared_families[implementation_type]
            elif implementation_type == "SqlxPreparedStatement":
                standard_core = "SqlxConnectionActor::execute_query/execute_update"
                behavior = "statement_executor_ownership_test::sqlx_standard_factory_emits_typed_runtime_trace"
            elif target_trait.endswith("::ConnectionFactory"):
                standard_core = f"{implementation_type}::create_adapter_core"
            elif target_trait.endswith("::Connection"):
                standard_core = (
                    "SqlxConnectionAdapter::standard_actor_core"
                    if implementation_type == "SqlxConnectionAdapter"
                    else f"{implementation_type}::standard_executor_core"
                )
            elif target_trait.endswith("::DatabaseMetaData"):
                standard_core = f"{implementation_type.split('<', 1)[0]}::database_product_name_core"
            elif target_trait.endswith("::ResultSet"):
                standard_core = (
                    f"{implementation_type}::next_core/value_core/close_core"
                )
            elif target_trait.endswith("::Wrapper"):
                standard_core = f"{implementation_type}::canonical_unwrap"
                behavior = "standard_adapter_conformance_test::catalog_data_sources_implement_canonical_wrapper_identity"
        rows.append(
            AdapterRow(
                path.as_posix(),
                trait_path,
                implementation_type,
                owner,
                target_trait,
                target_task,
                standard_type,
                standard_source,
                standard_core,
                behavior,
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
    excluded = production_excluded_indexes(tokens)
    context = module_context(path)
    proven = proven_paths or {old_path: old_path for old_path in mapping}
    declarations = declaration_paths or {}
    current_trait_paths = trait_paths or set(mapping)
    current_tasks = tasks or {old: "PLANNED_TARGET" for old in mapping}
    aliases: dict[str, tuple[str, ...]] = {
        "druid_rdbc": ("druid_rdbc",),
        "druid": ("druid",),
    }
    identity_imports: dict[str, tuple[str, ...]] = {}
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
                token.kind == "IDENT"
                and canonical_name(token) == name
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
            tokens[index].text == "extern"
            and tokens[index + 1].text == "crate"
            and tokens[index + 2].kind == "IDENT"
            and tokens[index + 3].text == "as"
            and tokens[index + 4].kind == "IDENT"
        ):
            root_name = canonical_name(tokens[index + 2])
            if root_name in {"druid", "druid_rdbc"}:
                alias = canonical_name(tokens[index + 4])
                aliases[alias] = (root_name,)
                identity_imports[alias] = (root_name,)
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
                    fail(
                        f"wildcard import can hide standardized symbols in {path}: {'::'.join(expanded)}"
                    )
                continue
            alias = entry.alias
            if expanded:
                bound_identity = alias or expanded[-1]
                previous_identity = identity_imports.get(bound_identity)
                if previous_identity is None:
                    identity_imports[bound_identity] = expanded
                elif previous_identity != expanded:
                    # Invalid or cfg-overlapping duplicate imports provide no
                    # exact identity proof; keep the name deliberately unresolved.
                    identity_imports.pop(bound_identity, None)
            old_path = proven_old_path(expanded, mapping, proven)
            if (
                old_path is None
                and expanded
                and expanded[0]
                not in {
                    "druid",
                    "druid_rdbc",
                    "druid_wrapper",
                }
            ):
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
        if (
            token_index in covered
            or token.kind != "IDENT"
            or declaration_name(tokens, token_index)
        ):
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
    lexical_bindings, accessors, impl_scopes = derive_receiver_proofs(
        tokens,
        aliases,
        bindings,
        mapping,
        context,
        proven,
        current_trait_paths,
        excluded,
        identity_imports,
    )
    occurrences.extend(
        receiver_call_rows(
            path,
            tokens,
            lexical_bindings,
            accessors,
            impl_scopes,
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


def derive_live_manifests(
    root: Path,
    legacy_physical: Path = P03_MANIFEST,
    *,
    phase: str = "transitional",
) -> LiveManifests:
    """Derive all three tables from the immutable P03 base and current production tokens."""

    # The argument is retained so the documented command names the artifact being refreshed.
    # The denominator itself comes from Git P03 plus current source, never from mutable TSV rows.
    _ = legacy_physical
    if phase not in {"transitional", "task5"}:
        fail(f"unknown denominator phase: {phase}")
    physical = derive_physical_rows(
        root,
        allow_task5_internalized=phase == "task5",
    )
    mapping = {row.old_path: row.target_path for row in physical}
    tasks = {row.old_path: row.target_task for row in physical}
    declaration_paths = {row.old_path: row.source_file for row in physical}
    trait_paths = {
        row.old_path
        for row in physical
        if row.normalized_signature.startswith("trait ")
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


def normalize_adapter_phase(rows: tuple[AdapterRow, ...]) -> tuple[AdapterRow, ...]:
    """Project Task 4 LIVE state back to the frozen Task 2 denominator state."""

    return tuple(
        AdapterRow(
            row.source_path,
            row.current_trait,
            row.implementation_type,
            row.owner,
            row.target_trait,
            row.target_task,
            row.standard_implementation_type,
            row.standard_source_path,
            row.standard_core_evidence,
            row.behavior_evidence,
            "CURRENT_IMPL_TARGET_PLANNED",
        )
        for row in rows
    )


def normalize_consumer_phase(rows: tuple[ConsumerRow, ...]) -> tuple[ConsumerRow, ...]:
    """Project Task 4 LIVE state back without changing the frozen occurrence coordinates."""

    return tuple(
        ConsumerRow(
            row.source_path,
            row.occurrence_kind,
            row.symbol,
            row.old_path,
            row.target_path,
            row.target_task,
            "CURRENT_REFERENCE_TARGET_PLANNED",
        )
        for row in rows
    )


def pinned_task4_closed_consumers(root: Path) -> tuple[ConsumerRow, ...]:
    """Load and authenticate the exact 21 pre-cutover wrapper identities from 8252a20."""

    source = DEFAULT_CONSUMERS.as_posix()
    result = subprocess.run(
        ["git", "show", f"{TASK4_PRE_CUTOVER_COMMIT}:{source}"],
        cwd=root,
        check=False,
        capture_output=True,
    )
    if result.returncode != 0:
        detail = result.stderr.decode("utf-8", errors="replace").strip()
        fail(
            "cannot load pinned Task 4 consumer history from "
            f"{TASK4_PRE_CUTOVER_COMMIT}: {detail or 'git show failed'}"
        )
    pinned = tuple(
        sorted(
            ConsumerRow(
                row.source_path,
                row.occurrence_kind,
                row.symbol,
                row.old_path,
                row.target_path,
                row.target_task,
                "STANDARD_REFERENCE_CLOSED",
            )
            for row in parse_consumer_payload(result.stdout)
            if row.target_task == "P04_TASK_4_ADAPTER_CUTOVER"
            and row.source_path.startswith("crates/druid-wrapper/")
            and row.old_path
            in {
                "druid_rdbc::spi::SqlTextStatement",
                "druid_rdbc::spi::SqlTextPreparedStatement",
            }
        )
    )
    if len(pinned) != 21:
        fail(f"pinned Task 4 consumer history expected 21 rows, found {len(pinned)}")
    digest = hashlib.sha256(render_consumer_rows(pinned)).hexdigest()
    if digest != TASK4_CLOSED_HISTORY_SHA256:
        fail(
            "pinned Task 4 consumer history hash drift: "
            f"expected {TASK4_CLOSED_HISTORY_SHA256}, found {digest}"
        )
    return pinned


def pinned_task5_consumers(root: Path) -> tuple[ConsumerRow, ...]:
    """Load the complete source-derived Task 5 denominator from its authorized base."""

    source = DEFAULT_CONSUMERS.as_posix()
    result = subprocess.run(
        ["git", "show", f"{TASK5_PRE_CUTOVER_COMMIT}:{source}"],
        cwd=root,
        check=False,
        capture_output=True,
    )
    if result.returncode != 0:
        detail = result.stderr.decode("utf-8", errors="replace").strip()
        fail(
            f"cannot load pinned Task 5 consumer history: {detail or 'git show failed'}"
        )
    rows = tuple(
        row
        for row in parse_consumer_payload(result.stdout)
        if row.target_task == "P04_TASK_5_DRUID_INTERNALIZATION"
    )
    if len(rows) != 357:
        fail(f"pinned Task 5 consumer denominator expected 357 rows, found {len(rows)}")
    return rows


def task5_consumer_identity(row: ConsumerRow) -> tuple[str, str, str, str, str, str]:
    """Compare source occurrences while permitting honest token-coordinate movement."""

    symbol = re.sub(r"@(?:qualified-)?token=.*$", "", row.symbol)
    return (
        row.source_path,
        row.occurrence_kind,
        symbol,
        row.old_path,
        row.target_path,
        row.target_task,
    )


def require_task4_adapter_payload(
    path: Path, live: tuple[AdapterRow, ...]
) -> tuple[AdapterRow, ...]:
    """Require exact source impl rows while allowing only Task 4 state advancement."""

    if not path.is_file():
        fail(f"adapter manifest not found: {path}")
    payload = path.read_bytes()
    checked = tuple(parse_adapter_payload(payload))
    normalized = normalize_adapter_phase(checked)
    if normalized != live:
        fail("adapter manifest rows do not match the live denominator")
    return normalized


def require_task4_consumer_payload(
    root: Path,
    path: Path,
    live: tuple[ConsumerRow, ...],
) -> tuple[ConsumerRow, ...]:
    """Require every Task 2 field, including source coordinates, to match fresh source."""

    if not path.is_file():
        fail(f"consumer manifest not found: {path}")
    payload = path.read_bytes()
    checked = tuple(parse_consumer_payload(payload))
    closed = tuple(row for row in checked if row.state == "STANDARD_REFERENCE_CLOSED")
    pinned_closed = pinned_task4_closed_consumers(root)
    if closed != pinned_closed:
        fail(
            "Task 4 historical CLOSED consumers do not match pinned 8252a20 identities"
        )
    task5_all = tuple(
        row for row in checked if row.target_task == "P04_TASK_5_DRUID_INTERNALIZATION"
    )
    task5_baseline = pinned_task5_consumers(root)
    checked_task5_identities = Counter(map(task5_consumer_identity, task5_all))
    baseline_task5_identities = Counter(map(task5_consumer_identity, task5_baseline))
    if baseline_task5_identities - checked_task5_identities:
        fail("Task 5 LIVE/CLOSED consumers omit the pinned dynamic denominator")
    additions = checked_task5_identities - baseline_task5_identities
    task6_addition_paths = {
        "crates/druid/src/core/mod.rs",
        "crates/druid/src/core/physical_connection_info.rs",
        "crates/druid/src/core/prepared_statement_key.rs",
        "crates/druid/src/spi/mod.rs",
        "crates/druid/src/spi/rdbc_resource_capabilities.rs",
        "crates/druid/src/spi/rdbc_resource_context.rs",
        "crates/druid/src/spi/rdbc_resource_factory.rs",
        "crates/druid/src/spi/rdbc_resource_id.rs",
        "crates/druid/src/spi/rdbc_resource_kind.rs",
        "crates/druid/src/spi/rdbc_resource_owner.rs",
        "crates/druid/src/spi/rdbc_resource_state.rs",
    }
    if any(
        not (
            identity[0].startswith("crates/druid/src/core/compat/")
            or identity[0] in task6_addition_paths
        )
        for identity in additions
    ):
        fail("Task 5 consumer was added outside the Task 6 compatibility boundary")
    task5_state_counts = Counter(row.state for row in task5_all)
    if task5_state_counts["DRUID_INTERNAL_REFERENCE_CLOSED"] != 180:
        fail(
            "Task 5 consumer manifest must contain exactly 180 internalized CLOSED rows"
        )
    if task5_state_counts["TASK7_RESOURCE_OPERATIONS_PENDING"] != 108:
        fail("Task 5 consumer manifest must contain exactly 108 Task 7 handoff rows")
    if task5_state_counts["TASK6_COMPATIBILITY_PENDING"] < 69:
        fail("Task 5 consumer manifest lost the 69-row Task 6 compatibility floor")
    active = tuple(
        row
        for row in checked
        if row.state != "STANDARD_REFERENCE_CLOSED"
        and row.target_task != "P04_TASK_5_DRUID_INTERNALIZATION"
    )
    normalized_active = normalize_consumer_phase(active)
    live_non_task5 = tuple(
        row for row in live if row.target_task != "P04_TASK_5_DRUID_INTERNALIZATION"
    )
    if normalized_active != live_non_task5:
        fail("active consumer manifest rows do not match the live denominator")
    live_identities = {
        (row.source_path, row.occurrence_kind, row.old_path, row.target_path)
        for row in live
    }
    for row in closed:
        identity = (row.source_path, row.occurrence_kind, row.old_path, row.target_path)
        if identity in live_identities:
            fail(f"closed consumer still exists in live source: {identity}")
    return tuple(
        sorted(
            (
                *normalized_active,
                *normalize_consumer_phase(closed),
                *normalize_consumer_phase(task5_all),
            )
        )
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

    live = live or derive_live_manifests(root, physical_path, phase="task5")
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
    projected_adapters = require_task4_adapter_payload(adapters, live.adapters)
    projected_consumers = require_task4_consumer_payload(
        root, consumers, live.consumers
    )
    return LiveManifests(live.physical, projected_adapters, projected_consumers)


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
        (
            consumer_path,
            render_consumer_rows(
                tuple(sorted((*live.consumers, *pinned_task4_closed_consumers(root))))
            ),
        ),
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
            live = emit_live(
                root, arguments.physical, arguments.adapters, arguments.consumers
            )
        else:
            live = verify_denominator(
                root, arguments.physical, arguments.adapters, arguments.consumers
            )
    except VerificationError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print_summary(live)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
