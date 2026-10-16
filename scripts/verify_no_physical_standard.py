#!/usr/bin/env python3
"""Verify the atomic P04 Task 6 cutover to one RDBC standard surface."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import os
import re
import subprocess
import sys
import tempfile
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable, NoReturn

import verify_task6_deleted_test_parity as deleted_test_parity
import verify_task6_public_contract as public_contract
from verify_rdbc_public_paths import Token, tokenize
from verify_task6_migration_authority import (
    load_rows as load_migration_authority,
    module_declares_exact_target,
    verify_authority_schema,
    verify_closed_targets,
    verify_derived_document,
    verify_handoff_ledgers,
    verify_standardization,
)


TASK6_BASE = "5afe8609b177544ff7d18461aa84f69efe8ee90e"
TARGET_PACKAGES = frozenset(
    {
        "druid-rdbc",
        "druid",
        "druid-wrapper",
        "druid-admin",
        "druid-metrics",
        "druid-bench",
    }
)
STRICT_TRICK_PACKAGES = frozenset({"druid-rdbc", "druid", "druid-wrapper"})
FORBIDDEN_PHYSICAL = re.compile(r"\bPhysical[A-Z][A-Za-z0-9_]*\b")
FORBIDDEN_SPI_TEXT = re.compile(r"\bdruid_rdbc\s*::\s*(?:r#)?spi\b")
ALLOWED_DISPOSITIONS = frozenset(
    {"MERGED_STANDARD", "INTERNALIZED", "CONCRETE_MOVED", "DELETED"}
)
ALLOWED_TEST_DISPOSITIONS = frozenset(
    {"PURE_REMOVED_API_IDENTITY", "MIGRATED", "REPLACED"}
)
PHYSICAL_MANIFEST = Path(
    "docs/verification/rdbc-first/rdbc-physical-standardization.tsv"
)
CONSUMER_MANIFEST = Path("docs/verification/rdbc-first/rdbc-production-consumers.tsv")
MIGRATION_DOCUMENT = Path("docs/rdbc/physical-to-standard-migration.md")
MIGRATION_AUTHORITY = Path(
    "docs/verification/rdbc-first/task6-migration-authority.tsv"
)
API_SNAPSHOT = Path("api/druid-rdbc.single-standard.after.txt")
# 固定编译器日期；省略 host 后缀，由 rustup 选择当前主机的工具链。
PUBLIC_API_TOOLCHAIN = "nightly-2026-08-07"
TASK6_PENDING = "TASK6_COMPATIBILITY_PENDING"
TASK6_CLOSED = "TASK6_COMPATIBILITY_CLOSED"


class VerificationError(RuntimeError):
    """Raised when the final surface is transitional, forgeable, or incomplete."""


def fail(message: str) -> NoReturn:
    """Stop with one stable fail-closed diagnostic."""

    raise VerificationError(message)


@dataclass(frozen=True)
class MigrationRow:
    """One exact old identity disposition from the Markdown migration table."""

    old_path: str
    disposition: str
    target_path: str
    target_task: str
    state: str
    evidence: str


@dataclass(frozen=True)
class TraitDeclaration:
    """One source trait and the direct method names in its declared body."""

    path: Path
    name: str
    methods: frozenset[str]


@dataclass(frozen=True)
class TestDispositionRow:
    """One deleted integration-test file and its canonical replacement evidence."""

    deleted_test_path: str
    disposition: str
    replacement_tests: str
    behavioral_assertions: str


def run_checked(
    command: list[str],
    *,
    cwd: Path,
    env: dict[str, str] | None = None,
    label: str,
    timeout: int | None = 600,
) -> subprocess.CompletedProcess[str]:
    """Run one required bounded local command and retain diagnostic output."""

    try:
        result = subprocess.run(
            command,
            cwd=cwd,
            check=False,
            capture_output=True,
            text=True,
            timeout=timeout,
            env=env,
        )
    except (OSError, subprocess.TimeoutExpired) as error:
        fail(f"{label} NOT_RUN/ERROR: {error}")
    if result.returncode != 0:
        detail = (result.stderr or result.stdout).strip()
        fail(f"{label} failed: {detail or f'exit {result.returncode}'}")
    return result


def load_cargo_metadata(root: Path) -> dict[str, object]:
    """Load Cargo's effective workspace/package/feature/target denominator."""

    result = run_checked(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        cwd=root,
        label="cargo metadata",
    )
    try:
        document = json.loads(result.stdout)
    except json.JSONDecodeError as error:
        fail(f"cargo metadata JSON is malformed: {error}")
    if not isinstance(document, dict) or not isinstance(document.get("packages"), list):
        fail("cargo metadata schema is incomplete")
    return document


def package_records(metadata: dict[str, object]) -> list[dict[str, object]]:
    """Select every declared production workspace package in deterministic order."""

    raw_packages = metadata.get("packages")
    if not isinstance(raw_packages, list):
        fail("cargo metadata packages are missing")
    packages = [
        package
        for package in raw_packages
        if isinstance(package, dict) and package.get("name") in TARGET_PACKAGES
    ]
    names = {str(package["name"]) for package in packages}
    missing = sorted(TARGET_PACKAGES.difference(names))
    # Unit fixtures may intentionally contain a strict subset; the real workspace may not.
    workspace_root = Path(str(metadata.get("workspace_root", "")))
    if workspace_root.name == "druid-rust" and missing:
        fail(f"workspace production package denominator is incomplete: {missing}")
    return sorted(packages, key=lambda package: str(package["name"]))


def package_source_root(package: dict[str, object]) -> Path:
    """Resolve the conventional source tree from the effective manifest path."""

    manifest = package.get("manifest_path")
    if not isinstance(manifest, str) or not manifest:
        fail("cargo metadata package manifest_path is missing")
    return Path(manifest).parent / "src"


def discover_production_files(
    root: Path, metadata: dict[str, object]
) -> tuple[Path, ...]:
    """Walk every package source tree, including cfg-disabled and unlisted Rust files."""

    files: set[Path] = set()
    for package in package_records(metadata):
        source_root = package_source_root(package)
        if not source_root.is_dir():
            fail(f"production source root is missing: {source_root}")
        files.update(
            path.resolve().relative_to(root.resolve())
            for path in source_root.rglob("*.rs")
            if path.is_file()
        )
        features = package.get("features")
        if not isinstance(features, dict):
            fail(f"Cargo feature map is missing: {package.get('name')}")
        for feature in features:
            if FORBIDDEN_PHYSICAL.search(str(feature)) or str(feature).lower() == "spi":
                fail(
                    f"Cargo feature retains a forbidden compatibility identity: {feature}"
                )
    return tuple(sorted(files))


def package_for_path(root: Path, path: Path, metadata: dict[str, object]) -> str:
    """Return the owning Cargo package for one discovered source path."""

    absolute = (root / path).resolve()
    owners = []
    for package in package_records(metadata):
        source_root = package_source_root(package).resolve()
        if absolute.is_relative_to(source_root):
            owners.append(str(package["name"]))
    if len(owners) != 1:
        fail(f"production source has no unique Cargo owner: {path}: {owners}")
    return owners[0]


def token_text(source: str) -> str:
    """Render code tokens with separators so grouped/raw aliases remain observable."""

    return " ".join(token.text.removeprefix("r#") for token in tokenize(source))


def crate_aliases(tokens: str) -> frozenset[str]:
    """Collect Rust 2015/2018 aliases for the druid-rdbc crate identity."""

    aliases = {"druid_rdbc"}
    patterns = (
        r"\bextern\s+crate\s+druid_rdbc\s+as\s+([A-Za-z_][A-Za-z0-9_]*)",
        r"\buse\s+druid_rdbc\s+as\s+([A-Za-z_][A-Za-z0-9_]*)\s*;",
    )
    for pattern in patterns:
        aliases.update(re.findall(pattern, tokens))
    return frozenset(aliases)


def verify_source_surface(root: Path, metadata: dict[str, object]) -> tuple[Path, ...]:
    """Reject all Physical/SPI identities in the complete production source closure."""

    files = discover_production_files(root, metadata)
    for relative in files:
        package = package_for_path(root, relative, metadata)
        if package in STRICT_TRICK_PACKAGES and any(
            part.lower() == "spi" or part.lower().startswith("physical_")
            for part in relative.parts
        ):
            fail(f"forbidden compatibility source path remains: {relative}")
        try:
            source = (root / relative).read_text(encoding="utf-8")
        except UnicodeDecodeError as error:
            fail(f"production source is not UTF-8: {relative}: {error}")
        physical = FORBIDDEN_PHYSICAL.search(source)
        if physical:
            fail(f"forbidden Physical identity in {relative}: {physical.group(0)}")
        tokens = token_text(source)
        if FORBIDDEN_SPI_TEXT.search(source):
            fail(f"forbidden druid_rdbc::spi path in {relative}")
        for alias in crate_aliases(tokens):
            if re.search(rf"\b{re.escape(alias)}\s*::\s*(?:\{{\s*)?spi\b", tokens):
                fail(f"forbidden aliased/grouped druid_rdbc::spi path in {relative}")
        if package == "druid-rdbc" and re.search(r"\bspi\b", tokens):
            fail(f"forbidden spi identity remains in druid-rdbc source: {relative}")
        if package in STRICT_TRICK_PACKAGES:
            if re.search(r"#\s*\[\s*path\s*=", source):
                fail(f"path attribute can evade final source closure: {relative}")
            if re.search(r"\binclude\s*!", source):
                fail(f"include! can evade final source closure: {relative}")
    return files


def matching_delimiters(tokens: list[Token], path: Path) -> dict[int, int]:
    """Build a strict delimiter map for trait-body extraction."""

    pairs = {"(": ")", "[": "]", "{": "}"}
    stack: list[tuple[str, int]] = []
    result: dict[int, int] = {}
    for index, token in enumerate(tokens):
        if token.text in pairs:
            stack.append((token.text, index))
        elif token.text in pairs.values():
            if not stack:
                fail(f"unbalanced delimiter in {path}")
            opening, opening_index = stack.pop()
            if pairs[opening] != token.text:
                fail(f"mismatched delimiter in {path}")
            result[opening_index] = index
    if stack:
        fail(f"unclosed delimiter in {path}")
    return result


def trait_declarations(path: Path, source: str) -> tuple[TraitDeclaration, ...]:
    """Parse trait declarations, including declarations nested inside macros/cfg blocks."""

    tokens = tokenize(source)
    matching = matching_delimiters(tokens, path)
    declarations: list[TraitDeclaration] = []
    for index, token in enumerate(tokens[:-1]):
        if token.text != "trait" or tokens[index + 1].kind != "IDENT":
            continue
        name = tokens[index + 1].text.removeprefix("r#")
        opening = next(
            (
                cursor
                for cursor in range(index + 2, len(tokens))
                if tokens[cursor].text == "{"
            ),
            None,
        )
        if opening is None or opening not in matching:
            fail(f"trait body is missing: {path}:{name}")
        closing = matching[opening]
        methods: set[str] = set()
        depth = 0
        cursor = opening + 1
        while cursor < closing:
            text = tokens[cursor].text
            if text in {"{", "(", "["}:
                depth += 1
            elif text in {"}", ")", "]"}:
                depth -= 1
            elif (
                depth == 0
                and text == "fn"
                and cursor + 1 < closing
                and tokens[cursor + 1].kind == "IDENT"
            ):
                methods.add(tokens[cursor + 1].text.removeprefix("r#"))
            cursor += 1
        declarations.append(TraitDeclaration(path, name, frozenset(methods)))
    return tuple(declarations)


def verify_no_duplicate_trait_tree(root: Path, metadata: dict[str, object]) -> None:
    """Reject a private renamed copy of a canonical RDBC trait method family."""

    files = discover_production_files(root, metadata)
    declarations: list[TraitDeclaration] = []
    for relative in files:
        source = (root / relative).read_text(encoding="utf-8")
        declarations.extend(trait_declarations(relative, source))
    canonical = [
        declaration
        for declaration in declarations
        if declaration.path.parts[:3] == ("crates", "druid-rdbc", "src")
        and len(declaration.methods) >= 4
    ]
    for candidate in declarations:
        if candidate in canonical or len(candidate.methods) < 4:
            continue
        for standard in canonical:
            overlap = candidate.methods.intersection(standard.methods)
            threshold = max(4, (len(standard.methods) * 3 + 4) // 5)
            if len(overlap) >= threshold:
                fail(
                    "private duplicate standard trait tree: "
                    f"{candidate.path}:{candidate.name} duplicates "
                    f"{standard.path}:{standard.name} via {sorted(overlap)}"
                )


def read_tsv(path: Path) -> list[dict[str, str]]:
    """Read a strict UTF-8 TSV with one final LF and no empty cells."""

    raw = path.read_bytes()
    if not raw.endswith(b"\n") or raw.endswith(b"\n\n") or b"\r" in raw:
        fail(f"TSV is not canonical: {path}")
    rows = list(csv.DictReader(raw.decode("utf-8").splitlines(), delimiter="\t"))
    if any(None in row or any(value == "" for value in row.values()) for row in rows):
        fail(f"TSV contains an empty or malformed row: {path}")
    return rows


def parse_migration_table(path: Path) -> tuple[MigrationRow, ...]:
    """Parse the six-column derived table, retaining four-column self-test compatibility."""

    rows: list[MigrationRow] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("|"):
            continue
        cells = [cell.strip().strip("`") for cell in line.strip().strip("|").split("|")]
        if cells in (
            ["old_path", "disposition", "target_path", "evidence"],
            [
                "old_path",
                "disposition",
                "target_path",
                "target_task",
                "state",
                "evidence",
            ],
        ):
            continue
        if cells and all(set(cell) <= {"-", ":"} for cell in cells):
            continue
        if len(cells) == 4 and cells[1] in ALLOWED_DISPOSITIONS:
            rows.append(
                MigrationRow(
                    cells[0], cells[1], cells[2], "-", "TASK6_CLOSED", cells[3]
                )
            )
        elif len(cells) == 6 and cells[1] in ALLOWED_DISPOSITIONS:
            rows.append(MigrationRow(*cells))
    if not rows:
        fail(f"migration table contains no disposition rows: {path}")
    return tuple(rows)


def parse_test_disposition_table(path: Path) -> tuple[TestDispositionRow, ...]:
    """Parse the exact deleted-test disposition table from the migration document."""

    rows: list[TestDispositionRow] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("|"):
            continue
        cells = [cell.strip().strip("`") for cell in line.strip().strip("|").split("|")]
        if cells == [
            "deleted_test_path",
            "disposition",
            "replacement_tests",
            "behavioral_assertions",
        ]:
            continue
        if cells and all(set(cell) <= {"-", ":"} for cell in cells):
            continue
        if len(cells) == 4 and cells[1] in ALLOWED_TEST_DISPOSITIONS:
            rows.append(TestDispositionRow(*cells))
    if not rows:
        fail(f"deleted-test disposition table is missing: {path}")
    return tuple(rows)


def baseline_deleted_tests(root: Path) -> set[str]:
    """Derive every Druid/wrapper integration-test file removed since the Task 6 base."""

    result = run_checked(
        [
            "git",
            "ls-tree",
            "-r",
            "--name-only",
            TASK6_BASE,
            "--",
            "crates/druid/tests",
            "crates/druid-wrapper/tests",
        ],
        cwd=root,
        label="Task 6 baseline test inventory",
    )
    candidates = {
        path
        for path in result.stdout.splitlines()
        if path.endswith(".rs")
        and (
            path.startswith("crates/druid/tests/")
            or path.startswith("crates/druid-wrapper/tests/")
        )
    }
    return {path for path in candidates if not (root / path).is_file()}


def verify_test_disposition_table(
    root: Path,
    migration: Path,
    required_deleted: set[str],
) -> tuple[TestDispositionRow, ...]:
    """Require an exact disposition and real canonical replacement for every deleted test."""

    rows = parse_test_disposition_table(migration)
    counts = Counter(row.deleted_test_path for row in rows)
    duplicate = sorted(path for path, count in counts.items() if count != 1)
    if duplicate:
        fail(f"deleted-test disposition table contains duplicates: {duplicate}")
    observed = set(counts)
    missing = sorted(required_deleted.difference(observed))
    extra = sorted(observed.difference(required_deleted))
    if missing or extra:
        fail(
            f"deleted-test disposition denominator mismatch: missing={missing}, extra={extra}"
        )
    function_pattern = re.compile(r"\b(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\b")
    for row in rows:
        if not row.behavioral_assertions or row.behavioral_assertions == "-":
            fail(
                f"deleted-test behavioral assertions are missing: {row.deleted_test_path}"
            )
        if (root / row.deleted_test_path).exists():
            fail(
                f"deleted-test disposition names a current file: {row.deleted_test_path}"
            )
        if row.disposition == "PURE_REMOVED_API_IDENTITY":
            if row.replacement_tests != "-":
                fail(
                    f"pure removed identity must use '-' replacement: {row.deleted_test_path}"
                )
            continue
        replacements = [
            item.strip() for item in row.replacement_tests.split(";") if item.strip()
        ]
        if not replacements:
            fail(f"deleted-test replacement is missing: {row.deleted_test_path}")
        for replacement in replacements:
            path_text, separator, function = replacement.partition("#")
            path = root / path_text
            if not separator or not function or not path.is_file():
                fail(
                    f"deleted-test replacement does not exist: {row.deleted_test_path} -> {replacement}"
                )
            functions = set(function_pattern.findall(path.read_text(encoding="utf-8")))
            if function not in functions:
                fail(f"deleted-test replacement function does not exist: {replacement}")
    return rows


def task6_manifest_paths(physical: Path, consumers: Path) -> set[str]:
    """Derive old public/consumer paths that cross the atomic Task 6 boundary."""

    task7_aliases = {
        "druid_rdbc::resource::Array",
        "druid_rdbc::resource::Blob",
        "druid_rdbc::resource::Clob",
        "druid_rdbc::resource::NClob",
        "druid_rdbc::resource::Ref",
        "druid_rdbc::resource::SqlXml",
    }
    required = {
        row["old_path"]
        for row in read_tsv(physical)
        if row["old_path"] not in task7_aliases
    }
    required.update(
        row["old_path"]
        for row in read_tsv(consumers)
        if row["state"] in {TASK6_PENDING, TASK6_CLOSED}
    )
    required.add("druid_rdbc::spi")
    return required


def baseline_extra_identities(root: Path, covered: Iterable[str]) -> set[str]:
    """Freeze newly discovered baseline Physical identifiers not owned by an old public path."""

    covered_leaves = {path.rsplit("::", 1)[-1] for path in covered}
    result = run_checked(
        [
            "git",
            "grep",
            "-h",
            "-o",
            "-E",
            "Physical[A-Z][A-Za-z0-9_]*",
            TASK6_BASE,
            "--",
            "crates/*/src/*.rs",
            "crates/*/src/**/*.rs",
        ],
        cwd=root,
        label="Task 6 baseline Physical identity scan",
    )
    identities = set(result.stdout.splitlines())
    return {f"rust::{identity}" for identity in identities.difference(covered_leaves)}


def source_declares_target(root: Path, target: str) -> bool:
    """Prove a private Druid/wrapper target at its exact declared module path."""

    return module_declares_exact_target(root, target)


def target_exists(root: Path, target: str, public_api_items: set[str]) -> bool:
    """Require every non-deleted target to exist on the current public/private surface."""

    if target.startswith("druid_rdbc::"):
        return target in public_api_items
    return source_declares_target(root, target)


def verify_migration_table(
    root: Path,
    physical: Path,
    consumers: Path,
    migration: Path,
    *,
    public_api_items: set[str],
    required_extra: Iterable[str] = (),
) -> tuple[MigrationRow, ...]:
    """Require one and only one approved disposition for the complete frozen old surface."""

    required = task6_manifest_paths(physical, consumers)
    required.update(required_extra)
    rows = parse_migration_table(migration)
    counts = Counter(row.old_path for row in rows)
    duplicate = sorted(path for path, count in counts.items() if count != 1)
    if duplicate:
        fail(f"migration table contains duplicate old paths: {duplicate}")
    observed = set(counts)
    missing = sorted(required.difference(observed))
    extra = sorted(observed.difference(required))
    if missing or extra:
        fail(f"migration table denominator mismatch: missing={missing}, extra={extra}")
    for row in rows:
        if row.disposition not in ALLOWED_DISPOSITIONS:
            fail(f"migration disposition is not approved: {row}")
        if not row.evidence or row.evidence == "-":
            fail(f"migration evidence is missing: {row.old_path}")
        if row.disposition == "DELETED":
            if row.target_path != "-":
                fail(f"deleted migration row must use '-' target: {row.old_path}")
        elif row.target_path == "-" or (
            row.state != "TASK7_PENDING"
            and not target_exists(root, row.target_path, public_api_items)
        ):
            fail(
                f"migration target does not exist: {row.old_path} -> {row.target_path}"
            )
    return rows


def verify_text_artifact(label: str, text: str) -> None:
    """Reject Physical names and the retired module in API/rustdoc output."""

    physical = FORBIDDEN_PHYSICAL.search(text)
    if physical:
        fail(f"{label} contains forbidden identity: {physical.group(0)}")
    if FORBIDDEN_SPI_TEXT.search(text) or re.search(r"\bdruid_rdbc\s*::\s*spi\b", text):
        fail(f"{label} contains forbidden druid_rdbc::spi module")


def public_api_items(text: str) -> set[str]:
    """Extract complete druid_rdbc paths from cargo-public-api's stable text format."""

    return set(re.findall(r"\bdruid_rdbc(?:::[A-Za-z_][A-Za-z0-9_]*)+", text))


def generate_public_api(root: Path) -> str:
    """Run the installed cargo-public-api with the repository's pinned nightly."""

    environment = os.environ.copy()
    environment["RUSTUP_TOOLCHAIN"] = PUBLIC_API_TOOLCHAIN
    result = run_checked(
        [
            "cargo",
            "public-api",
            "--manifest-path",
            "crates/druid-rdbc/Cargo.toml",
            "-sss",
            "--all-features",
        ],
        cwd=root,
        env=environment,
        label="cargo public-api",
    )
    return result.stdout


def generate_rustdoc_text(root: Path) -> str:
    """Build isolated current rustdoc and return all generated HTML for scanning."""

    with tempfile.TemporaryDirectory(prefix="druid-rdbc-task6-rustdoc-") as temporary:
        target = Path(temporary)
        environment = os.environ.copy()
        environment["RUSTDOCFLAGS"] = "-D warnings"
        run_checked(
            [
                "cargo",
                "doc",
                "-p",
                "druid-rdbc",
                "--all-features",
                "--no-deps",
                "--target-dir",
                str(target),
            ],
            cwd=root,
            env=environment,
            label="druid-rdbc rustdoc",
        )
        documentation = target / "doc" / "druid_rdbc"
        if not documentation.is_dir():
            fail("druid-rdbc rustdoc output is missing")
        html = sorted(documentation.rglob("*.html"))
        if not html:
            fail("druid-rdbc rustdoc output contains no HTML")
        return "\n".join(path.read_text(encoding="utf-8") for path in html)


def consumer_identity(row: dict[str, str]) -> tuple[str, ...]:
    """Ignore only regenerated token coordinates when authenticating closed history."""

    symbol = re.sub(r"@(?:qualified-)?token=.*$", "", row["symbol"])
    return (
        row["source_path"],
        row["occurrence_kind"],
        symbol,
        row["old_path"],
        row["target_path"],
        row["target_task"],
    )


def verify_final_consumer_history(root: Path, current: Path) -> int:
    """Require the exact frozen75 identities to remain CLOSED and no active transition."""

    baseline = run_checked(
        ["git", "show", f"{TASK6_BASE}:{current.as_posix()}"],
        cwd=root,
        label="Task 6 baseline consumer manifest",
    )
    baseline_rows = list(csv.DictReader(baseline.stdout.splitlines(), delimiter="\t"))
    frozen = [row for row in baseline_rows if row["state"] == TASK6_PENDING]
    if len(frozen) != 75:
        fail(f"Task 6 frozen consumer denominator expected 75, found {len(frozen)}")
    current_rows = read_tsv(root / current)
    active = [row for row in current_rows if row["state"] == TASK6_PENDING]
    if active:
        fail(f"active Task 6 transitional consumers remain: {len(active)}")
    closed = [row for row in current_rows if row["state"] == TASK6_CLOSED]
    if Counter(map(consumer_identity, closed)) != Counter(
        map(consumer_identity, frozen)
    ):
        fail(
            "Task 6 CLOSED consumer history does not equal frozen69 plus six additions"
        )
    return len(closed)


def verify_repository(root: Path) -> tuple[int, int, int, int, int, int]:
    """Run the complete source, API, rustdoc, migration, and history gate."""

    metadata = load_cargo_metadata(root)
    files = verify_source_surface(root, metadata)
    verify_no_duplicate_trait_tree(root, metadata)
    api = generate_public_api(root)
    verify_text_artifact("public API", api)
    snapshot = root / API_SNAPSHOT
    if not snapshot.is_file():
        fail(f"public API snapshot is missing: {API_SNAPSHOT}")
    checked_snapshot = snapshot.read_text(encoding="utf-8")
    if checked_snapshot != api:
        fail(
            "public API snapshot is stale: "
            f"checked={hashlib.sha256(checked_snapshot.encode()).hexdigest()} "
            f"live={hashlib.sha256(api.encode()).hexdigest()}"
        )
    rustdoc = generate_rustdoc_text(root)
    verify_text_artifact("rustdoc", rustdoc)
    run_checked(
        [sys.executable, "scripts/verify_task6_method_parity.py"],
        cwd=root,
        label="Task 6 exact method parity",
        timeout=None,
    )
    try:
        public_contract_rows, _, _ = public_contract.verify_repository(root)
    except (
        OSError,
        subprocess.TimeoutExpired,
        public_contract.VerificationError,
    ) as error:
        fail(f"Druid/wrapper/admin public contract failed: {error}")
    authority_rows = load_migration_authority(root / MIGRATION_AUTHORITY)
    verify_authority_schema(authority_rows)
    verify_standardization(
        authority_rows, tuple(read_tsv(root / PHYSICAL_MANIFEST))
    )
    verify_closed_targets(root, authority_rows, public_api_items(api))
    verify_handoff_ledgers(root, authority_rows)
    verify_derived_document(root, authority_rows)
    manifest_paths = task6_manifest_paths(
        root / PHYSICAL_MANIFEST, root / CONSUMER_MANIFEST
    )
    extras = baseline_extra_identities(root, manifest_paths)
    migration_rows = verify_migration_table(
        root,
        root / PHYSICAL_MANIFEST,
        root / CONSUMER_MANIFEST,
        root / MIGRATION_DOCUMENT,
        public_api_items=public_api_items(api),
        required_extra=extras,
    )
    authority_identity = {
        (
            row["old_path"],
            row["disposition"],
            row["target_path"],
            row["target_task"],
            row["state"],
            row["evidence"],
        )
        for row in authority_rows
    }
    document_identity = {
        (
            row.old_path,
            row.disposition,
            row.target_path,
            row.target_task,
            row.state,
            row.evidence,
        )
        for row in migration_rows
    }
    if document_identity != authority_identity:
        fail("derived migration document does not match the authoritative TSV")
    closed = verify_final_consumer_history(root, CONSUMER_MANIFEST)
    try:
        base_tests = deleted_test_parity.derive_base_tests(root)
        test_rows = deleted_test_parity.verify_ledger(
            base_tests,
            deleted_test_parity.parse_ledger(root / deleted_test_parity.DEFAULT_LEDGER),
            root,
        )
        support_targets = deleted_test_parity.verify_zero_test_support(root)
        deleted_test_parity.execute_targets(root, test_rows, support_targets)
    except deleted_test_parity.VerificationError as error:
        fail(f"exact deleted-test parity failed: {error}")
    return (
        len(files),
        len(migration_rows),
        len(extras),
        closed,
        len(test_rows),
        public_contract_rows,
    )


def main() -> int:
    """CLI entry point."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root", type=Path, default=Path(__file__).resolve().parent.parent
    )
    arguments = parser.parse_args()
    try:
        files, migration_rows, extras, closed, test_rows, public_contract_rows = (
            verify_repository(arguments.root.resolve())
        )
    except (OSError, UnicodeError, VerificationError) as error:
        print(f"NO_PHYSICAL_STANDARD_FAIL={error}")
        return 1
    print(f"PRODUCTION_RUST_FILES_SCANNED={files}")
    print("PUBLIC_PHYSICAL_NAMES=0")
    print("PRODUCTION_PHYSICAL_NAMES=0")
    print("RUSTDOC_PHYSICAL_NAMES=0")
    print("DRUID_RDBC_SPI_MODULES=0")
    print(f"MIGRATION_IDENTITIES_CLOSED={migration_rows}")
    print(f"NEWLY_DISCOVERED_IDENTITIES_CLOSED={extras}")
    print(f"TRANSITIONAL_CONSUMER_IDENTITIES_CLOSED={closed}")
    print(f"DELETED_TEST_FUNCTION_MIGRATIONS={test_rows}")
    print(f"PUBLIC_CONFIG_WIRE_BREAKS_DISPOSITIONED={public_contract_rows}")
    print("ACTIVE_TRANSITIONAL_CONSUMERS=0")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
