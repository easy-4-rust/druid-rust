#!/usr/bin/env python3
"""Adversarial tests for the final no-Physical standard verifier."""

from __future__ import annotations

import json
import inspect
import subprocess
import tempfile
import unittest
from unittest.mock import patch
from pathlib import Path

import verify_no_physical_standard as verifier
from verify_no_physical_standard import (
    VerificationError,
    discover_production_files,
    parse_migration_table,
    verify_migration_table,
    verify_no_duplicate_trait_tree,
    verify_source_surface,
    verify_test_disposition_table,
    verify_text_artifact,
)


class RepositoryFixture:
    """Small workspace fixture whose package/source denominator is metadata-driven."""

    def __init__(self, root: Path) -> None:
        self.root = root
        self.packages = root / "crates"
        self.write("Cargo.toml", '[workspace]\nmembers = ["crates/*"]\n')
        self.add_package("druid-rdbc")
        self.add_package("druid")

    def write(self, relative: str, text: str) -> None:
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        # 固定 UTF-8/LF 字节，避免 Windows 文本模式把规范夹具转换为 CRLF。
        path.write_bytes(text.encode("utf-8"))

    def add_package(self, name: str) -> None:
        self.write(
            f"crates/{name}/Cargo.toml",
            "[package]\n"
            f'name = "{name}"\n'
            'version = "0.0.0"\n'
            'edition = "2021"\n'
            "\n[features]\ndefault = []\n",
        )
        self.write(f"crates/{name}/src/lib.rs", "#![forbid(unsafe_code)]\n")

    def metadata(self) -> dict[str, object]:
        packages = []
        for name in ("druid-rdbc", "druid"):
            package_root = self.packages / name
            packages.append(
                {
                    "name": name,
                    "manifest_path": str(package_root / "Cargo.toml"),
                    "targets": [
                        {
                            "src_path": str(package_root / "src/lib.rs"),
                            "kind": ["lib"],
                        }
                    ],
                    "features": {"default": []},
                }
            )
        return {"workspace_root": str(self.root), "packages": packages}


class NoPhysicalVerifierTest(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.fixture = RepositoryFixture(self.root)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def assert_rejected(self, expected: str, operation) -> None:
        with self.assertRaisesRegex(VerificationError, expected):
            operation()

    def test_fixture_preserves_lf_and_verifier_rejects_crlf(self) -> None:
        """规范夹具保持 LF；真实 CRLF 输入仍必须被严格拒绝。"""

        canonical = "old_path\ttarget_path\nold\tnew\n"
        self.fixture.write("canonical.tsv", canonical)
        path = self.root / "canonical.tsv"
        self.assertEqual(path.read_bytes(), canonical.encode("utf-8"))
        self.assertEqual(
            verifier.read_tsv(path), [{"old_path": "old", "target_path": "new"}]
        )
        path.write_bytes(canonical.replace("\n", "\r\n").encode("utf-8"))
        self.assert_rejected("TSV is not canonical", lambda: verifier.read_tsv(path))

    def test_public_api_generators_keep_date_without_foreign_host(self) -> None:
        """API 两入口固定同一日期，主机由 rustup 选择，不使用外国主机工具链。"""

        import subprocess
        import verify_task6_public_contract as public_contract

        for label, generate in (
            ("rdbc", lambda: verifier.generate_public_api(self.root)),
            ("druid", lambda: public_contract.generate_api(self.root, "druid")),
        ):
            with self.subTest(label=label):
                output = subprocess.CompletedProcess([], 0, stdout="canonical API", stderr="")
                with patch("subprocess.run", return_value=output) as run:
                    self.assertEqual(generate(), "canonical API")
                command = run.call_args.args[0]
                self.assertEqual(command[:2], ["cargo", "public-api"])
                self.assertIn("--all-features", command)
                self.assertIn("-sss", command)
                self.assertEqual(
                    run.call_args.kwargs["env"]["RUSTUP_TOOLCHAIN"], "nightly-2026-08-07"
                )

    def test_bounded_runner_allows_explicit_long_parity_budget(self) -> None:
        output = subprocess.CompletedProcess([], 0, stdout="ok", stderr="")
        with patch("subprocess.run", return_value=output) as run:
            self.assertEqual(
                verifier.run_checked(
                    ["python", "parity.py"],
                    cwd=self.root,
                    label="parity",
                    timeout=1800,
                ).stdout,
                "ok",
            )
        self.assertEqual(run.call_args.kwargs["timeout"], 1800)

    def test_nested_parity_runner_waits_for_child_and_propagates_failure(self) -> None:
        success = subprocess.CompletedProcess([], 0, stdout="complete", stderr="")
        with patch("subprocess.run", return_value=success) as run:
            self.assertEqual(
                verifier.run_checked(
                    ["python", "parity.py"],
                    cwd=self.root,
                    label="Task 6 exact method parity",
                    timeout=None,
                ).stdout,
                "complete",
            )
        self.assertIsNone(run.call_args.kwargs["timeout"])

        failure = subprocess.CompletedProcess([], 7, stdout="", stderr="child failed")
        with patch("subprocess.run", return_value=failure):
            self.assert_rejected(
                "Task 6 exact method parity failed: child failed",
                lambda: verifier.run_checked(
                    ["python", "parity.py"],
                    cwd=self.root,
                    label="Task 6 exact method parity",
                    timeout=None,
                ),
            )

    def test_discovers_every_package_source_instead_of_a_fixed_file_list(self) -> None:
        self.fixture.write(
            "crates/druid/src/unlisted/deep.rs",
            '#[cfg(feature = "legacy")]\n'
            "macro_rules! hidden {\n"
            "    () => { pub type Alias = druid_rdbc::spi::PhysicalConnection; };\n"
            "}\n",
        )
        files = discover_production_files(self.root, self.fixture.metadata())
        self.assertIn(Path("crates/druid/src/unlisted/deep.rs"), files)
        self.assert_rejected(
            "PhysicalConnection",
            lambda: verify_source_surface(self.root, self.fixture.metadata()),
        )

    def test_rejects_group_alias_macro_cfg_path_and_rustdoc_tricks(self) -> None:
        cases = {
            "grouped": "use druid_rdbc::{spi::{self as driver_spi}};\n",
            "raw": "use druid_rdbc::r#spi as port;\n",
            "macro": "macro_rules! port { () => { druid_rdbc::spi::Connection }; }\n",
            "cfg": "#[cfg(any())] use druid_rdbc::spi;\n",
            "path": '#[path = "shadow.rs"] mod shadow;\n',
            "include": 'include!("shadow.rs");\n',
            "rustdoc": "/// The old `PhysicalConnection` contract.\npub struct Canonical;\n",
        }
        for name, source in cases.items():
            with self.subTest(name=name):
                self.fixture.write("crates/druid/src/lib.rs", source)
                self.assert_rejected(
                    "forbidden|path attribute|include!",
                    lambda: verify_source_surface(self.root, self.fixture.metadata()),
                )

    def test_rejects_private_duplicate_standard_trait_without_physical_name(
        self,
    ) -> None:
        self.fixture.write(
            "crates/druid-rdbc/src/lib.rs",
            "pub mod connection;\n",
        )
        self.fixture.write(
            "crates/druid-rdbc/src/connection.rs",
            "pub trait Connection {\n"
            "    fn create_statement(&mut self);\n"
            "    fn prepare_statement(&mut self);\n"
            "    fn commit(&mut self);\n"
            "    fn rollback(&mut self);\n"
            "    fn close(&mut self);\n"
            "}\n",
        )
        self.fixture.write(
            "crates/druid/src/private_port.rs",
            "trait RawPort {\n"
            "    fn create_statement(&mut self);\n"
            "    fn prepare_statement(&mut self);\n"
            "    fn commit(&mut self);\n"
            "    fn rollback(&mut self);\n"
            "    fn close(&mut self);\n"
            "}\n",
        )
        self.assert_rejected(
            "duplicate standard trait",
            lambda: verify_no_duplicate_trait_tree(self.root, self.fixture.metadata()),
        )

    def test_migration_table_is_exact_unique_and_target_bound(self) -> None:
        self.fixture.write(
            "physical.tsv",
            "old_path\tclassification\ttarget_path\tcompatibility\tsource_file\t"
            "normalized_signature\ttarget_task\n"
            "druid_rdbc::spi::PhysicalConnection\tMERGE_STANDARD\t"
            "druid_rdbc::connection::Connection\tRENAME_AND_RETAIN_UNTIL_P04_TASK6\t"
            "crates/druid-rdbc/src/spi/physical_connection.rs\t"
            "trait PhysicalConnection\tP04_TASK_3_STANDARD_TRAITS\n",
        )
        self.fixture.write(
            "consumers.tsv",
            "source_path\toccurrence_kind\tsymbol\told_path\ttarget_path\t"
            "target_task\tstate\n"
            "crates/druid/src/lib.rs\tIMPORT\tPhysicalConnection@token=1\t"
            "druid_rdbc::spi::PhysicalConnection\tdruid_rdbc::connection::Connection\t"
            "P04_TASK_5_DRUID_INTERNALIZATION\tTASK6_COMPATIBILITY_PENDING\n",
        )
        self.fixture.write(
            "migration.md",
            "| old_path | disposition | target_path | evidence |\n"
            "| --- | --- | --- | --- |\n"
            "| `druid_rdbc::spi::PhysicalConnection` | `MERGED_STANDARD` | "
            "`druid_rdbc::connection::Connection` | canonical public API |\n"
            "| `druid_rdbc::spi` | `DELETED` | `-` | retired module |\n",
        )
        rows = parse_migration_table(self.root / "migration.md")
        self.assertEqual(len(rows), 2)
        verify_migration_table(
            self.root,
            self.root / "physical.tsv",
            self.root / "consumers.tsv",
            self.root / "migration.md",
            public_api_items={"druid_rdbc::connection::Connection"},
        )

        self.fixture.write(
            "migration.md",
            "| old_path | disposition | target_path | evidence |\n"
            "| --- | --- | --- | --- |\n"
            "| `druid_rdbc::spi::PhysicalConnection` | `MERGED_STANDARD` | "
            "`druid_rdbc::connection::Missing` | stale target |\n"
            "| `druid_rdbc::spi::PhysicalConnection` | `DELETED` | `-` | duplicate |\n"
            "| `druid_rdbc::spi` | `DELETED` | `-` | retired module |\n",
        )
        self.assert_rejected(
            "duplicate|does not exist",
            lambda: verify_migration_table(
                self.root,
                self.root / "physical.tsv",
                self.root / "consumers.tsv",
                self.root / "migration.md",
                public_api_items={"druid_rdbc::connection::Connection"},
            ),
        )

    def test_public_api_and_rustdoc_outputs_are_scanned(self) -> None:
        self.assert_rejected(
            "PhysicalConnection",
            lambda: verify_text_artifact(
                "public API", "pub mod druid_rdbc::spi; pub trait PhysicalConnection"
            ),
        )
        self.assert_rejected(
            "PhysicalResultSet",
            lambda: verify_text_artifact("rustdoc", "<code>PhysicalResultSet</code>"),
        )
        verify_text_artifact(
            "public API", "pub trait druid_rdbc::connection::Connection"
        )

    def test_deleted_test_dispositions_are_exact_and_replacement_bound(self) -> None:
        self.fixture.write(
            "crates/druid/tests/canonical_test.rs",
            "#[test]\nfn canonical_close_is_exactly_once() {}\n",
        )
        self.fixture.write(
            "migration.md",
            "| deleted_test_path | disposition | replacement_tests | behavioral_assertions |\n"
            "| --- | --- | --- | --- |\n"
            "| `crates/druid/tests/old_close_test.rs` | `REPLACED` | "
            "`crates/druid/tests/canonical_test.rs#canonical_close_is_exactly_once` | "
            "raw close is attempted once |\n"
            "| `crates/druid-wrapper/tests/old_identity_test.rs` | "
            "`PURE_REMOVED_API_IDENTITY` | `-` | retired trait identity only |\n",
        )
        required = {
            "crates/druid/tests/old_close_test.rs",
            "crates/druid-wrapper/tests/old_identity_test.rs",
        }
        rows = verify_test_disposition_table(
            self.root, self.root / "migration.md", required
        )
        self.assertEqual(len(rows), 2)

        self.fixture.write(
            "migration.md",
            "| deleted_test_path | disposition | replacement_tests | behavioral_assertions |\n"
            "| --- | --- | --- | --- |\n"
            "| `crates/druid/tests/old_close_test.rs` | `REPLACED` | "
            "`crates/druid/tests/canonical_test.rs#missing_test` | raw close once |\n",
        )
        self.assert_rejected(
            "denominator mismatch|function does not exist",
            lambda: verify_test_disposition_table(
                self.root,
                self.root / "migration.md",
                {"crates/druid/tests/old_close_test.rs"},
            ),
        )

    def test_live_gate_requires_exact_method_test_and_public_contract_execution(self) -> None:
        source = inspect.getsource(verifier.verify_repository)
        self.assertIn("verify_task6_method_parity.py", source)
        self.assertIn("sys.executable", source)
        self.assertIn("timeout=None", source)
        self.assertIn("verify_handoff_ledgers", source)
        self.assertIn("deleted_test_parity.execute_targets", source)
        self.assertIn("public_contract.verify_repository", source)
        import verify_task6_public_contract as public_contract
        self.assertIn("sys.executable", inspect.getsource(public_contract.verify_repository))


if __name__ == "__main__":
    unittest.main()
