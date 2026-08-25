#!/usr/bin/env python3
"""Fail-closed tests for the P04 Task 4 wrapper-standard verifier."""

from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from verify_wrapper_standard_impls import (
    AdapterRow,
    CfgProfile,
    VerificationError,
    extract_behavior_function,
    extract_impl_bodies,
    verify_adapter_source,
    verify_sqlx_ownership,
)


class WrapperStandardVerifierTest(unittest.TestCase):
    """Reject Physical-only, duplicated-core, and borrowed SQLx ownership fixtures."""

    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        self.source_path = Path("crates/druid-wrapper/src/demo.rs")
        (self.root / self.source_path).parent.mkdir(parents=True)
        tests = self.root / "crates/druid-wrapper/tests"
        tests.mkdir(parents=True)
        (tests / "demo.rs").write_text(
            "#[test]\nfn runs() { assert!(true); }\n", encoding="utf-8"
        )
        self.row = AdapterRow(
            source_path=self.source_path,
            current_trait="druid_rdbc::spi::PhysicalConnection",
            implementation_type="DemoConnection",
            target_trait="druid_rdbc::connection::Connection",
            standard_implementation_type="DemoConnection",
            standard_source_path=self.source_path,
            standard_core_evidence="DemoConnection::run_core",
            behavior_evidence="demo::runs",
            state="STANDARD_IMPL_LIVE",
        )

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def write_source(self, source: str) -> None:
        (self.root / self.source_path).write_text(source, encoding="utf-8")

    def write_valid_adapter_source(self) -> None:
        self.write_source(
            "use druid_rdbc::connection::Connection;\n"
            "impl PhysicalConnection for DemoConnection { fn run(&mut self) { self.run_core(); } }\n"
            "impl Connection for DemoConnection {\n"
            " fn create_statement(&mut self) { self.run_core(); }\n"
            " fn prepare_statement(&mut self) { self.run_core(); }\n"
            "}\n"
        )

    def verify_remote_behavior(self, behavior_source: str) -> None:
        """Run the full adapter verifier for one HTTP SQL behavior fixture."""

        self.write_source(
            "use druid_rdbc::connection::Connection;\n"
            "struct HttpSqlConnectionAdapter;\n"
            "trait AdapterExecutionCore {}\n"
            "impl AdapterExecutionCore for HttpSqlConnectionAdapter {}\n"
            "impl HttpSqlConnectionAdapter { fn run_core(&mut self) {} }\n"
            "impl PhysicalConnection for HttpSqlConnectionAdapter {\n"
            " fn run(&mut self) { self.run_core(); }\n"
            "}\n"
            "impl Connection for HttpSqlConnectionAdapter {\n"
            " fn create_statement(&mut self) {\n"
            "  self.run_core(); StandardAdapterStatement::new();\n"
            " }\n"
            " fn prepare_statement(&mut self) {\n"
            "  self.run_core(); StandardAdapterPreparedStatement::new();\n"
            " }\n"
            "}\n"
        )
        (self.root / "crates/druid-wrapper/tests/demo.rs").write_text(
            behavior_source,
            encoding="utf-8",
        )
        row = AdapterRow(
            source_path=self.source_path,
            current_trait="druid_rdbc::spi::PhysicalConnection",
            implementation_type="HttpSqlConnectionAdapter",
            target_trait="druid_rdbc::connection::Connection",
            standard_implementation_type="HttpSqlConnectionAdapter",
            standard_source_path=self.source_path,
            standard_core_evidence="HttpSqlConnectionAdapter::run_core",
            behavior_evidence="demo::runs",
            state="STANDARD_IMPL_LIVE",
        )
        verify_adapter_source(
            self.root,
            row,
            profile=CfgProfile("fixture", {"test": True}),
            enabled_tests=("runs",),
        )

    @staticmethod
    def remote_chain_steps() -> tuple[str, ...]:
        """Return the ordered factory-to-result evidence required for remote families."""

        return (
            "ConnectionFactory::create();\n",
            "connection.create_statement();\n",
            "statement.execute_update();\n",
            "connection.prepare_statement();\n",
            "prepared.set_parameter();\n",
            "prepared.execute_prepared_query();\n",
            "result.next();\n",
            "result.meta_data();\n",
            "connection.database_meta_data();\n",
            "Connection::close(&mut connection);\n",
        )

    @staticmethod
    def remote_chain() -> str:
        """Return the literal factory-to-result evidence required for remote families."""

        return "".join(WrapperStandardVerifierTest.remote_chain_steps())

    def test_rejects_adapter_that_only_implements_physical(self) -> None:
        self.write_source("impl PhysicalConnection for DemoConnection {}\n")
        with self.assertRaisesRegex(VerificationError, "implements only Physical"):
            verify_adapter_source(self.root, self.row)

    def test_rejects_duplicated_sql_in_standard_impl(self) -> None:
        self.write_source(
            "use druid_rdbc::connection::Connection;\n"
            "impl PhysicalConnection for DemoConnection {}\n"
            "impl Connection for DemoConnection {\n"
            "  async fn create_statement(&mut self) { sqlx::query(\"select 1\").execute(); }\n"
            "}\n"
        )
        with self.assertRaisesRegex(VerificationError, "duplicates SQL"):
            verify_adapter_source(self.root, self.row)

    def test_accepts_parallel_impls_that_delegate_one_core(self) -> None:
        self.write_source(
            "use druid_rdbc::connection::Connection;\n"
            "impl PhysicalConnection for DemoConnection { fn run(&mut self) { self.run_core(); } }\n"
            "impl Connection for DemoConnection {\n"
            " fn create_statement(&mut self) { self.run_core(); }\n"
            " fn prepare_statement(&mut self) { self.run_core(); }\n"
            "}\n"
        )
        verify_adapter_source(self.root, self.row)

    def test_impl_parser_handles_generic_metadata(self) -> None:
        source = "impl<'a> DatabaseMetaData for DemoMetaData<'a> { fn x(&self) {} }"
        self.assertEqual(
            len(extract_impl_bodies(source, "druid_rdbc::metadata::DatabaseMetaData", "DemoMetaData<'_>")),
            1,
        )

    def test_rejects_missing_sqlx_actor(self) -> None:
        with self.assertRaisesRegex(VerificationError, "statement-owned executor"):
            verify_sqlx_ownership(self.root)

    def test_rejects_empty_default_and_split_core_standard_impls(self) -> None:
        for source in (
            "impl PhysicalConnection for DemoConnection {}\nimpl Connection for DemoConnection {}\n",
            "impl PhysicalConnection for DemoConnection { fn run(&mut self) { self.physical_core(); } }\n"
            "impl Connection for DemoConnection { fn run(&mut self) { self.standard_core(); } }\n",
        ):
            self.write_source(source)
            with self.assertRaises(VerificationError):
                verify_adapter_source(self.root, self.row)

    def test_rejects_standard_to_physical_local_leaf_cfg_disabled_and_alias_identity(self) -> None:
        fixtures = (
            "impl PhysicalConnection for DemoConnection {}\n"
            "impl Connection for DemoConnection { fn run(&mut self) { PhysicalConnection::run(self); } }\n",
            "trait Connection {}\nimpl PhysicalConnection for DemoConnection {}\nimpl Connection for DemoConnection {}\n",
            "impl PhysicalConnection for DemoConnection {}\n#[cfg(any())] impl Connection for DemoConnection {}\n",
            "type Alias = DemoConnection;\nimpl PhysicalConnection for DemoConnection {}\nimpl Connection for Alias {}\n",
        )
        for source in fixtures:
            self.write_source(source)
            with self.assertRaises(VerificationError):
                verify_adapter_source(self.root, self.row)

    def test_rejects_comment_only_actor_ownership_markers(self) -> None:
        actor = self.root / "crates/druid-wrapper/src/sqlx/sqlx_connection_actor.rs"
        actor.parent.mkdir(parents=True, exist_ok=True)
        actor.write_text(
            "// struct SqlxConnectionActor Arc<SqlxConnectionActor> tokio::sync::Mutex "
            "Box<dyn ResultSet> fetch_rows_with_labels\n",
            encoding="utf-8",
        )
        for name, source in {
            "sqlx_prepared_statement.rs": "// Arc<SqlxConnectionActor> Arc<dyn ParameterMetaData>\n",
            "sqlx_streaming_result_set.rs": "// Arc<SqlxConnectionActor> ResultSetMetaDataSnapshot Arc<dyn ResultSetMetaData>\n",
            "sqlx_database_meta_data.rs": "// Arc<SqlxConnectionActor>\n",
        }.items():
            (actor.parent / name).write_text(source, encoding="utf-8")
        with self.assertRaises(VerificationError):
            verify_sqlx_ownership(self.root)

    def test_rejects_comment_only_behavior_evidence(self) -> None:
        """A manifest test name must resolve to executable factory-to-result behavior."""

        self.write_source(
            "use druid_rdbc::connection::Connection;\n"
            "impl PhysicalConnection for DemoConnection { fn run(&mut self) { self.run_core(); } }\n"
            "impl Connection for DemoConnection {\n"
            " fn create_statement(&mut self) { self.run_core(); }\n"
            " fn prepare_statement(&mut self) { self.run_core(); }\n"
            "}\n"
        )
        tests = self.root / "crates/druid-wrapper/tests/demo.rs"
        tests.write_text("// fn runs() {}\n", encoding="utf-8")
        with self.assertRaisesRegex(VerificationError, "behavior evidence"):
            verify_adapter_source(self.root, self.row)

    def test_rejects_test_name_only_and_empty_assertion_behavior_evidence(self) -> None:
        """A named test without executable assertions cannot carry a LIVE adapter row."""

        self.write_source(
            "use druid_rdbc::connection::Connection;\n"
            "impl PhysicalConnection for DemoConnection { fn run(&mut self) { self.run_core(); } }\n"
            "impl Connection for DemoConnection {\n"
            " fn create_statement(&mut self) { self.run_core(); }\n"
            " fn prepare_statement(&mut self) { self.run_core(); }\n"
            "}\n"
        )
        tests = self.root / "crates/druid-wrapper/tests/demo.rs"
        for source in (
            "const TEST_NAME: &str = \"runs\";\n",
            "#[test]\nfn runs() {}\n",
        ):
            tests.write_text(source, encoding="utf-8")
            with self.assertRaisesRegex(VerificationError, "behavior evidence"):
                verify_adapter_source(self.root, self.row)

    def test_rejects_cfg_feature_target_and_enclosing_disabled_behavior_tests(self) -> None:
        """A source-named test must be enabled in the declared current profile."""

        self.write_valid_adapter_source()
        tests = self.root / "crates/druid-wrapper/tests/demo.rs"
        fixtures = (
            "#[cfg(any())]\n#[test]\nfn runs() { assert!(true); }\n",
            "#[cfg(feature = \"disabled\")]\n#[test]\nfn runs() { assert!(true); }\n",
            "#[cfg(target_os = \"definitely_not_current\")]\n#[test]\nfn runs() { assert!(true); }\n",
            "#[cfg_attr(all(), cfg_attr(all(), cfg(any())))]\n"
            "#[test]\nfn runs() { assert!(true); }\n",
            "#[cfg_attr(all(), cfg_attr(any(), test))]\n"
            "fn runs() { assert!(true); }\n",
            "#[cfg(any())]\nmod disabled { #[test] fn runs() { assert!(true); } }\n",
            "struct Holder;\n#[cfg(any())]\nimpl Holder {\n"
            " #[test] fn runs() { assert!(true); }\n}\n",
        )
        for source in fixtures:
            tests.write_text(source, encoding="utf-8")
            with self.assertRaisesRegex(VerificationError, "behavior evidence"):
                verify_adapter_source(self.root, self.row)

    def test_excludes_cfg_disabled_and_statically_dead_behavior_tokens(self) -> None:
        """Required calls and assertions only count in an enabled reachable function branch."""

        for source in (
            "#[test]\nfn runs() { #[cfg(any())] { execute_update(); } assert!(true); }\n",
            "#[test]\nfn runs() { #[cfg_attr(all(), cfg(any()))] { execute_update(); } assert!(true); }\n",
            "#[test]\nfn runs() { if false { execute_update(); } assert!(true); }\n",
            "#[test]\nfn runs() { while false { execute_update(); } assert!(true); }\n",
        ):
            paths = extract_behavior_function(source, "runs")
            body = "\n".join(" ".join(path.tokens) for path in paths)
            self.assertNotIn("execute_update", body)

        for source in (
            "#[test]\nfn runs() { if false { assert!(true); } }\n",
            "#[test]\nfn runs() { #[cfg(any())] { assert!(true); } }\n",
        ):
            with self.assertRaisesRegex(VerificationError, "observable assertion"):
                extract_behavior_function(source, "runs")

    def test_accepts_enabled_default_feature_target_tokio_and_live_branch_controls(self) -> None:
        """Legitimate enabled cfg/cfg_attr tests retain their reachable behavior tokens."""

        profile = CfgProfile(
            "fixture-all-features",
            {
                'feature="enabled"': True,
                'target_os="fixture_os"': True,
                "test": True,
            },
        )
        paths = extract_behavior_function(
            "#[cfg(all(feature = \"enabled\", target_os = \"fixture_os\"))]\n"
            "#[cfg_attr(feature = \"enabled\", tokio::test)]\n"
            "async fn runs() {\n"
            "  if true { execute_update(); assert!(true); }\n"
            "  else { disabled_call(); }\n"
            "}\n",
            "runs",
            profile,
        )
        body = "\n".join(" ".join(path.tokens) for path in paths)
        self.assertIn("execute_update", body)
        self.assertIn("assert !", body)
        self.assertNotIn("disabled_call", body)

    def test_rejects_unproven_custom_cfg_atom(self) -> None:
        """Unknown build-system cfg names cannot be guessed enabled or disabled."""

        self.write_valid_adapter_source()
        tests = self.root / "crates/druid-wrapper/tests/demo.rs"
        tests.write_text(
            "#[cfg(custom_build_cfg)]\n#[test]\nfn runs() { assert!(true); }\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "cannot prove atom"):
            verify_adapter_source(self.root, self.row)

    def test_full_verifier_rejects_path_incoherent_branches_and_early_exit(self) -> None:
        """One candidate path must own the ordered chain and its observable assertion."""

        steps = self.remote_chain_steps()
        fixtures = {
            "cross-if-else": (
                "if runtime_condition() {\n"
                + "".join(steps[:5])
                + "} else {\n"
                + "".join(steps[5:])
                + "}\nassert!(true);\n"
            ),
            "nested-runtime": (
                "if outer_condition() {\n"
                " if inner_condition() {\n"
                + "".join(steps[:4])
                + " } else {\n"
                + "".join(steps[4:7])
                + " }\n} else {\n"
                + "".join(steps[7:])
                + "}\nassert!(true);\n"
            ),
            "early-return": (
                "".join(steps[:4])
                + "if runtime_condition() {\n"
                + "".join(steps[4:6])
                + " return;\n}\n"
                + "".join(steps[6:])
                + "assert!(true);\n"
            ),
            "assertion-other-branch": (
                "if runtime_condition() {\n"
                + self.remote_chain()
                + "} else { assert!(true); }\n"
            ),
            "split-match-arms": (
                "match runtime_value() {\n"
                " Some(_) => {\n"
                + "".join(steps[:5])
                + " },\n None => {\n"
                + "".join(steps[5:])
                + " assert!(true); },\n}\n"
            ),
        }
        for label, body in fixtures.items():
            with self.subTest(label=label):
                source = f"#[test]\nfn runs() {{\n{body}}}\n"
                with self.assertRaises(VerificationError):
                    self.verify_remote_behavior(source)

    def test_full_verifier_rejects_loop_only_and_deferred_execution_evidence(self) -> None:
        """Maybe-zero loops and uncalled deferred bodies cannot impersonate test execution."""

        fixtures = {
            "nested-if-false": (
                "if true { if false {\n"
                + self.remote_chain()
                + "} assert!(true); }\n"
            ),
            "nested-while-false": (
                "if true { while false {\n"
                + self.remote_chain()
                + "} assert!(true); }\n"
            ),
            "runtime-while": (
                "while runtime_condition() {\n"
                + self.remote_chain()
                + "assert!(true); }\n"
            ),
            "maybe-empty-for": (
                "for value in maybe_empty() { let _ = value;\n"
                + self.remote_chain()
                + "assert!(true); }\n"
            ),
            "uncalled-closure": (
                "let evidence = || {\n"
                + self.remote_chain()
                + "assert!(true); };\nlet _ = evidence;\n"
            ),
            "unawaited-async-block": (
                "let evidence = async move {\n"
                + self.remote_chain()
                + "assert!(true); };\nlet _ = evidence;\n"
            ),
            "uncalled-async-closure": (
                "let evidence = async || {\n"
                + self.remote_chain()
                + "assert!(true); };\nlet _ = evidence;\n"
            ),
            "uncalled-helper": (
                "fn helper() {\n"
                + self.remote_chain()
                + "assert!(true); }\nlet _ = helper;\n"
            ),
            "cfg-disabled": (
                "#[cfg(any())] {\n"
                + self.remote_chain()
                + "assert!(true); }\n"
            ),
            "comment-and-string": (
                "let text = \"ConnectionFactory::create create_statement execute_update "
                "prepare_statement set_parameter execute_prepared_query next meta_data "
                "database_meta_data Connection::close assert!\";\n"
                "// ConnectionFactory::create(); result.next(); assert!(true);\n"
                "let _ = text;\n"
            ),
        }
        for label, body in fixtures.items():
            with self.subTest(label=label):
                source = f"#[test]\nfn runs() {{\n{body}}}\n"
                with self.assertRaises(VerificationError):
                    self.verify_remote_behavior(source)

    def test_full_verifier_rejects_macro_definition_order_and_path_explosion_attacks(self) -> None:
        """Definitions, marker substrings, wrong order, and unbounded paths fail closed."""

        steps = self.remote_chain_steps()
        branching = "".join(
            f"if condition_{index}() {{ branch_{index}(); }}\n" for index in range(7)
        )
        fixtures = {
            "uninvoked-macro-definition": (
                "macro_rules! evidence { () => {{\n"
                + self.remote_chain()
                + "assert!(true); }}; }\nassert!(true);\n"
            ),
            "multi-macro-arms": (
                "macro_rules! evidence {\n"
                " () => {{\n"
                + "".join(steps[:5])
                + " }};\n ($value:expr) => {{ let _ = $value;\n"
                + "".join(steps[5:])
                + "assert!(true); }};\n}\nassert!(true);\n"
            ),
            "uninvoked-macro-2-definition": (
                "macro evidence($value:expr) { let _ = $value;\n"
                + self.remote_chain()
                + "assert!(true); }\nassert!(true);\n"
            ),
            "invoked-macro-name-only": (
                "macro_rules! evidence { () => {{\n"
                + self.remote_chain()
                + "assert!(true); }}; }\nevidence!();\nassert!(true);\n"
            ),
            "wrong-token-order": "".join(reversed(steps)) + "assert!(true);\n",
            "same-leaf-substring": (
                "".join(step for step in steps if "result.meta_data" not in step)
                + "connection.database_meta_data();\nassert!(true);\n"
            ),
            "path-explosion": branching + self.remote_chain() + "assert!(true);\n",
        }
        for label, body in fixtures.items():
            with self.subTest(label=label):
                source = f"#[test]\nfn runs() {{\n{body}}}\n"
                with self.assertRaises(VerificationError):
                    self.verify_remote_behavior(source)

    def test_full_verifier_accepts_only_path_coherent_remote_evidence(self) -> None:
        """Straight, selected-static, and independently complete runtime paths remain valid."""

        fixtures = {
            "straight-line": self.remote_chain() + "assert!(true);\n",
            "one-complete-runtime-branch": (
                "if runtime_condition() {\n"
                + self.remote_chain()
                + "assert!(true);\n} else { harmless(); }\n"
            ),
            "literal-true": (
                "if true {\n"
                + self.remote_chain()
                + "assert!(true);\n} else { unreachable_call(); }\n"
            ),
            "literal-false-live-else": (
                "if false { unreachable_call(); } else {\n"
                + self.remote_chain()
                + "assert!(true);\n}\n"
            ),
            "both-runtime-branches-complete": (
                "if runtime_condition() {\n"
                + self.remote_chain()
                + "assert!(true);\n} else {\n"
                + self.remote_chain()
                + "assert_eq!(1, 1);\n}\n"
            ),
            "one-complete-match-arm": (
                "match runtime_value() {\n"
                " Some(_) => {\n"
                + self.remote_chain()
                + "assert!(true); },\n None => { harmless(); },\n}\n"
            ),
            "compile-backed-direct-macro-arguments": (
                "macro_rules! execute_direct { ($($body:tt)*) => {{ $($body)* }}; }\n"
                "execute_direct!(\n"
                + self.remote_chain()
                + "assert!(true);\n);\n"
            ),
        }
        for label, body in fixtures.items():
            with self.subTest(label=label):
                source = f"#[test]\nfn runs() {{\n{body}}}\n"
                self.verify_remote_behavior(source)


if __name__ == "__main__":
    unittest.main()
