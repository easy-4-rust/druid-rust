#!/usr/bin/env python3
"""Verify that external-pool leases preserve non-equivalent PhysicalConnection SPI methods."""

from __future__ import annotations

import re
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PHYSICAL_CONNECTION = ROOT / "crates/druid-rdbc/src/spi/physical_connection.rs"
OLD_PHYSICAL_CONNECTION = ROOT / "crates/druid/src/core/physical_connection.rs"
PHYSICAL_CONNECTION_LEASE = ROOT / "crates/druid/src/core/physical_connection_lease.rs"
EXPECTED_OLD_SHIM_TOKENS = (
    "pub",
    "use",
    "druid_rdbc",
    "::",
    "spi",
    "::",
    "PhysicalConnection",
    ";",
)

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


def source_without_comments(source: str) -> str:
    """Remove Rust comments while preserving every non-comment token for exact comparison."""

    output: list[str] = []
    index = 0
    while index < len(source):
        if source.startswith("//", index):
            newline = source.find("\n", index + 2)
            if newline == -1:
                break
            output.append("\n")
            index = newline + 1
            continue
        if source.startswith("/*", index):
            depth = 1
            index += 2
            while index < len(source) and depth:
                if source.startswith("/*", index):
                    depth += 1
                    index += 2
                elif source.startswith("*/", index):
                    depth -= 1
                    index += 2
                else:
                    index += 1
            if depth:
                raise ValueError("unclosed block comment in old PhysicalConnection shim")
            output.append(" ")
            continue

        raw_prefix_length = 0
        if source[index] == "r":
            raw_prefix_length = 1
        elif source.startswith("br", index):
            raw_prefix_length = 2
        if raw_prefix_length:
            cursor = index + raw_prefix_length
            while cursor < len(source) and source[cursor] == "#":
                cursor += 1
            if cursor < len(source) and source[cursor] == '"':
                hashes = source[index + raw_prefix_length : cursor]
                terminator = '"' + hashes
                end = source.find(terminator, cursor + 1)
                if end == -1:
                    raise ValueError("unclosed raw string in old PhysicalConnection shim")
                end += len(terminator)
                output.append(source[index:end])
                index = end
                continue

        quote_index = index + 1 if source.startswith("b\"", index) else index
        if quote_index < len(source) and source[quote_index] in {'"', "'"}:
            quote = source[quote_index]
            cursor = quote_index + 1
            escaped = False
            while cursor < len(source):
                character = source[cursor]
                if escaped:
                    escaped = False
                elif character == "\\":
                    escaped = True
                elif character == quote:
                    cursor += 1
                    break
                cursor += 1
            else:
                raise ValueError("unclosed literal in old PhysicalConnection shim")
            output.append(source[index:cursor])
            index = cursor
            continue

        output.append(source[index])
        index += 1
    return "".join(output)


def source_tokens(source: str) -> tuple[str, ...]:
    """Tokenize the exact shim grammar after comments and module docs are removed."""

    return tuple(
        re.findall(
            r"::|[A-Za-z_][A-Za-z0-9_]*|[^\s]",
            source_without_comments(source),
        )
    )


def old_shim_errors(path: Path = OLD_PHYSICAL_CONNECTION) -> list[str]:
    """Reject a missing or non-exact old PhysicalConnection compatibility shim."""

    if not path.is_file():
        return ["FAIL: old PhysicalConnection shim is missing"]
    try:
        tokens = source_tokens(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, ValueError):
        return ["FAIL: old PhysicalConnection shim is not an exact directed re-export"]
    if tokens != EXPECTED_OLD_SHIM_TOKENS:
        return ["FAIL: old PhysicalConnection shim is not an exact directed re-export"]
    return []


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
    if errors != [expected]:
        print(
            "FAIL: verifier evolution probe did not reject synthetic trait method",
            file=sys.stderr,
        )
        for error in errors:
            print(error, file=sys.stderr)
        return 1

    with tempfile.TemporaryDirectory(prefix="physical-connection-shim-") as directory:
        shim = Path(directory) / "physical_connection.rs"
        cases = (
            (
                "missing",
                None,
                ["FAIL: old PhysicalConnection shim is missing"],
            ),
            (
                "wrong target",
                "pub use druid_rdbc::spi::PhysicalConnectionFactory;\n",
                ["FAIL: old PhysicalConnection shim is not an exact directed re-export"],
            ),
            (
                "copied trait",
                "pub trait PhysicalConnection {}\n",
                ["FAIL: old PhysicalConnection shim is not an exact directed re-export"],
            ),
            (
                "valid",
                "//! Compatibility shim.\n\npub use druid_rdbc::spi::PhysicalConnection;\n",
                [],
            ),
        )
        for name, source, expected_errors in cases:
            if shim.exists():
                shim.unlink()
            if source is not None:
                shim.write_text(source, encoding="utf-8")
            actual_errors = old_shim_errors(shim)
            if actual_errors != expected_errors:
                print(
                    f"FAIL: old-shim self-test case {name!r} returned {actual_errors!r}",
                    file=sys.stderr,
                )
                return 1

    print("PASS: verifier evolution and old-shim probes reject unsafe drift")
    return 0


def main(arguments: list[str]) -> int:
    if arguments == ["--self-test"]:
        return self_test()
    if arguments:
        print("usage: verify_physical_connection_lease_surface.py [--self-test]", file=sys.stderr)
        return 2

    shim_errors = old_shim_errors()
    if shim_errors:
        for error in shim_errors:
            print(error, file=sys.stderr)
        return 1

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
