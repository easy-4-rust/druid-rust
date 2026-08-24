#!/usr/bin/env python3
"""Verify the P04 Task 1 exit from the transitional ``DruidError`` waiver.

The disposition denominator is derived from Rust identifier tokens in the
frozen Git baseline. Comments and string/character literals therefore cannot
create false production rows. Each row names one production file and the
required disposition for every baseline ``DruidError`` identifier in it.
"""

from __future__ import annotations

import argparse
import hashlib
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import NoReturn

from verify_rdbc_public_paths import VerificationError as RustLexError
from verify_rdbc_public_paths import tokenize


BASELINE = "bb91193d9bb488b107b8c9ac6dd56f8756ed9593"
MANIFEST_HEADER = (
    "path\tdomain\tbaseline_occurrences\tbaseline_sha256\tdisposition"
)
RDBC_SOURCE_ROOT = Path("crates/druid-rdbc/src")
WRAPPER_SOURCE_ROOT = Path("crates/druid-wrapper/src")
COMPAT_DEFINITION = Path("crates/druid-rdbc/src/compat/druid_error.rs")
COMPAT_FACADE = Path("crates/druid-rdbc/src/compat/mod.rs")
PRODUCT_ERROR = Path("crates/druid/src/core/error.rs")
PRODUCT_FACADE = Path("crates/druid/src/core/mod.rs")
DOMAINS = {"RDBC_COMPAT", "RDBC_STANDARD", "WRAPPER_ADAPTER"}
DISPOSITIONS = {
    "DELETE_COMPAT_DEFINITION",
    "REMOVE_COMPAT_EXPORT",
    "USE_RDBC_ERROR",
}


class VerificationError(RuntimeError):
    """Raised when the waiver-exit evidence is missing or inconsistent."""


@dataclass(frozen=True)
class DispositionRow:
    """One baseline production file containing live ``DruidError`` tokens."""

    path: Path
    domain: str
    baseline_occurrences: int
    baseline_sha256: str
    disposition: str


@dataclass(frozen=True)
class GateResult:
    """One exact waiver-exit gate and its diagnostic result."""

    gate: str
    passed: bool
    detail: str


def repository_root() -> Path:
    """Return the repository root independent of the caller's directory."""

    return Path(__file__).resolve().parent.parent


def fail(message: str) -> NoReturn:
    """Raise a verification failure with a stable diagnostic."""

    raise VerificationError(message)


def run_git(root: Path, arguments: list[str]) -> bytes:
    """Run one read-only Git command and return its exact stdout bytes."""

    process = subprocess.run(
        ["git", *arguments],
        cwd=root,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    if process.returncode != 0:
        diagnostic = process.stderr.decode("utf-8", errors="replace").strip()
        fail(f"git {' '.join(arguments)} failed: {diagnostic}")
    return process.stdout


def rust_identifier_count(source: bytes, identifier: str) -> int:
    """Count one Rust identifier outside comments and literals."""

    try:
        text = source.decode("utf-8")
        tokens = tokenize(text)
    except (UnicodeError, RustLexError) as error:
        fail(f"cannot tokenize Rust source: {error}")
    return sum(token.kind == "IDENT" and token.text == identifier for token in tokens)


def rust_files_at_baseline(root: Path, source_root: Path) -> list[Path]:
    """List tracked Rust production files below one root at the frozen baseline."""

    output = run_git(
        root,
        ["ls-tree", "-r", "--name-only", BASELINE, "--", source_root.as_posix()],
    ).decode("utf-8")
    return sorted(
        Path(line)
        for line in output.splitlines()
        if line.endswith(".rs")
    )


def baseline_source(root: Path, path: Path) -> bytes:
    """Read one tracked file exactly as stored in the frozen baseline."""

    return run_git(root, ["show", f"{BASELINE}:{path.as_posix()}"])


def classify(path: Path) -> tuple[str, str]:
    """Return the reviewed domain and required disposition for one live row."""

    if path == COMPAT_DEFINITION:
        return "RDBC_COMPAT", "DELETE_COMPAT_DEFINITION"
    if path == COMPAT_FACADE:
        return "RDBC_COMPAT", "REMOVE_COMPAT_EXPORT"
    if path.is_relative_to(RDBC_SOURCE_ROOT):
        return "RDBC_STANDARD", "USE_RDBC_ERROR"
    if path.is_relative_to(WRAPPER_SOURCE_ROOT):
        return "WRAPPER_ADAPTER", "USE_RDBC_ERROR"
    fail(f"unclassified production row: {path}")


def derive_baseline_rows(root: Path) -> list[DispositionRow]:
    """Derive the complete frozen Rust-token denominator from Git."""

    rows: list[DispositionRow] = []
    for source_root in (RDBC_SOURCE_ROOT, WRAPPER_SOURCE_ROOT):
        for path in rust_files_at_baseline(root, source_root):
            source = baseline_source(root, path)
            occurrences = rust_identifier_count(source, "DruidError")
            if occurrences == 0:
                continue
            domain, disposition = classify(path)
            rows.append(
                DispositionRow(
                    path=path,
                    domain=domain,
                    baseline_occurrences=occurrences,
                    baseline_sha256=hashlib.sha256(source).hexdigest(),
                    disposition=disposition,
                )
            )
    if not rows:
        fail("frozen baseline produced no live DruidError rows")
    return sorted(rows, key=lambda row: row.path.as_posix())


def render_manifest(rows: list[DispositionRow]) -> bytes:
    """Render deterministic TSV bytes for the derived live denominator."""

    lines = [MANIFEST_HEADER]
    lines.extend(
        "\t".join(
            (
                row.path.as_posix(),
                row.domain,
                str(row.baseline_occurrences),
                row.baseline_sha256,
                row.disposition,
            )
        )
        for row in rows
    )
    return ("\n".join(lines) + "\n").encode("utf-8")


def emit_live(root: Path, destination: Path) -> None:
    """Write the exact frozen production denominator to ``destination``."""

    rows = derive_baseline_rows(root)
    payload = render_manifest(rows)
    output = destination if destination.is_absolute() else root / destination
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_bytes(payload)
    print(f"LIVE_ROWS={len(rows)}")
    print(f"LIVE_SHA256={hashlib.sha256(payload).hexdigest()}")


def load_manifest(root: Path, manifest: Path) -> tuple[list[DispositionRow], bytes]:
    """Load and strictly validate one checked-in disposition manifest."""

    source = manifest if manifest.is_absolute() else root / manifest
    if not source.is_file():
        fail(f"disposition manifest not found: {manifest}")
    payload = source.read_bytes()
    try:
        lines = payload.decode("utf-8").splitlines()
    except UnicodeError as error:
        fail(f"disposition manifest is not UTF-8: {error}")
    if not lines or lines[0] != MANIFEST_HEADER:
        fail("disposition manifest has an invalid header")
    rows: list[DispositionRow] = []
    seen: set[Path] = set()
    for line_number, line in enumerate(lines[1:], start=2):
        fields = line.split("\t")
        if len(fields) != 5:
            fail(f"disposition row {line_number} must have five fields")
        path_text, domain, occurrences_text, digest, disposition = fields
        path = Path(path_text)
        if (
            not path_text
            or path.is_absolute()
            or ".." in path.parts
            or path.suffix != ".rs"
        ):
            fail(f"disposition row {line_number} has an invalid path")
        if path in seen:
            fail(f"disposition row {line_number} duplicates {path}")
        seen.add(path)
        if domain not in DOMAINS:
            fail(f"disposition row {line_number} has an invalid domain")
        if disposition not in DISPOSITIONS:
            fail(f"disposition row {line_number} has an invalid disposition")
        try:
            occurrences = int(occurrences_text)
        except ValueError:
            fail(f"disposition row {line_number} has an invalid occurrence count")
        if occurrences <= 0:
            fail(f"disposition row {line_number} must describe a live token")
        if (
            len(digest) != 64
            or digest != digest.lower()
            or any(character not in "0123456789abcdef" for character in digest)
        ):
            fail(f"disposition row {line_number} has an invalid SHA-256")
        rows.append(DispositionRow(path, domain, occurrences, digest, disposition))
    expected = derive_baseline_rows(root)
    if rows != expected:
        fail("disposition manifest does not match the frozen live denominator")
    return rows, payload


def current_rust_sources(root: Path, source_root: Path) -> list[Path]:
    """List current production Rust files without traversing generated outputs."""

    directory = root / source_root
    if not directory.is_dir():
        fail(f"production source root not found: {source_root}")
    return sorted(path.relative_to(root) for path in directory.rglob("*.rs"))


def current_identifier_rows(
    root: Path, source_root: Path, identifier: str
) -> dict[Path, int]:
    """Return current production files containing one live Rust identifier."""

    rows: dict[Path, int] = {}
    for path in current_rust_sources(root, source_root):
        count = rust_identifier_count((root / path).read_bytes(), identifier)
        if count:
            rows[path] = count
    return rows


def contains_sequence(source: bytes, sequence: tuple[str, ...]) -> bool:
    """Return whether token text contains one contiguous syntax sequence."""

    try:
        texts = [token.text for token in tokenize(source.decode("utf-8"))]
    except (UnicodeError, RustLexError) as error:
        fail(f"cannot tokenize Rust source: {error}")
    width = len(sequence)
    return any(tuple(texts[index:index + width]) == sequence for index in range(len(texts) - width + 1))


def exports_named_item(source: bytes, module: str, item: str) -> bool:
    """Recognize direct or grouped public re-exports of one named Rust item."""

    try:
        texts = [token.text for token in tokenize(source.decode("utf-8"))]
    except (UnicodeError, RustLexError) as error:
        fail(f"cannot tokenize Rust source: {error}")
    direct = ("pub", "use", module, "::", item)
    if any(
        tuple(texts[index:index + len(direct)]) == direct
        for index in range(len(texts) - len(direct) + 1)
    ):
        return True
    grouped = ("pub", "use", module, "::", "{")
    for index in range(len(texts) - len(grouped) + 1):
        if tuple(texts[index:index + len(grouped)]) != grouped:
            continue
        cursor = index + len(grouped)
        while cursor < len(texts) and texts[cursor] != "}":
            if texts[cursor] == item:
                return True
            cursor += 1
    return False


def gate_compat_removed(root: Path) -> GateResult:
    """Require deletion of the transitional definition and export."""

    failures: list[str] = []
    if (root / COMPAT_DEFINITION).exists():
        failures.append(f"still exists: {COMPAT_DEFINITION}")
    compat_rows = current_identifier_rows(root, RDBC_SOURCE_ROOT, "DruidError")
    if compat_rows:
        failures.append(f"live DruidError remains in {len(compat_rows)} druid-rdbc file(s)")
    return GateResult(
        "WAIVER_EXIT_GATE_1_COMPAT_REMOVED",
        not failures,
        "; ".join(failures) if failures else "transitional definition/export absent",
    )


def gate_standard_clean(root: Path) -> GateResult:
    """Require the RDBC production tree to own and use only ``RdbcError``."""

    failures: list[str] = []
    druid_rows = current_identifier_rows(root, RDBC_SOURCE_ROOT, "DruidError")
    if druid_rows:
        failures.append(f"{len(druid_rows)} standard file(s) still use DruidError")
    error_source = root / "crates/druid-rdbc/src/error/rdbc_error.rs"
    facade_source = root / "crates/druid-rdbc/src/error/mod.rs"
    if not error_source.is_file() or not contains_sequence(
        error_source.read_bytes(), ("pub", "enum", "RdbcError")
    ):
        failures.append("RdbcError is not a public enum in error/rdbc_error.rs")
    if not facade_source.is_file() or not exports_named_item(
        facade_source.read_bytes(), "rdbc_error", "RdbcError"
    ):
        failures.append("error/mod.rs does not export RdbcError")
    return GateResult(
        "WAIVER_EXIT_GATE_2_STANDARD_CLEAN",
        not failures,
        "; ".join(failures) if failures else "standard tree uses RdbcError",
    )


def gate_adapters_clean(root: Path) -> GateResult:
    """Require every wrapper production row to use ``RdbcError``."""

    druid_rows = current_identifier_rows(root, WRAPPER_SOURCE_ROOT, "DruidError")
    rdbc_rows = current_identifier_rows(root, WRAPPER_SOURCE_ROOT, "RdbcError")
    failures: list[str] = []
    if druid_rows:
        failures.append(f"{len(druid_rows)} adapter file(s) still use DruidError")
    if not rdbc_rows:
        failures.append("wrapper has no live RdbcError production rows")
    return GateResult(
        "WAIVER_EXIT_GATE_3_ADAPTERS_CLEAN",
        not failures,
        "; ".join(failures) if failures else f"{len(rdbc_rows)} adapter file(s) use RdbcError",
    )


def druid_error_definitions(root: Path) -> list[Path]:
    """Find every current product ``enum DruidError`` definition."""

    definitions: list[Path] = []
    source_root = Path("crates/druid/src")
    for path in current_rust_sources(root, source_root):
        if contains_sequence((root / path).read_bytes(), ("enum", "DruidError")):
            definitions.append(path)
    return definitions


def gate_product_owner_unique(root: Path) -> GateResult:
    """Require one Druid-owned product definition and its local facade export."""

    failures: list[str] = []
    definitions = druid_error_definitions(root)
    if definitions != [PRODUCT_ERROR]:
        failures.append(
            "DruidError definitions are not uniquely owned by core/error.rs: "
            + ",".join(path.as_posix() for path in definitions)
        )
    facade = root / PRODUCT_FACADE
    if not facade.is_file() or not contains_sequence(
        facade.read_bytes(), ("pub", "use", "error", "::", "DruidError")
    ):
        failures.append("core/mod.rs does not locally export error::DruidError")
    product = root / PRODUCT_ERROR
    if not product.is_file() or not contains_sequence(
        product.read_bytes(), ("Rdbc", "(", "RdbcError", ")")
    ):
        failures.append("DruidError does not contain the single RdbcError wrapper variant")
    return GateResult(
        "WAIVER_EXIT_GATE_4_PRODUCT_OWNER_UNIQUE",
        not failures,
        "; ".join(failures) if failures else "Druid uniquely owns DruidError and wraps RdbcError",
    )


def gate_baseline_disposed(root: Path, rows: list[DispositionRow]) -> GateResult:
    """Require every frozen live row to realize its reviewed disposition."""

    failures: list[str] = []
    for row in rows:
        source = root / row.path
        if row.disposition == "DELETE_COMPAT_DEFINITION":
            if source.exists():
                failures.append(f"not deleted: {row.path}")
            continue
        if not source.is_file():
            failures.append(f"missing disposed production file: {row.path}")
            continue
        druid_count = rust_identifier_count(source.read_bytes(), "DruidError")
        rdbc_count = rust_identifier_count(source.read_bytes(), "RdbcError")
        if druid_count:
            failures.append(f"DruidError remains: {row.path}")
        if row.disposition == "USE_RDBC_ERROR" and rdbc_count == 0:
            failures.append(f"RdbcError replacement absent: {row.path}")
    return GateResult(
        "WAIVER_EXIT_GATE_5_BASELINE_DISPOSED",
        not failures,
        "; ".join(failures[:8]) if failures else f"all {len(rows)} baseline rows disposed",
    )


def run_contract_command(root: Path, command: list[str]) -> tuple[bool, str]:
    """Run one focused contract test and retain a compact diagnostic."""

    process = subprocess.run(
        command,
        cwd=root,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    lines = [line.strip() for line in process.stdout.splitlines() if line.strip()]
    detail = " | ".join(lines[-4:]) if lines else f"exit {process.returncode}"
    return process.returncode == 0, detail


def gate_compat_verified(root: Path, prerequisites_passed: bool) -> GateResult:
    """Run the two focused executable error contracts after static ownership passes."""

    if not prerequisites_passed:
        return GateResult(
            "WAIVER_EXIT_GATE_6_COMPAT_VERIFIED",
            False,
            "ownership/disposition prerequisites are not green",
        )
    commands = (
        ["cargo", "test", "-p", "druid-rdbc", "--test", "error_contract_test"],
        ["cargo", "test", "-p", "druid", "--test", "druid_error_transition_test"],
    )
    failures: list[str] = []
    for command in commands:
        passed, detail = run_contract_command(root, command)
        if not passed:
            failures.append(f"{' '.join(command)}: {detail}")
    return GateResult(
        "WAIVER_EXIT_GATE_6_COMPAT_VERIFIED",
        not failures,
        "; ".join(failures) if failures else "focused RDBC and Druid error contracts pass",
    )


def render_exit_report(
    manifest: Path,
    rows: list[DispositionRow],
    payload: bytes,
    gates: list[GateResult],
) -> str:
    """Render the canonical human-readable waiver-exit report."""

    domain_counts = {
        domain: sum(row.domain == domain for row in rows) for domain in sorted(DOMAINS)
    }
    lines = [
        "# RDBC error waiver exit",
        "",
        f"- Baseline: `{BASELINE}`",
        f"- Disposition: `{manifest.as_posix()}`",
        f"- Live rows: `{len(rows)}`",
        f"- Manifest SHA-256: `{hashlib.sha256(payload).hexdigest()}`",
        "- Domain rows: " + ", ".join(f"`{key}={value}`" for key, value in domain_counts.items()),
        "",
        "## Canonical gates",
        "",
    ]
    lines.extend(
        f"- `{gate.gate}`: {'PASS' if gate.passed else 'FAIL'} — {gate.detail}"
        for gate in gates
    )
    lines.append("")
    return "\n".join(lines)


def verify(root: Path, manifest: Path, exit_report: Path | None) -> bool:
    """Verify all six exact gates and optionally write the canonical report."""

    rows, payload = load_manifest(root, manifest)
    gates = [
        gate_compat_removed(root),
        gate_standard_clean(root),
        gate_adapters_clean(root),
        gate_product_owner_unique(root),
    ]
    gates.append(gate_baseline_disposed(root, rows))
    gates.append(gate_compat_verified(root, all(gate.passed for gate in gates)))
    for gate in gates:
        print(f"{gate.gate}={'PASS' if gate.passed else 'FAIL'} {gate.detail}")
    print(f"LIVE_ROWS={len(rows)}")
    print(f"LIVE_SHA256={hashlib.sha256(payload).hexdigest()}")
    passed = all(gate.passed for gate in gates)
    if exit_report is not None:
        if not passed:
            fail("refusing to write an exit report while a waiver gate is red")
        output = exit_report if exit_report.is_absolute() else root / exit_report
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(
            render_exit_report(manifest, rows, payload, gates), encoding="utf-8"
        )
    return passed


def parse_args() -> argparse.Namespace:
    """Parse the task's emit/verify command-line contract."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--emit-live",
        metavar="PATH",
        type=Path,
        help="write the frozen live production-row disposition TSV",
    )
    parser.add_argument(
        "--disposition",
        metavar="PATH",
        type=Path,
        help="verify the checked-in disposition TSV",
    )
    parser.add_argument(
        "--exit-report",
        metavar="PATH",
        type=Path,
        help="write the canonical report after all gates pass",
    )
    args = parser.parse_args()
    if (args.emit_live is None) == (args.disposition is None):
        parser.error("choose exactly one of --emit-live or --disposition")
    if args.exit_report is not None and args.disposition is None:
        parser.error("--exit-report requires --disposition")
    return args


def main() -> int:
    """Run the requested verifier mode with stable failure reporting."""

    args = parse_args()
    root = repository_root()
    try:
        if args.emit_live is not None:
            emit_live(root, args.emit_live)
            return 0
        return 0 if verify(root, args.disposition, args.exit_report) else 1
    except VerificationError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
