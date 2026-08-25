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
MAX_BEHAVIOR_PATHS = 64
APPROVED_ASSERTION_MACROS = {
    "assert",
    "assert_eq",
    "assert_impl_all",
    "assert_matches",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "matches",
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


@dataclass(frozen=True)
class BehaviorPathSummary:
    """One ordered feasible token path through a behavior-evidence test body."""

    tokens: tuple[str, ...] = ()
    flow: str = "fallthrough"


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


def bounded_behavior_paths(
    paths: list[BehaviorPathSummary],
) -> list[BehaviorPathSummary]:
    """Deduplicate path summaries and reject analysis that exceeds the fixed budget."""

    unique = list(dict.fromkeys(paths))
    if len(unique) > MAX_BEHAVIOR_PATHS:
        fail(
            "behavior evidence path explosion exceeds "
            f"{MAX_BEHAVIOR_PATHS} candidate paths"
        )
    return unique


def append_behavior_paths(
    prefixes: list[BehaviorPathSummary],
    alternatives: list[BehaviorPathSummary],
) -> list[BehaviorPathSummary]:
    """Append alternatives only to prefixes that can still execute the suffix."""

    combined: list[BehaviorPathSummary] = []
    for prefix in prefixes:
        if prefix.flow != "fallthrough":
            combined.append(prefix)
            continue
        combined.extend(
            BehaviorPathSummary(prefix.tokens + alternative.tokens, alternative.flow)
            for alternative in alternatives
        )
    return bounded_behavior_paths(combined)


def behavior_path_summaries(
    tokens: list[object],
    start: int,
    end: int,
    excluded: set[int],
    *,
    compile_backed: bool,
) -> tuple[BehaviorPathSummary, ...]:
    """Build bounded ordered paths without interpreting arbitrary Rust expressions."""

    delimiter_pairs = {"{": "}", "(": ")", "[": "]"}

    def balanced_end(opening: int, limit: int) -> int:
        """Return the exclusive end of one balanced Rust delimiter group."""

        opener = tokens[opening].text
        if opener not in delimiter_pairs:
            fail(f"behavior evidence expected a delimiter at token {opening}")
        closer = delimiter_pairs[opener]
        stack = [closer]
        cursor = opening + 1
        while cursor < limit and stack:
            text = tokens[cursor].text
            if text in delimiter_pairs:
                stack.append(delimiter_pairs[text])
            elif text in delimiter_pairs.values():
                if text != stack[-1]:
                    fail("behavior evidence contains mismatched delimiters")
                stack.pop()
            cursor += 1
        if stack:
            fail("behavior evidence contains an unbalanced delimiter group")
        return cursor

    def control_body_opening(control_index: int, limit: int) -> int:
        """Find the outer body of an if/loop/match with simple Rust conditions."""

        paren = bracket = 0
        cursor = control_index + 1
        while cursor < limit:
            text = tokens[cursor].text
            if text == "(":
                paren += 1
            elif text == ")":
                paren -= 1
            elif text == "[":
                bracket += 1
            elif text == "]":
                bracket -= 1
            elif text == "{" and paren == bracket == 0:
                return cursor
            if paren < 0 or bracket < 0:
                fail("behavior evidence control expression is unbalanced")
            cursor += 1
        fail("behavior evidence control expression has no body")

    def literal_condition(condition_start: int, condition_end: int) -> bool | None:
        """Recognize only a direct true/false literal, optionally parenthesized."""

        values = [
            tokens[index].text
            for index in range(condition_start, condition_end)
            if index not in excluded
        ]
        while len(values) >= 2 and values[0] == "(" and values[-1] == ")":
            depth = 0
            encloses_all = True
            for index, value in enumerate(values):
                depth += value == "("
                depth -= value == ")"
                if depth == 0 and index != len(values) - 1:
                    encloses_all = False
                    break
            if not encloses_all:
                break
            values = values[1:-1]
        if values == ["true"]:
            return True
        if values == ["false"]:
            return False
        return None

    def append_token_values(
        paths: list[BehaviorPathSummary],
        values: tuple[str, ...],
    ) -> list[BehaviorPathSummary]:
        """Append ordinary executed tokens while preserving terminated paths."""

        return bounded_behavior_paths(
            [
                BehaviorPathSummary(path.tokens + values, path.flow)
                if path.flow == "fallthrough"
                else path
                for path in paths
            ]
        )

    def macro_definition_end(index: int, limit: int) -> int | None:
        """Return the end of a macro_rules/macro-2 definition, never its expansion."""

        text = tokens[index].text
        if text == "macro_rules":
            if index + 2 >= limit or tokens[index + 1].text != "!":
                fail("behavior evidence macro_rules definition is malformed")
            cursor = index + 3
        elif text == "macro":
            if index + 1 >= limit or tokens[index + 1].kind != "IDENT":
                return None
            cursor = index + 2
        else:
            return None
        while cursor < limit and tokens[cursor].text not in delimiter_pairs:
            cursor += 1
        if cursor >= limit:
            fail("behavior evidence macro definition has no body")
        if text == "macro" and tokens[cursor].text in {"(", "["}:
            cursor = balanced_end(cursor, limit)
            while cursor < limit and tokens[cursor].text != "{":
                cursor += 1
            if cursor >= limit:
                fail("behavior evidence macro-2 definition has no expansion body")
        definition_end = balanced_end(cursor, limit)
        if definition_end < limit and tokens[definition_end].text == ";":
            definition_end += 1
        return definition_end

    def nested_function_end(index: int, limit: int) -> int | None:
        """Return the end of one nested named function item, excluding its body."""

        if (
            tokens[index].text != "fn"
            or index + 1 >= limit
            or tokens[index + 1].kind != "IDENT"
        ):
            return None
        paren = bracket = 0
        cursor = index + 2
        while cursor < limit:
            text = tokens[cursor].text
            if text == "(":
                paren += 1
            elif text == ")":
                paren -= 1
            elif text == "[":
                bracket += 1
            elif text == "]":
                bracket -= 1
            elif text == "{" and paren == bracket == 0:
                return balanced_end(cursor, limit)
            elif text == ";" and paren == bracket == 0:
                return cursor + 1
            cursor += 1
        fail("behavior evidence nested function item has no complete body")

    def previous_live_text(index: int, lower_bound: int) -> str | None:
        """Return the previous non-cfg-disabled token in the current range."""

        cursor = index - 1
        while cursor >= lower_bound:
            if cursor not in excluded:
                return tokens[cursor].text
            cursor -= 1
        return None

    def closure_end(index: int, limit: int, lower_bound: int) -> int | None:
        """Return an unexpanded closure body's end when `|` starts a closure."""

        if tokens[index].text != "|" or previous_live_text(index, lower_bound) not in {
            None,
            "=",
            "(",
            ",",
            ";",
            "{",
            ">",
            "async",
            "move",
            "return",
        }:
            return None
        cursor = index + 1
        paren = bracket = 0
        while cursor < limit:
            text = tokens[cursor].text
            if text == "(" and bracket == 0:
                paren += 1
            elif text == ")" and bracket == 0:
                paren -= 1
            elif text == "[":
                bracket += 1
            elif text == "]":
                bracket -= 1
            elif text == "|" and paren == bracket == 0:
                break
            cursor += 1
        if cursor >= limit:
            fail("behavior evidence closure parameters are unbalanced")
        body_start = cursor + 1
        if body_start < limit and tokens[body_start].text == "{":
            return balanced_end(body_start, limit)
        paren = bracket = brace = 0
        cursor = body_start
        while cursor < limit:
            text = tokens[cursor].text
            if text == "(":
                paren += 1
            elif text == ")":
                if paren == 0 and bracket == brace == 0:
                    return cursor
                paren -= 1
            elif text == "[":
                bracket += 1
            elif text == "]":
                bracket -= 1
            elif text == "{":
                brace += 1
            elif text == "}":
                if brace == 0 and paren == bracket == 0:
                    return cursor
                brace -= 1
            elif text in {",", ";"} and paren == bracket == brace == 0:
                return cursor
            cursor += 1
        return cursor

    def parse_if(index: int, limit: int, loop_depth: int) -> tuple[list[BehaviorPathSummary], int]:
        """Fork runtime branches and select only a proven literal branch."""

        opening = control_body_opening(index, limit)
        then_end = balanced_end(opening, limit)
        then_paths = parse_sequence(opening + 1, then_end - 1, loop_depth)
        expression_end = then_end
        else_paths: list[BehaviorPathSummary] = [BehaviorPathSummary()]
        if then_end < limit and tokens[then_end].text == "else":
            branch_start = then_end + 1
            if branch_start >= limit:
                fail("behavior evidence else branch has no body")
            if tokens[branch_start].text == "{":
                expression_end = balanced_end(branch_start, limit)
                else_paths = parse_sequence(
                    branch_start + 1,
                    expression_end - 1,
                    loop_depth,
                )
            elif tokens[branch_start].text == "if":
                else_paths, expression_end = parse_if(branch_start, limit, loop_depth)
            else:
                fail("behavior evidence else branch is not a block or else-if")
        literal = literal_condition(index + 1, opening)
        if literal is True:
            return then_paths, expression_end
        if literal is False:
            return else_paths, expression_end
        return bounded_behavior_paths(then_paths + else_paths), expression_end

    def parse_match(index: int, limit: int, loop_depth: int) -> tuple[list[BehaviorPathSummary], int]:
        """Fork match arms without combining mutually exclusive arm bodies."""

        opening = control_body_opening(index, limit)
        match_end = balanced_end(opening, limit)
        alternatives: list[BehaviorPathSummary] = []
        cursor = opening + 1
        while cursor < match_end - 1:
            while cursor < match_end - 1 and (
                cursor in excluded or tokens[cursor].text == ","
            ):
                cursor += 1
            if cursor >= match_end - 1:
                break
            paren = bracket = brace = 0
            arrow = cursor
            while arrow < match_end - 1:
                text = tokens[arrow].text
                if (
                    text == "="
                    and arrow + 1 < match_end - 1
                    and tokens[arrow + 1].text == ">"
                    and paren == bracket == brace == 0
                ):
                    break
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
                arrow += 1
            if arrow >= match_end - 1:
                fail("behavior evidence match arm lacks an arrow")
            expression_start = arrow + 2
            if expression_start >= match_end - 1:
                fail("behavior evidence match arm lacks an expression")
            if tokens[expression_start].text == "{":
                expression_end = balanced_end(expression_start, match_end)
                arm_paths = parse_sequence(
                    expression_start + 1,
                    expression_end - 1,
                    loop_depth,
                )
                cursor = expression_end
            else:
                expression_end = expression_start
                paren = bracket = brace = 0
                while expression_end < match_end - 1:
                    text = tokens[expression_end].text
                    if text == "," and paren == bracket == brace == 0:
                        break
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
                    expression_end += 1
                arm_paths = parse_sequence(
                    expression_start,
                    expression_end,
                    loop_depth,
                )
                cursor = expression_end
            alternatives.extend(arm_paths)
        if not alternatives:
            fail("behavior evidence match has no analyzable arm")
        return bounded_behavior_paths(alternatives), match_end

    def loop_alternatives(
        body_paths: list[BehaviorPathSummary],
    ) -> list[BehaviorPathSummary]:
        """Only an explicit break proves that an unconditional loop reaches its suffix."""

        alternatives: list[BehaviorPathSummary] = []
        for path in body_paths:
            if path.flow == "break":
                alternatives.append(BehaviorPathSummary(path.tokens, "fallthrough"))
            elif path.flow == "return":
                alternatives.append(path)
            else:
                alternatives.append(BehaviorPathSummary(path.tokens, "diverge"))
        return bounded_behavior_paths(alternatives)

    def parse_sequence(
        range_start: int,
        range_end: int,
        loop_depth: int,
    ) -> list[BehaviorPathSummary]:
        """Summarize one sequential token range and recursively compose its controls."""

        paths = [BehaviorPathSummary()]
        index = range_start
        while index < range_end:
            if index in excluded:
                index += 1
                continue
            text = tokens[index].text

            definition_end = macro_definition_end(index, range_end)
            if definition_end is not None:
                index = definition_end
                continue

            function_end = nested_function_end(index, range_end)
            if function_end is not None:
                index = function_end
                continue

            deferred_closure_end = closure_end(index, range_end, range_start)
            if deferred_closure_end is not None:
                index = deferred_closure_end
                continue

            if text == "async":
                opening = index + 1
                if opening < range_end and tokens[opening].text == "move":
                    opening += 1
                if opening < range_end and tokens[opening].text == "{":
                    async_end = balanced_end(opening, range_end)
                    awaited = (
                        async_end + 1 < range_end
                        and tokens[async_end].text == "."
                        and tokens[async_end + 1].text == "await"
                    )
                    if awaited:
                        paths = append_behavior_paths(
                            paths,
                            parse_sequence(opening + 1, async_end - 1, loop_depth),
                        )
                        index = async_end + 2
                    else:
                        index = async_end
                    continue

            if text == "if":
                alternatives, expression_end = parse_if(index, range_end, loop_depth)
                paths = append_behavior_paths(paths, alternatives)
                index = expression_end
                continue

            if text == "match":
                alternatives, expression_end = parse_match(index, range_end, loop_depth)
                paths = append_behavior_paths(paths, alternatives)
                index = expression_end
                continue

            if text in {"while", "for", "loop"}:
                opening = control_body_opening(index, range_end)
                body_end = balanced_end(opening, range_end)
                if text == "while":
                    literal = literal_condition(index + 1, opening)
                    if literal is False:
                        alternatives = [BehaviorPathSummary()]
                    elif literal is True:
                        alternatives = loop_alternatives(
                            parse_sequence(opening + 1, body_end - 1, loop_depth + 1)
                        )
                    else:
                        # A runtime while can execute zero times; its body is not mandatory evidence.
                        alternatives = [BehaviorPathSummary()]
                elif text == "for":
                    # A for iterator can be empty; do not use its body as execution evidence.
                    alternatives = [BehaviorPathSummary()]
                else:
                    alternatives = loop_alternatives(
                        parse_sequence(opening + 1, body_end - 1, loop_depth + 1)
                    )
                paths = append_behavior_paths(paths, alternatives)
                index = body_end
                continue

            if (
                tokens[index].kind == "IDENT"
                and index + 2 < range_end
                and tokens[index + 1].text == "!"
                and tokens[index + 2].text in delimiter_pairs
            ):
                opening = index + 2
                invocation_end = balanced_end(opening, range_end)
                macro_name = tokens[index].text.removeprefix("r#")
                if macro_name in APPROVED_ASSERTION_MACROS or compile_backed:
                    paths = append_token_values(paths, (macro_name, "!"))
                    paths = append_behavior_paths(
                        paths,
                        parse_sequence(opening + 1, invocation_end - 1, loop_depth),
                    )
                index = invocation_end
                continue

            if text == "{":
                block_end = balanced_end(index, range_end)
                paths = append_behavior_paths(
                    paths,
                    parse_sequence(index + 1, block_end - 1, loop_depth),
                )
                index = block_end
                continue

            if text == "return":
                paths = [
                    BehaviorPathSummary(path.tokens + ("return",), "return")
                    if path.flow == "fallthrough"
                    else path
                    for path in paths
                ]
                while index < range_end and tokens[index].text != ";":
                    index += 1
                if index < range_end:
                    index += 1
                continue

            if text in {"break", "continue"}:
                if loop_depth == 0:
                    fail(f"behavior evidence has {text} outside an analyzed loop")
                paths = [
                    BehaviorPathSummary(path.tokens + (text,), text)
                    if path.flow == "fallthrough"
                    else path
                    for path in paths
                ]
                while index < range_end and tokens[index].text != ";":
                    index += 1
                if index < range_end:
                    index += 1
                continue

            if text == "else":
                fail("behavior evidence contains an unsupported standalone else")

            paths = append_token_values(paths, (text,))
            index += 1
        return bounded_behavior_paths(paths)

    summaries = parse_sequence(start, end, 0)
    if any(path.flow in {"break", "continue"} for path in summaries):
        fail("behavior evidence leaves an unresolved loop control path")
    feasible = tuple(path for path in summaries if path.flow != "diverge")
    if not feasible:
        fail("behavior evidence has no terminating feasible path")
    return feasible


def path_has_observable_assertion(path: BehaviorPathSummary) -> bool:
    """Return whether one path invokes an approved assertion macro directly."""

    return any(
        token in APPROVED_ASSERTION_MACROS
        and index + 1 < len(path.tokens)
        and path.tokens[index + 1] == "!"
        for index, token in enumerate(path.tokens)
    )


def ordered_remote_chain_length(path: BehaviorPathSummary) -> int:
    """Return the number of ordered exact call markers satisfied by one path."""

    cursor = 0
    for marker_index, marker in enumerate(REMOTE_BEHAVIOR_CHAIN):
        found = False
        while cursor + len(marker) < len(path.tokens):
            if path.tokens[cursor : cursor + len(marker)] == marker:
                call_end = cursor + len(marker)
                if path.tokens[call_end] == "(":
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
    *,
    compile_backed: bool = False,
) -> tuple[BehaviorPathSummary, ...]:
    """Return enabled ordered test-body paths for one exact cfg profile."""

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
    paths = behavior_path_summaries(
        tokens,
        opening + 1,
        closing - 1,
        excluded,
        compile_backed=compile_backed,
    )
    if not any(path.tokens for path in paths) or not any(
        path_has_observable_assertion(path) for path in paths
    ):
        fail(f"adapter behavior evidence has no observable assertion: {function_name}")
    return paths


def behavior_evidence_body(
    root: Path,
    evidence: str,
    profile: CfgProfile,
    enabled_tests: tuple[str, ...] | None,
) -> tuple[BehaviorPathSummary, ...]:
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
        compile_backed=enabled_tests is not None,
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
    behavior_paths = behavior_evidence_body(
        root,
        row.behavior_evidence,
        profile,
        enabled_tests,
    )
    if concrete_type(row.implementation_type).startswith(("HttpSql", "LibSql", "JdbcAgent")):
        complete_paths = [
            path
            for path in behavior_paths
            if path_has_observable_assertion(path)
            and ordered_remote_chain_length(path) == len(REMOTE_BEHAVIOR_CHAIN)
        ]
        if not complete_paths:
            best_length = max(
                (ordered_remote_chain_length(path) for path in behavior_paths),
                default=0,
            )
            missing = [" :: ".join(marker) for marker in REMOTE_BEHAVIOR_CHAIN[best_length:]]
            fail(
                "remote adapter behavior evidence lacks one ordered path-coherent "
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
