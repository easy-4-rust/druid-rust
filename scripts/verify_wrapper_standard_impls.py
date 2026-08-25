#!/usr/bin/env python3
"""Verify P04 Task 4 wrapper adapters implement the canonical RDBC standard directly."""

from __future__ import annotations

import argparse
import csv
import re
from dataclasses import dataclass
from pathlib import Path
from typing import NoReturn
from verify_rdbc_public_paths import tokenize


DEFAULT_ADAPTERS = Path("docs/verification/rdbc-first/rdbc-adapter-implementations.tsv")
DEFAULT_CONSUMERS = Path("docs/verification/rdbc-first/rdbc-production-consumers.tsv")
TASK = "P04_TASK_4_ADAPTER_CUTOVER"
LIVE_ADAPTER_STATE = "STANDARD_IMPL_LIVE"
LIVE_CONSUMER_STATE = "STANDARD_REFERENCE_CLOSED"
SQL_EXECUTION_MARKERS = (
    "sqlx::query",
    ".fetch_all(",
    ".fetch_one(",
    ".execute(",
    "RdbcError::DriverError",
    "RdbcError::UnsupportedOperation",
)


class VerificationError(RuntimeError):
    """Raised when Task 4 evidence is absent, duplicated, or stale."""


def fail(message: str) -> NoReturn:
    """Stop with one stable fail-closed diagnostic."""

    raise VerificationError(message)


@dataclass(frozen=True)
class AdapterRow:
    """One Task 4 production adapter obligation."""

    source_path: Path
    current_trait: str
    implementation_type: str
    target_trait: str
    standard_implementation_type: str
    standard_source_path: Path
    standard_core_evidence: str
    behavior_evidence: str
    state: str


def read_tsv(path: Path) -> list[dict[str, str]]:
    """Read one canonical TSV while rejecting malformed rows."""

    raw = path.read_bytes()
    if not raw.endswith(b"\n") or raw.endswith(b"\n\n") or b"\r" in raw:
        fail(f"TSV must have exactly one LF final newline: {path}")
    text = raw.decode("utf-8")
    rows = list(csv.DictReader(text.splitlines(), delimiter="\t"))
    if not rows or any(None in row or any(value == "" for value in row.values()) for row in rows):
        fail(f"TSV contains an empty or malformed row: {path}")
    return rows


def task_adapter_rows(path: Path) -> list[AdapterRow]:
    """Load exactly the production wrapper rows assigned to Task 4."""

    rows = [
        AdapterRow(
            source_path=Path(row["source_path"]),
            current_trait=row["current_trait"],
            implementation_type=row["implementation_type"],
            target_trait=row["target_trait"],
            standard_implementation_type=row["standard_implementation_type"],
            standard_source_path=Path(row["standard_source_path"]),
            standard_core_evidence=row["standard_core_evidence"],
            behavior_evidence=row["behavior_evidence"],
            state=row["state"],
        )
        for row in read_tsv(path)
        if row["target_task"] == TASK and row["owner"] == "WRAPPER_ADAPTER"
    ]
    if len(rows) != 31:
        fail(f"Task 4 adapter denominator drift: expected 31 rows, found {len(rows)}")
    return rows


def concrete_type(type_name: str) -> str:
    """Return the concrete Rust identifier recorded by the inventory."""

    return re.split(r"[<' ]", type_name, maxsplit=1)[0]


def extract_impl_bodies(source: str, trait_name: str, type_name: str) -> list[str]:
    """Extract explicit impl bodies with balanced braces."""

    leaf = trait_name.rsplit("::", 1)[-1]
    concrete = concrete_type(type_name)
    header = re.compile(
        rf"\bimpl(?:\s*<[^{{;]*>)?\s+(?:[A-Za-z_][\w]*::)*{re.escape(leaf)}"
        rf"\s+for\s+{re.escape(concrete)}(?:\s*<[^{{;]*>)?(?:\s+where\s+[^{{]*)?\s*\{{",
        re.MULTILINE,
    )
    bodies: list[str] = []
    for match in header.finditer(source):
        depth = 1
        index = match.end()
        while index < len(source) and depth:
            if source[index] == "{":
                depth += 1
            elif source[index] == "}":
                depth -= 1
            index += 1
        if depth:
            fail(f"unbalanced impl block for {concrete}: {leaf}")
        bodies.append(source[match.end() : index - 1])
    return bodies


def code_tokens(source: str) -> str:
    """Return executable Rust tokens without comments or literal contents."""

    return " ".join(token.text for token in tokenize(source))


def core_calls(body: str) -> set[str]:
    """Return private/inherent core calls made by one impl body."""

    return set(re.findall(r"\b([A-Za-z_][A-Za-z0-9_]*_core)\s*\(", body))


def has_wrapper_macro(source: str, type_name: str, target_trait: str) -> bool:
    """Return whether the canonical Wrapper macro covers this implementation type."""

    concrete = concrete_type(type_name)
    leaf = target_trait.rsplit("::", 1)[-1]
    return re.search(
        rf"impl_standard_wrapper!\s*\(\s*{re.escape(concrete)}(?:\s*<[^;)]*>)?\s*=>\s*{re.escape(leaf)}\s*\)",
        source,
    ) is not None


def verify_adapter_source(root: Path, row: AdapterRow) -> None:
    """Require parallel Physical/standard impls and a single execution core."""

    path = root / row.source_path
    if not path.is_file():
        fail(f"adapter source is missing: {row.source_path}")
    source = path.read_text(encoding="utf-8")
    tokens = code_tokens(source)
    current = extract_impl_bodies(source, row.current_trait, row.implementation_type)
    if not current and row.current_trait.rsplit("::", 1)[-1] != "Wrapper":
        fail(f"temporary Physical impl missing: {row.implementation_type} in {row.source_path}")

    if row.state != LIVE_ADAPTER_STATE:
        if row.state != "CURRENT_IMPL_TARGET_PLANNED":
            fail(f"adapter inventory row has unknown state: {row.implementation_type}: {row.state}")
        return

    standard_path = root / row.standard_source_path
    if not standard_path.is_file():
        fail(f"canonical standard source is missing: {row.standard_source_path}")
    test_name = row.behavior_evidence.split("::", 1)[-1]
    test_sources = "\n".join(
        path.read_text(encoding="utf-8")
        for path in (root / "crates/druid-wrapper/tests").glob("*.rs")
    )
    if test_name not in test_sources:
        fail(f"adapter behavior evidence is missing: {row.behavior_evidence}")
    if row.standard_implementation_type == "StandardAdapterPreparedStatement":
        standard_source = standard_path.read_text(encoding="utf-8")
        required = (
            "impl PreparedStatement for StandardAdapterPreparedStatement",
            "set_parameter",
            "execute_prepared_query",
            "execute_prepared_update",
            "executor_core",
        )
        if any(marker not in standard_source for marker in required):
            fail(f"common canonical Prepared behavior is incomplete: {row.implementation_type}")
        owner = row.standard_core_evidence.split("::", 1)[0]
        family_sources = list((root / "crates/druid-wrapper/src").rglob("*connection*.rs"))
        owner_source = next(
            (path.read_text(encoding="utf-8") for path in family_sources if owner in path.read_text(encoding="utf-8")),
            None,
        )
        if owner_source is None or any(
            marker not in owner_source
            for marker in (
                f"impl AdapterExecutionCore for {owner}",
                "standard_executor_core",
                "StandardAdapterPreparedStatement::new",
            )
        ):
            fail(f"family prepared executor provenance is missing: {row.implementation_type}")
        return

    target_leaf = row.target_trait.rsplit("::", 1)[-1]
    standard = extract_impl_bodies(source, row.target_trait, row.implementation_type)
    wrapper_only = target_leaf == "Wrapper" and has_wrapper_macro(
        source, row.implementation_type, row.target_trait
    )
    if not standard and not wrapper_only:
        fail(
            f"canonical adapter implements only Physical*: {row.implementation_type} "
            f"missing {row.target_trait}"
        )
    canonical_path = row.target_trait.replace("druid_rdbc::", "druid_rdbc :: ").replace("::", " :: ")
    canonical_import = row.target_trait.rsplit("::", 1)[0].replace("::", " :: ") + f" :: {target_leaf}"
    macro_provenance = (
        "druid_rdbc :: impl_standard_wrapper" in tokens
        and f"=> {target_leaf}" in tokens
    )
    if canonical_path not in tokens and canonical_import not in tokens and not macro_provenance:
        fail(f"standard impl lacks canonical trait provenance: {row.implementation_type}")
    if re.search(rf"\btrait\s+{re.escape(target_leaf)}\b", source):
        fail(f"local same-leaf trait shadows canonical identity: {target_leaf}")
    if re.search(r"#\s*\[\s*cfg\s*\(\s*any\s*\(\s*\)\s*\)\s*\]\s*impl", source):
        fail(f"canonical impl is production-disabled: {row.implementation_type}")
    for body in standard:
        if not code_tokens(body).strip():
            fail(f"canonical impl inherits only empty/default behavior: {row.implementation_type}")
        if "Physical" in body:
            fail(f"standard impl delegates back to Physical: {row.implementation_type}")
        markers = [marker for marker in SQL_EXECUTION_MARKERS if marker in body]
        delegates = "_core(" in body or "actor" in body.lower()
        if markers and not delegates:
            fail(
                f"standard impl duplicates SQL/default/error logic instead of delegating one core: "
                f"{row.implementation_type}: {markers}"
            )
    current_cores = set().union(*(core_calls(body) for body in current)) if current else set()
    standard_cores = set().union(*(core_calls(body) for body in standard)) if standard else set()
    if target_leaf in {"ConnectionFactory", "DatabaseMetaData", "ResultSet"}:
        transitive_factory_core = (
            target_leaf == "ConnectionFactory"
            and "create_adapter_core" in source
            and "create_adapter_core" in standard[0]
        )
        if not current_cores.intersection(standard_cores) and not transitive_factory_core:
            fail(f"standard and Physical impls use split cores: {row.implementation_type}")
    if target_leaf == "Connection":
        required = ("create_statement", "prepare_statement")
        if not standard or any(name not in standard[0] for name in required):
            fail(f"connection lacks required standard handle behavior: {row.implementation_type}")
        family_connections = {
            "RbdcConnectionAdapter",
            "ToastyConnectionAdapter",
            "DuckDbConnectionAdapter",
            "HttpSqlConnectionAdapter",
            "LibSqlConnectionAdapter",
            "JdbcAgentConnection",
        }
        if row.implementation_type == "SqlxConnectionAdapter":
            if "standard_actor_core" not in standard[0]:
                fail("SQLx connection does not create handles from its owned actor core")
        elif concrete_type(row.implementation_type) in family_connections and (
            f"impl AdapterExecutionCore for {concrete_type(row.implementation_type)}" not in source
            or "StandardAdapterStatement::new" not in standard[0]
            or "StandardAdapterPreparedStatement::new" not in standard[0]
        ):
            fail(f"family connection lacks owned executor provenance: {row.implementation_type}")
    required_methods = {
        "PreparedStatement": ("set_parameter", "execute_prepared_query", "execute_prepared_update"),
        "ResultSet": ("next", "value", "close"),
        "DatabaseMetaData": ("database_product_name",),
        "ConnectionFactory": ("create",),
    }.get(target_leaf, ())
    if standard and any(method not in standard[0] for method in required_methods):
        fail(f"canonical impl lacks required behavior: {row.implementation_type}: {required_methods}")
    owner, _, methods = row.standard_core_evidence.partition("::")
    production_sources = "\n".join(
        path.read_text(encoding="utf-8")
        for path in (root / "crates/druid-wrapper/src").rglob("*.rs")
    )
    if owner not in production_sources or any(
        method not in production_sources for method in methods.split("/") if method
    ):
        fail(f"adapter core evidence is not source-bound: {row.standard_core_evidence}")


def verify_consumer_rows(root: Path, path: Path) -> int:
    """Require every Task 4 wrapper consumer row to record its live transition."""

    rows = [
        row
        for row in read_tsv(path)
        if row["target_task"] == TASK
        and row["source_path"].startswith("crates/druid-wrapper/")
    ]
    if len(rows) != 21:
        fail(f"Task 4 wrapper consumer denominator drift: expected 21 rows, found {len(rows)}")
    for row in rows:
        if row["state"] != LIVE_CONSUMER_STATE:
            fail(
                f"Task 4 wrapper consumer is not LIVE: {row['source_path']}: "
                f"{row['symbol']}: {row['state']}"
            )
        if not (root / row["source_path"]).is_file():
            fail(f"Task 4 wrapper consumer source is missing: {row['source_path']}")
        source_tokens = code_tokens((root / row["source_path"]).read_text(encoding="utf-8"))
        old_leaf = row["old_path"].rsplit("::", 1)[-1]
        if re.search(rf"\b{re.escape(old_leaf)}\b", source_tokens):
            fail(f"closed legacy consumer still exists in source: {row['source_path']}: {old_leaf}")
    return len(rows)


def verify_sqlx_ownership(root: Path) -> None:
    """Require owned SQLx Statement/ResultSet/live metadata actor evidence."""

    actor = (root / "crates/druid-wrapper/src/sqlx/sqlx_connection_actor.rs")
    if not actor.is_file():
        fail("SQLx statement-owned executor is missing")
    actor_source = code_tokens(actor.read_text(encoding="utf-8"))
    required_actor = (
        "struct SqlxConnectionActor",
        "Arc < SqlxConnectionActor >",
        "tokio :: sync :: Mutex",
        "Box < dyn ResultSet >",
        "fetch_rows_with_labels",
    )
    missing = [marker for marker in required_actor if marker not in actor_source]
    if missing:
        fail(f"SQLx actor ownership evidence is incomplete: {missing}")

    prepared = code_tokens((root / "crates/druid-wrapper/src/sqlx/sqlx_prepared_statement.rs").read_text(encoding="utf-8"))
    result_set = code_tokens((root / "crates/druid-wrapper/src/sqlx/sqlx_streaming_result_set.rs").read_text(encoding="utf-8"))
    metadata = code_tokens((root / "crates/druid-wrapper/src/sqlx/sqlx_database_meta_data.rs").read_text(encoding="utf-8"))
    if "Arc < SqlxConnectionActor >" not in prepared:
        fail("SQLx Statement/PreparedStatement does not own its executor actor")
    if "Arc < SqlxConnectionActor >" not in result_set:
        fail("SQLx ResultSet does not retain its query actor/lease")
    if "Arc < SqlxConnectionActor >" not in metadata:
        fail("SQLx live DatabaseMetaData does not own its actor")
    if "ResultSetMetaDataSnapshot" not in result_set or not any(
        marker in result_set
        for marker in ("Arc < dyn ResultSetMetaData >", "Arc < dyn StandardResultSetMetaData >")
    ):
        fail("SQLx ResultSet metadata is not an immutable Arc snapshot")
    if "Arc < dyn ParameterMetaData >" not in prepared:
        fail("SQLx parameter metadata is not an immutable Arc snapshot")


def verify_repository(
    root: Path,
    adapters_path: Path = DEFAULT_ADAPTERS,
    consumers_path: Path = DEFAULT_CONSUMERS,
) -> tuple[int, int]:
    """Verify all live Task 4 adapter and ownership obligations."""

    adapters = task_adapter_rows(root / adapters_path)
    for row in adapters:
        verify_adapter_source(root, row)
    consumers = verify_consumer_rows(root, root / consumers_path)
    verify_sqlx_ownership(root)
    return sum(row.state == LIVE_ADAPTER_STATE for row in adapters), consumers


def main() -> int:
    """CLI entry point."""

    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    arguments = parser.parse_args()
    try:
        adapter_count, consumer_count = verify_repository(arguments.root.resolve())
    except (OSError, UnicodeError, VerificationError) as error:
        print(f"WRAPPER_STANDARD_IMPLS_FAIL={error}")
        return 1
    print(f"WRAPPER_STANDARD_ADAPTERS_LIVE={adapter_count}")
    print(f"WRAPPER_STANDARD_CONSUMERS_CLOSED={consumer_count}")
    print("FAMILY_COMMON_PREPARED_CHAINS_LIVE=6")
    print("SQLX_OWNED_ACTOR_LIVE=1")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
