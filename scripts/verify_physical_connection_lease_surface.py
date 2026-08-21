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

# Leave this empty until a PhysicalConnection default is proven observationally equivalent to
# delegating through an external-pool lease. Every unlisted trait method fails closed.
EQUIVALENT_DEFAULTS = frozenset()


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


def validation_errors(
    trait_methods: set[str],
    lease_methods: set[str],
    named_required: frozenset[str] = REQUIRED_DELEGATES,
) -> list[str]:
    """Return every stale named requirement or missing transparent lease delegate."""

    stale_required = named_required - trait_methods
    stale_equivalents = EQUIVALENT_DEFAULTS - trait_methods
    required_delegates = trait_methods - EQUIVALENT_DEFAULTS
    missing_delegates = required_delegates - lease_methods
    errors = []
    if stale_required:
        errors.append(
            "FAIL: verifier required methods absent from PhysicalConnection: "
            + ", ".join(sorted(stale_required))
        )
    if stale_equivalents:
        errors.append(
            "FAIL: verifier equivalent defaults absent from PhysicalConnection: "
            + ", ".join(sorted(stale_equivalents))
        )
    if missing_delegates:
        errors.append(
            "FAIL: PhysicalConnectionLease misses delegates: "
            + ", ".join(sorted(missing_delegates))
        )
    return errors


def parse_surface() -> tuple[set[str], set[str]]:
    """Parse the current trait and lease implementation method surfaces."""

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
    return trait_methods, lease_methods


def self_test() -> int:
    """Inject a future trait default and prove that an unmodified lease fails validation."""

    synthetic_trait = """
        pub trait PhysicalConnection {
            async fn existing(&mut self) {}
            async fn future_default(&mut self) {}
        }
    """
    synthetic_lease = """
        impl<L> PhysicalConnection for PhysicalConnectionLease<L> {
            async fn existing(&mut self) {}
        }
    """
    trait_methods = method_names(
        block_after(
            synthetic_trait,
            re.compile(r"\bpub\s+trait\s+PhysicalConnection\b"),
        )
    )
    lease_methods = method_names(
        block_after(
            synthetic_lease,
            re.compile(
                r"\bimpl\s*<[^>]+>\s+PhysicalConnection\s+for\s+"
                r"PhysicalConnectionLease\s*<[^>]+>",
                re.DOTALL,
            ),
        )
    )
    errors = validation_errors(trait_methods, lease_methods, frozenset())
    expected = "FAIL: PhysicalConnectionLease misses delegates: future_default"
    if errors == [expected]:
        print("PASS: verifier evolution probe rejects synthetic trait method")
        return 0

    print("FAIL: verifier evolution probe did not reject synthetic trait method", file=sys.stderr)
    for error in errors:
        print(error, file=sys.stderr)
    return 1


def main(arguments: list[str]) -> int:
    if arguments == ["--self-test"]:
        return self_test()
    if arguments:
        print("usage: verify_physical_connection_lease_surface.py [--self-test]", file=sys.stderr)
        return 2

    try:
        trait_methods, lease_methods = parse_surface()
    except (OSError, ValueError) as error:
        print(f"FAIL: could not parse PhysicalConnection lease surface: {error}", file=sys.stderr)
        return 1

    errors = validation_errors(trait_methods, lease_methods)
    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1

    print("PASS: PhysicalConnectionLease delegates all required methods")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
