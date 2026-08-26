#!/usr/bin/env python3
"""Render/check Task 6's exact public/config/wire breaking allowlist."""

from __future__ import annotations

import argparse
import csv
import hashlib
import io
import re
from pathlib import Path
from typing import NoReturn


BASE_SNAPSHOTS = {
    "druid": Path("api/druid-public-api.rdbc-first.before.txt"),
    "druid-wrapper": Path("api/druid-wrapper.rdbc-first.before.txt"),
}
CURRENT_SNAPSHOTS = {
    "druid": Path("api/druid.single-standard.after.txt"),
    "druid-wrapper": Path("api/druid-wrapper.single-standard.after.txt"),
}
RDBC_CURRENT_SNAPSHOT = Path("api/druid-rdbc.single-standard.after.txt")
OUTPUT = Path("docs/verification/rdbc-first/task6-public-contract-allowlist.tsv")
HEADER = (
    "contract_kind",
    "package",
    "old_path",
    "disposition",
    "target_path",
    "baseline_signature_sha256",
    "evidence_test",
    "notes",
)
HELPER_SIGNATURE = re.compile(
    r"PhysicalConnection(?:Connect|Close)(?:FilterChain|Result|Context)"
)
FIELD_PATTERN = re.compile(r"^pub ([A-Za-z0-9_]+(?:::[A-Za-z0-9_]+)*):\s")
FUNCTION_PATTERN = re.compile(r"^pub(?:(?: async| const| unsafe))* fn (.+)$")
RETAINED_ALIASES = {
    "druid::dynamic::high_available_data_source::HighAvailableDataSource::set_physical_timeout",
    "druid::pool::config::DruidPoolBuilder::physical_connection_timeout",
    "druid::pool::config::DruidPoolBuilder::phy_timeout",
    "druid::pool::config::PoolInnerConfig::physical_connection_timeout",
    "druid::pool::PoolInnerConfig::physical_connection_timeout",
}
CONFIG_EVIDENCE = "crates/druid/tests/task6_public_config_compatibility_test.rs#deprecated_timeout_entrypoints_delegate_to_driver_named_configuration"
METHOD_EVIDENCE = (
    "docs/verification/rdbc-first/task6-method-migration.tsv#canonical-standard-methods"
)
TEST_LEDGER_EVIDENCE = "docs/verification/rdbc-first/task6-deleted-test-migration.tsv#exact-denominator=1283"
DRIVER_METHOD_EVIDENCE = {
    "name": "crates/druid-rdbc/tests/driver_contract_test.rs#driver_exposes_its_registry_name",
    "rdbc_compliant": "crates/druid-rdbc/tests/driver_contract_test.rs#driver_defaults_preserve_jdbc_compliance_and_parent_logger",
    "parent_logger": "crates/druid-rdbc/tests/driver_contract_test.rs#driver_defaults_preserve_jdbc_compliance_and_parent_logger",
    "get_parent_logger": "crates/druid-rdbc/tests/driver_contract_test.rs#driver_defaults_preserve_jdbc_compliance_and_parent_logger",
}
DRIVER_PROPERTY_EVIDENCE = "crates/druid-rdbc/tests/driver_contract_test.rs#driver_property_constructor_preserves_jdbc_defaults"
WIRE_ROWS = (
    (
        "RUST_FIELD",
        "druid-admin",
        "druid_admin::model::dto::DataSourceContent::physical_connect_count",
        "APPROVED_BREAKING_RENAME",
        "druid_admin::model::dto::DataSourceContent::driver_connect_count",
        "crates/druid-admin/tests/task6_admin_wire_contract_test.rs#task6_admin_counter_keys_use_the_approved_driver_contract",
        "Rust DTO field renamed with the same counter value",
    ),
    (
        "RUST_FIELD",
        "druid-admin",
        "druid_admin::model::dto::DataSourceContent::physical_close_count",
        "APPROVED_BREAKING_RENAME",
        "druid_admin::model::dto::DataSourceContent::driver_close_count",
        "crates/druid-admin/tests/task6_admin_wire_contract_test.rs#task6_admin_counter_keys_use_the_approved_driver_contract",
        "Rust DTO field renamed with the same counter value",
    ),
    (
        "RUST_FIELD",
        "druid-admin",
        "druid_admin::model::dto::DataSourceContent::physical_connect_error_count",
        "APPROVED_BREAKING_RENAME",
        "druid_admin::model::dto::DataSourceContent::driver_connect_error_count",
        "crates/druid-admin/tests/task6_admin_wire_contract_test.rs#task6_admin_counter_keys_use_the_approved_driver_contract",
        "Rust DTO field renamed with the same counter value",
    ),
    (
        "SERIALIZED_KEY",
        "druid-admin",
        "admin::DataSourceContent::PhysicalConnectCount",
        "APPROVED_BREAKING_RENAME",
        "admin::DataSourceContent::DriverConnectCount",
        "crates/druid-admin/tests/task6_admin_wire_contract_test.rs#task6_admin_counter_keys_use_the_approved_driver_contract",
        "admin JSON emits the approved driver-named key only",
    ),
    (
        "SERIALIZED_KEY",
        "druid-admin",
        "admin::DataSourceContent::PhysicalCloseCount",
        "APPROVED_BREAKING_RENAME",
        "admin::DataSourceContent::DriverCloseCount",
        "crates/druid-admin/tests/task6_admin_wire_contract_test.rs#task6_admin_counter_keys_use_the_approved_driver_contract",
        "admin JSON emits the approved driver-named key only",
    ),
    (
        "SERIALIZED_KEY",
        "druid-admin",
        "admin::DataSourceContent::PhysicalConnectErrorCount",
        "APPROVED_BREAKING_RENAME",
        "admin::DataSourceContent::DriverConnectErrorCount",
        "crates/druid-admin/tests/task6_admin_wire_contract_test.rs#task6_admin_counter_keys_use_the_approved_driver_contract",
        "admin JSON emits the approved driver-named key only",
    ),
)


class VerificationError(RuntimeError):
    """Raised for incomplete or stale public-contract dispositions."""


def fail(message: str) -> NoReturn:
    """Stop instead of silently approving an unknown public break."""

    raise VerificationError(message)


def snapshot_items(path: Path) -> dict[str, str]:
    """Index public methods/fields by their exact cargo-public-api path."""

    signatures: dict[str, set[str]] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        function = FUNCTION_PATTERN.match(line)
        if function:
            raw = function.group(1).split("(", 1)[0]
            previous = None
            while previous != raw:
                previous = raw
                raw = re.sub(r"<[^<>]*>", "", raw)
            item = raw
        else:
            match = FIELD_PATTERN.match(line)
            item = match.group(1) if match else ""
        if not item or not re.fullmatch(r"[A-Za-z0-9_]+(?:::[A-Za-z0-9_]+)*", item):
            continue
        signatures.setdefault(item, set()).add(line)
    return {item: "\n".join(sorted(lines)) for item, lines in signatures.items()}


def affected_items(path: Path) -> dict[str, str]:
    """Select exact renamed identifiers plus filter signatures using retired helper types."""

    affected: dict[str, str] = {}
    for item, line in snapshot_items(path).items():
        leaf = item.rsplit("::", 1)[-1].lower()
        if "physical" in leaf or leaf == "phy_timeout" or HELPER_SIGNATURE.search(line):
            affected[item] = line
    return affected


def candidate_target(old_path: str) -> str:
    """Apply only the reviewed identifier mappings; unknown removals fail closed."""

    target = old_path
    target = target.replace(
        "druid::core::physical_connection_lease::PhysicalConnectionLease::",
        "druid::core::external_connection_lease::ExternalConnectionLease::",
    )
    target = target.replace(
        "druid::core::connection::PhysicalConnection::",
        "druid_rdbc::connection::Connection::",
    )
    target = target.replace(
        "druid::core::physical_connection::PhysicalConnection::",
        "druid_rdbc::connection::Connection::",
    )
    target = target.replace(
        "druid::core::PhysicalConnection::", "druid_rdbc::connection::Connection::"
    )
    target = target.replace(
        "druid::core::connection::Connection::", "druid_rdbc::connection::Connection::"
    )
    target = target.replace(
        "druid::core::Connection::", "druid_rdbc::connection::Connection::"
    )
    target = target.replace(
        "::physical_connection::PhysicalConnection::", "::connection::Connection::"
    )
    target = target.replace(
        "::connection::PhysicalConnection::", "::connection::Connection::"
    )
    target = target.replace("::PhysicalConnection::", "::Connection::")
    target = target.replace(
        "::PhysicalConnectionCloseContext::", "::ConnectionCloseContext::"
    )
    target = target.replace(
        "::PhysicalConnectionCloseFilterChain::", "::ConnectionCloseFilterChain::"
    )
    target = target.replace(
        "::PhysicalConnectionConnectFilterChain::", "::ConnectionOpenFilterChain::"
    )
    target = target.replace(
        "::PhysicalConnectionConnectResult::", "::ConnectionOpenResult::"
    )
    replacements = {
        "::create_physical_statement": "::create_statement",
        "::prepare_physical_statement": "::prepare_statement",
        "::prepare_physical_call": "::prepare_call",
        "::has_physical_connection": "::has_standard_connection",
        "::physical_age": "::connection_age",
        "::physical_connection_connect": "::connection_open",
        "::physical_connection_close": "::connection_close",
        "::physical_connect_count": "::driver_connect_count",
        "::physical_close_count": "::driver_close_count",
        "::physical_connect_error_count": "::driver_connect_error_count",
        "::set_physical_timeout": "::set_driver_timeout",
    }
    for old, new in replacements.items():
        if target.endswith(old):
            target = target[: -len(old)] + new
            break
    return target


def known_internal_or_deleted(old_path: str) -> tuple[str, str, str]:
    """Disposition lower-case runtime accessors that intentionally left the public surface."""

    leaf = old_path.rsplit("::", 1)[-1]
    if old_path.endswith("::PhysicalConnectionConnectResult::into_parts"):
        return (
            "APPROVED_INTERNALIZED",
            "druid::core::filter_chain_impl::ConnectionOpenResult::into_parts",
            "connection acquisition envelope is private",
        )
    if "PhysicalConnectionInfo" in old_path:
        target = {
            "physical_connection": "druid::core::connection_info::ConnectionInfo::connection_mut",
            "physical_connection_box_mut": "druid::core::connection_info::ConnectionInfo::connection_mut",
            "take_physical_connection": "druid::core::connection_info::ConnectionInfo::take_connection",
        }.get(leaf)
        if target:
            return "APPROVED_INTERNALIZED", target, "acquisition envelope is private"
    if "DruidConnectionHolder" in old_path:
        target = {
            "physical_connection": "druid::core::druid_connection_holder::DruidConnectionHolder::standard_connection",
            "physical_connection_mut": "druid::core::druid_connection_holder::DruidConnectionHolder::standard_connection_mut",
            "physical_connection_box_mut": "druid::core::druid_connection_holder::DruidConnectionHolder::standard_connection_mut",
            "take_physical_connection": "druid::core::druid_connection_holder::DruidConnectionHolder::take_standard_connection",
        }.get(leaf)
        if target:
            return "APPROVED_INTERNALIZED", target, "raw holder access is private"
    value_targets = {
        "druid::core::rdbc_opaque_object::RdbcOpaqueObject::physical": "druid_rdbc::types::RdbcOpaqueObject::value",
        "druid::core::rdbc_xml_result::RdbcXmlResult::physical": "druid_rdbc::resource::RdbcXmlResult::value",
        "druid::core::rdbc_xml_source::RdbcXmlSource::physical": "druid_rdbc::resource::RdbcXmlSource::value",
    }
    if old_path in value_targets:
        return (
            "APPROVED_BREAKING_RENAME",
            value_targets[old_path],
            "backend accessor renamed value",
        )
    if leaf in {
        "physical_connection",
        "physical_connection_mut",
        "physical_connection_box_mut",
        "take_physical_connection",
        "physical_callable_statement",
        "physical_column",
        "set_physical_column_map",
        "physical",
        "from_physical",
    }:
        return "APPROVED_BREAKING_DELETE", "-", "retired raw runtime accessor"
    fail(f"unreviewed public/config rename: {old_path}")


def evidence_for(package: str, old_path: str) -> str:
    """Bind each exact break to the corresponding executable denominator."""

    if old_path in RETAINED_ALIASES:
        return CONFIG_EVIDENCE
    if package == "druid-wrapper" or any(
        token in old_path
        for token in (
            "::Connection::create_physical_statement",
            "::Connection::prepare_physical_statement",
            "::Connection::prepare_physical_call",
            "::PhysicalConnection::create_physical_statement",
            "::PhysicalConnection::prepare_physical_statement",
            "::PhysicalConnection::prepare_physical_call",
        )
    ):
        return METHOD_EVIDENCE
    return TEST_LEDGER_EVIDENCE


def build_rows(root: Path) -> tuple[dict[str, str], ...]:
    """Join every exact BASE identifier to a retained, renamed, internalized, or deleted target."""

    rows: list[dict[str, str]] = []
    rdbc_current = snapshot_items(root / RDBC_CURRENT_SNAPSHOT)
    for package in ("druid", "druid-wrapper"):
        base_path = root / BASE_SNAPSHOTS[package]
        current_path = root / CURRENT_SNAPSHOTS[package]
        base = affected_items(base_path)
        current = snapshot_items(current_path)
        for old_path in sorted(base):
            baseline = base[old_path]
            if old_path in RETAINED_ALIASES:
                if old_path not in current:
                    fail(f"deprecated alias is missing from current API: {old_path}")
                disposition = "DEPRECATED_ALIAS_RETAINED"
                target = old_path
                notes = "one-release compatibility alias"
            elif old_path in current:
                disposition = "APPROVED_BREAKING_SIGNATURE"
                target = old_path
                notes = "same method/field path now uses canonical standard types"
            else:
                candidate = candidate_target(old_path)
                if candidate != old_path and (
                    candidate in current or candidate in rdbc_current
                ):
                    disposition = "APPROVED_BREAKING_RENAME"
                    target = candidate
                    notes = "exact canonical identifier replacement"
                else:
                    disposition, target, notes = known_internal_or_deleted(old_path)
            rows.append(
                {
                    "contract_kind": "PUBLIC_API",
                    "package": package,
                    "old_path": old_path,
                    "disposition": disposition,
                    "target_path": target,
                    "baseline_signature_sha256": hashlib.sha256(
                        baseline.encode("utf-8")
                    ).hexdigest(),
                    "evidence_test": evidence_for(package, old_path),
                    "notes": notes,
                }
            )
    base_druid = snapshot_items(root / BASE_SNAPSHOTS["druid"])
    for old_path in sorted(base_druid):
        parts = old_path.split("::")
        if (
            len(parts) < 2
            or parts[-2] != "Driver"
            or parts[-1] not in DRIVER_METHOD_EVIDENCE
        ):
            continue
        method = parts[-1]
        target = f"druid_rdbc::driver::Driver::{method}"
        if target not in rdbc_current:
            fail(f"restored canonical Driver method is absent: {target}")
        rows.append(
            {
                "contract_kind": "RESTORED_PUBLIC_API",
                "package": "druid-rdbc",
                "old_path": old_path,
                "disposition": "RESTORED_CANONICAL_EXACT",
                "target_path": target,
                "baseline_signature_sha256": hashlib.sha256(
                    base_druid[old_path].encode("utf-8")
                ).hexdigest(),
                "evidence_test": DRIVER_METHOD_EVIDENCE[method],
                "notes": "BASE Driver semantic restored on the canonical trait",
            }
        )
    property_old = "druid::core::driver::DriverProperty::new"
    property_target = "druid_rdbc::driver::DriverPropertyInfo::new"
    if property_old not in base_druid or property_target not in rdbc_current:
        fail("restored canonical DriverProperty constructor is absent")
    rows.append(
        {
            "contract_kind": "RESTORED_PUBLIC_API",
            "package": "druid-rdbc",
            "old_path": property_old,
            "disposition": "RESTORED_CANONICAL_EXACT",
            "target_path": property_target,
            "baseline_signature_sha256": hashlib.sha256(
                base_druid[property_old].encode("utf-8")
            ).hexdigest(),
            "evidence_test": DRIVER_PROPERTY_EVIDENCE,
            "notes": "BASE DriverProperty defaults restored on DriverPropertyInfo",
        }
    )
    for kind, package, old_path, disposition, target, evidence, notes in WIRE_ROWS:
        rows.append(
            {
                "contract_kind": kind,
                "package": package,
                "old_path": old_path,
                "disposition": disposition,
                "target_path": target,
                "baseline_signature_sha256": "-",
                "evidence_test": evidence,
                "notes": notes,
            }
        )
    keys = [(row["contract_kind"], row["package"], row["old_path"]) for row in rows]
    if len(set(keys)) != len(keys):
        fail("public contract allowlist contains duplicate identities")
    return tuple(rows)


def render_rows(rows: tuple[dict[str, str], ...]) -> bytes:
    """Render canonical LF TSV."""

    output = io.StringIO(newline="")
    writer = csv.DictWriter(
        output, fieldnames=HEADER, delimiter="\t", lineterminator="\n"
    )
    writer.writeheader()
    for row in rows:
        ordered = {field: row[field] for field in HEADER}
        if any(
            "\t" in value or "\n" in value or "\r" in value
            for value in ordered.values()
        ):
            fail(f"allowlist value contains a TSV delimiter/control: {row['old_path']}")
        writer.writerow(ordered)
    return output.getvalue().encode("utf-8")


def main() -> int:
    """Write on request; otherwise require the checked allowlist to match exactly."""

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root", type=Path, default=Path(__file__).resolve().parent.parent
    )
    parser.add_argument("--write", action="store_true")
    arguments = parser.parse_args()
    root = arguments.root.resolve()
    try:
        rows = build_rows(root)
        expected = render_rows(rows)
        output = root / OUTPUT
        if arguments.write:
            output.write_bytes(expected)
        elif not output.is_file() or output.read_bytes() != expected:
            fail("checked public contract allowlist is missing or stale")
    except (OSError, UnicodeError, VerificationError) as error:
        print(f"TASK6_PUBLIC_CONTRACT_ALLOWLIST_FAIL={error}")
        return 1
    print(f"TASK6_PUBLIC_CONTRACT_ALLOWLIST_ROWS={len(rows)}")
    print(
        f"TASK6_PUBLIC_CONTRACT_ALLOWLIST_SHA256={hashlib.sha256(expected).hexdigest()}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
