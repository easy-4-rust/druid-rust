#!/usr/bin/env python3
"""验证 Task 6 retired trait 的精确方法分母与 canonical target。

本脚本只信任固定 BASE 的 Rust 源码、当前 checkout 的真实 target source，以及 checked TSV。
它不会把类型级 disposition、文件存在或测试名称当成方法语义迁移证明。
"""

from __future__ import annotations

import argparse
import csv
import functools
import hashlib
import io
import re
import subprocess
import sys
import tarfile
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Iterable, Mapping, Sequence


TASK6_SOURCE_BASE = "5afe8609b177544ff7d18461aa84f69efe8ee90e"
DEFAULT_MATRIX = Path("docs/verification/rdbc-first/task6-method-migration.tsv")
DEFAULT_METHOD_CALL_INDEX = Path(
    "docs/verification/rdbc-first/task6-deleted-test-method-call-index.tsv"
)
FALLBACK_EVIDENCE_PATH = (
    "crates/druid-rdbc/tests/task6_method_default_evidence_test.rs"
)
INTERNALIZED_EVIDENCE_PATH = (
    "crates/druid-wrapper/tests/task6_internalized_method_behavior_test.rs"
)
FALLBACK_EVIDENCE_PATHS = frozenset(
    {FALLBACK_EVIDENCE_PATH, INTERNALIZED_EVIDENCE_PATH}
)
FALLBACK_EVIDENCE_FUNCTION_PREFIX = "task6_method_behavior_"
TRAIT_SPECS: dict[str, str] = {
    "ConnectionExt": "crates/druid/src/core/connection_ext.rs",
    "PhysicalConnection": "crates/druid-rdbc/src/spi/physical_connection.rs",
    "PhysicalStatement": "crates/druid-rdbc/src/spi/physical_statement.rs",
    "PhysicalPreparedStatement": "crates/druid-rdbc/src/spi/physical_prepared_statement.rs",
    "PhysicalCallableStatement": "crates/druid-rdbc/src/spi/physical_callable_statement.rs",
    "PhysicalResultSet": "crates/druid-rdbc/src/spi/rdbc_result_set.rs",
    "PhysicalDatabaseMetaData": "crates/druid-rdbc/src/spi/physical_database_meta_data.rs",
    "PhysicalResultSetMetaData": "crates/druid-rdbc/src/spi/physical_result_set_meta_data.rs",
}
ALLOWED_DISPOSITIONS = {
    "MERGED_EXACT",
    "MERGED_TYPED",
    "INTERNALIZED",
    "TASK7_PENDING",
    "APPROVED_BREAK",
}
METHOD_HEADER = (
    "source_base",
    "source_object",
    "source_trait_path",
    "method",
    "source_signature",
    "source_signature_sha256",
    "base_default_kind",
    "base_override_paths",
    "base_call_paths",
    "disposition",
    "target_entry",
    "target_path",
    "target_symbol",
    "conversion",
    "approval_ref",
    "behavior_evidence",
    "notes",
    "target_trait",
    "target_signature",
    "target_signature_sha256",
    "target_ufcs",
)
METHOD_CALL_INDEX_HEADER = (
    "base_identity",
    "method_call",
    "base_call_count",
    "replacement_selector",
    "disposition",
    "behavioral_theme",
)
INTEGRATION_EVIDENCE_SELECTOR = re.compile(
    r"^(crates/[a-z0-9_-]+/tests/[a-zA-Z0-9_./-]+\.rs)"
    r"#([A-Za-z_][A-Za-z0-9_]*)(?:::([A-Za-z0-9_]+))?$"
)
UNIT_EVIDENCE_SELECTOR = re.compile(
    r"^(crates/[a-z0-9_-]+/src/[a-z0-9_./-]+\.rs)"
    r"#((?:[A-Za-z_][A-Za-z0-9_]*::)+[A-Za-z_][A-Za-z0-9_]*)$"
)
_METHOD_CALL_LOOKUP_CACHE: dict[
    int, tuple[Sequence[MethodCallEvidence], dict[str, tuple[BehaviorSelector, ...]]]
] = {}


class VerificationError(RuntimeError):
    """方法迁移证据不闭合。"""


@dataclass(frozen=True, order=True)
class ParsedMethod:
    """从一个 trait body tokenized 得到的 direct method。"""

    name: str
    signature: str
    signature_sha256: str
    default_kind: str


@dataclass(frozen=True, order=True)
class MethodRow:
    """checked TSV 的一条 BASE method identity disposition。"""

    source_base: str
    source_object: str
    source_trait_path: str
    method: str
    source_signature: str
    source_signature_sha256: str
    base_default_kind: str
    base_override_paths: str
    base_call_paths: str
    disposition: str
    target_entry: str
    target_path: str
    target_symbol: str
    conversion: str
    approval_ref: str
    behavior_evidence: str
    notes: str
    target_trait: str
    target_signature: str
    target_signature_sha256: str
    target_ufcs: str


@dataclass(frozen=True, order=True)
class MethodCallEvidence:
    """C2 BASE method-call reverse index row。"""

    base_identity: str
    method_call: str
    base_call_count: int
    replacement_selector: str
    disposition: str
    behavioral_theme: str


@dataclass(frozen=True, order=True)
class BehaviorSelector:
    """一个精确 current test 或 parameterized case selector。"""

    source_path: str
    function: str
    case_id: str | None = None

    def render(self) -> str:
        base = f"{self.source_path}#{self.function}"
        return base if self.case_id is None else f"{base}::{self.case_id}"


def sha256(payload: bytes) -> str:
    """返回 lowercase SHA-256。"""

    return hashlib.sha256(payload).hexdigest()


@functools.lru_cache(maxsize=1_024)
def _read_utf8(path: Path) -> str:
    """在单次 verifier/render 进程内缓存 immutable checkout source。"""

    return path.read_text(encoding="utf-8")


@functools.lru_cache(maxsize=1_024)
def _compiled_rust_source(path: Path) -> str:
    """展开 literal `include!("...")`，得到该 test target 的编译源码。"""

    source = _read_utf8(path)
    expanded = [source]
    for include_path in re.findall(r'\binclude!\s*\(\s*"([^"]+)"\s*\)', source):
        target = (path.parent / include_path).resolve()
        if not target.is_file():
            raise VerificationError(f"Rust include path does not exist: {target}")
        expanded.append(_compiled_rust_source(target))
    return "\n".join(expanded)


def _mask_rust(source: str) -> str:
    """遮蔽 comments/string/char 内容，同时保留长度、换行与结构符号。"""

    output = list(source)
    index = 0
    length = len(source)
    while index < length:
        if source.startswith("//", index):
            end = source.find("\n", index + 2)
            if end < 0:
                end = length
            for offset in range(index, end):
                output[offset] = " "
            index = end
            continue
        if source.startswith("/*", index):
            depth = 1
            end = index + 2
            while end < length and depth:
                if source.startswith("/*", end):
                    depth += 1
                    end += 2
                elif source.startswith("*/", end):
                    depth -= 1
                    end += 2
                else:
                    end += 1
            for offset in range(index, min(end, length)):
                if output[offset] != "\n":
                    output[offset] = " "
            index = end
            continue
        if source[index] == '"':
            end = index + 1
            while end < length:
                if source[end] == "\\":
                    end += 2
                    continue
                if source[end] == '"':
                    end += 1
                    break
                end += 1
            for offset in range(index, min(end, length)):
                if output[offset] != "\n":
                    output[offset] = " "
            index = end
            continue
        if source[index] == "'":
            # Lifetimes (`'a`) are not char literals. Only mask a closed char token.
            end = index + 1
            if end < length and source[end] == "\\":
                end += 2
            else:
                end += 1
            if end < length and source[end] == "'":
                end += 1
                for offset in range(index, end):
                    output[offset] = " "
                index = end
                continue
        index += 1
    return "".join(output)


def _matching_brace(masked: str, opening: int) -> int:
    if opening >= len(masked) or masked[opening] != "{":
        raise VerificationError("internal parser expected an opening brace")
    depth = 1
    for index in range(opening + 1, len(masked)):
        if masked[index] == "{":
            depth += 1
        elif masked[index] == "}":
            depth -= 1
            if depth == 0:
                return index
    raise VerificationError("unbalanced Rust braces")


def _named_block(source: str, pattern: str) -> tuple[int, int]:
    masked = _mask_rust(source)
    match = re.search(pattern, masked, flags=re.MULTILINE | re.DOTALL)
    if match is None:
        raise VerificationError(f"Rust block not found: {pattern}")
    opening = masked.find("{", match.start(), match.end())
    if opening < 0:
        opening = masked.find("{", match.end())
    if opening < 0:
        raise VerificationError(f"Rust block has no opening brace: {pattern}")
    return opening, _matching_brace(masked, opening)


def _normalize_signature(signature: str) -> str:
    tokens = re.findall(
        r"'[A-Za-z_][A-Za-z0-9_]*|[A-Za-z_][A-Za-z0-9_]*|::|->|=>|\.\.|[^\s]",
        signature,
    )
    output = ""
    previous = ""
    for token in tokens:
        word = token[0].isalnum() or token[0] == "_" or token.startswith("'")
        previous_word = bool(previous) and (
            previous[0].isalnum() or previous[0] == "_" or previous.startswith("'")
        )
        if output and word and previous_word:
            output += " "
        output += token
        previous = token
    return output.rstrip(";")


def _classify_default(body: str | None) -> str:
    if body is None:
        return "REQUIRED"
    masked = _mask_rust(body)
    compact = re.sub(r"\s+", "", masked).strip("{}")
    if re.search(
        r"UnsupportedOperation|feature_not_supported|unsupported_operation|JAVA_DEFAULT_UNSUPPORTED",
        body,
        flags=re.IGNORECASE,
    ):
        return "DEFAULT_UNSUPPORTED"
    if compact in {"", "Ok(())", "()"}:
        return "DEFAULT_NOOP"
    if re.fullmatch(
        r"(?:true|false|-?\d+|None|Some\([^{}]*\)|Ok\([^{}]*\)|[A-Za-z0-9_:]+::default\(\)|Vec::new\(\)|\"\")",
        compact,
    ):
        return "DEFAULT_CONSTANT"
    return "DEFAULT_IMPLEMENTED"


def _methods_in_body(source: str, opening: int, closing: int) -> tuple[ParsedMethod, ...]:
    masked = _mask_rust(source)
    body_start = opening + 1
    depth = 0
    index = body_start
    methods: list[ParsedMethod] = []
    while index < closing:
        character = masked[index]
        if character == "{":
            depth += 1
            index += 1
            continue
        if character == "}":
            depth -= 1
            index += 1
            continue
        if depth != 0:
            index += 1
            continue
        match = re.match(r"fn\s+([A-Za-z_][A-Za-z0-9_]*)", masked[index:])
        if match is None:
            index += 1
            continue
        name = match.group(1)
        signature_start = index
        prefix = masked[max(body_start, index - 24) : index]
        async_match = re.search(r"\basync\s+$", prefix)
        if async_match is not None:
            signature_start = max(body_start, index - 24) + async_match.start()
        cursor = index + match.end()
        paren = bracket = angle = 0
        terminator = None
        while cursor < closing:
            token = masked[cursor]
            if token == "(":
                paren += 1
            elif token == ")":
                paren -= 1
            elif token == "[":
                bracket += 1
            elif token == "]":
                bracket -= 1
            elif token == "<":
                angle += 1
            elif token == ">" and angle > 0:
                angle -= 1
            elif token in ";{" and paren == 0 and bracket == 0 and angle == 0:
                terminator = token
                break
            cursor += 1
        if terminator is None:
            raise VerificationError(f"method signature is unterminated: {name}")
        signature = _normalize_signature(source[signature_start:cursor])
        default_body = None
        if terminator == "{":
            method_close = _matching_brace(masked, cursor)
            default_body = source[cursor : method_close + 1]
            index = method_close + 1
        else:
            index = cursor + 1
        methods.append(
            ParsedMethod(
                name=name,
                signature=signature,
                signature_sha256=sha256(signature.encode("utf-8")),
                default_kind=_classify_default(default_body),
            )
        )
    duplicate_names = [name for name, count in Counter(m.name for m in methods).items() if count > 1]
    if duplicate_names:
        raise VerificationError(f"duplicate Rust trait methods: {duplicate_names}")
    return tuple(methods)


def parse_direct_trait_methods(source: str, trait_name: str) -> tuple[ParsedMethod, ...]:
    """解析一个命名 trait 的 direct methods；忽略同文件 impl/inherent methods。"""

    opening, closing = _named_block(
        source,
        rf"\b(?:pub(?:\([^)]*\))?\s+)?trait\s+{re.escape(trait_name)}\b[^{{]*{{",
    )
    return _methods_in_body(source, opening, closing)


def _impl_method_paths(
    base_sources: Mapping[str, str], trait_name: str, method_names: set[str]
) -> dict[str, set[str]]:
    result: dict[str, set[str]] = defaultdict(set)
    pattern = re.compile(
        rf"\bimpl\b[^{{;]*\b{re.escape(trait_name)}\b[^{{;]*\bfor\b[^{{;]*{{",
        flags=re.MULTILINE | re.DOTALL,
    )
    for path, source in base_sources.items():
        masked = _mask_rust(source)
        for match in pattern.finditer(masked):
            opening = masked.find("{", match.start(), match.end())
            if opening < 0:
                continue
            closing = _matching_brace(masked, opening)
            for method in _methods_in_body(source, opening, closing):
                if method.name in method_names:
                    result[method.name].add(path)
    return result


def _call_paths(
    base_sources: Mapping[str, str], trait_name: str, method_names: set[str]
) -> dict[str, set[str]]:
    result: dict[str, set[str]] = defaultdict(set)
    for path, source in base_sources.items():
        if trait_name not in source:
            continue
        masked = _mask_rust(source)
        for method in method_names:
            if re.search(
                rf"(?:\.\s*{re.escape(method)}\s*\(|\b{re.escape(trait_name)}\s*::\s*{re.escape(method)}\s*\()",
                masked,
            ):
                result[method].add(path)
    return result


def _paths_text(paths: Iterable[str]) -> str:
    ordered = sorted(set(paths))
    return ";".join(ordered) if ordered else "-"


def _target_symbol_exists(source: str, target_symbol: str) -> bool:
    if "::" not in target_symbol:
        return False
    owner, symbol = target_symbol.rsplit("::", 1)
    masked = _mask_rust(source)
    if owner[:1].islower() and re.search(rf"\bfn\s+{re.escape(symbol)}\b", masked):
        return True
    owner_pattern = re.compile(
        rf"\b(?:trait|struct|enum)\s+{re.escape(owner)}\b|\bimpl\b[^{{;]*\b{re.escape(owner)}\b[^{{;]*{{",
        flags=re.MULTILINE | re.DOTALL,
    )
    if owner_pattern.search(masked) is None:
        return False
    if re.search(rf"\bfn\s+{re.escape(symbol)}\b", masked):
        return True
    try:
        opening, closing = _named_block(
            source,
            rf"\b(?:pub(?:\([^)]*\))?\s+)?enum\s+{re.escape(owner)}\b[^{{]*{{",
        )
    except VerificationError:
        return False
    return re.search(rf"\b{re.escape(symbol)}\b", masked[opening + 1 : closing]) is not None


def _parse_free_function(source: str, function_name: str) -> ParsedMethod:
    """解析模块级命名函数的精确签名。"""

    masked = _mask_rust(source)
    matches = list(
        re.finditer(rf"\bfn\s+{re.escape(function_name)}\s*\(", masked)
    )
    if len(matches) != 1:
        raise VerificationError(
            f"target free function is missing or ambiguous: {function_name}"
        )
    match = matches[0]
    signature_start = match.start()
    prefix = masked[max(0, signature_start - 24) : signature_start]
    async_match = re.search(r"\basync\s+$", prefix)
    if async_match is not None:
        signature_start = max(0, signature_start - 24) + async_match.start()
    cursor = match.end()
    paren = 1
    bracket = angle = 0
    while cursor < len(masked):
        token = masked[cursor]
        if token == "(":
            paren += 1
        elif token == ")":
            paren -= 1
        elif token == "[":
            bracket += 1
        elif token == "]":
            bracket -= 1
        elif token == "<":
            angle += 1
        elif token == ">" and angle > 0:
            angle -= 1
        elif token == "{" and paren == 0 and bracket == 0 and angle == 0:
            break
        cursor += 1
    if cursor >= len(masked):
        raise VerificationError(f"target free function is unterminated: {function_name}")
    signature = _normalize_signature(source[signature_start:cursor])
    return ParsedMethod(
        function_name,
        signature,
        sha256(signature.encode("utf-8")),
        "INHERENT",
    )


def parse_target_method(
    source: str, target_trait: str, target_method: str
) -> ParsedMethod:
    """从 canonical trait/type/module 中解析精确 target method。"""

    try:
        methods = parse_direct_trait_methods(source, target_trait)
    except VerificationError:
        methods = ()
    matches = [method for method in methods if method.name == target_method]
    if len(matches) == 1:
        return matches[0]

    masked = _mask_rust(source)
    impl_pattern = re.compile(
        rf"\bimpl\b[^{{;]*\b{re.escape(target_trait)}\b[^{{;]*{{",
        flags=re.MULTILINE | re.DOTALL,
    )
    inherent_matches: list[ParsedMethod] = []
    for implementation in impl_pattern.finditer(masked):
        opening = masked.find("{", implementation.start(), implementation.end())
        if opening < 0:
            continue
        closing = _matching_brace(masked, opening)
        inherent_matches.extend(
            method
            for method in _methods_in_body(source, opening, closing)
            if method.name == target_method
        )
    if len(inherent_matches) == 1:
        return inherent_matches[0]
    if target_trait[:1].islower():
        return _parse_free_function(source, target_method)
    raise VerificationError(
        f"target method is missing or ambiguous: {target_trait}::{target_method}"
    )


def parse_rows(payload: bytes) -> tuple[MethodRow, ...]:
    """读取严格 UTF-8/LF TSV。"""

    try:
        text = payload.decode("utf-8")
    except UnicodeDecodeError as error:
        raise VerificationError(f"method TSV is not UTF-8: {error}") from error
    if "\r" in text or (text and not text.endswith("\n")):
        raise VerificationError("method TSV must use canonical LF and final newline")
    reader = csv.reader(io.StringIO(text), delimiter="\t", strict=True)
    records = list(reader)
    if not records or tuple(records[0]) != METHOD_HEADER:
        raise VerificationError(f"method TSV header mismatch; expected {METHOD_HEADER}")
    rows: list[MethodRow] = []
    for line_number, fields in enumerate(records[1:], start=2):
        if len(fields) != len(METHOD_HEADER):
            raise VerificationError(f"method TSV line {line_number} has {len(fields)} fields")
        if any(not field for field in fields):
            raise VerificationError(f"method TSV line {line_number} has an empty field")
        rows.append(MethodRow(*fields))
    return tuple(rows)


def render_rows(rows: Sequence[MethodRow]) -> bytes:
    """按 source object/method/signature 形成唯一 canonical serialization。"""

    output = io.StringIO(newline="")
    writer = csv.writer(output, delimiter="\t", lineterminator="\n")
    writer.writerow(METHOD_HEADER)
    for row in sorted(rows, key=lambda item: (item.source_object, item.method, item.source_signature)):
        writer.writerow(tuple(getattr(row, field) for field in METHOD_HEADER))
    return output.getvalue().encode("utf-8")


def parse_method_call_index(payload: bytes) -> tuple[MethodCallEvidence, ...]:
    """读取 C2 checked BASE-call reverse index。"""

    try:
        text = payload.decode("utf-8")
    except UnicodeDecodeError as error:
        raise VerificationError(f"method-call index is not UTF-8: {error}") from error
    if "\r" in text or (text and not text.endswith("\n")):
        raise VerificationError("method-call index must use canonical LF and final newline")
    records = list(csv.reader(io.StringIO(text), delimiter="\t", strict=True))
    if not records or tuple(records[0]) != METHOD_CALL_INDEX_HEADER:
        raise VerificationError(
            f"method-call index header mismatch; expected {METHOD_CALL_INDEX_HEADER}"
        )
    rows: list[MethodCallEvidence] = []
    for line_number, fields in enumerate(records[1:], start=2):
        if len(fields) != len(METHOD_CALL_INDEX_HEADER) or any(not field for field in fields):
            raise VerificationError(f"method-call index line {line_number} is malformed")
        try:
            count = int(fields[2])
        except ValueError as error:
            raise VerificationError(
                f"method-call index line {line_number} has invalid count"
            ) from error
        if count <= 0:
            raise VerificationError(
                f"method-call index line {line_number} has non-positive count"
            )
        parse_behavior_selector(fields[3])
        rows.append(
            MethodCallEvidence(
                base_identity=fields[0],
                method_call=fields[1],
                base_call_count=count,
                replacement_selector=fields[3],
                disposition=fields[4],
                behavioral_theme=fields[5],
            )
        )
    duplicates = [row for row, count in Counter(rows).items() if count > 1]
    if duplicates:
        raise VerificationError(f"duplicate method-call index rows: {duplicates[:5]}")
    return tuple(rows)


@functools.lru_cache(maxsize=32_768)
def parse_behavior_selector(value: str) -> BehaviorSelector:
    """解析严格的 `path#test[::case]` selector。"""

    integration = INTEGRATION_EVIDENCE_SELECTOR.fullmatch(value)
    if integration is not None:
        return BehaviorSelector(
            integration.group(1), integration.group(2), integration.group(3)
        )
    unit = UNIT_EVIDENCE_SELECTOR.fullmatch(value)
    if unit is not None:
        return BehaviorSelector(unit.group(1), unit.group(2))
    raise VerificationError(f"invalid behavior evidence selector: {value}")


def parse_behavior_evidence(value: str) -> tuple[BehaviorSelector, ...]:
    """解析一行一个或多个分号分隔的 exact selectors。"""

    if value == "-" or value.startswith("BASE_") or value.endswith(".tsv"):
        raise VerificationError(f"whole-ledger/default evidence is forbidden: {value}")
    selectors = tuple(parse_behavior_selector(part) for part in value.split(";"))
    duplicates = [selector for selector, count in Counter(selectors).items() if count > 1]
    if duplicates:
        raise VerificationError(f"duplicate behavior evidence selectors: {duplicates}")
    return selectors


def _snake_case(value: str) -> str:
    value = re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1_\2", value)
    return re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", value).lower()


def fallback_case_id(row: MethodRow) -> str:
    """为无 BASE direct test 的 method row 创建唯一 stable case id。"""

    return f"{_snake_case(row.source_object)}_{row.method}"


def fallback_selector(row: MethodRow) -> BehaviorSelector:
    source_path = (
        INTERNALIZED_EVIDENCE_PATH
        if row.disposition == "INTERNALIZED"
        else FALLBACK_EVIDENCE_PATH
    )
    return BehaviorSelector(
        source_path,
        f"{FALLBACK_EVIDENCE_FUNCTION_PREFIX}{fallback_case_id(row)}",
    )


def _method_names(row: MethodRow) -> set[str]:
    return {row.method, row.target_symbol.rsplit("::", 1)[-1]}


def _relevant_method_call(row: MethodRow, evidence: MethodCallEvidence) -> bool:
    """只做 reverse-index method identity 连接；禁止 filename/family hints。"""

    return evidence.method_call in _method_names(row)


def _method_call_selector_lookup(
    method_call_index: Sequence[MethodCallEvidence],
) -> dict[str, tuple[BehaviorSelector, ...]]:
    cache_key = id(method_call_index)
    cached = _METHOD_CALL_LOOKUP_CACHE.get(cache_key)
    if cached is not None and cached[0] is method_call_index:
        return cached[1]
    grouped: dict[str, set[BehaviorSelector]] = defaultdict(set)
    for evidence in method_call_index:
        grouped[evidence.method_call].add(
            parse_behavior_selector(evidence.replacement_selector)
        )
    lookup = {method: tuple(sorted(selectors)) for method, selectors in grouped.items()}
    _METHOD_CALL_LOOKUP_CACHE[cache_key] = (method_call_index, lookup)
    return lookup


def behavior_candidates(
    row: MethodRow,
    method_call_index: Sequence[MethodCallEvidence],
    repository_root: Path | None = None,
) -> tuple[BehaviorSelector, ...]:
    """返回 C2 reverse-index 中 receiver-exact 且断言可观察的 selectors。"""

    lookup = _method_call_selector_lookup(method_call_index)
    selectors = sorted(
        {
            selector
            for method_name in _method_names(row)
            for selector in lookup.get(method_name, ())
        }
    )
    if repository_root is None:
        return tuple(selectors)
    valid: list[BehaviorSelector] = []
    for selector in selectors:
        try:
            _verify_selector_source(repository_root, selector)
            _verify_selector_contract(repository_root, selector, row)
        except VerificationError:
            continue
        valid.append(selector)
    return tuple(valid)


def select_behavior_evidence(
    row: MethodRow,
    method_call_index: Sequence[MethodCallEvidence],
    repository_root: Path,
) -> str:
    """优先绑定 C2 exact selector；无 BASE direct call 时绑定唯一 fallback case。"""

    candidates = behavior_candidates(row, method_call_index, repository_root)
    if not candidates:
        return fallback_selector(row).render()
    return min(candidates).render()


def _test_function_exists(source: str, function: str) -> bool:
    masked = _mask_rust(source)
    pattern = re.compile(rf"\b(?:async\s+)?fn\s+{re.escape(function)}\s*\(")
    for match in pattern.finditer(masked):
        prefix = masked[max(0, match.start() - 240) : match.start()]
        if re.search(r"#\s*\[\s*(?:tokio\s*::\s*)?test\b", prefix):
            return True
    return False


@functools.lru_cache(maxsize=4_096)
def _test_function_body(source: str, function: str) -> str:
    """返回命名 test function 的完整源码 body，拒绝同名歧义。"""

    masked = _mask_rust(source)
    leaf = function.rsplit("::", 1)[-1]
    matches = list(
        re.finditer(
            rf"\b(?:async\s+)?fn\s+{re.escape(leaf)}"
            rf"(?:\s*<[^>{{}}]*>)?\s*\(",
            masked,
        )
    )
    if len(matches) != 1:
        raise VerificationError(
            f"behavior evidence test function is missing or ambiguous: {function}"
        )
    opening = masked.find("{", matches[0].end())
    if opening < 0:
        raise VerificationError(f"behavior evidence test has no body: {function}")
    closing = _matching_brace(masked, opening)
    return source[matches[0].start() : closing + 1]


def _reachable_task6_helper_bodies(source: str, body: str) -> tuple[str, ...]:
    """解析 case arm/测试直接到达的唯一 `task6_*` helper bodies。"""

    pending = list(
        dict.fromkeys(
            re.findall(r"\b(task6_[A-Za-z_][A-Za-z0-9_]*)\s*\(", _mask_rust(body))
        )
    )
    visited: set[str] = set()
    helpers: list[str] = []
    while pending:
        helper = pending.pop(0)
        if helper in visited:
            continue
        visited.add(helper)
        helper_body = _test_function_body(source, helper)
        helpers.append(helper_body)
        pending.extend(
            name
            for name in re.findall(
                r"\b(task6_[A-Za-z_][A-Za-z0-9_]*)\s*\(",
                _mask_rust(helper_body),
            )
            if name not in visited
        )
    return tuple(helpers)


@functools.lru_cache(maxsize=4_096)
def _selector_evidence_body(source: str, selector: BehaviorSelector) -> str:
    """返回 test 或精确 case arm 加唯一 reachable helper 的源码。"""

    function_body = _test_function_body(source, selector.function)
    if selector.case_id is None:
        selected = function_body
    else:
        arm_pattern = re.compile(
            rf'"{re.escape(selector.case_id)}"\s*=>\s*{{'
        )
        matches = list(arm_pattern.finditer(source))
        if len(matches) != 1:
            raise VerificationError(
                "behavior evidence case must have exactly one literal match arm: "
                f"{selector.render()}"
            )
        opening = source.find("{", matches[0].start(), matches[0].end())
        closing = _matching_brace(_mask_rust(source), opening)
        selected = source[matches[0].start() : closing + 1]
        if selected not in function_body:
            raise VerificationError(
                f"behavior evidence case arm is outside claimed test: {selector.render()}"
            )
    helpers = _reachable_task6_helper_bodies(source, selected)
    return "\n".join((selected, *helpers))


def _has_non_tautological_assertion(body: str, row: MethodRow) -> bool:
    """证明 exact UFCS result participates in a strong observable assertion。"""

    masked = _mask_rust(body)
    if re.search(
        r"\.is_ok\s*\(\s*\)\s*\|\|\s*[^;]+\.is_err\s*\(\s*\)"
        r"|\.is_err\s*\(\s*\)\s*\|\|\s*[^;]+\.is_ok\s*\(\s*\)",
        masked,
        flags=re.DOTALL,
    ):
        return False
    if re.search(r"(?:Ok|Err)\s*\(\s*_\s*\)", masked):
        return False
    if re.search(r"assert!\s*\([^;]*\.is_(?:ok|err)\s*\(\s*\)\s*\)", masked):
        return False
    typed_trace = re.search(
        rf"\btask6_assert_typed_operation_trace\s*\(\s*"
        rf'"{re.escape(row.target_ufcs)}"\s*,\s*'
        rf"([A-Za-z_][A-Za-z0-9_]*)\s*,\s*"
        rf"([A-Za-z_][A-Za-z0-9_]*)\s*[,)]",
        body,
        flags=re.DOTALL,
    )
    if typed_trace is not None:
        if typed_trace.group(1) == typed_trace.group(2):
            return False
        return re.search(
            r"fn\s+task6_assert_typed_operation_trace\b[\s\S]*?"
            r"assert_eq!\s*\(\s*actual\s*,\s*expected\s*\)",
            body,
        ) is not None

    target_method = row.target_symbol.rsplit("::", 1)[-1]
    ufcs_pattern = re.compile(
        rf"\b{re.escape(row.target_trait)}\s*::\s*"
        rf"{re.escape(target_method)}\s*\("
    )
    call = ufcs_pattern.search(masked)
    if call is None:
        return False
    statement_start = max(
        masked.rfind(";", 0, call.start()),
        masked.rfind("{", 0, call.start()),
    ) + 1
    statement_end = masked.find(";", call.end())
    if statement_end < 0:
        statement_end = len(masked)
    statement = masked[statement_start : statement_end + 1]
    if re.search(r"\blet\s+_\s*=", statement):
        return False
    if re.search(r"\.(?:unwrap|expect)\s*\(", statement) and "->Result<()," in row.target_signature:
        return True

    direct_assertions = (
        re.compile(r"assert_(?:eq|ne)!\s*\([^;]*", flags=re.DOTALL),
        re.compile(r"assert!\s*\(\s*matches!\s*\([^;]*", flags=re.DOTALL),
    )
    for assertion in direct_assertions:
        for match in assertion.finditer(masked):
            end = masked.find(";", match.start())
            if end < 0:
                end = len(masked)
            if match.start() <= call.start() <= end:
                return True

    assignment = re.search(
        r"\blet\s+(?:mut\s+)?([A-Za-z_][A-Za-z0-9_]*)"
        r"(?:\s*:\s*[^=]+)?\s*=\s*$",
        masked[statement_start : call.start()],
    )
    if assignment is None:
        return False
    variable = assignment.group(1)
    tail = masked[statement_end + 1 :]
    return re.search(
        rf"(?:assert_(?:eq|ne)!|assert!\s*\(\s*matches!|assert_trace\s*\()"
        rf"[^;]*\b{re.escape(variable)}\b",
        tail,
        flags=re.DOTALL,
    ) is not None


def _verify_selector_contract(
    repository_root: Path, selector: BehaviorSelector, row: MethodRow
) -> None:
    """证明 exact receiver/method call 与非 tautological observable assertion。"""

    source = _compiled_rust_source(repository_root / selector.source_path)
    body = _selector_evidence_body(source, selector)
    method = row.target_symbol.rsplit("::", 1)[-1]
    exact_ufcs = re.compile(
        rf"\b{re.escape(row.target_trait)}\s*::\s*{re.escape(method)}\s*\("
    )
    typed_trace = re.search(
        rf"\btask6_assert_typed_operation_trace\s*\(\s*"
        rf'"{re.escape(row.target_ufcs)}"\s*,',
        body,
        flags=re.DOTALL,
    )
    if exact_ufcs.search(_mask_rust(body)) is None and typed_trace is None:
        raise VerificationError(
            "behavior evidence canonical receiver mismatch: "
            f"expected {row.target_trait}::{method} in {selector.render()}"
        )
    if not _has_non_tautological_assertion(body, row):
        raise VerificationError(
            "behavior evidence has no non-tautological observable assertion for "
            f"{row.target_trait}::{method}: {selector.render()}"
        )


def _verify_selector_source(repository_root: Path, selector: BehaviorSelector) -> None:
    target = repository_root / selector.source_path
    if not target.is_file():
        raise VerificationError(f"behavior evidence path does not exist: {selector.source_path}")
    source = _compiled_rust_source(target)
    function = selector.function.rsplit("::", 1)[-1]
    if "/src/" in selector.source_path:
        crate_relative = selector.source_path.split("/src/", 1)[1].removesuffix(".rs")
        if crate_relative.endswith("/mod"):
            crate_relative = crate_relative.removesuffix("/mod")
        expected_module = crate_relative.replace("/", "::")
        if not selector.function.startswith(f"{expected_module}::"):
            raise VerificationError(
                "behavior evidence unit-test module does not match its source path: "
                f"{selector.render()}"
            )
    if not _test_function_exists(source, function):
        raise VerificationError(
            "behavior evidence is not an executable test in the claimed file: "
            f"{selector.render()}"
        )
    if selector.case_id is not None and re.search(
        rf'"{re.escape(selector.case_id)}"', source
    ) is None:
        raise VerificationError(
            f"behavior evidence case does not exist in claimed file: {selector.render()}"
        )


def verify_rows(
    rows: Sequence[MethodRow],
    *,
    repository_root: Path,
    source_base: str,
    base_sources: Mapping[str, str],
    trait_specs: Mapping[str, str],
    method_call_index: Sequence[MethodCallEvidence],
) -> tuple[BehaviorSelector, ...]:
    """验证 exact BASE denominator、source evidence 和真实 current targets。"""

    expected: dict[tuple[str, str, str], tuple[str, ParsedMethod]] = {}
    evidence: dict[tuple[str, str], tuple[str, str]] = {}
    for trait_name, source_path in trait_specs.items():
        try:
            source = base_sources[source_path]
        except KeyError as error:
            raise VerificationError(f"BASE trait source missing: {source_path}") from error
        methods = parse_direct_trait_methods(source, trait_name)
        names = {method.name for method in methods}
        overrides = _impl_method_paths(base_sources, trait_name, names)
        calls = _call_paths(base_sources, trait_name, names)
        for method in methods:
            key = (trait_name, method.name, method.signature)
            expected[key] = (source_path, method)
            evidence[(trait_name, method.name)] = (
                _paths_text(overrides.get(method.name, set())),
                _paths_text(calls.get(method.name, set())),
            )

    actual_keys = [(row.source_object, row.method, row.source_signature) for row in rows]
    duplicates = [key for key, count in Counter(actual_keys).items() if count > 1]
    if duplicates:
        raise VerificationError(f"duplicate BASE method rows: {duplicates[:5]}")
    missing = sorted(set(expected).difference(actual_keys))
    extra = sorted(set(actual_keys).difference(expected))
    if missing or extra:
        raise VerificationError(
            f"BASE method denominator mismatch: missing={missing[:5]} extra={extra[:5]}"
        )

    executed_evidence: list[BehaviorSelector] = []
    fallback_bindings: dict[BehaviorSelector, list[tuple[str, str]]] = defaultdict(list)
    for row in rows:
        key = (row.source_object, row.method, row.source_signature)
        source_path, method = expected[key]
        if row.source_base != source_base:
            raise VerificationError(f"{row.source_object}::{row.method} source_base drift")
        if row.source_trait_path != source_path:
            raise VerificationError(f"{row.source_object}::{row.method} trait path drift")
        if row.source_signature_sha256 != method.signature_sha256:
            raise VerificationError(f"{row.source_object}::{row.method} signature hash drift")
        if row.base_default_kind != method.default_kind:
            raise VerificationError(f"{row.source_object}::{row.method} default-kind drift")
        expected_overrides, expected_calls = evidence[(row.source_object, row.method)]
        if row.base_override_paths != expected_overrides:
            raise VerificationError(
                f"{row.source_object}::{row.method} override evidence drift: "
                f"expected {expected_overrides}, got {row.base_override_paths}"
            )
        if row.base_call_paths != expected_calls:
            raise VerificationError(
                f"{row.source_object}::{row.method} call evidence drift: "
                f"expected {expected_calls}, got {row.base_call_paths}"
            )
        if row.disposition not in ALLOWED_DISPOSITIONS:
            raise VerificationError(f"unknown method disposition: {row.disposition}")

        selectors = parse_behavior_evidence(row.behavior_evidence)
        for selector in selectors:
            _verify_selector_source(repository_root, selector)
        if row.disposition == "APPROVED_BREAK":
            if method.default_kind != "DEFAULT_UNSUPPORTED":
                raise VerificationError(
                    "APPROVED_BREAK requires a BASE default-unsupported method"
                )
            if expected_overrides != "-" or expected_calls != "-":
                raise VerificationError(
                    "APPROVED_BREAK cannot hide BASE overrides or calls"
                )
            if row.approval_ref == "-" or any(
                value != "-"
                for value in (
                    row.target_entry,
                    row.target_path,
                    row.target_symbol,
                    row.target_trait,
                    row.target_signature,
                    row.target_signature_sha256,
                    row.target_ufcs,
                )
            ):
                raise VerificationError(
                    "APPROVED_BREAK requires ruling and '-' target fields"
                )
            executed_evidence.extend(selectors)
            continue
        for selector in selectors:
            _verify_selector_contract(repository_root, selector, row)
        candidates = set(
            behavior_candidates(row, method_call_index, repository_root)
        )
        fallback = fallback_selector(row)
        if candidates:
            unexpected = [selector for selector in selectors if selector not in candidates]
            if unexpected:
                raise VerificationError(
                    f"{row.source_object}::{row.method} evidence is not derived from its "
                    f"C2 BASE method calls: {unexpected}"
                )
        elif selectors != (fallback,):
            raise VerificationError(
                f"{row.source_object}::{row.method} has no BASE direct test and requires "
                f"its unique fallback case {fallback.render()}"
            )
        for selector in selectors:
            executed_evidence.append(selector)
            if selector.source_path in FALLBACK_EVIDENCE_PATHS:
                fallback_bindings[selector].append((row.source_object, row.method))

        if row.target_entry == "-" or row.target_path == "-" or row.target_symbol == "-":
            raise VerificationError(f"{row.disposition} requires a concrete target")
        if row.conversion == "-":
            raise VerificationError(f"{row.disposition} requires an explicit conversion")
        target_path = repository_root / row.target_path
        if not target_path.is_file():
            raise VerificationError(f"target path does not exist: {row.target_path}")
        target_source = target_path.read_text(encoding="utf-8")
        if not _target_symbol_exists(target_source, row.target_symbol):
            raise VerificationError(
                f"target symbol does not exist at claimed path: {row.target_symbol} @ {row.target_path}"
            )
        target_trait, target_method_name = row.target_symbol.rsplit("::", 1)
        if row.target_trait != target_trait:
            raise VerificationError(
                f"{row.source_object}::{row.method} target trait drift: "
                f"expected {target_trait}, got {row.target_trait}"
            )
        if row.target_ufcs != row.target_entry or not row.target_ufcs.endswith(
            f"::{row.target_trait}::{target_method_name}"
        ):
            raise VerificationError(
                f"{row.source_object}::{row.method} target UFCS drift: {row.target_ufcs}"
            )
        target_method = parse_target_method(
            target_source, row.target_trait, target_method_name
        )
        if row.target_signature != target_method.signature:
            raise VerificationError(
                f"{row.source_object}::{row.method} target signature drift"
            )
        if row.target_signature_sha256 != target_method.signature_sha256:
            raise VerificationError(
                f"{row.source_object}::{row.method} target signature hash drift"
            )

    duplicate_fallbacks = {
        selector: bindings
        for selector, bindings in fallback_bindings.items()
        if len(bindings) != 1
    }
    if duplicate_fallbacks:
        raise VerificationError(f"fallback evidence cases are not unique: {duplicate_fallbacks}")
    return tuple(sorted(set(executed_evidence)))


def load_base_sources(repository_root: Path, source_base: str) -> dict[str, str]:
    """从一个固定 commit 的 production src trees 读取 Rust source，不读取 worktree BASE。"""

    archive = subprocess.check_output(
        [
            "git",
            "archive",
            "--format=tar",
            source_base,
            "crates/druid-rdbc/src",
            "crates/druid/src",
            "crates/druid-wrapper/src",
        ],
        cwd=repository_root,
    )
    sources: dict[str, str] = {}
    with tarfile.open(fileobj=io.BytesIO(archive), mode="r:") as bundle:
        for member in bundle.getmembers():
            if member.isfile() and member.name.endswith(".rs"):
                extracted = bundle.extractfile(member)
                if extracted is None:
                    raise VerificationError(f"cannot read BASE archive member: {member.name}")
                sources[member.name] = extracted.read().decode("utf-8")
    return sources


def _target_package(source_path: str) -> tuple[str, str]:
    for prefix, package in (
        ("crates/druid-rdbc/tests/", "druid-rdbc"),
        ("crates/druid/tests/", "druid"),
        ("crates/druid-wrapper/tests/", "druid-wrapper"),
    ):
        if source_path.startswith(prefix):
            relative = source_path[len(prefix) :]
            if "/" in relative or not relative.endswith(".rs"):
                raise VerificationError(
                    f"method evidence must be a root integration target: {source_path}"
                )
            return package, relative.removesuffix(".rs")
    # 私有 lib 证据只复用 C2 的显式来源登记，不允许任意 src 文件充当测试目标。
    import verify_task6_deleted_test_parity as deleted_test_verifier

    try:
        return deleted_test_verifier.target_package(source_path)
    except deleted_test_verifier.VerificationError as error:
        raise VerificationError(f"unknown method evidence package: {source_path}") from error


def _executed_functions(output: str) -> set[str]:
    return {
        match.group(1)
        for match in re.finditer(
            r"^test\s+([^\s]+)\s+\.\.\.\s+ok$", output, re.MULTILINE
        )
    }


def _case_completion_digests(output: str) -> dict[str, tuple[str, ...]]:
    digests: dict[str, tuple[str, ...]] = {}
    for match in re.finditer(
        r"^TASK6_CASE_DIGEST=([A-Za-z_][A-Za-z0-9_]*):(.*)$",
        output,
        re.MULTILINE,
    ):
        function, payload = match.groups()
        if function in digests:
            raise VerificationError(f"duplicate runtime case digest: {function}")
        digests[function] = tuple(payload.split("|")) if payload else ()
    return digests


def execute_behavior_evidence(
    repository_root: Path,
    selectors: Sequence[BehaviorSelector],
    *,
    runner: Callable[..., subprocess.CompletedProcess[str]] = subprocess.run,
) -> tuple[int, int, int]:
    """执行每个 path-qualified integration target 并证明 named tests 实际 PASS。"""

    expected_by_target: dict[tuple[str, str], set[str]] = defaultdict(set)
    expected_cases_by_target: dict[
        tuple[str, str], dict[str, list[str]]
    ] = defaultdict(lambda: defaultdict(list))
    for selector in selectors:
        package, target = _target_package(selector.source_path)
        if target == "__lib__":
            import verify_task6_deleted_test_parity as deleted_test_verifier

            registered = deleted_test_verifier.LIB_TEST_TARGETS[selector.source_path]
            if selector.case_id is not None or selector.function not in registered.values():
                raise VerificationError(
                    f"unregistered qualified unit method evidence: {selector.render()}"
                )
        expected_by_target[(package, target)].add(selector.function)
        if selector.case_id is not None:
            expected_cases_by_target[(package, target)][selector.function].append(
                selector.case_id
            )

    for (package, target), expected in sorted(expected_by_target.items()):
        is_fallback_target = any(
            _target_package(path) == (package, target)
            for path in FALLBACK_EVIDENCE_PATHS
        )
        command = [
            "cargo",
            "test",
            "--offline",
            "--locked",
            "-p",
            package,
            "--all-features",
        ]
        if target == "__lib__":
            command.append("--lib")
        else:
            command.extend(["--test", target])
        # 保留成功函数整行，并在 successes stdout 中取得完整案例 digest。
        command.extend(["--", "--show-output"])
        try:
            result = runner(
                command,
                cwd=repository_root,
                check=False,
                capture_output=True,
                text=True,
                timeout=1_800,
            )
        except (OSError, subprocess.TimeoutExpired) as error:
            raise VerificationError(
                f"method evidence target NOT_RUN/ERROR for {package}/{target}: {error}"
            ) from error
        output = result.stdout + "\n" + result.stderr
        if result.returncode != 0:
            raise VerificationError(
                f"method evidence target failed for {package}/{target}: {output[-20_000:]}"
            )
        executed = _executed_functions(output)
        missing = [
            function
            for function in sorted(expected)
            if not any(
                name == function
                or (target != "__lib__" and name.endswith(f"::{function}"))
                for name in executed
            )
        ]
        if missing:
            raise VerificationError(
                "method evidence selectors were not executed in their claimed target: "
                f"{package}/{target}: {missing}"
            )
        digests = _case_completion_digests(output)
        missing_cases: list[str] = []
        for function, expected_cases in expected_cases_by_target[(package, target)].items():
            actual_cases = digests.get(function, ())
            missing_cases.extend(
                f"{function}::{case_id}"
                for case_id in expected_cases
                if case_id not in actual_cases
            )
            if is_fallback_target:
                if tuple(expected_cases) != actual_cases:
                    missing_cases.extend(
                        f"{function}::{case_id}" for case_id in expected_cases
                    )
        if missing_cases:
            raise VerificationError(
                "method evidence parameter cases were not executed at runtime in their "
                f"claimed target: {package}/{target}: {missing_cases[:20]}"
            )
    functions = {(selector.source_path, selector.function) for selector in selectors}
    return len(expected_by_target), len(functions), len(set(selectors))


def verify_c2_runtime_evidence(
    repository_root: Path,
    *,
    runner: Callable[..., subprocess.CompletedProcess[str]] = subprocess.run,
) -> None:
    """调用 C2 authoritative verifier，并只接受其完整 runtime digest PASS。"""

    command = [
        sys.executable,
        "-X",
        "utf8",
        "scripts/verify_task6_deleted_test_parity.py",
    ]
    try:
        result = runner(
            command,
            cwd=repository_root,
            check=False,
            capture_output=True,
            text=True,
            encoding="utf-8",
            timeout=7_200,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise VerificationError(f"C2 runtime evidence verifier NOT_RUN/ERROR: {error}") from error
    output = result.stdout + "\n" + result.stderr
    if result.returncode != 0:
        raise VerificationError(f"C2 runtime evidence verifier failed: {output[-20_000:]}")
    if re.search(
        r"PASS task6 deleted-test parity: .*base_tests=1283 .*executed=yes",
        output,
    ) is None:
        raise VerificationError(
            "C2 verifier did not return its authoritative 1283-row executed=yes proof"
        )


def verify_c2_source_evidence(
    repository_root: Path,
    *,
    runner: Callable[..., subprocess.CompletedProcess[str]] = subprocess.run,
) -> None:
    """消费 C2 structural proof，确保 reverse index 本身未漂移或伪造。"""

    command = [
        sys.executable,
        "-X",
        "utf8",
        "scripts/verify_task6_deleted_test_parity.py",
        "--no-execute",
    ]
    try:
        result = runner(
            command,
            cwd=repository_root,
            check=False,
            capture_output=True,
            text=True,
            encoding="utf-8",
            timeout=1_800,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise VerificationError(f"C2 source evidence verifier NOT_RUN/ERROR: {error}") from error
    output = result.stdout + "\n" + result.stderr
    if result.returncode != 0:
        raise VerificationError(f"C2 source evidence verifier failed: {output[-20_000:]}")
    if re.search(
        r"PASS task6 deleted-test parity: .*base_tests=1283 .*executed=no",
        output,
    ) is None:
        raise VerificationError(
            "C2 verifier did not return its authoritative 1283-row executed=no source proof"
        )


def verify_generated_behavior_matrix(
    repository_root: Path,
    *,
    runner: Callable[..., subprocess.CompletedProcess[str]] = subprocess.run,
) -> None:
    """调用 checked renderer，拒绝 fallback source 与 479-row matrix 漂移。"""

    command = [
        sys.executable,
        "scripts/render_task6_method_default_evidence.py",
        "--check",
    ]
    try:
        result = runner(
            command,
            cwd=repository_root,
            check=False,
            capture_output=True,
            text=True,
            timeout=1_800,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        raise VerificationError(
            f"generated method behavior matrix NOT_RUN/ERROR: {error}"
        ) from error
    output = result.stdout + "\n" + result.stderr
    if result.returncode != 0:
        raise VerificationError(
            f"generated method behavior matrix check failed: {output[-20_000:]}"
        )
    if "PASS generated method behavior matrix: files=14" not in output:
        raise VerificationError(
            "generated method behavior renderer did not attest all 14 checked files"
        )


def execute_all_behavior_evidence(
    repository_root: Path, selectors: Sequence[BehaviorSelector]
) -> tuple[int, int, int]:
    """消费 C2 runtime proof，然后直接执行全部唯一 C1 evidence targets。"""

    c2_selectors = tuple(
        selector
        for selector in selectors
        if selector.source_path not in FALLBACK_EVIDENCE_PATHS
    )
    if c2_selectors:
        verify_c2_runtime_evidence(repository_root)
    return execute_behavior_evidence(repository_root, selectors)


def _summary(
    rows: Sequence[MethodRow],
    payload: bytes,
    selectors: Sequence[BehaviorSelector],
    execution: tuple[int, int, int] | None,
) -> str:
    dispositions = Counter(row.disposition for row in rows)
    objects = Counter(row.source_object for row in rows)
    disposition_text = ",".join(f"{key}={dispositions[key]}" for key in sorted(dispositions))
    object_text = ",".join(f"{key}={objects[key]}" for key in sorted(objects))
    fallback_count = sum(
        selector.source_path in FALLBACK_EVIDENCE_PATHS for selector in selectors
    )
    fallback_rows = sum(
        any(
            row.behavior_evidence.startswith(f"{path}#")
            for path in FALLBACK_EVIDENCE_PATHS
        )
        for row in rows
    )
    evidence_payload = "".join(
        f"{row.source_object}::{row.method}@{row.source_signature_sha256}\t"
        f"{row.behavior_evidence}\n"
        for row in sorted(rows)
    ).encode("utf-8")
    parameterized_rows = sum(
        any(selector.case_id is not None for selector in parse_behavior_evidence(row.behavior_evidence))
        for row in rows
        if row.disposition != "APPROVED_BREAK"
    )
    execution_text = (
        "TASK6_METHOD_EVIDENCE_EXECUTED=no"
        if execution is None
        else (
            f"TASK6_METHOD_EVIDENCE_TARGETS={execution[0]}\n"
            f"TASK6_METHOD_EVIDENCE_FUNCTIONS={execution[1]}\n"
            f"TASK6_METHOD_EVIDENCE_SELECTORS={execution[2]}\n"
            "TASK6_METHOD_EVIDENCE_EXECUTED=yes"
        )
    )
    return (
        f"TASK6_METHOD_ROWS={len(rows)}\n"
        f"TASK6_METHOD_OBJECTS={object_text}\n"
        f"TASK6_METHOD_DISPOSITIONS={disposition_text}\n"
        f"TASK6_METHOD_SHA256={sha256(payload)}\n"
        f"TASK6_METHOD_EVIDENCE_ROWS={len(rows)}\n"
        f"TASK6_METHOD_EVIDENCE_UNIQUE={len(set(selectors))}\n"
        f"TASK6_METHOD_EVIDENCE_C2_ROWS={len(rows) - fallback_rows}\n"
        f"TASK6_METHOD_EVIDENCE_FALLBACK={fallback_count}\n"
        f"TASK6_METHOD_EVIDENCE_PARAMETERIZED_ROWS={parameterized_rows}\n"
        f"TASK6_METHOD_EVIDENCE_SHA256={sha256(evidence_payload)}\n"
        f"{execution_text}"
    )


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository-root", type=Path, default=Path(__file__).resolve().parent.parent)
    parser.add_argument("--source-base", default=TASK6_SOURCE_BASE)
    parser.add_argument("--matrix", type=Path, default=DEFAULT_MATRIX)
    parser.add_argument(
        "--method-call-index", type=Path, default=DEFAULT_METHOD_CALL_INDEX
    )
    parser.add_argument(
        "--no-execute",
        action="store_true",
        help="validate selectors structurally only; cannot prove executed evidence",
    )
    arguments = parser.parse_args(argv)

    root = arguments.repository_root.resolve()
    matrix = arguments.matrix if arguments.matrix.is_absolute() else root / arguments.matrix
    payload = matrix.read_bytes()
    rows = parse_rows(payload)
    canonical = render_rows(rows)
    if payload != canonical:
        raise VerificationError("method TSV bytes are not canonical/sorted")
    base_sources = load_base_sources(root, arguments.source_base)
    method_call_index_path = arguments.method_call_index
    if not method_call_index_path.is_absolute():
        method_call_index_path = root / method_call_index_path
    method_call_index = parse_method_call_index(method_call_index_path.read_bytes())
    if arguments.no_execute:
        verify_c2_source_evidence(root)
    selectors = verify_rows(
        rows,
        repository_root=root,
        source_base=arguments.source_base,
        base_sources=base_sources,
        trait_specs=TRAIT_SPECS,
        method_call_index=method_call_index,
    )
    verify_generated_behavior_matrix(root)
    execution = (
        None
        if arguments.no_execute
        else execute_all_behavior_evidence(root, selectors)
    )
    print(_summary(rows, payload, selectors, execution))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, subprocess.CalledProcessError, VerificationError) as error:
        print(f"TASK6_METHOD_PARITY_ERROR={error}")
        raise SystemExit(1) from error
