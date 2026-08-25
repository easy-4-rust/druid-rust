#!/usr/bin/env python3
"""Fail-closed tests for the P04 Task 4 wrapper runtime-evidence verifier."""

from __future__ import annotations

import json
import inspect
import os
import shutil
import tempfile
import time
import unittest
from dataclasses import replace
from pathlib import Path
from unittest.mock import patch

import verify_wrapper_standard_impls as verifier
from verify_wrapper_standard_impls import (
    AdapterRow,
    BoundedProcessResult,
    EvidenceSpec,
    TRACE_STAGES,
    TRACE_VERSION,
    VerificationError,
    cargo_evidence_command,
    evidence_specs,
    execute_evidence,
    extract_impl_bodies,
    validate_runtime_trace_output,
    task_adapter_rows,
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
        challenge: str = "nonce-a",
    ) -> str:
        events = [
            {
                "family": family,
                "instance_id": instance_id,
                "stage": stage,
                "caller_file": verifier.TRACE_CALLSITE_RULES.get(family, {}).get(
                    stage, "untrusted.rs"
                ),
                "caller_line": 1,
            }
            for stage in (event_stages if event_stages is not None else TRACE_STAGES)
        ]
        payload = {
            "evidence_id": evidence_id,
            "version": version,
            "challenge": challenge,
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
            "nonce-a",
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
            "wrong-challenge": self.marker(challenge="nonce-old"),
            "wrong-caller-file": self.marker().replace(
                "crates/druid-wrapper/src/http_sql/http_sql_connection_factory.rs",
                "crates/druid-wrapper/tests/helper.rs",
                1,
            ),
            "wrong-caller-line": self.marker().replace('"caller_line":1', '"caller_line":0', 1),
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
            completed = BoundedProcessResult(
                0,
                stdout="test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured;\n",
                stderr="",
            )
            with self.subTest(test_id=test_id), patch(
                "verify_wrapper_standard_impls.verify_evidence_source_closure"
            ), patch(
                "verify_wrapper_standard_impls.run_bounded_process", return_value=completed
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
            BoundedProcessResult(1, "", "failed"),
            BoundedProcessResult(0, self.marker(), ""),
        )
        for completed in fixtures:
            with patch(
                "verify_wrapper_standard_impls.verify_evidence_source_closure"
            ), patch(
                "verify_wrapper_standard_impls.run_bounded_process", return_value=completed
            ):
                with self.assertRaises(VerificationError):
                    execute_evidence(self.root, spec)

    def test_record_callsites_outside_production_allowlist_are_rejected(self) -> None:
        source = self.root / "crates/druid-wrapper/src/unapproved.rs"
        source.parent.mkdir(parents=True, exist_ok=True)
        source.write_text(
            "fn fake(trace: Trace) { trace.task4_trace_parameter_set(); }\n",
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "callsite multiset drift"):
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
                "--offline",
                "--locked",
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

    def test_complete_source_closure_rejects_helper_marker_fake_summary_and_exit(self) -> None:
        """A sibling helper cannot forge marker/completion output outside the root test file."""

        checker = getattr(verifier, "verify_evidence_source_closure", None)
        self.assertIsNotNone(checker, "complete Rust evidence source-closure gate is missing")
        if checker is None:
            return
        tests = self.root / "crates/druid-wrapper/tests"
        tests.mkdir(parents=True, exist_ok=True)
        (tests / "demo.rs").write_text(
            "mod helper;\n#[test]\nfn runs() { helper::emit(); }\n",
            encoding="utf-8",
        )
        (tests / "helper.rs").write_text(
            "use std::io::Write;\n"
            "pub fn emit() {\n"
            " let prefix = [\"DRUID_TASK4\", \"_RUNTIME_TRACE\"].concat();\n"
            " let mut stderr = std::io::stderr();\n"
            " writeln!(stderr, \"{}\\t{}\", prefix, \"{}\").unwrap();\n"
            " println!(\"test result: ok. 1 passed; 0 failed; 0 ignored;\");\n"
            " std::process::exit(0);\n"
            "}\n",
            encoding="utf-8",
        )
        spec = EvidenceSpec(
            "test:demo",
            "runs",
            "default",
            "RUNTIME_TRACE",
            "HTTP_SQL",
            TRACE_VERSION,
            "task4.http-sql.runtime-trace-v1",
        )
        with self.assertRaises(VerificationError):
            checker(self.root, spec)

    def test_record_provenance_rejects_allowed_file_helper_and_unapproved_ufcs(self) -> None:
        """Path allowlisting cannot authorize helper, macro, or UFCS stage recording."""

        repository_source = (
            Path(__file__).resolve().parents[1] / "crates/druid-wrapper/src"
        )
        temporary_source = self.root / "crates/druid-wrapper/src"
        shutil.copytree(repository_source, temporary_source, dirs_exist_ok=True)
        allowed = (
            self.root
            / "crates/druid-wrapper/src/http_sql/http_sql_connection_factory.rs"
        )
        original = allowed.read_text(encoding="utf-8")
        allowed.write_text(
            original
            + "\nfn helper(trace: &RuntimeTraceEmitter) {\n"
            " trace.task4_trace_parameter_set();\n"
            "}\n",
            encoding="utf-8",
        )
        with self.assertRaises(VerificationError):
            verify_trace_record_callsites(self.root)

        allowed.write_text(original, encoding="utf-8")
        unapproved = self.root / "crates/druid-wrapper/src/unapproved.rs"
        unapproved.write_text(
            "fn helper(trace: &RuntimeTraceEmitter) {\n"
            " RuntimeTraceEmitter::task4_trace_parameter_set(trace);\n"
            "}\n",
            encoding="utf-8",
        )
        with self.assertRaises(VerificationError):
            verify_trace_record_callsites(self.root)

    def test_runtime_manifest_rejects_http_libsql_family_binding_swap_before_dedup(self) -> None:
        """Every source row, not just the seven-spec set, owns one canonical family binding."""

        repository = Path(__file__).resolve().parents[1]
        rows = task_adapter_rows(
            repository
            / "docs/verification/rdbc-first/rdbc-adapter-implementations.tsv",
            repository / "docs/verification/rdbc-first/rdbc-adapter-runtime-traces.tsv",
        )
        http = next(row for row in rows if "http_sql/" in row.source_path.as_posix())
        libsql = next(row for row in rows if "libsql/" in row.source_path.as_posix())

        def swapped(row: AdapterRow) -> AdapterRow:
            donor = (
                libsql
                if "http_sql/" in row.source_path.as_posix()
                else http
                if "libsql/" in row.source_path.as_posix()
                else None
            )
            if donor is None:
                return row
            return replace(
                row,
                behavior_test_target=donor.behavior_test_target,
                cargo_test_id=donor.cargo_test_id,
                behavior_profile=donor.behavior_profile,
                trace_family=donor.trace_family,
                trace_version=donor.trace_version,
                trace_evidence_id=donor.trace_evidence_id,
            )

        with self.assertRaises(VerificationError):
            evidence_specs([swapped(row) for row in rows])

    def test_runner_is_offline_locked_bounded_and_kills_hanging_process_group(self) -> None:
        """Evidence execution cannot resolve online, run forever, or emit unbounded output."""

        spec = EvidenceSpec(
            "test:demo",
            "runs",
            "default",
            "RUNTIME_TRACE",
            "HTTP_SQL",
            TRACE_VERSION,
            "task4.http-sql.runtime-trace-v1",
        )
        command = cargo_evidence_command(spec)
        self.assertIn("--offline", command)
        self.assertIn("--locked", command)
        runner = getattr(verifier, "run_bounded_process", None)
        self.assertIsNotNone(runner, "bounded process-group runner is missing")
        if runner is None:
            return
        child_pid_file = self.root / "child.pid"
        hanging_script = (
            "import subprocess,time; "
            "child=subprocess.Popen(['python3','-c','import time; time.sleep(10)']); "
            f"open({str(child_pid_file)!r},'w').write(str(child.pid)); "
            "time.sleep(10)"
        )
        started = time.monotonic()
        with self.assertRaisesRegex(VerificationError, "timed out"):
            runner(
                ["python3", "-c", hanging_script],
                cwd=self.root,
                timeout_seconds=0.1,
                output_limit=4096,
            )
        self.assertLess(time.monotonic() - started, 2.0)
        child_pid = int(child_pid_file.read_text(encoding="utf-8"))
        child_alive = True
        for _ in range(20):
            try:
                os.kill(child_pid, 0)
            except ProcessLookupError:
                child_alive = False
                break
            time.sleep(0.05)
        self.assertFalse(child_alive, f"evidence child process survived group kill: {child_pid}")
        with self.assertRaisesRegex(VerificationError, "output limit"):
            runner(
                ["python3", "-c", "print('x' * 10000)"],
                cwd=self.root,
                timeout_seconds=2.0,
                output_limit=1024,
            )

    def test_runtime_marker_requires_per_run_challenge_and_caller_provenance(self) -> None:
        """An old structurally valid marker cannot satisfy a fresh verifier run."""

        parameters = inspect.signature(validate_runtime_trace_output).parameters
        self.assertIn("expected_challenge", parameters)
        self.assertIsNotNone(
            getattr(verifier, "TRACE_CALLSITE_RULES", None),
            "typed caller provenance rules are missing",
        )

    def test_caller_line_must_remain_inside_the_exact_production_function_region(self) -> None:
        """A same-file helper call is not a valid factory execution callsite."""

        repository = Path(__file__).resolve().parents[1]
        family = "HTTP_SQL"
        payload = json.loads(self.marker().split("\t", 1)[1])
        copied: set[str] = set()
        for event in payload["events"]:
            relative = verifier.TRACE_CALLSITE_RULES[family][event["stage"]]
            source = repository / relative
            destination = self.root / relative
            if relative not in copied:
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_text(source.read_text(encoding="utf-8"), encoding="utf-8")
                copied.add(relative)
            method = verifier.TRACE_STAGE_METHODS[event["stage"]]
            call_lines = [
                index
                for index, line in enumerate(
                    destination.read_text(encoding="utf-8").splitlines(), start=1
                )
                if f".{method}(" in line
            ]
            self.assertTrue(call_lines, f"missing canonical callsite {relative}:{method}")
            event["caller_line"] = call_lines[0]
        marker = "DRUID_TASK4_RUNTIME_TRACE\t" + json.dumps(
            payload, separators=(",", ":")
        )
        validate_runtime_trace_output(
            marker,
            "task4.http-sql.runtime-trace-v1",
            family,
            TRACE_VERSION,
            "nonce-a",
            self.root,
        )

        factory_relative = verifier.TRACE_CALLSITE_RULES[family]["FactoryCreate"]
        factory = self.root / factory_relative
        helper_line = len(factory.read_text(encoding="utf-8").splitlines()) + 2
        with factory.open("a", encoding="utf-8") as stream:
            stream.write(
                "\nfn forged_helper(trace: &RuntimeTraceEmitter) {\n"
                "    trace.task4_trace_factory_create();\n"
                "}\n"
            )
        payload["events"][0]["caller_line"] = helper_line + 1
        forged = "DRUID_TASK4_RUNTIME_TRACE\t" + json.dumps(
            payload, separators=(",", ":")
        )
        with self.assertRaisesRegex(VerificationError, "function region"):
            validate_runtime_trace_output(
                forged,
                "task4.http-sql.runtime-trace-v1",
                family,
                TRACE_VERSION,
                "nonce-a",
                self.root,
            )


if __name__ == "__main__":
    unittest.main()
