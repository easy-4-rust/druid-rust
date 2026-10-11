#!/usr/bin/env python3
"""Compare actual cargo-semver-checks findings with reviewed pre-release breaks."""

from __future__ import annotations

import argparse
import csv
import hashlib
import re
import sys
from pathlib import Path

ANSI_ESCAPE = re.compile(r"\x1b\[[0-9;]*m")
SECTION = re.compile(r"(?m)^--- failure ([a-z_]+):[^\n]*$")
WORKSPACE_PREFIX = re.compile(
    r"/home/runner/work/druid-rust/druid-rust/"
    r"(?:target/semver-checks/git-[^/]+/[^/]+/)?"
)
SOURCE_LINE = re.compile(r"(?<=\.rs):\d+")
FAILURE_SUMMARY = re.compile(
    r"Summary semver requires new (?:major|minor) version: "
    r"(\d+) major and (\d+) minor checks failed"
)
PASS_SUMMARY = re.compile(r"Summary no semver update required")
HEADER = ("crate", "lint", "count", "sha256", "status", "reason")


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
            finding = WORKSPACE_PREFIX.sub("", line.strip())
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


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--allowlist", type=Path, required=True)
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

    allowlist = read_allowlist(args.allowlist)
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
