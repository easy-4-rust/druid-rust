#!/usr/bin/env python3
"""Verify that canonical RDBC standard modules remain product-independent."""

from __future__ import annotations

import argparse
from pathlib import Path
from typing import NoReturn

from verify_rdbc_public_paths import tokenize
from verify_wrapper_rdbc_import_boundary import parse_use_tree


REQUIRED_CANONICAL_MODULES = (
    "connection",
    "data_source",
    "driver",
    "extensions",
    "metadata",
    "result_set",
    "statement",
    "wrapper",
)
TRANSITIONAL_ROOT_MODULES = {"compat", "error", "resource", "spi", "types"}
ALLOWED_EXTERNAL_IMPORT_ROOTS = {"alloc", "async_trait", "core", "std"}
ALLOWED_CRATE_MODULES = {*REQUIRED_CANONICAL_MODULES, "error", "types"}


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


def discover_canonical_modules(root: Path) -> tuple[str, ...]:
    """Discover every canonical public root module, retaining required Task 3 roots."""

    modules = set(REQUIRED_CANONICAL_MODULES)
    crate_root = root / "lib.rs"
    if crate_root.is_file():
        tokens = tokenize(crate_root.read_text(encoding="utf-8"))
        for index in range(len(tokens) - 2):
            if (
                tokens[index].text == "pub"
                and tokens[index + 1].text == "mod"
                and tokens[index + 2].kind == "IDENT"
            ):
                name = tokens[index + 2].text.removeprefix("r#")
                if name not in TRANSITIONAL_ROOT_MODULES:
                    modules.add(name)
    return tuple(sorted(modules))


def discover_standard_files(root: Path) -> tuple[Path, ...]:
    """Dynamically enumerate every Rust file under the canonical public module tree."""

    files: list[Path] = []
    missing: list[str] = []
    for module in discover_canonical_modules(root):
        directory = root / module
        module_root = directory / "mod.rs"
        if not module_root.is_file():
            missing.append(module_root.relative_to(root).as_posix())
            continue
        discovered = sorted(path for path in directory.rglob("*.rs") if path.is_file())
        if len(discovered) < 2:
            missing.append(f"{module}/<canonical-object>.rs")
            continue
        files.extend(discovered)
    if missing:
        fail(f"standard scaffold is incomplete: {missing}")
    return tuple(sorted(path.relative_to(root) for path in files))


def verify_imports(relative: Path, tokens: list[object], local_roots: set[str]) -> None:
    """Reject imports outside the canonical crate/std allowlist."""

    index = 0
    while index < len(tokens):
        if tokens[index].text != "use":
            index += 1
            continue
        try:
            entries, end = parse_use_tree(tokens, index + 1)
        except Exception as error:
            fail(f"cannot parse canonical import in {relative}: {error}")
        for entry in entries:
            path = tuple(part.removeprefix("r#") for part in entry.path)
            if not path:
                fail(f"empty canonical import in {relative}")
            if path[-1] == "*":
                fail(f"wildcard import is forbidden in canonical source: {relative}")
            root_name = path[0]
            if root_name == "crate":
                if len(path) < 2 or path[1] not in ALLOWED_CRATE_MODULES:
                    fail(f"crate import escapes canonical allowlist in {relative}: {'::'.join(path)}")
            elif root_name in {"self", "super"}:
                continue
            elif root_name not in ALLOWED_EXTERNAL_IMPORT_ROOTS and root_name not in local_roots:
                fail(f"backend/external import is forbidden in {relative}: {'::'.join(path)}")
        index = max(index + 1, end + 1)


def verify_tree(root: Path, *, phase: str) -> tuple[Path, ...]:
    """Verify one source tree for the requested migration phase."""

    if phase not in {"scaffold", "transitional"}:
        fail(f"unknown purity phase: {phase}")
    standard_files = discover_standard_files(root)
    for relative in standard_files:
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
        local_roots = {
            child.stem
            for child in path.parent.iterdir()
            if child.is_file() and child.suffix == ".rs" and child.name != "mod.rs"
        }
        local_roots.update(
            child.name for child in path.parent.iterdir() if child.is_dir()
        )
        verify_imports(relative, tokens, local_roots)
        if relative.name == "mod.rs":
            verify_module_root(relative, token_texts)
    return standard_files


def main() -> int:
    """Run the repository purity gate."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--phase", choices=("scaffold", "transitional"), required=True)
    arguments = parser.parse_args()
    root = Path(__file__).resolve().parent.parent / "crates/druid-rdbc/src"
    try:
        standard_files = verify_tree(root, phase=arguments.phase)
    except VerificationError as error:
        print(f"ERROR: {error}")
        return 1
    print(f"STANDARD_FILES={len(standard_files)}")
    print(f"PURITY_PHASE={arguments.phase}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
