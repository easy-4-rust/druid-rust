#!/usr/bin/env python3
"""Verify that canonical RDBC standard modules remain product-independent."""

from __future__ import annotations

import argparse
from pathlib import Path
from typing import NoReturn

from verify_rdbc_public_paths import tokenize


EXPECTED_STANDARD_FILES = (
    Path("connection/mod.rs"),
    Path("connection/connection.rs"),
    Path("statement/mod.rs"),
    Path("statement/statement.rs"),
    Path("statement/prepared_statement.rs"),
    Path("statement/callable_statement.rs"),
    Path("result_set/mod.rs"),
    Path("result_set/result_set.rs"),
    Path("metadata/mod.rs"),
    Path("metadata/database_meta_data.rs"),
    Path("metadata/result_set_meta_data.rs"),
    Path("metadata/parameter_meta_data.rs"),
    Path("driver/mod.rs"),
    Path("driver/driver.rs"),
    Path("driver/connection_factory.rs"),
    Path("driver/driver_extension_descriptor.rs"),
    Path("driver/driver_registry.rs"),
    Path("data_source/mod.rs"),
    Path("data_source/common_data_source.rs"),
    Path("data_source/data_source.rs"),
    Path("data_source/pooled_connection.rs"),
    Path("data_source/connection_pool_data_source.rs"),
    Path("data_source/xa_connection.rs"),
    Path("data_source/xa_data_source.rs"),
    Path("wrapper/mod.rs"),
    Path("wrapper/wrapper.rs"),
    Path("wrapper/standard_unwrapped.rs"),
    Path("extensions/mod.rs"),
    Path("extensions/connection_capabilities.rs"),
)


class VerificationError(RuntimeError):
    """Raised when the canonical standard surface is missing or polluted."""


def fail(message: str) -> NoReturn:
    """Stop with one stable fail-closed diagnostic."""

    raise VerificationError(message)


def forbidden_identity(identifier: str) -> bool:
    """Return whether one Rust identifier belongs to transitional/product runtime."""

    normalized = identifier.removeprefix("r#").replace("_", "").lower()
    if any(
        fragment in normalized
        for fragment in (
            "physical",
            "druid",
            "holder",
            "lease",
            "cache",
            "filter",
            "resourceowner",
        )
    ):
        return True
    if normalized.startswith("pooledconnection") or normalized == "connectionpooldatasource":
        return False
    if identifier in {"spi", "compat"}:
        return True
    return "pool" in normalized


def verify_module_root(path: Path, token_texts: list[str]) -> None:
    """Require module roots to contain declarations and re-exports only."""

    forbidden_items = {"trait", "struct", "enum", "union", "impl", "fn", "const", "static", "type"}
    present = sorted(forbidden_items.intersection(token_texts))
    if present:
        fail(f"module root defines items instead of declaring/re-exporting modules: {path}: {present}")


def verify_tree(root: Path, *, phase: str) -> None:
    """Verify one source tree for the requested migration phase."""

    if phase not in {"scaffold", "transitional"}:
        fail(f"unknown purity phase: {phase}")
    missing = [relative.as_posix() for relative in EXPECTED_STANDARD_FILES if not (root / relative).is_file()]
    if missing:
        fail(f"standard scaffold is incomplete: {missing}")
    for relative in EXPECTED_STANDARD_FILES:
        path = root / relative
        try:
            source = path.read_text(encoding="utf-8")
        except UnicodeDecodeError as error:
            fail(f"standard source is not UTF-8: {relative}: {error}")
        tokens = tokenize(source)
        token_texts = [token.text for token in tokens]
        polluted = sorted(
            {
                token.text
                for token in tokens
                if token.kind == "IDENT" and forbidden_identity(token.text)
            }
        )
        if polluted:
            fail(f"standard source contains product/runtime identities: {relative}: {polluted}")
        if relative.name == "mod.rs":
            verify_module_root(relative, token_texts)


def main() -> int:
    """Run the repository purity gate."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--phase", choices=("scaffold", "transitional"), required=True)
    arguments = parser.parse_args()
    root = Path(__file__).resolve().parent.parent / "crates/druid-rdbc/src"
    try:
        verify_tree(root, phase=arguments.phase)
    except VerificationError as error:
        print(f"ERROR: {error}")
        return 1
    print(f"STANDARD_FILES={len(EXPECTED_STANDARD_FILES)}")
    print(f"PURITY_PHASE={arguments.phase}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
