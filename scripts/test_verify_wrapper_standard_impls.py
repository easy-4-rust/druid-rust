#!/usr/bin/env python3
"""Fail-closed tests for the P04 Task 4 wrapper runtime-evidence verifier."""

from __future__ import annotations

import json
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from verify_wrapper_standard_impls import (
    AdapterRow,
    EvidenceSpec,
    TRACE_STAGES,
    TRACE_VERSION,
    VerificationError,
    cargo_evidence_command,
    execute_evidence,
    extract_impl_bodies,
    validate_runtime_trace_output,
    verify_adapter_source,
    verify_runtime_trace_source_hygiene,
    verify_sqlx_ownership,
    verify_trace_record_callsites,
)


class WrapperStandardVerifierTest(unittest.TestCase):
    """Reject split standard identity, forged traces, and runtime execution bypasses."""

    def setUp(self) -> None:
        self.temporary_directory = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary_directory.name)
        self.source_path = Path("crates/druid-wrapper/src/demo.rs")
        (self.root / self.source_path).parent.mkdir(parents=True)
        self.row = AdapterRow(
            source_path=self.source_path,
            current_trait="druid_rdbc::spi::PhysicalConnection",
            implementation_type="DemoConnection",
            target_trait="druid_rdbc::connection::Connection",
            standard_implementation_type="DemoConnection",
            standard_source_path=self.source_path,
            standard_core_evidence="DemoConnection::run_core",
            behavior_evidence="runs",
            state="STANDARD_IMPL_LIVE",
        )

    def tearDown(self) -> None:
        self.temporary_directory.cleanup()

    def write_source(self, source: str) -> None:
        (self.root / self.source_path).write_text(source, encoding="utf-8")

    def test_rejects_adapter_that_only_implements_physical(self) -> None:
        self.write_source("impl PhysicalConnection for DemoConnection {}\n")
        with self.assertRaisesRegex(VerificationError, "implements only Physical"):
            verify_adapter_source(self.root, self.row)

    def test_rejects_duplicated_sql_in_standard_impl(self) -> None:
        self.write_source(
            "use druid_rdbc::connection::Connection;\n"
            "impl PhysicalConnection for DemoConnection {}\n"
            "impl Connection for DemoConnection {\n"
            " async fn create_statement(&mut self) { sqlx::query(\"select 1\").execute(); }\n"
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
            len(
                extract_impl_bodies(
                    source,
                    "druid_rdbc::metadata::DatabaseMetaData",
                    "DemoMetaData<'_>",
                )
            ),
            1,
        )

    def test_rejects_missing_sqlx_actor(self) -> None:
        with self.assertRaisesRegex(VerificationError, "statement-owned executor"):
            verify_sqlx_ownership(self.root)

    def test_rejects_empty_default_and_split_core_standard_impls(self) -> None:
        for source in (
            "impl PhysicalConnection for DemoConnection {}\nimpl Connection for DemoConnection {}\n",
            "impl PhysicalConnection for DemoConnection { fn run(&mut self) { self.physical_core(); } }\n"
            "impl Connection for DemoConnection { fn run(&mut self) { self.standard_core(); }\n"
            "fn create_statement(&mut self) {} fn prepare_statement(&mut self) {} }\n",
        ):
            self.write_source(source)
            with self.assertRaises(VerificationError):
                verify_adapter_source(self.root, self.row)

    def test_rejects_standard_to_physical_local_leaf_cfg_disabled_and_alias_identity(self) -> None:
        fixtures = (
            "impl PhysicalConnection for DemoConnection {}\n"
            "impl Connection for DemoConnection { fn run(&mut self) { PhysicalConnection::run(self); } }\n",
            "trait Connection {}\nimpl PhysicalConnection for DemoConnection {}\n"
            "impl Connection for DemoConnection {}\n",
            "impl PhysicalConnection for DemoConnection {}\n"
            "#[cfg(any())] impl Connection for DemoConnection {}\n",
            "type Alias = DemoConnection;\nimpl PhysicalConnection for DemoConnection {}\n"
            "impl Connection for Alias {}\n",
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
            "sqlx_streaming_result_set.rs": (
                "// Arc<SqlxConnectionActor> ResultSetMetaDataSnapshot Arc<dyn ResultSetMetaData>\n"
            ),
            "sqlx_database_meta_data.rs": "// Arc<SqlxConnectionActor>\n",
        }.items():
            (actor.parent / name).write_text(source, encoding="utf-8")
        with self.assertRaises(VerificationError):
            verify_sqlx_ownership(self.root)

    @staticmethod
    def marker(
        *,
        family: str = "HTTP_SQL",
        instance_id: int = 41,
        event_stages: list[str] | None = None,
        evidence_id: str = "task4.http-sql.runtime-trace-v1",
        version: str = TRACE_VERSION,
    ) -> str:
        events = [
            {"family": family, "instance_id": instance_id, "stage": stage}
            for stage in (event_stages if event_stages is not None else TRACE_STAGES)
        ]
        payload = {
            "evidence_id": evidence_id,
            "version": version,
            "family": family,
            "instance_id": instance_id,
            "events": events,
        }
        return "DRUID_TASK4_RUNTIME_TRACE\t" + json.dumps(payload, separators=(",", ":"))

    def validate_marker(self, output: str) -> None:
        validate_runtime_trace_output(
            output,
            "task4.http-sql.runtime-trace-v1",
            "HTTP_SQL",
            TRACE_VERSION,
        )

    def test_runtime_trace_output_is_typed_exact_and_fail_closed(self) -> None:
        self.validate_marker(self.marker())
        malformed = {
            "missing": self.marker(event_stages=list(TRACE_STAGES[:-1])),
            "out-of-order": self.marker(
                event_stages=[TRACE_STAGES[1], TRACE_STAGES[0], *TRACE_STAGES[2:]]
            ),
            "duplicate": self.marker(
                event_stages=[*TRACE_STAGES[:4], TRACE_STAGES[3], *TRACE_STAGES[4:]]
            ),
            "wrong-family": self.marker(family="LIBSQL"),
            "wrong-instance": self.marker().replace(
                '"instance_id":41,"stage":"PreparedQuery"',
                '"instance_id":42,"stage":"PreparedQuery"',
            ),
            "wrong-version": self.marker(version="task4-runtime-trace-v0"),
            "wrong-evidence": self.marker(evidence_id="task4.other.runtime-trace-v1"),
            "no-marker": "test result: ok. 1 passed; 0 failed; 0 ignored;",
            "duplicate-lines": self.marker() + "\n" + self.marker(),
        }
        for label, output in malformed.items():
            with self.subTest(label=label):
                with self.assertRaises(VerificationError):
                    self.validate_marker(output)

    def test_runtime_trace_source_hygiene_rejects_fake_marker_and_recording(self) -> None:
        fixtures = {
            "fake-marker": 'fn runs() { eprintln!("DRUID_TASK4_RUNTIME_TRACE\\t{}"); }',
            "fake-record": "fn send(trace: Trace) { trace.task4_trace_result_next(); }",
            "fake-event": "struct TraceEvent { stage: TraceStage }",
        }
        for label, source in fixtures.items():
            with self.subTest(label=label):
                with self.assertRaises(VerificationError):
                    verify_runtime_trace_source_hygiene(Path(f"{label}.rs"), source)

    def test_exit_zero_without_marker_rejects_all_reviewer_execution_bypasses(self) -> None:
        """Dropped futures, process exit, bare matches, and pre-chain assert all emit no trace."""

        integration = self.root / "crates/druid-wrapper/tests/reviewer_bypass.rs"
        integration.parent.mkdir(parents=True)
        integration.write_text(
            "#[test]\nfn dropped_futures() {}\n"
            "#[test]\nfn process_exit() {}\n"
            "#[test]\nfn bare_matches() {}\n"
            "#[test]\nfn assert_before_chain() {}\n",
            encoding="utf-8",
        )
        for test_id in (
            "dropped_futures",
            "process_exit",
            "bare_matches",
            "assert_before_chain",
        ):
            spec = EvidenceSpec(
                "test:reviewer_bypass",
                test_id,
                "default",
                "RUNTIME_TRACE",
                "HTTP_SQL",
                TRACE_VERSION,
                "task4.http-sql.runtime-trace-v1",
            )
            completed = subprocess.CompletedProcess(
                args=cargo_evidence_command(spec),
                returncode=0,
                stdout="test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured;\n",
                stderr="",
            )
            with self.subTest(test_id=test_id), patch(
                "verify_wrapper_standard_impls.subprocess.run", return_value=completed
            ):
                with self.assertRaisesRegex(VerificationError, "exactly one"):
                    execute_evidence(self.root, spec)

    def test_exact_runner_rejects_nonzero_and_missing_libtest_completion(self) -> None:
        integration = self.root / "crates/druid-wrapper/tests/exact.rs"
        integration.parent.mkdir(parents=True)
        integration.write_text("#[test]\nfn runs() {}\n", encoding="utf-8")
        spec = EvidenceSpec(
            "test:exact",
            "runs",
            "default",
            "RUNTIME_TRACE",
            "HTTP_SQL",
            TRACE_VERSION,
            "task4.http-sql.runtime-trace-v1",
        )
        fixtures = (
            subprocess.CompletedProcess([], 1, "", "failed"),
            subprocess.CompletedProcess([], 0, self.marker(), ""),
        )
        for completed in fixtures:
            with patch("verify_wrapper_standard_impls.subprocess.run", return_value=completed):
                with self.assertRaises(VerificationError):
                    execute_evidence(self.root, spec)

    def test_record_callsites_outside_production_allowlist_are_rejected(self) -> None:
        source = self.root / "crates/druid-wrapper/src/unapproved.rs"
        source.parent.mkdir(parents=True, exist_ok=True)
        source.write_text(
            "fn fake(trace: Trace) { trace.task4_trace_parameter_set(); }\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "outside production allowlist"):
            verify_trace_record_callsites(self.root)

    def test_cargo_command_binds_exact_target_test_and_profile(self) -> None:
        spec = EvidenceSpec(
            "test:demo",
            "runs",
            "all-features",
            "RUNTIME_TRACE",
            "HTTP_SQL",
            TRACE_VERSION,
            "task4.http-sql.runtime-trace-v1",
        )
        self.assertEqual(
            cargo_evidence_command(spec),
            [
                "cargo",
                "test",
                "-p",
                "druid-wrapper",
                "--all-features",
                "--test",
                "demo",
                "runs",
                "--",
                "--exact",
                "--nocapture",
            ],
        )


if __name__ == "__main__":
    unittest.main()
