#!/usr/bin/env python3
"""Verify Task 6 Druid/wrapper public API snapshots and exact contract allowlist."""

from __future__ import annotations

import argparse
import csv
import hashlib
import os
import re
import subprocess
from pathlib import Path
from typing import NoReturn

from render_task6_public_contract_allowlist import (
    CURRENT_SNAPSHOTS,
    OUTPUT,
    RETAINED_ALIASES,
    build_rows,
    render_rows,
)
from verify_task6_migration_authority import module_declares_exact_target


# 与 RDBC API 门禁使用同一 nightly 日期，由 rustup 选择当前主机。
TOOLCHAIN = "nightly-2026-08-07"
MANIFESTS = {
    "druid": Path("crates/druid/Cargo.toml"),
    "druid-wrapper": Path("crates/druid-wrapper/Cargo.toml"),
}
FORBIDDEN = re.compile(r"\bPhysical[A-Z][A-Za-z0-9_]*\b")


class VerificationError(RuntimeError):
    """Raised when live public/config/wire evidence differs from the approved contract."""


def fail(message: str) -> NoReturn:
    """Stop with a stable failure."""

    raise VerificationError(message)


def generate_api(root: Path, package: str) -> str:
    """Run the installed cargo-public-api on one current package."""

    environment = os.environ.copy()
    environment["RUSTUP_TOOLCHAIN"] = TOOLCHAIN
    result = subprocess.run(
        [
            "cargo",
            "public-api",
            "--manifest-path",
            str(root / MANIFESTS[package]),
            "-sss",
            "--all-features",
        ],
        cwd=root,
        env=environment,
        check=False,
        capture_output=True,
        text=True,
        timeout=900,
    )
    if result.returncode != 0:
        fail(
            f"cargo-public-api {package} failed: {(result.stderr or result.stdout).strip()}"
        )
    return result.stdout


def run_gate(root: Path, command: list[str], label: str, timeout: int = 900) -> None:
    """Run one required executable contract gate with bounded captured output."""

    result = subprocess.run(
        command,
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
        timeout=timeout,
    )
    if result.returncode != 0:
        fail(f"{label} failed: {(result.stderr or result.stdout).strip()}")


def verify_evidence_references(root: Path, rows: tuple[dict[str, str], ...]) -> None:
    """Require every allowlist row to name a real test function, not a prose claim."""

    cache: dict[Path, str] = {}
    for row in rows:
        reference = row["evidence_test"]
        path_text, separator, selector = reference.partition("#")
        path = root / path_text
        if not separator or not selector or not path.is_file():
            fail(f"allowlist evidence does not exist: {row['old_path']} -> {reference}")
        source = cache.setdefault(path, path.read_text(encoding="utf-8"))
        if selector == "canonical-standard-methods":
            if "source_object" not in source or "target_path" not in source:
                fail(f"method ledger evidence schema is stale: {reference}")
        elif selector == "exact-denominator=1283":
            rows = tuple(csv.DictReader(source.splitlines(), delimiter="\t"))
            if len(rows) != 1283 or any(
                row.get("disposition") in {"MISSING", "UNREVIEWED"} for row in rows
            ):
                fail(f"test ledger evidence is incomplete: {reference}")
        elif not re.search(rf"\bfn\s+{re.escape(selector)}\b", source):
            fail(f"allowlist evidence function is missing: {reference}")


def verify_deprecated_alias_sources(root: Path) -> None:
    """Require retained names to be explicit, time-bounded deprecated aliases."""

    sources = {
        root
        / "crates/druid/src/pool/config.rs": {
            "physical_connection_timeout",
            "phy_timeout",
        },
        root
        / "crates/druid/src/dynamic/high_available_data_source.rs": {
            "set_physical_timeout"
        },
    }
    observed: set[str] = set()
    for path, names in sources.items():
        text = path.read_text(encoding="utf-8")
        for name in names:
            declaration = re.compile(
                rf"#\[deprecated\([\s\S]{{0,300}}?\)\][\s\S]{{0,160}}?\b(?:fn\s+|pub\s+){re.escape(name)}\b"
            )
            if not declaration.search(text):
                fail(f"retained compatibility alias is not deprecated: {name}")
            observed.add(name)
    expected_names = {path.rsplit("::", 1)[-1] for path in RETAINED_ALIASES}
    if observed != expected_names:
        fail(
            f"deprecated alias source denominator drift: {observed} != {expected_names}"
        )


def verify_admin_contract(root: Path) -> None:
    """Require only driver-named Rust fields/serialized keys in production source."""

    path = root / "crates/druid-admin/src/model/dto/data_source_result.rs"
    source = path.read_text(encoding="utf-8")
    for name in (
        "driver_connect_count",
        "driver_close_count",
        "driver_connect_error_count",
        'rename = "DriverConnectCount"',
        'rename = "DriverCloseCount"',
        'rename = "DriverConnectErrorCount"',
    ):
        if name not in source:
            fail(f"admin driver contract is missing: {name}")
    if FORBIDDEN.search(source):
        fail("admin production wire source retains a retired Pascal-case key")


def verify_private_targets(root: Path, rows: tuple[dict[str, str], ...]) -> None:
    """Require every internalized mapping to resolve in its exact private module."""

    for row in rows:
        if row["disposition"] != "APPROVED_INTERNALIZED":
            continue
        if not module_declares_exact_target(root, row["target_path"]):
            fail(f"internalized public target is not exact: {row['target_path']}")


def verify_repository(root: Path) -> tuple[int, dict[str, str], str]:
    """Generate live APIs and bind snapshots, allowlist, aliases, and wire evidence."""

    hashes: dict[str, str] = {}
    for package in ("druid", "druid-wrapper"):
        live = generate_api(root, package)
        retired = FORBIDDEN.search(live)
        if retired:
            fail(f"{package} public API retains {retired.group(0)}")
        snapshot = root / CURRENT_SNAPSHOTS[package]
        if not snapshot.is_file() or snapshot.read_text(encoding="utf-8") != live:
            fail(f"{package} current public API snapshot is missing or stale")
        hashes[package] = hashlib.sha256(live.encode("utf-8")).hexdigest()
    rows = build_rows(root)
    expected = render_rows(rows)
    allowlist = root / OUTPUT
    if not allowlist.is_file() or allowlist.read_bytes() != expected:
        fail("public/config/wire allowlist is missing or stale")
    verify_evidence_references(root, rows)
    verify_deprecated_alias_sources(root)
    verify_private_targets(root, rows)
    verify_admin_contract(root)
    run_gate(
        root,
        [
            "cargo",
            "test",
            "-p",
            "druid",
            "--test",
            "task6_public_config_compatibility_test",
        ],
        "Druid public/config golden",
    )
    run_gate(
        root,
        [
            "cargo",
            "test",
            "-p",
            "druid-admin",
            "--test",
            "task6_admin_wire_contract_test",
        ],
        "admin JSON golden",
    )
    run_gate(
        root,
        [
            "python3",
            "scripts/verify_task6_external_public_contract.py",
            "--root",
            str(root),
        ],
        "external public consumer",
    )
    return len(rows), hashes, hashlib.sha256(expected).hexdigest()


def main() -> int:
    """CLI entry point."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root", type=Path, default=Path(__file__).resolve().parent.parent
    )
    arguments = parser.parse_args()
    try:
        rows, hashes, allowlist_hash = verify_repository(arguments.root.resolve())
    except (
        OSError,
        UnicodeError,
        subprocess.TimeoutExpired,
        VerificationError,
    ) as error:
        print(f"TASK6_PUBLIC_CONTRACT_FAIL={error}")
        return 1
    print(f"TASK6_PUBLIC_CONTRACT_ALLOWLIST_ROWS={rows}")
    print(f"DRUID_PUBLIC_API_SHA256={hashes['druid']}")
    print(f"DRUID_WRAPPER_PUBLIC_API_SHA256={hashes['druid-wrapper']}")
    print(f"TASK6_PUBLIC_CONTRACT_ALLOWLIST_SHA256={allowlist_hash}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
