#!/usr/bin/env python3
"""Compare actual cargo-semver-checks findings with reviewed pre-release breaks."""

from __future__ import annotations

import argparse
import csv
import hashlib
import io
import re
import sys
from collections import Counter
from pathlib import Path

ANSI_ESCAPE = re.compile(r"\x1b\[[0-9;]*m")
SECTION = re.compile(r"(?m)^--- failure ([a-z_]+):[^\n]*$")
WORKSPACE_PREFIX = re.compile(
    r"/home/runner/work/druid-rust/druid-rust/"
    r"(?:target/semver-checks/git-[^/]+/[^/]+/)?"
)
WINDOWS_PREFIX = re.compile(
    r"[A-Za-z]:/(?:[^/\n]+/)*"
    r"(?:druid-rust/(?:target/semver-checks/git-[^/]+/[^/]+/)?|"
    r"semver-checks/git-[^/]+/[^/]+/)(?=crates/)"
)
SOURCE_LINE = re.compile(r"(?<=\.rs):\d+")
FAILURE_SUMMARY = re.compile(
    r"Summary semver requires new (?:major|minor) version: "
    r"(\d+) major and (\d+) minor checks failed"
)
PASS_SUMMARY = re.compile(r"Summary no semver update required")
HEADER = ("crate", "lint", "count", "sha256", "status", "reason")
MODULE_MAP_HEADER = ("old_module", "target_file", "transition", "test_file")
MODULE_FINDING = re.compile(r"(?m)^  mod (druid::[A-Za-z0-9_:]+), previously in file ")
MODULE_TRANSITIONS = {"STANDARD_PORT", "PRIVATE_OWNER", "SPLIT_STANDARD"}
STRUCT_MAP_HEADER = ("old_type", "target_file", "target_symbol", "transition", "test_file")
STRUCT_FINDING = re.compile(
    r"(?m)^  struct (druid::[A-Za-z0-9_:]+), previously in file ([^\n]+)$"
)
STRUCT_TRANSITIONS = {"STANDARD_PORT", "PRIVATE_OWNER", "RENAMED"}
ARITY_MAP_HEADER = ("owner", "findings", "shifts", "test_files")
ARITY_FINDING = re.compile(
    r"(?m)^  (druid::[A-Za-z0-9_:]+)::([A-Za-z0-9_]+) takes ([0-9]+) "
    r"parameters in [^\n]+, but now takes ([0-9]+) parameters in [^\n]+$"
)
METHOD_MIGRATION_HEADER = (
    "crate",
    "lint",
    "old_identity",
    "baseline_source",
    "baseline_line",
    "report_occurrences",
    "disposition",
    "standard_contract_entry",
)
METHOD_BREAK_LINTS = {"inherent_method_missing", "trait_method_missing"}
METHOD_MIGRATION_LEDGER = Path("api/task6-prerelease-method-migration.tsv")
METHOD_FINDING = re.compile(
    r"^  (?P<identity>.+), previously in file (?P<source>.+\.rs):(?P<line>[0-9]+)$"
)


def normalize_report_path(value: str) -> str:
    """Make approved Linux and Windows SemVer checkout paths repository-relative."""

    forward_slashes = value.replace("\\", "/")
    return WINDOWS_PREFIX.sub("", WORKSPACE_PREFIX.sub("", forward_slashes))


def findings_by_lint(report: str, exit_code: int) -> dict[str, tuple[int, str]]:
    """Return a count and order-independent digest for each failed lint."""
    report = ANSI_ESCAPE.sub("", report)
    sections = list(SECTION.finditer(report))
    summary = FAILURE_SUMMARY.search(report)
    if exit_code == 0 and not sections and PASS_SUMMARY.search(report):
        return {}
    if summary is None or len(sections) != sum(map(int, summary.groups())):
        raise ValueError("SemVer report is incomplete or does not list every failed lint")
    if exit_code != 100:
        raise ValueError(f"SemVer failed-lint exit code must be 100, got {exit_code}")

    result: dict[str, tuple[int, str]] = {}
    for index, section in enumerate(sections):
        lint = section.group(1)
        if lint in result:
            raise ValueError(f"duplicate SemVer lint section: {lint}")
        end = sections[index + 1].start() if index + 1 < len(sections) else len(report)
        body = report[section.end() : end]
        if "Failed in:" not in body:
            raise ValueError(f"SemVer lint has no findings: {lint}")
        findings = []
        for line in body.split("Failed in:", 1)[1].splitlines():
            if not line.startswith("  ") or line.lstrip().startswith(("Summary ", "Finished ")):
                continue
            finding = normalize_report_path(line.strip())
            finding = SOURCE_LINE.sub(":<line>", finding)
            findings.append(finding)
        if not findings:
            raise ValueError(f"SemVer lint has no findings: {lint}")
        payload = "\n".join(sorted(findings)).encode("utf-8")
        result[lint] = (len(findings), hashlib.sha256(payload).hexdigest())
    return result


def read_allowlist(path: Path) -> dict[tuple[str, str], tuple[int, str, str, str]]:
    with path.open(encoding="utf-8", newline="") as stream:
        reader = csv.DictReader(stream, delimiter="\t")
        if tuple(reader.fieldnames or ()) != HEADER:
            raise ValueError("breaking allowlist has an unexpected TSV header")
        result = {}
        for row in reader:
            key = (row["crate"], row["lint"])
            if key in result:
                raise ValueError(f"duplicate breaking allowlist row: {key}")
            result[key] = (
                int(row["count"]),
                row["sha256"],
                row["status"],
                row["reason"],
            )
    return result


def method_migration_rows(report: str) -> tuple[tuple[str, ...], ...]:
    """Aggregate exact retired-method identities while preserving SemVer occurrence counts."""

    report = ANSI_ESCAPE.sub("", report)
    sections = list(SECTION.finditer(report))
    observed: Counter[tuple[str, str, str, str]] = Counter()
    for index, section in enumerate(sections):
        lint = section.group(1)
        if lint not in METHOD_BREAK_LINTS:
            continue
        end = sections[index + 1].start() if index + 1 < len(sections) else len(report)
        body = report[section.end() : end]
        if "Failed in:" not in body:
            raise ValueError(f"SemVer method lint has no findings: {lint}")
        finding_lines = [line for line in body.split("Failed in:", 1)[1].splitlines() if line.startswith("  ")]
        parsed = 0
        for line in finding_lines:
            match = METHOD_FINDING.fullmatch(line)
            if match is None:
                raise ValueError(f"unrecognized {lint} finding: {line.strip()}")
            source = normalize_report_path(match.group("source"))
            source = SOURCE_LINE.sub(":<line>", source)
            if (
                Path(source).is_absolute()
                or re.match(r"^[A-Za-z]:/", source)
                or ".." in Path(source).parts
            ):
                raise ValueError(f"SemVer source path is not repository-relative: {source}")
            key = (lint, match.group("identity"), source, match.group("line"))
            observed[key] += 1
            parsed += 1
        if parsed == 0:
            raise ValueError(f"SemVer method lint has no findings: {lint}")

    if not observed:
        raise ValueError("SemVer report has no retired-method findings")
    rows = []
    for (lint, identity, source, line), occurrences in sorted(observed.items()):
        route = standard_contract_entry(lint, identity)
        disposition = (
            "APPROVED_PRE_RELEASE_STANDARD_MIGRATION"
            if route.startswith("druid::DataSource")
            else "APPROVED_PRE_RELEASE_INTERNAL_RETIREMENT"
        )
        rows.append(
            (
                "druid",
                lint,
                identity,
                source,
                line,
                str(occurrences),
                disposition,
                route,
            )
        )
    return tuple(rows)


def standard_contract_entry(lint: str, identity: str) -> str:
    """Describe the standard contract family or internal owner replacing an old path."""

    if lint == "trait_method_missing":
        return "retired internal filter/statistics extension surface"
    owner = identity.split("::", 1)[0]
    standard_owners = (
        ("DruidPooledConnection", "druid::DataSource -> druid_rdbc::connection::Connection"),
        ("DruidPooledPreparedStatement", "druid::DataSource -> druid_rdbc::statement::PreparedStatement"),
        ("DruidPooledCallableStatement", "druid::DataSource -> druid_rdbc::statement::CallableStatement"),
        ("DruidPooledStatement", "druid::DataSource -> druid_rdbc::statement::Statement"),
        ("DruidPooledResultSet", "druid::DataSource -> druid_rdbc::result_set::ResultSet"),
        ("DatabaseMetaDataProxyImpl", "druid::DataSource -> druid_rdbc::metadata::DatabaseMetaData"),
    )
    for prefix, target in standard_owners:
        if owner == prefix:
            return target
    return "retired internal pool/filter implementation surface"


def render_method_migration_ledger(report: str) -> bytes:
    """Render deterministic item-level evidence for all old-method findings."""

    output = io.StringIO(newline="")
    writer = csv.writer(output, delimiter="\t", lineterminator="\n")
    writer.writerow(METHOD_MIGRATION_HEADER)
    writer.writerows(method_migration_rows(report))
    return output.getvalue().encode("utf-8")


def verify_method_migration_ledger(report: str, path: Path) -> None:
    """Require the checked-in item ledger to match the full live SemVer report."""

    expected = render_method_migration_ledger(report)
    if not path.is_file() or path.read_bytes() != expected:
        raise ValueError("retired-method migration ledger is missing or differs from SemVer findings")


def verify_module_migration_map(report: str, path: Path, root: Path) -> None:
    """Bind every retired druid module to an existing target and regression test."""
    report = ANSI_ESCAPE.sub("", report)
    sections = list(SECTION.finditer(report))
    matching = [index for index, match in enumerate(sections) if match.group(1) == "module_missing"]
    if len(matching) != 1:
        raise ValueError("expected one druid/module_missing report section")
    index = matching[0]
    end = sections[index + 1].start() if index + 1 < len(sections) else len(report)
    body = report[sections[index].end() : end]
    modules = MODULE_FINDING.findall(body)
    finding_lines = [line for line in body.splitlines() if line.startswith("  mod ")]
    if not modules or len(modules) != len(set(modules)):
        raise ValueError("module_missing paths are empty or duplicated")
    if len(modules) != len(finding_lines):
        raise ValueError("unrecognized module_missing finding")

    with path.open(encoding="utf-8", newline="") as stream:
        reader = csv.DictReader(stream, delimiter="\t")
        if tuple(reader.fieldnames or ()) != MODULE_MAP_HEADER:
            raise ValueError("module migration map has an unexpected TSV header")
        rows = list(reader)
    mapped: set[str] = set()
    for row in rows:
        old_module = row["old_module"]
        if old_module in mapped:
            raise ValueError(f"duplicate module migration: {old_module}")
        mapped.add(old_module)
        if row["transition"] not in MODULE_TRANSITIONS:
            raise ValueError(f"unknown module transition: {old_module}")
        for column in ("target_file", "test_file"):
            relative = Path(row[column])
            if relative.is_absolute() or ".." in relative.parts or relative.suffix != ".rs":
                raise ValueError(f"invalid module {column}: {old_module}")
            if not (root / relative).is_file():
                raise ValueError(f"missing module {column}: {old_module}: {relative}")
    if mapped != set(modules):
        raise ValueError(
            f"module migration mismatch: missing={sorted(set(modules) - mapped)} "
            f"extra={sorted(mapped - set(modules))}"
        )


def verify_struct_migration_map(report: str, path: Path, root: Path) -> None:
    """Bind each retired Druid struct identity to one concrete migration target."""
    report = ANSI_ESCAPE.sub("", report)
    sections = list(SECTION.finditer(report))
    matching = [index for index, match in enumerate(sections) if match.group(1) == "struct_missing"]
    if len(matching) != 1:
        raise ValueError("expected one druid/struct_missing report section")
    index = matching[0]
    end = sections[index + 1].start() if index + 1 < len(sections) else len(report)
    body = report[sections[index].end() : end]
    findings = STRUCT_FINDING.findall(body)
    paths = [item for item, _ in findings]
    finding_lines = [line for line in body.splitlines() if line.startswith("  struct ")]
    if not paths or len(paths) != len(set(paths)) or len(paths) != len(finding_lines):
        raise ValueError("struct_missing paths are empty, duplicated, or unrecognized")
    names = {item.rsplit("::", 1)[-1] for item in paths}
    source_by_name: dict[str, str] = {}
    for item, source in findings:
        name = item.rsplit("::", 1)[-1]
        normalized_source = SOURCE_LINE.sub(":<line>", WORKSPACE_PREFIX.sub("", source))
        if name in source_by_name and source_by_name[name] != normalized_source:
            raise ValueError(f"ambiguous retired struct identity: {name}")
        source_by_name[name] = normalized_source

    with path.open(encoding="utf-8", newline="") as stream:
        reader = csv.DictReader(stream, delimiter="\t")
        if tuple(reader.fieldnames or ()) != STRUCT_MAP_HEADER:
            raise ValueError("struct migration map has an unexpected TSV header")
        rows = list(reader)
    mapped: set[str] = set()
    for row in rows:
        old_type = row["old_type"]
        target_symbol = row["target_symbol"]
        if old_type in mapped:
            raise ValueError(f"duplicate struct migration: {old_type}")
        mapped.add(old_type)
        if row["transition"] not in STRUCT_TRANSITIONS:
            raise ValueError(f"unknown struct transition: {old_type}")
        if not re.fullmatch(r"[A-Za-z][A-Za-z0-9_]*", target_symbol):
            raise ValueError(f"invalid target symbol: {old_type}")
        for column in ("target_file", "test_file"):
            relative = Path(row[column])
            if relative.is_absolute() or ".." in relative.parts or relative.suffix != ".rs":
                raise ValueError(f"invalid struct {column}: {old_type}")
            if not (root / relative).is_file():
                raise ValueError(f"missing struct {column}: {old_type}: {relative}")
        source = (root / row["target_file"]).read_text(encoding="utf-8")
        declaration = re.compile(
            rf"(?m)^\s*(?:pub(?:\([^)]*\))?\s+)?"
            rf"(?:struct|trait|enum|type)\s+{re.escape(target_symbol)}\b"
        )
        if not declaration.search(source):
            raise ValueError(f"missing migration target symbol: {old_type}:{target_symbol}")
    if mapped != names:
        raise ValueError(
            f"struct migration mismatch: missing={sorted(names - mapped)} "
            f"extra={sorted(mapped - names)}"
        )


def verify_method_arity_migration_map(report: str, path: Path, root: Path) -> None:
    """Bind changed call arities to exact owner and old/new arity counts."""
    report = ANSI_ESCAPE.sub("", report)
    sections = list(SECTION.finditer(report))
    matching = [
        index for index, match in enumerate(sections)
        if match.group(1) == "method_parameter_count_changed"
    ]
    if len(matching) != 1:
        raise ValueError("expected one druid/method_parameter_count_changed report section")
    index = matching[0]
    end = sections[index + 1].start() if index + 1 < len(sections) else len(report)
    body = report[sections[index].end() : end]
    findings = ARITY_FINDING.findall(body)
    finding_lines = [line for line in body.splitlines() if line.startswith("  druid::")]
    if not findings or len(findings) != len(finding_lines):
        raise ValueError("method arity findings are empty or unrecognized")
    observed: dict[str, Counter[str]] = {}
    for owner_path, _, old, new in findings:
        owner = owner_path.rsplit("::", 1)[-1]
        observed.setdefault(owner, Counter())[f"{old}>{new}"] += 1

    with path.open(encoding="utf-8", newline="") as stream:
        reader = csv.DictReader(stream, delimiter="\t")
        if tuple(reader.fieldnames or ()) != ARITY_MAP_HEADER:
            raise ValueError("method arity map has an unexpected TSV header")
        rows = list(reader)
    mapped: set[str] = set()
    for row in rows:
        owner = row["owner"]
        if owner in mapped:
            raise ValueError(f"duplicate method arity owner: {owner}")
        mapped.add(owner)
        test_paths = row["test_files"].split(";")
        if not test_paths or len(test_paths) != len(set(test_paths)):
            raise ValueError(f"duplicate or empty method arity test_files: {owner}")
        for test_path in test_paths:
            relative = Path(test_path)
            if relative.is_absolute() or ".." in relative.parts or relative.suffix != ".rs":
                raise ValueError(f"invalid method arity test_file: {owner}")
            if not (root / relative).is_file():
                raise ValueError(f"missing method arity test_file: {owner}:{relative}")
        shifts: Counter[str] = Counter()
        for entry in row["shifts"].split(";"):
            if not re.fullmatch(r"[0-9]+>[0-9]+:[1-9][0-9]*", entry):
                raise ValueError(f"invalid method arity shift: {owner}:{entry}")
            shift, count = entry.split(":", 1)
            if shift in shifts:
                raise ValueError(f"duplicate method arity shift: {owner}:{shift}")
            shifts[shift] = int(count)
        if int(row["findings"]) != sum(shifts.values()) or shifts != observed.get(owner):
            raise ValueError(f"method arity profile mismatch: {owner}")
    if mapped != set(observed):
        raise ValueError(
            f"method arity owners mismatch: missing={sorted(set(observed) - mapped)} "
            f"extra={sorted(mapped - set(observed))}"
        )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--allowlist", type=Path, required=True)
    parser.add_argument("--module-migration-map", type=Path)
    parser.add_argument("--struct-migration-map", type=Path)
    parser.add_argument("--arity-migration-map", type=Path)
    parser.add_argument(
        "--method-migration-ledger",
        type=Path,
        default=Path(__file__).resolve().parent.parent / METHOD_MIGRATION_LEDGER,
    )
    parser.add_argument(
        "--write-method-migration-ledger",
        action="store_true",
        help="write the parsed retired-method findings for human review",
    )
    for short, crate in (("rdbc", "druid-rdbc"), ("druid", "druid"), ("wrapper", "druid-wrapper")):
        parser.add_argument(f"--{short}-report", type=Path, required=True)
        parser.add_argument(f"--{short}-exit-code", type=int, required=True)
    parser.add_argument(
        "--write-candidate",
        action="store_true",
        help="write observed findings as pending rows for human review",
    )
    args = parser.parse_args()

    actual = {}
    for short, crate in (("rdbc", "druid-rdbc"), ("druid", "druid"), ("wrapper", "druid-wrapper")):
        report = getattr(args, f"{short}_report").read_text(encoding="utf-8")
        status = getattr(args, f"{short}_exit_code")
        for lint, fingerprint in findings_by_lint(report, status).items():
            actual[(crate, lint)] = fingerprint

    if args.write_candidate:
        with args.allowlist.open("w", encoding="utf-8", newline="") as stream:
            writer = csv.writer(stream, delimiter="\t", lineterminator="\n")
            writer.writerow(HEADER)
            for (crate, lint), (count, digest) in sorted(actual.items()):
                writer.writerow((crate, lint, count, digest, "pending", "review migration and consumers"))
        print(f"Wrote {len(actual)} pending breaking categories to {args.allowlist}")
        return 0

    if args.write_method_migration_ledger:
        ledger = args.method_migration_ledger
        ledger.parent.mkdir(parents=True, exist_ok=True)
        ledger.write_bytes(render_method_migration_ledger(args.druid_report.read_text(encoding="utf-8")))
        print(f"Wrote {len(method_migration_rows(args.druid_report.read_text(encoding='utf-8')))} unique method identities to {ledger}")
        return 0

    allowlist = read_allowlist(args.allowlist)
    if any(("druid", lint) in actual for lint in METHOD_BREAK_LINTS):
        verify_method_migration_ledger(
            args.druid_report.read_text(encoding="utf-8"),
            args.method_migration_ledger,
        )
    if ("druid", "module_missing") in actual:
        if args.module_migration_map is None:
            raise ValueError("druid/module_missing requires an exact module migration map")
        verify_module_migration_map(
            args.druid_report.read_text(encoding="utf-8"),
            args.module_migration_map,
            Path(__file__).resolve().parent.parent,
        )
    if ("druid", "struct_missing") in actual:
        if args.struct_migration_map is None:
            raise ValueError("druid/struct_missing requires an exact struct migration map")
        verify_struct_migration_map(
            args.druid_report.read_text(encoding="utf-8"),
            args.struct_migration_map,
            Path(__file__).resolve().parent.parent,
        )
    if ("druid", "method_parameter_count_changed") in actual:
        if args.arity_migration_map is None:
            raise ValueError("druid/method_parameter_count_changed requires an exact arity migration map")
        verify_method_arity_migration_map(
            args.druid_report.read_text(encoding="utf-8"),
            args.arity_migration_map,
            Path(__file__).resolve().parent.parent,
        )
    missing = sorted(actual.keys() - allowlist.keys())
    extra = sorted(allowlist.keys() - actual.keys())
    changed = sorted(
        key for key in actual.keys() & allowlist.keys() if actual[key] != allowlist[key][:2]
    )
    pending = sorted(
        key
        for key in actual.keys() & allowlist.keys()
        if allowlist[key][2] != "approved" or not allowlist[key][3].strip()
    )
    for label, rows in (("unlisted", missing), ("no longer present", extra), ("changed", changed), ("not approved", pending)):
        for crate, lint in rows:
            print(f"{label}: {crate}/{lint}", file=sys.stderr)
    if missing or extra or changed or pending:
        return 1
    print(f"Verified {len(actual)} exact approved SemVer breaking categories")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError) as error:
        print(f"SemVer verification failed: {error}", file=sys.stderr)
        raise SystemExit(1) from error
