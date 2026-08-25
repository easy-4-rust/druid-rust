#!/usr/bin/env python3
"""Verify P04 Task 4 wrapper adapters implement the canonical RDBC standard directly."""

from __future__ import annotations

import argparse
import csv
import functools
import re
import subprocess
import tomllib
from dataclasses import dataclass
from pathlib import Path
from typing import NoReturn
from verify_rdbc_public_paths import tokenize
from verify_rdbc_standardization_denominator import (
    cfg_atoms,
    cfg_profile_excluded_indexes,
    evaluate_cfg,
    parse_cfg_expression,
    parse_cfg_meta,
    rust_tokens,
    split_top_level_ranges,
)


DEFAULT_ADAPTERS = Path("docs/verification/rdbc-first/rdbc-adapter-implementations.tsv")
DEFAULT_CONSUMERS = Path("docs/verification/rdbc-first/rdbc-production-consumers.tsv")
TASK = "P04_TASK_4_ADAPTER_CUTOVER"
LIVE_ADAPTER_STATE = "STANDARD_IMPL_LIVE"
LIVE_CONSUMER_STATE = "STANDARD_REFERENCE_CLOSED"
SQL_EXECUTION_MARKERS = (
    "sqlx::query",
    ".fetch_all(",
    ".fetch_one(",
    ".execute(",
    "RdbcError::DriverError",
    "RdbcError::UnsupportedOperation",
)
APPROVED_ASSERTION_MACROS = {
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "matches",
}
STRAIGHT_LINE_FORBIDDEN_KEYWORDS = {
    "if": "branch `if`",
    "else": "branch `else`",
    "match": "branch `match`",
    "loop": "loop `loop`",
    "while": "loop `while`",
    "for": "loop `for`",
    "return": "early exit `return`",
    "break": "early exit `break`",
    "continue": "early exit `continue`",
    "async": "deferred `async` block or closure",
    "move": "deferred `move` closure",
    "type": "local type definition",
    "struct": "local struct definition",
    "enum": "local enum definition",
    "union": "local union definition",
    "trait": "local trait definition",
    "impl": "local impl definition",
    "mod": "local module definition",
    "macro_rules": "local macro_rules definition",
    "macro": "local macro definition",
    "const": "local const item",
    "static": "local static item",
    "use": "local use item",
    "extern": "local extern item",
    "unsafe": "unsafe block",
    "try": "try block",
    "yield": "yield expression",
}
REMOTE_BEHAVIOR_CHAIN = (
    ("ConnectionFactory", "::", "create"),
    ("create_statement",),
    ("execute_update",),
    ("prepare_statement",),
    ("set_parameter",),
    ("execute_prepared_query",),
    ("next",),
    ("meta_data",),
    ("database_meta_data",),
    ("Connection", "::", "close"),
)


class VerificationError(RuntimeError):
    """Raised when Task 4 evidence is absent, duplicated, or stale."""


def fail(message: str) -> NoReturn:
    """Stop with one stable fail-closed diagnostic."""

    raise VerificationError(message)


@dataclass(frozen=True)
class AdapterRow:
    """One Task 4 production adapter obligation."""

    source_path: Path
    current_trait: str
    implementation_type: str
    target_trait: str
    standard_implementation_type: str
    standard_source_path: Path
    standard_core_evidence: str
    behavior_evidence: str
    state: str


@dataclass(frozen=True)
class CfgProfile:
    """One exact wrapper test profile over Cargo features and current rustc target cfg."""

    name: str
    assignment: dict[str, bool]


def read_tsv(path: Path) -> list[dict[str, str]]:
    """Read one canonical TSV while rejecting malformed rows."""

    raw = path.read_bytes()
    if not raw.endswith(b"\n") or raw.endswith(b"\n\n") or b"\r" in raw:
        fail(f"TSV must have exactly one LF final newline: {path}")
    text = raw.decode("utf-8")
    rows = list(csv.DictReader(text.splitlines(), delimiter="\t"))
    if not rows or any(None in row or any(value == "" for value in row.values()) for row in rows):
        fail(f"TSV contains an empty or malformed row: {path}")
    return rows


def task_adapter_rows(path: Path) -> list[AdapterRow]:
    """Load exactly the production wrapper rows assigned to Task 4."""

    rows = [
        AdapterRow(
            source_path=Path(row["source_path"]),
            current_trait=row["current_trait"],
            implementation_type=row["implementation_type"],
            target_trait=row["target_trait"],
            standard_implementation_type=row["standard_implementation_type"],
            standard_source_path=Path(row["standard_source_path"]),
            standard_core_evidence=row["standard_core_evidence"],
            behavior_evidence=row["behavior_evidence"],
            state=row["state"],
        )
        for row in read_tsv(path)
        if row["target_task"] == TASK and row["owner"] == "WRAPPER_ADAPTER"
    ]
    if len(rows) != 31:
        fail(f"Task 4 adapter denominator drift: expected 31 rows, found {len(rows)}")
    return rows


def concrete_type(type_name: str) -> str:
    """Return the concrete Rust identifier recorded by the inventory."""

    return re.split(r"[<' ]", type_name, maxsplit=1)[0]


def extract_impl_bodies(source: str, trait_name: str, type_name: str) -> list[str]:
    """Extract explicit impl bodies with balanced braces."""

    leaf = trait_name.rsplit("::", 1)[-1]
    concrete = concrete_type(type_name)
    header = re.compile(
        rf"\bimpl(?:\s*<[^{{;]*>)?\s+(?:[A-Za-z_][\w]*::)*{re.escape(leaf)}"
        rf"\s+for\s+{re.escape(concrete)}(?:\s*<[^{{;]*>)?(?:\s+where\s+[^{{]*)?\s*\{{",
        re.MULTILINE,
    )
    bodies: list[str] = []
    for match in header.finditer(source):
        depth = 1
        index = match.end()
        while index < len(source) and depth:
            if source[index] == "{":
                depth += 1
            elif source[index] == "}":
                depth -= 1
            index += 1
        if depth:
            fail(f"unbalanced impl block for {concrete}: {leaf}")
        bodies.append(source[match.end() : index - 1])
    return bodies


def code_tokens(source: str) -> str:
    """Return executable Rust tokens without comments or literal contents."""

    return " ".join(token.text for token in tokenize(source))


def core_calls(body: str) -> set[str]:
    """Return private/inherent core calls made by one impl body."""

    return set(re.findall(r"\b([A-Za-z_][A-Za-z0-9_]*_core)\s*\(", body))


@functools.lru_cache(maxsize=1)
def rustc_cfg_lines() -> tuple[str, ...]:
    """Return the current compiler target cfg as canonical atom strings."""

    result = subprocess.run(
        ["rustc", "--print", "cfg"],
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        fail(f"cannot determine current rustc cfg: {result.stderr.strip()}")
    return tuple(sorted(line.strip() for line in result.stdout.splitlines() if line.strip()))


def expanded_cargo_features(root: Path, all_features: bool) -> tuple[set[str], set[str]]:
    """Return enabled and declared druid-wrapper feature names for one Cargo profile."""

    manifest = root / "crates/druid-wrapper/Cargo.toml"
    if not manifest.is_file():
        return set(), set()
    payload = tomllib.loads(manifest.read_text(encoding="utf-8"))
    table = payload.get("features", {})
    if not isinstance(table, dict):
        fail("druid-wrapper Cargo features table is malformed")
    declared = {str(name) for name in table if name != "default"}
    if all_features:
        return set(declared), declared
    enabled: set[str] = set()
    pending = [str(value) for value in table.get("default", [])]
    while pending:
        feature = pending.pop()
        if feature.startswith("dep:") or "/" in feature or feature in enabled:
            continue
        if feature not in table:
            fail(f"default Cargo feature references unknown feature: {feature}")
        enabled.add(feature)
        pending.extend(str(value) for value in table.get(feature, []))
    return enabled, declared


@functools.lru_cache(maxsize=8)
def cfg_profile(root_text: str, profile_name: str) -> CfgProfile:
    """Build a complete feature/target assignment for default or all-features tests."""

    if profile_name not in {"default", "all-features"}:
        fail(f"unknown behavior evidence profile: {profile_name}")
    root = Path(root_text)
    enabled, declared = expanded_cargo_features(root, profile_name == "all-features")
    true_atoms = set(rustc_cfg_lines())
    true_atoms.update(f'feature="{feature}"' for feature in enabled)
    assignment = {atom: True for atom in true_atoms}
    for feature in declared:
        assignment.setdefault(f'feature="{feature}"', False)
    # Cargo treats an undeclared feature predicate as disabled; target keys are closed below.
    assignment["test"] = True
    return CfgProfile(profile_name, assignment)


def assignment_for_expressions(
    expressions: list[tuple[object, ...]],
    profile: CfgProfile,
) -> dict[str, bool]:
    """Complete known feature/target atoms and fail closed for custom cfg names."""

    assignment = dict(profile.assignment)
    true_atoms = {atom for atom, value in assignment.items() if value}
    target_keys = {
        atom.split("=", 1)[0]
        for atom in rustc_cfg_lines()
        if "=" in atom
    }
    known_flags = {atom for atom in rustc_cfg_lines() if "=" not in atom}
    for expression in expressions:
        for atom in cfg_atoms(expression):
            if atom in assignment:
                continue
            key = atom.split("=", 1)[0]
            if key == "feature":
                assignment[atom] = False
            elif key in target_keys:
                assignment[atom] = atom in true_atoms
            elif atom in known_flags:
                assignment[atom] = True
            elif atom in {"unix", "windows"}:
                assignment[atom] = atom in true_atoms
            else:
                fail(f"cfg profile cannot prove atom: {atom}")
    return assignment


def matching_open_bracket(tokens: list[object], close_index: int) -> int:
    """Return the matching `[` for one outer attribute closing bracket."""

    depth = 1
    index = close_index - 1
    while index >= 0:
        depth += tokens[index].text == "]"
        depth -= tokens[index].text == "["
        if depth == 0:
            return index
        index -= 1
    fail("behavior evidence attribute is unbalanced")


def preceding_attribute_ranges(tokens: list[object], fn_index: int) -> list[tuple[int, int]]:
    """Return the exact consecutive outer attributes attached to one test function."""

    item_start = fn_index
    while item_start > 0 and tokens[item_start - 1].text in {"async", "const", "unsafe"}:
        item_start -= 1
    ranges: list[tuple[int, int]] = []
    cursor = item_start
    while cursor > 0 and tokens[cursor - 1].text == "]":
        opening = matching_open_bracket(tokens, cursor - 1)
        if opening == 0 or tokens[opening - 1].text != "#":
            break
        ranges.append((opening - 1, cursor - 1))
        cursor = opening - 1
    ranges.reverse()
    return ranges


def cfg_value(expression: tuple[object, ...], profile: CfgProfile) -> bool:
    """Evaluate one cfg expression with test=true and a complete current profile."""

    assignment = assignment_for_expressions([expression], profile)
    value = evaluate_cfg(expression, assignment, test_value=True)
    if value is None:
        fail(f"cfg profile cannot prove expression: {expression}")
    return value


def cfg_meta_enables_test(
    tokens: list[object],
    start: int,
    end: int,
    profile: CfgProfile,
) -> bool:
    """Evaluate direct or cfg_attr-provided test attributes under one profile."""

    if start >= end or tokens[start].kind != "IDENT":
        return False
    name = tokens[start].text
    if name == "test" and start + 1 == end:
        return True
    if (
        name == "tokio"
        and start + 2 < end
        and tokens[start + 1].text == "::"
        and tokens[start + 2].text == "test"
    ):
        return True
    if name != "cfg_attr" or start + 1 >= end or tokens[start + 1].text != "(":
        return False
    arguments = split_top_level_ranges(tokens, start + 2, end - 1)
    if len(arguments) < 2:
        return False
    condition, _ = parse_cfg_expression(tokens, *arguments[0])
    if not cfg_value(condition, profile):
        return False
    return any(
        cfg_meta_enables_test(tokens, nested_start, nested_end, profile)
        for nested_start, nested_end in arguments[1:]
    )


def file_inner_cfg_enabled(tokens: list[object], profile: CfgProfile) -> bool:
    """Evaluate crate-level `#![cfg]` and nested `#![cfg_attr(..., cfg(...))]`."""

    constraints: list[tuple[object, ...]] = []
    index = 0
    while index + 2 < len(tokens):
        if not (
            tokens[index].text == "#"
            and tokens[index + 1].text == "!"
            and tokens[index + 2].text == "["
        ):
            index += 1
            continue
        depth = 1
        bracket_end = index + 3
        while bracket_end < len(tokens) and depth:
            depth += tokens[bracket_end].text == "["
            depth -= tokens[bracket_end].text == "]"
            bracket_end += 1
        if depth:
            fail("behavior evidence inner attribute is unbalanced")
        constraints.extend(parse_cfg_meta(tokens, index + 3, bracket_end - 1))
        index = bracket_end
    if not constraints:
        return True
    conjunction: tuple[object, ...] = ("all", tuple(constraints))
    return cfg_value(conjunction, profile)


def all_cfg_expressions(tokens: list[object]) -> list[tuple[object, ...]]:
    """Collect every outer/inner cfg constraint so the profile assignment is complete."""

    expressions: list[tuple[object, ...]] = []
    index = 0
    while index + 1 < len(tokens):
        if tokens[index].text != "#":
            index += 1
            continue
        inner = index + 2 < len(tokens) and tokens[index + 1].text == "!" and tokens[index + 2].text == "["
        opening = index + 2 if inner else index + 1
        if opening >= len(tokens) or tokens[opening].text != "[":
            index += 1
            continue
        depth = 1
        end = opening + 1
        while end < len(tokens) and depth:
            depth += tokens[end].text == "["
            depth -= tokens[end].text == "]"
            end += 1
        if depth:
            fail("behavior evidence cfg attribute is unbalanced")
        meta_start = opening + 1
        expressions.extend(parse_cfg_meta(tokens, meta_start, end - 1))
        index = end
    return expressions


def straight_line_body_tokens(
    tokens: list[object],
    start: int,
    end: int,
    excluded: set[int],
    function_name: str,
) -> tuple[str, ...]:
    """Validate and return one direct, sequential fallthrough test body."""

    body_indexes = list(range(start, end))
    if not body_indexes:
        fail(f"adapter behavior evidence has an empty body: {function_name}")

    closure_prefixes = {
        None,
        "=",
        "&",
        "(",
        "[",
        "{",
        ",",
        ":",
        ";",
        ">",
        "=>",
        "async",
        "move",
        "return",
    }
    delimiters = {"(", "[", "{"}
    for index in body_indexes:
        text = tokens[index].text
        if (
            text == "#"
            and index + 2 < end
            and tokens[index + 1].text == "["
            and tokens[index + 2].text in {"cfg", "cfg_attr"}
        ):
            fail(
                "adapter behavior evidence straight-line profile forbids "
                f"body-level static cfg: {function_name}"
            )

        forbidden = STRAIGHT_LINE_FORBIDDEN_KEYWORDS.get(text)
        if forbidden is not None:
            fail(
                "adapter behavior evidence straight-line profile forbids "
                f"{forbidden}: {function_name}"
            )
        if (
            text == "fn"
            and index + 1 < end
            and tokens[index + 1].kind == "IDENT"
        ):
            fail(
                "adapter behavior evidence straight-line profile forbids "
                f"local helper function definition: {function_name}"
            )
        if text == "?":
            fail(
                "adapter behavior evidence straight-line profile forbids "
                f"question-mark early return: {function_name}"
            )

        previous = tokens[index - 1].text if index > start else None
        following = tokens[index + 1].text if index + 1 < end else None
        if text in {"&", "|"} and following == text:
            if text == "|" and previous in closure_prefixes:
                construct = "closure syntax"
            else:
                construct = f"short-circuit operator {text}{text}"
            fail(
                "adapter behavior evidence straight-line profile forbids "
                f"{construct}: {function_name}"
            )
        if text == "|" and previous in closure_prefixes:
            fail(
                "adapter behavior evidence straight-line profile forbids "
                f"closure syntax: {function_name}"
            )

        if (
            tokens[index].kind == "IDENT"
            and index + 2 < end
            and tokens[index + 1].text == "!"
            and tokens[index + 2].text in delimiters
        ):
            macro_name = text.removeprefix("r#")
            qualified = index > start and tokens[index - 1].text == "::"
            if macro_name not in APPROVED_ASSERTION_MACROS or qualified:
                fail(
                    "adapter behavior evidence straight-line profile forbids "
                    f"user or qualified function-like macro {macro_name}!: {function_name}"
                )

    if any(index in excluded for index in body_indexes):
        fail(
            "adapter behavior evidence straight-line profile forbids "
            f"cfg-disabled body tokens: {function_name}"
        )
    return tuple(tokens[index].text for index in body_indexes)


def body_has_observable_assertion(body_tokens: tuple[str, ...]) -> bool:
    """Return whether the direct body invokes an approved assertion macro."""

    return any(
        token.removeprefix("r#") in APPROVED_ASSERTION_MACROS
        and index + 1 < len(body_tokens)
        and body_tokens[index + 1] == "!"
        for index, token in enumerate(body_tokens)
    )


def reject_assertion_macro_shadows(tokens: list[object], function_name: str) -> None:
    """Reject file-local definitions that impersonate an approved assertion name."""

    for index, token in enumerate(tokens):
        if token.text.removeprefix("r#") == "macro_use":
            fail(
                "adapter behavior evidence assertion macro provenance is "
                f"ambiguous through macro_use: {function_name}"
            )
        candidate: str | None = None
        if (
            token.text == "macro_rules"
            and index + 2 < len(tokens)
            and tokens[index + 1].text == "!"
            and tokens[index + 2].kind == "IDENT"
        ):
            candidate = tokens[index + 2].text.removeprefix("r#")
        elif (
            token.text == "macro"
            and index + 1 < len(tokens)
            and tokens[index + 1].kind == "IDENT"
        ):
            candidate = tokens[index + 1].text.removeprefix("r#")
        if candidate in APPROVED_ASSERTION_MACROS:
            fail(
                "adapter behavior evidence assertion macro shadow is forbidden: "
                f"{candidate}!: {function_name}"
            )
        if token.text == "use":
            cursor = index + 1
            use_tokens: list[str] = []
            while cursor < len(tokens) and tokens[cursor].text != ";":
                use_tokens.append(tokens[cursor].text)
                if (
                    tokens[cursor].kind == "IDENT"
                    and tokens[cursor].text.removeprefix("r#")
                    in APPROVED_ASSERTION_MACROS
                ):
                    fail(
                        "adapter behavior evidence assertion macro shadow import is "
                        f"forbidden: {tokens[cursor].text}: {function_name}"
                    )
                cursor += 1
            if "*" in use_tokens and use_tokens != ["super", "::", "*"]:
                fail(
                    "adapter behavior evidence assertion macro provenance is "
                    f"ambiguous through glob import: {function_name}"
                )


def ordered_remote_chain_length(body_tokens: tuple[str, ...]) -> int:
    """Return the ordered exact remote call markers present in the direct body."""

    cursor = 0
    for marker_index, marker in enumerate(REMOTE_BEHAVIOR_CHAIN):
        found = False
        while cursor + len(marker) < len(body_tokens):
            if body_tokens[cursor : cursor + len(marker)] == marker:
                call_end = cursor + len(marker)
                if body_tokens[call_end] == "(":
                    cursor = call_end + 1
                    found = True
                    break
            cursor += 1
        if not found:
            return marker_index
    return len(REMOTE_BEHAVIOR_CHAIN)


def extract_behavior_function(
    source: str,
    function_name: str,
    profile: CfgProfile | None = None,
) -> tuple[str, ...]:
    """Return one enabled test body after enforcing the straight-line profile."""

    profile = profile or cfg_profile(str(Path(__file__).resolve().parents[1]), "default")
    tokens = rust_tokens(source)
    function_indexes = [
        index + 1
        for index, token in enumerate(tokens[:-1])
        if token.text == "fn" and tokens[index + 1].text == function_name
    ]
    if len(function_indexes) != 1:
        fail(f"adapter behavior evidence must resolve one function: {function_name}")
    name_index = function_indexes[0]
    fn_index = name_index - 1
    attributes = preceding_attribute_ranges(tokens, fn_index)
    expressions = all_cfg_expressions(tokens)
    assignment = assignment_for_expressions(expressions, profile)
    excluded = cfg_profile_excluded_indexes(
        tokens,
        assignment,
        test_value=True,
    )
    if not file_inner_cfg_enabled(tokens, profile) or fn_index in excluded:
        fail(f"adapter behavior evidence is cfg-disabled: {function_name}")
    if not any(
        cfg_meta_enables_test(tokens, start + 2, end, profile)
        for start, end in attributes
    ):
        fail(f"adapter behavior evidence is not an enabled Rust test: {function_name}")
    for start, end in attributes:
        attribute_tokens = {
            tokens[index].text.removeprefix("r#")
            for index in range(start, end + 1)
            if tokens[index].kind == "IDENT"
        }
        disallowed = attribute_tokens.intersection({"ignore", "should_panic"})
        if disallowed:
            fail(
                "adapter behavior evidence straight-line profile forbids "
                f"test attribute {sorted(disallowed)}: {function_name}"
            )
    reject_assertion_macro_shadows(tokens, function_name)
    opening = name_index + 1
    while opening < len(tokens) and tokens[opening].text != "{":
        opening += 1
    if opening >= len(tokens):
        fail(f"adapter behavior evidence has no body: {function_name}")
    depth = 1
    closing = opening + 1
    while closing < len(tokens) and depth:
        depth += tokens[closing].text == "{"
        depth -= tokens[closing].text == "}"
        closing += 1
    if depth:
        fail(f"adapter behavior evidence has an unbalanced body: {function_name}")
    body_tokens = straight_line_body_tokens(
        tokens,
        opening + 1,
        closing - 1,
        excluded,
        function_name,
    )
    if not body_tokens or not body_has_observable_assertion(body_tokens):
        fail(f"adapter behavior evidence has no observable assertion: {function_name}")
    return body_tokens


def behavior_evidence_body(
    root: Path,
    evidence: str,
    profile: CfgProfile,
    enabled_tests: tuple[str, ...] | None,
) -> tuple[str, ...]:
    """Resolve behavior evidence to one exact integration or source unit-test file."""

    module, separator, function_name = evidence.rpartition("::")
    if not separator or not module or not function_name:
        fail(f"adapter behavior evidence is malformed: {evidence}")
    integration = root / "crates/druid-wrapper/tests" / f"{module}.rs"
    candidates = [integration] if integration.is_file() else [
        path
        for path in (root / "crates/druid-wrapper/src").rglob(f"{module}.rs")
        if path.is_file()
    ]
    if len(candidates) != 1:
        fail(f"adapter behavior evidence file is not exact: {evidence}")
    if enabled_tests is not None:
        matches = [
            name
            for name in enabled_tests
            if name == function_name or name.endswith(f"::{function_name}")
        ]
        if len(matches) != 1:
            fail(
                f"adapter behavior evidence is not uniquely enabled in {profile.name}: "
                f"{evidence}: {matches}"
            )
    return extract_behavior_function(
        candidates[0].read_text(encoding="utf-8"),
        function_name,
        profile,
    )


def has_wrapper_macro(source: str, type_name: str, target_trait: str) -> bool:
    """Return whether the canonical Wrapper macro covers this implementation type."""

    concrete = concrete_type(type_name)
    leaf = target_trait.rsplit("::", 1)[-1]
    return re.search(
        rf"impl_standard_wrapper!\s*\(\s*{re.escape(concrete)}(?:\s*<[^;)]*>)?\s*=>\s*{re.escape(leaf)}\s*\)",
        source,
    ) is not None


def verify_adapter_source(
    root: Path,
    row: AdapterRow,
    *,
    profile: CfgProfile | None = None,
    enabled_tests: tuple[str, ...] | None = None,
) -> None:
    """Require parallel Physical/standard impls and a single execution core."""

    path = root / row.source_path
    if not path.is_file():
        fail(f"adapter source is missing: {row.source_path}")
    source = path.read_text(encoding="utf-8")
    tokens = code_tokens(source)
    current = extract_impl_bodies(source, row.current_trait, row.implementation_type)
    if not current and row.current_trait.rsplit("::", 1)[-1] != "Wrapper":
        fail(f"temporary Physical impl missing: {row.implementation_type} in {row.source_path}")

    if row.state != LIVE_ADAPTER_STATE:
        if row.state != "CURRENT_IMPL_TARGET_PLANNED":
            fail(f"adapter inventory row has unknown state: {row.implementation_type}: {row.state}")
        return

    standard_path = root / row.standard_source_path
    if not standard_path.is_file():
        fail(f"canonical standard source is missing: {row.standard_source_path}")
    profile = profile or cfg_profile(str(root), "default")
    behavior_tokens = behavior_evidence_body(
        root,
        row.behavior_evidence,
        profile,
        enabled_tests,
    )
    if concrete_type(row.implementation_type).startswith(("HttpSql", "LibSql", "JdbcAgent")):
        chain_length = ordered_remote_chain_length(behavior_tokens)
        if (
            not body_has_observable_assertion(behavior_tokens)
            or chain_length != len(REMOTE_BEHAVIOR_CHAIN)
        ):
            missing = [" :: ".join(marker) for marker in REMOTE_BEHAVIOR_CHAIN[chain_length:]]
            fail(
                "remote adapter behavior evidence lacks the direct straight-line "
                "factory-to-result chain plus assertion: "
                f"{row.behavior_evidence}: {missing}"
            )
    if row.standard_implementation_type == "StandardAdapterPreparedStatement":
        standard_source = standard_path.read_text(encoding="utf-8")
        required = (
            "impl PreparedStatement for StandardAdapterPreparedStatement",
            "set_parameter",
            "execute_prepared_query",
            "execute_prepared_update",
            "executor_core",
        )
        if any(marker not in standard_source for marker in required):
            fail(f"common canonical Prepared behavior is incomplete: {row.implementation_type}")
        owner = row.standard_core_evidence.split("::", 1)[0]
        family_sources = list((root / "crates/druid-wrapper/src").rglob("*connection*.rs"))
        owner_source = next(
            (path.read_text(encoding="utf-8") for path in family_sources if owner in path.read_text(encoding="utf-8")),
            None,
        )
        if owner_source is None or any(
            marker not in owner_source
            for marker in (
                f"impl AdapterExecutionCore for {owner}",
                "standard_executor_core",
                "StandardAdapterPreparedStatement::new",
            )
        ):
            fail(f"family prepared executor provenance is missing: {row.implementation_type}")
        return

    target_leaf = row.target_trait.rsplit("::", 1)[-1]
    standard = extract_impl_bodies(source, row.target_trait, row.implementation_type)
    wrapper_only = target_leaf == "Wrapper" and has_wrapper_macro(
        source, row.implementation_type, row.target_trait
    )
    if not standard and not wrapper_only:
        fail(
            f"canonical adapter implements only Physical*: {row.implementation_type} "
            f"missing {row.target_trait}"
        )
    canonical_path = row.target_trait.replace("druid_rdbc::", "druid_rdbc :: ").replace("::", " :: ")
    canonical_import = row.target_trait.rsplit("::", 1)[0].replace("::", " :: ") + f" :: {target_leaf}"
    macro_provenance = (
        "druid_rdbc :: impl_standard_wrapper" in tokens
        and f"=> {target_leaf}" in tokens
    )
    if canonical_path not in tokens and canonical_import not in tokens and not macro_provenance:
        fail(f"standard impl lacks canonical trait provenance: {row.implementation_type}")
    if re.search(rf"\btrait\s+{re.escape(target_leaf)}\b", source):
        fail(f"local same-leaf trait shadows canonical identity: {target_leaf}")
    if re.search(r"#\s*\[\s*cfg\s*\(\s*any\s*\(\s*\)\s*\)\s*\]\s*impl", source):
        fail(f"canonical impl is production-disabled: {row.implementation_type}")
    for body in standard:
        if not code_tokens(body).strip():
            fail(f"canonical impl inherits only empty/default behavior: {row.implementation_type}")
        if "Physical" in body:
            fail(f"standard impl delegates back to Physical: {row.implementation_type}")
        markers = [marker for marker in SQL_EXECUTION_MARKERS if marker in body]
        delegates = "_core(" in body or "actor" in body.lower()
        if markers and not delegates:
            fail(
                f"standard impl duplicates SQL/default/error logic instead of delegating one core: "
                f"{row.implementation_type}: {markers}"
            )
    current_cores = set().union(*(core_calls(body) for body in current)) if current else set()
    standard_cores = set().union(*(core_calls(body) for body in standard)) if standard else set()
    if target_leaf in {"ConnectionFactory", "DatabaseMetaData", "ResultSet"}:
        transitive_factory_core = (
            target_leaf == "ConnectionFactory"
            and "create_adapter_core" in source
            and "create_adapter_core" in standard[0]
        )
        if not current_cores.intersection(standard_cores) and not transitive_factory_core:
            fail(f"standard and Physical impls use split cores: {row.implementation_type}")
    if target_leaf == "Connection":
        required = ("create_statement", "prepare_statement")
        if not standard or any(name not in standard[0] for name in required):
            fail(f"connection lacks required standard handle behavior: {row.implementation_type}")
        family_connections = {
            "RbdcConnectionAdapter",
            "ToastyConnectionAdapter",
            "DuckDbConnectionAdapter",
            "HttpSqlConnectionAdapter",
            "LibSqlConnectionAdapter",
            "JdbcAgentConnection",
        }
        if row.implementation_type == "SqlxConnectionAdapter":
            if "standard_actor_core" not in standard[0]:
                fail("SQLx connection does not create handles from its owned actor core")
        elif concrete_type(row.implementation_type) in family_connections and (
            f"impl AdapterExecutionCore for {concrete_type(row.implementation_type)}" not in source
            or "StandardAdapterStatement::new" not in standard[0]
            or "StandardAdapterPreparedStatement::new" not in standard[0]
        ):
            fail(f"family connection lacks owned executor provenance: {row.implementation_type}")
    required_methods = {
        "PreparedStatement": ("set_parameter", "execute_prepared_query", "execute_prepared_update"),
        "ResultSet": ("next", "value", "close"),
        "DatabaseMetaData": ("database_product_name",),
        "ConnectionFactory": ("create",),
    }.get(target_leaf, ())
    if standard and any(method not in standard[0] for method in required_methods):
        fail(f"canonical impl lacks required behavior: {row.implementation_type}: {required_methods}")
    owner, _, methods = row.standard_core_evidence.partition("::")
    production_sources = "\n".join(
        path.read_text(encoding="utf-8")
        for path in (root / "crates/druid-wrapper/src").rglob("*.rs")
    )
    if owner not in production_sources or any(
        method not in production_sources for method in methods.split("/") if method
    ):
        fail(f"adapter core evidence is not source-bound: {row.standard_core_evidence}")


def verify_consumer_rows(root: Path, path: Path) -> int:
    """Require every Task 4 wrapper consumer row to record its live transition."""

    rows = [
        row
        for row in read_tsv(path)
        if row["target_task"] == TASK
        and row["source_path"].startswith("crates/druid-wrapper/")
    ]
    if len(rows) != 21:
        fail(f"Task 4 wrapper consumer denominator drift: expected 21 rows, found {len(rows)}")
    for row in rows:
        if row["state"] != LIVE_CONSUMER_STATE:
            fail(
                f"Task 4 wrapper consumer is not LIVE: {row['source_path']}: "
                f"{row['symbol']}: {row['state']}"
            )
        if not (root / row["source_path"]).is_file():
            fail(f"Task 4 wrapper consumer source is missing: {row['source_path']}")
        source_tokens = code_tokens((root / row["source_path"]).read_text(encoding="utf-8"))
        old_leaf = row["old_path"].rsplit("::", 1)[-1]
        if re.search(rf"\b{re.escape(old_leaf)}\b", source_tokens):
            fail(f"closed legacy consumer still exists in source: {row['source_path']}: {old_leaf}")
    return len(rows)


def verify_sqlx_ownership(root: Path) -> None:
    """Require owned SQLx Statement/ResultSet/live metadata actor evidence."""

    actor = (root / "crates/druid-wrapper/src/sqlx/sqlx_connection_actor.rs")
    if not actor.is_file():
        fail("SQLx statement-owned executor is missing")
    actor_source = code_tokens(actor.read_text(encoding="utf-8"))
    required_actor = (
        "struct SqlxConnectionActor",
        "Arc < SqlxConnectionActor >",
        "tokio :: sync :: Mutex",
        "Box < dyn ResultSet >",
        "fetch_rows_with_labels",
    )
    missing = [marker for marker in required_actor if marker not in actor_source]
    if missing:
        fail(f"SQLx actor ownership evidence is incomplete: {missing}")

    prepared = code_tokens((root / "crates/druid-wrapper/src/sqlx/sqlx_prepared_statement.rs").read_text(encoding="utf-8"))
    result_set = code_tokens((root / "crates/druid-wrapper/src/sqlx/sqlx_streaming_result_set.rs").read_text(encoding="utf-8"))
    metadata = code_tokens((root / "crates/druid-wrapper/src/sqlx/sqlx_database_meta_data.rs").read_text(encoding="utf-8"))
    if "Arc < SqlxConnectionActor >" not in prepared:
        fail("SQLx Statement/PreparedStatement does not own its executor actor")
    if "Arc < SqlxConnectionActor >" not in result_set:
        fail("SQLx ResultSet does not retain its query actor/lease")
    if "Arc < SqlxConnectionActor >" not in metadata:
        fail("SQLx live DatabaseMetaData does not own its actor")
    if "ResultSetMetaDataSnapshot" not in result_set or not any(
        marker in result_set
        for marker in ("Arc < dyn ResultSetMetaData >", "Arc < dyn StandardResultSetMetaData >")
    ):
        fail("SQLx ResultSet metadata is not an immutable Arc snapshot")
    if "Arc < dyn ParameterMetaData >" not in prepared:
        fail("SQLx parameter metadata is not an immutable Arc snapshot")


def behavior_profile_name(row: AdapterRow) -> str:
    """Select the Task 4 matrix profile that actually enables this family evidence."""

    concrete = concrete_type(row.implementation_type)
    if concrete.startswith(("DuckDb", "HttpSql", "LibSql", "JdbcAgent")):
        return "all-features"
    return "default"


@functools.lru_cache(maxsize=4)
def cargo_test_inventory(root_text: str, profile_name: str) -> tuple[str, ...]:
    """List the exact tests Cargo enables for one Task 4 profile."""

    command = ["cargo", "test", "-p", "druid-wrapper", "--tests"]
    if profile_name == "all-features":
        command.append("--all-features")
    elif profile_name != "default":
        fail(f"unknown Cargo test inventory profile: {profile_name}")
    command.extend(["--", "--list"])
    result = subprocess.run(
        command,
        cwd=Path(root_text),
        check=False,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip()
        fail(f"cannot list {profile_name} Task 4 tests: {detail}")
    tests = tuple(
        line[: -len(": test")]
        for line in result.stdout.splitlines()
        if line.endswith(": test")
    )
    if not tests:
        fail(f"Cargo listed no tests for Task 4 profile {profile_name}")
    return tests


def verify_repository(
    root: Path,
    adapters_path: Path = DEFAULT_ADAPTERS,
    consumers_path: Path = DEFAULT_CONSUMERS,
) -> tuple[int, int]:
    """Verify all live Task 4 adapter and ownership obligations."""

    adapters = task_adapter_rows(root / adapters_path)
    for row in adapters:
        profile_name = behavior_profile_name(row)
        verify_adapter_source(
            root,
            row,
            profile=cfg_profile(str(root), profile_name),
            enabled_tests=cargo_test_inventory(str(root), profile_name),
        )
    consumers = verify_consumer_rows(root, root / consumers_path)
    verify_sqlx_ownership(root)
    return sum(row.state == LIVE_ADAPTER_STATE for row in adapters), consumers


def main() -> int:
    """CLI entry point."""

    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    arguments = parser.parse_args()
    try:
        adapter_count, consumer_count = verify_repository(arguments.root.resolve())
    except (OSError, UnicodeError, VerificationError) as error:
        print(f"WRAPPER_STANDARD_IMPLS_FAIL={error}")
        return 1
    print(f"WRAPPER_STANDARD_ADAPTERS_LIVE={adapter_count}")
    print(f"WRAPPER_STANDARD_CONSUMERS_CLOSED={consumer_count}")
    print("FAMILY_COMMON_PREPARED_CHAINS_LIVE=6")
    print("SQLX_OWNED_ACTOR_LIVE=1")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
