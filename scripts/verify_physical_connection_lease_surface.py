#!/usr/bin/env python3
"""Verify that external-pool leases preserve non-equivalent PhysicalConnection SPI methods."""

from __future__ import annotations

import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PHYSICAL_CONNECTION = ROOT / "crates/druid/src/core/physical_connection.rs"
PHYSICAL_CONNECTION_LEASE = ROOT / "crates/druid/src/core/physical_connection_lease.rs"

# These PhysicalConnection defaults can hide an Adapter override or downgrade a driver-owned
# resource. A transparent lease must always invoke the physical connection's implementation.
REQUIRED_DELEGATES = frozenset(
    {
        "create_physical_statement",
        "exec_batch",
        "prepare_physical_statement",
        "prepare_physical_call",
        "exec_prepared",
        "fetch_prepared",
        "database_meta_data",
        "create_blob",
        "create_clob",
        "create_n_clob",
    }
)


def block_after(source: str, declaration: re.Pattern[str]) -> str:
    """Return the brace-balanced Rust block beginning at a declaration match."""

    match = declaration.search(source)
    if match is None:
        raise ValueError(f"declaration not found: {declaration.pattern}")
    opening = source.find("{", match.end())
    if opening == -1:
        raise ValueError(f"opening brace not found: {declaration.pattern}")

    depth = 0
    for index in range(opening, len(source)):
        character = source[index]
        if character == "{":
            depth += 1
        elif character == "}":
            depth -= 1
            if depth == 0:
                return source[opening + 1 : index]
    raise ValueError(f"unclosed block: {declaration.pattern}")


def method_names(block: str) -> set[str]:
    """Extract Rust trait or impl method declarations without depending on formatting."""

    return set(
        re.findall(r"(?m)^\s*(?:async\s+)?fn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(", block)
    )


def main() -> int:
    try:
        trait_methods = method_names(
            block_after(
                PHYSICAL_CONNECTION.read_text(encoding="utf-8"),
                re.compile(r"\bpub\s+trait\s+PhysicalConnection\b"),
            )
        )
        lease_methods = method_names(
            block_after(
                PHYSICAL_CONNECTION_LEASE.read_text(encoding="utf-8"),
                re.compile(
                    r"\bimpl\s*<[^>]+>\s+PhysicalConnection\s+for\s+"
                    r"PhysicalConnectionLease\s*<[^>]+>",
                    re.DOTALL,
                ),
            )
        )
    except (OSError, ValueError) as error:
        print(f"FAIL: could not parse PhysicalConnection lease surface: {error}", file=sys.stderr)
        return 1

    stale_required = REQUIRED_DELEGATES - trait_methods
    missing_delegates = REQUIRED_DELEGATES - lease_methods
    if stale_required or missing_delegates:
        if stale_required:
            print(
                "FAIL: verifier required methods absent from PhysicalConnection: "
                + ", ".join(sorted(stale_required)),
                file=sys.stderr,
            )
        if missing_delegates:
            print(
                "FAIL: PhysicalConnectionLease misses delegates: "
                + ", ".join(sorted(missing_delegates)),
                file=sys.stderr,
            )
        return 1

    print("PASS: PhysicalConnectionLease delegates all required methods")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
