#!/usr/bin/env python3
"""Verify that external-pool leases preserve non-equivalent PhysicalConnection SPI methods."""

from __future__ import annotations

import sys
import tempfile
from pathlib import Path

SCRIPT_DIRECTORY = Path(__file__).resolve().parent
if str(SCRIPT_DIRECTORY) not in sys.path:
    sys.path.insert(0, str(SCRIPT_DIRECTORY))

import verify_rdbc_public_paths as rust_source


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


def old_shim_errors(path: Path = OLD_PHYSICAL_CONNECTION) -> list[str]:
    """Reject a missing or non-exact old PhysicalConnection compatibility shim."""

    if not path.is_file():
        return ["FAIL: old PhysicalConnection shim is missing"]
    try:
        tokens = rust_source.tokenize(path.read_text(encoding="utf-8"))
        index = rust_source.consume_docs(tokens, 0)
        token_text = tuple(token.text for token in tokens[index:])
    except (OSError, UnicodeError, rust_source.VerificationError):
        return ["FAIL: old PhysicalConnection shim is not an exact directed re-export"]
    if token_text != EXPECTED_OLD_SHIM_TOKENS:
        return ["FAIL: old PhysicalConnection shim is not an exact directed re-export"]
    return []


def top_level_method_names(
    tokens: list[rust_source.Token], start: int, end: int
) -> set[str]:
    """Return function names directly inside one token-delimited trait or impl body."""

    names: set[str] = set()
    stack: list[str] = []
    index = start
    while index < end:
        token = tokens[index]
        if not stack and token.text == "fn":
            if index + 1 >= end or tokens[index + 1].kind != "IDENT":
                raise ValueError("malformed top-level function declaration")
            names.add(tokens[index + 1].text)
        if token.text in rust_source.DELIMITER_PAIRS:
            stack.append(rust_source.DELIMITER_PAIRS[token.text])
        elif token.text in rust_source.DELIMITER_PAIRS.values():
            if not stack or token.text != stack[-1]:
                raise ValueError("unbalanced delimiter in trait or impl body")
            stack.pop()
        index += 1
    if stack:
        raise ValueError("unclosed delimiter in trait or impl body")
    return names


def trait_method_names(source: str) -> set[str]:
    """Parse the canonical PhysicalConnection trait through the shared Rust tokenizer."""

    tokens = rust_source.tokenize(source)
    stack: list[str] = []
    for index, token in enumerate(tokens):
        if (
            not stack
            and token.text == "trait"
            and index + 1 < len(tokens)
            and tokens[index + 1].text == "PhysicalConnection"
        ):
            opening = next(
                (
                    cursor
                    for cursor in range(index + 2, len(tokens))
                    if tokens[cursor].text == "{"
                ),
                None,
            )
            if opening is None:
                raise ValueError("opening brace not found: PhysicalConnection trait")
            start, end = rust_source.find_matching_brace(
                tokens, opening, "PhysicalConnection trait"
            )
            return top_level_method_names(tokens, start, end)
        if token.text in rust_source.DELIMITER_PAIRS:
            stack.append(rust_source.DELIMITER_PAIRS[token.text])
        elif token.text in rust_source.DELIMITER_PAIRS.values():
            if not stack or token.text != stack[-1]:
                raise ValueError("unbalanced top-level trait delimiter")
            stack.pop()
    raise ValueError("declaration not found: PhysicalConnection trait")


def lease_method_names(source: str) -> set[str]:
    """Parse the PhysicalConnectionLease impl through the shared Rust tokenizer."""

    tokens = rust_source.tokenize(source)
    stack: list[str] = []
    index = 0
    while index < len(tokens):
        token = tokens[index]
        if not stack and token.text == "impl":
            opening = next(
                (
                    cursor
                    for cursor in range(index + 1, len(tokens))
                    if tokens[cursor].text == "{"
                ),
                None,
            )
            if opening is None:
                raise ValueError("opening brace not found: PhysicalConnectionLease impl")
            header = [item.text for item in tokens[index:opening]]
            if "for" in header:
                for_index = header.index("for")
                if (
                    "PhysicalConnection" in header[:for_index]
                    and "PhysicalConnectionLease" in header[for_index + 1 :]
                ):
                    start, end = rust_source.find_matching_brace(
                        tokens, opening, "PhysicalConnectionLease impl"
                    )
                    return top_level_method_names(tokens, start, end)
            _, end = rust_source.find_matching_brace(tokens, opening, "impl")
            index = end + 1
            continue
        if token.text in rust_source.DELIMITER_PAIRS:
            stack.append(rust_source.DELIMITER_PAIRS[token.text])
        elif token.text in rust_source.DELIMITER_PAIRS.values():
            if not stack or token.text != stack[-1]:
                raise ValueError("unbalanced top-level impl delimiter")
            stack.pop()
        index += 1
    raise ValueError("declaration not found: PhysicalConnectionLease impl")


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

    trait_methods = trait_method_names(
        PHYSICAL_CONNECTION.read_text(encoding="utf-8")
    )
    lease_methods = lease_method_names(
        PHYSICAL_CONNECTION_LEASE.read_text(encoding="utf-8")
    )
    return trait_methods, lease_methods


def self_test() -> int:
    """Inject a future trait default and prove that an unmodified lease fails validation."""

    synthetic_trait = r'''
        /// Documentation can contain a closing brace: }.
        pub trait PhysicalConnection {
            async fn existing(&mut self) {
                let normal = "}";
                let raw = r##"}"##;
                let bytes = b"}";
                let raw_bytes = br##"}"##;
                let character = '}';
                if true {
                    match normal {
                        "}" => {}
                        _ => {}
                    }
                }
            }
            async fn future_default(&mut self) {}
        }
    '''
    synthetic_lease = """
        impl<L> PhysicalConnection for PhysicalConnectionLease<L> {
            async fn existing(&mut self) {}
        }
    """
    trait_methods = trait_method_names(synthetic_trait)
    lease_methods = lease_method_names(synthetic_lease)
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
