#!/usr/bin/env python3
"""Fail-closed tests for the P04 Task 4 wrapper runtime-evidence verifier."""

from __future__ import annotations

import json
import hashlib
import inspect
import os
import signal
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
            current_trait="druid_rdbc::connection::Connection",
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
        with self.assertRaisesRegex(
            VerificationError, "canonical adapter implementation"
        ):
            verify_adapter_source(self.root, self.row)

    def test_rejects_duplicated_sql_in_standard_impl(self) -> None:
        self.write_source(
            "use druid_rdbc::connection::Connection;\n"
            "impl PhysicalConnection for DemoConnection {}\n"
            "impl Connection for DemoConnection {\n"
            ' async fn create_statement(&mut self) { sqlx::query("select 1").execute(); }\n'
            "}\n"
        )
        with self.assertRaisesRegex(VerificationError, "duplicates SQL"):
            verify_adapter_source(self.root, self.row)

    def test_accepts_canonical_impl_that_delegates_one_core(self) -> None:
        self.write_source(
            "use druid_rdbc::connection::Connection;\n"
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

    def test_sqlx_coverage_binding_and_call_chain_reject_retired_update_aliases(self) -> None:
        """Task 6 coverage binds the live update-result actor, never the retired alias。"""

        repository = Path(__file__).resolve().parents[1]
        sqlx_root = self.root / "crates/druid-wrapper/src/sqlx"
        sqlx_root.mkdir(parents=True)
        names = (
            "sqlx_connection_actor.rs",
            "sqlx_prepared_statement.rs",
            "sqlx_streaming_result_set.rs",
            "sqlx_database_meta_data.rs",
        )
        for name in names:
            (sqlx_root / name).write_bytes(
                (repository / "crates/druid-wrapper/src/sqlx" / name).read_bytes()
            )
        verify_sqlx_ownership(self.root)

        kind, path, function, anchor = verifier.canonical_coverage_requirement(
            "SQLX", "FAMILY_EXECUTOR_FUNCTION"
        )
        requirement = verifier.CoverageRequirement(
            requirement_id="FAMILY_EXECUTOR_FUNCTION",
            requirement_kind=kind,
            source_path=path,
            source_sha256="0" * 64,
            source_function=function,
            compiler_function_name="_Rcanonical_sqlx_update_result",
            anchor_id=anchor,
            minimum_count=1,
        )
        verifier.validate_coverage_requirement_binding("SQLX", requirement)
        for retired in (
            replace(requirement, source_function="execute_update"),
            replace(
                requirement,
                source_path=Path(
                    "crates/druid-wrapper/src/sqlx/sqlx_connection_adapter.rs"
                ),
                source_function="execute_update_core",
            ),
        ):
            with self.assertRaisesRegex(VerificationError, "not canonical"):
                verifier.validate_coverage_requirement_binding("SQLX", retired)

        actor = sqlx_root / "sqlx_connection_actor.rs"
        actor_source = actor.read_text(encoding="utf-8")
        actor.write_text(
            actor_source.replace("execute_update_result", "execute_update", 1),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "update actor/core"):
            verify_sqlx_ownership(self.root)

    def test_toasty_coverage_binding_and_call_chain_reject_retired_update_core(self) -> None:
        """Generated-key preserving result core is the only Toasty executor obligation。"""

        repository = Path(__file__).resolve().parents[1]
        paths = (
            "crates/druid-wrapper/src/standard_adapter_statement.rs",
            "crates/druid-wrapper/src/standard_adapter_executor.rs",
            "crates/druid-wrapper/src/toasty/toasty_connection_adapter.rs",
        )
        for relative in paths:
            target = self.root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes((repository / relative).read_bytes())
        verifier.verify_family_executor_call_chains(self.root)

        kind, path, function, anchor = verifier.canonical_coverage_requirement(
            "TOASTY", "FAMILY_EXECUTOR_FUNCTION"
        )
        requirement = verifier.CoverageRequirement(
            requirement_id="FAMILY_EXECUTOR_FUNCTION",
            requirement_kind=kind,
            source_path=path,
            source_sha256="0" * 64,
            source_function=function,
            compiler_function_name="_Rcanonical_toasty_update_result",
            anchor_id=anchor,
            minimum_count=1,
        )
        verifier.validate_coverage_requirement_binding("TOASTY", requirement)
        with self.assertRaisesRegex(VerificationError, "not canonical"):
            verifier.validate_coverage_requirement_binding(
                "TOASTY",
                replace(requirement, source_function="standard_execute_update_core"),
            )

        toasty = self.root / paths[-1]
        source = toasty.read_text(encoding="utf-8")
        toasty.write_text(
            source.replace(
                "async fn standard_execute_update_result_core",
                "async fn retired_standard_execute_update_result_core",
                1,
            ),
            encoding="utf-8",
        )
        with self.assertRaisesRegex(VerificationError, "Toasty canonical"):
            verifier.verify_family_executor_call_chains(self.root)

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

    def test_rejects_standard_to_physical_local_leaf_cfg_disabled_and_alias_identity(
        self,
    ) -> None:
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
        return "DRUID_TASK4_RUNTIME_TRACE\t" + json.dumps(
            payload, separators=(",", ":")
        )

    def validate_marker(self, output: str) -> None:
        validate_runtime_trace_output(
            output,
            "task4.http-sql.runtime-trace-v1",
            "HTTP_SQL",
            TRACE_VERSION,
            "nonce-a",
        )

    def synthetic_coverage_fixture(
        self,
        active: dict[str, set[str]],
    ) -> tuple[EvidenceSpec, tuple[object, ...], dict[str, object]]:
        """Build exact compiler-coverage JSON without reusing verifier production logic."""

        requirement_type = getattr(verifier, "CoverageRequirement", None)
        family_type = getattr(verifier, "CoverageFamilySpec", None)
        requirement_ids = getattr(verifier, "COVERAGE_REQUIREMENT_IDS", None)
        coverage_version = getattr(verifier, "COVERAGE_VERSION", None)
        self.assertIsNotNone(
            requirement_type, "compiler coverage requirement type is missing"
        )
        self.assertIsNotNone(family_type, "compiler coverage family type is missing")
        self.assertIsNotNone(
            requirement_ids, "compiler coverage denominator is missing"
        )
        self.assertIsNotNone(coverage_version, "compiler coverage version is missing")
        if (
            requirement_type is None
            or family_type is None
            or requirement_ids is None
            or coverage_version is None
        ):
            self.fail("compiler coverage API is incomplete")

        wrapper = self.root / "crates/druid-wrapper"
        wrapper.mkdir(parents=True, exist_ok=True)
        (self.root / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
        family_specs: list[object] = []
        functions: list[dict[str, object]] = []
        files: list[dict[str, object]] = []
        bindings = {
            "HTTP_SQL": (
                "lib",
                "http_sql_exact_test",
                "all-features",
                "task4.http-sql.runtime-trace-v1",
            ),
            "LIBSQL": (
                "lib",
                "libsql_exact_test",
                "all-features",
                "task4.libsql.runtime-trace-v1",
            ),
        }
        for family, (target, test_id, profile, evidence_id) in bindings.items():
            relative = Path(
                f"crates/druid-wrapper/src/{family.lower()}_synthetic_coverage.rs"
            )
            source = self.root / relative
            source.parent.mkdir(parents=True, exist_ok=True)
            lines: list[str] = []
            identities: list[tuple[str, str, str, int, int]] = []
            for requirement_id in requirement_ids:
                function_name = f"{family.lower()}_{requirement_id.lower()}"
                region_line = f"{function_name}_region();"
                lines.extend((f"fn {function_name}() {{", f"    {region_line}", "}"))
                line_number = len(lines) - 1
                column = 5
                identities.append(
                    (requirement_id, function_name, region_line, line_number, column)
                )
            source_text = "\n".join(lines) + "\n"
            source.write_text(source_text, encoding="utf-8")
            digest = hashlib.sha256(source_text.encode("utf-8")).hexdigest()
            requirements: list[object] = []
            segments: list[list[object]] = []
            for (
                requirement_id,
                function_name,
                region_line,
                line_number,
                column,
            ) in identities:
                count = int(requirement_id in active.get(family, set()))
                requirements.append(
                    requirement_type(
                        requirement_id=requirement_id,
                        requirement_kind=(
                            "TEST_IDENTITY_FUNCTION"
                            if requirement_id == "EXACT_TEST_IDENTITY"
                            else (
                                "FAMILY_FACTORY_FUNCTION"
                                if requirement_id == "FAMILY_FACTORY_FUNCTION"
                                else (
                                    "FAMILY_EXECUTOR_FUNCTION"
                                    if requirement_id == "FAMILY_EXECUTOR_FUNCTION"
                                    else (
                                        "TRACE_VALIDATION_SUCCESS_FUNCTION"
                                        if requirement_id == "TRACE_VALIDATION_SUCCESS"
                                        else "STAGE_ANCHOR_FUNCTION"
                                    )
                                )
                            )
                        ),
                        source_path=relative,
                        source_sha256=digest,
                        source_function=function_name,
                        compiler_function_name=(
                            f"_synthetic_{family}_{requirement_id}"
                        ),
                        anchor_id=f"synthetic.{family}.{requirement_id}",
                        minimum_count=1,
                    )
                )
                functions.append(
                    {
                        "name": f"_synthetic_{family}_{requirement_id}",
                        "count": count,
                        "filenames": [str(source)],
                        "branches": [],
                        "mcdc_records": [],
                        "regions": [
                            [
                                line_number,
                                column,
                                line_number,
                                column + len(region_line),
                                count,
                                0,
                                0,
                                0,
                            ]
                        ],
                    }
                )
                segments.append([line_number, column, count, True, True, False])
            files.append({"filename": str(source), "segments": segments, "summary": {}})
            family_specs.append(
                family_type(
                    family=family,
                    test_target=target,
                    test_id=test_id,
                    profile=profile,
                    trace_version=TRACE_VERSION,
                    trace_evidence_id=evidence_id,
                    coverage_version=coverage_version,
                    requirements=tuple(requirements),
                )
            )
        expected = EvidenceSpec(
            "lib",
            "http_sql_exact_test",
            "all-features",
            "RUNTIME_TRACE",
            "HTTP_SQL",
            TRACE_VERSION,
            "task4.http-sql.runtime-trace-v1",
        )
        report: dict[str, object] = {
            "cargo_llvm_cov": {
                "version": "0.8.7",
                "manifest_path": str(self.root / "Cargo.toml"),
            },
            "data": [{"files": files, "functions": functions, "totals": {}}],
            "type": "llvm.coverage.json.export",
            "version": "3.1.0",
        }
        return expected, tuple(family_specs), report

    def assert_coverage_rejected(
        self,
        active: dict[str, set[str]],
        pattern: str,
    ) -> None:
        """Require the future compiler-coverage validator to fail closed."""

        validator = getattr(verifier, "validate_compiler_coverage", None)
        self.assertIsNotNone(validator, "compiler coverage validator is missing")
        if validator is None:
            return
        expected, family_specs, report = self.synthetic_coverage_fixture(active)
        with self.assertRaisesRegex(VerificationError, pattern):
            validator(self.root, expected, family_specs, report)

    def assert_same_function_region_fallback_rejected(
        self, requirement_id: str
    ) -> None:
        """A positive sibling region must not replace one zero exact target region."""

        requirement_ids = set(verifier.COVERAGE_REQUIREMENT_IDS)
        expected, family_specs, report = self.synthetic_coverage_fixture(
            {"HTTP_SQL": requirement_ids}
        )
        functions = report["data"][0]["functions"]
        target_name = f"_synthetic_HTTP_SQL_{requirement_id}"
        target = next(
            function for function in functions if function["name"] == target_name
        )
        exact_region = target["regions"][0]
        target["count"] = 0
        exact_region[4] = 0
        functions.append(
            {
                "name": f"_synthetic_unrelated_{requirement_id}",
                "count": 1,
                "filenames": list(target["filenames"]),
                "branches": [],
                "mcdc_records": [],
                "regions": [
                    [
                        exact_region[0] - 1,
                        1,
                        exact_region[0] - 1,
                        3,
                        1,
                        0,
                        0,
                        0,
                    ]
                ],
            }
        )
        pattern = (
            "another exact test|EXACT_TEST_IDENTITY.*zero|zero.*EXACT_TEST_IDENTITY"
            if requirement_id == "EXACT_TEST_IDENTITY"
            else f"{requirement_id}.*zero|zero.*{requirement_id}"
        )
        with self.assertRaisesRegex(VerificationError, pattern):
            verifier.validate_compiler_coverage(
                self.root, expected, family_specs, report
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
            "wrong-caller-line": self.marker().replace(
                '"caller_line":1', '"caller_line":0', 1
            ),
            "no-marker": "test result: ok. 1 passed; 0 failed; 0 ignored;",
            "duplicate-lines": self.marker() + "\n" + self.marker(),
        }
        for label, output in malformed.items():
            with self.subTest(label=label):
                with self.assertRaises(VerificationError):
                    self.validate_marker(output)

    def test_exact_test_identity_rejects_positive_sibling_function_region(self) -> None:
        """The exact test identity region cannot borrow another region's count."""

        self.assert_same_function_region_fallback_rejected("EXACT_TEST_IDENTITY")

    def test_stage_rejects_positive_sibling_caller_function_region(self) -> None:
        """A stage anchor at zero cannot borrow its caller function's positive count."""

        self.assert_same_function_region_fallback_rejected("STAGE_STATEMENT_UPDATE")

    def test_validation_success_rejects_positive_sibling_verify_region(self) -> None:
        """Entering verify_and_emit cannot replace its zero success exit."""

        self.assert_same_function_region_fallback_rejected("TRACE_VALIDATION_SUCCESS")

    def test_duplicate_exact_coverage_function_is_ambiguous(self) -> None:
        """Two compiler functions for one exact identity must fail closed."""

        requirement_ids = set(verifier.COVERAGE_REQUIREMENT_IDS)
        expected, family_specs, report = self.synthetic_coverage_fixture(
            {"HTTP_SQL": requirement_ids}
        )
        functions = report["data"][0]["functions"]
        target = next(
            function
            for function in functions
            if function["name"] == "_synthetic_HTTP_SQL_STAGE_STATEMENT_UPDATE"
        )
        functions.append(json.loads(json.dumps(target)))
        with self.assertRaisesRegex(VerificationError, "duplicate|ambiguous"):
            verifier.validate_compiler_coverage(
                self.root, expected, family_specs, report
            )

    def test_missing_exact_stage_anchor_function_is_rejected(self) -> None:
        """An absent compiler function cannot satisfy a present source anchor."""

        requirement_ids = set(verifier.COVERAGE_REQUIREMENT_IDS)
        expected, family_specs, report = self.synthetic_coverage_fixture(
            {"HTTP_SQL": requirement_ids}
        )
        functions = report["data"][0]["functions"]
        functions[:] = [
            function
            for function in functions
            if function["name"] != "_synthetic_HTTP_SQL_STAGE_STATEMENT_UPDATE"
        ]
        with self.assertRaisesRegex(
            VerificationError,
            "STAGE_STATEMENT_UPDATE.*zero|zero.*STAGE_STATEMENT_UPDATE",
        ):
            verifier.validate_compiler_coverage(
                self.root, expected, family_specs, report
            )

    def test_wrong_source_hash_or_compiler_function_identity_is_rejected(self) -> None:
        """Neither stale source bytes nor a different exact function can be credited."""

        repository = Path(__file__).resolve().parents[1]
        canonical = verifier.load_coverage_manifest(repository)
        first = canonical[0]
        stale = replace(
            first.requirements[0],
            source_sha256="0" * 64,
        )
        with self.assertRaisesRegex(VerificationError, "source hash is stale"):
            verifier.validate_coverage_manifest(
                repository,
                (
                    replace(
                        first,
                        requirements=(stale, *first.requirements[1:]),
                    ),
                    *canonical[1:],
                ),
            )

        requirement_ids = set(verifier.COVERAGE_REQUIREMENT_IDS)
        expected, family_specs, report = self.synthetic_coverage_fixture(
            {"HTTP_SQL": requirement_ids}
        )
        http = family_specs[0]
        stage_index = next(
            index
            for index, requirement in enumerate(http.requirements)
            if requirement.requirement_id == "STAGE_STATEMENT_UPDATE"
        )
        requirements = list(http.requirements)
        requirements[stage_index] = replace(
            requirements[stage_index],
            compiler_function_name="_synthetic_wrong_exact_function",
        )
        with self.assertRaisesRegex(
            VerificationError,
            "STAGE_STATEMENT_UPDATE.*zero|zero.*STAGE_STATEMENT_UPDATE",
        ):
            verifier.validate_compiler_coverage(
                self.root,
                expected,
                (replace(http, requirements=tuple(requirements)), *family_specs[1:]),
                report,
            )

    def test_matching_runtime_record_without_production_coverage_is_rejected(
        self,
    ) -> None:
        """A matching diagnostic marker never substitutes for compiler coverage."""

        self.assertIn("DRUID_TASK4_RUNTIME_TRACE", self.marker())
        self.assert_coverage_rejected(
            {"HTTP_SQL": {"EXACT_TEST_IDENTITY"}},
            "coverage requirement",
        )

    def test_dropped_future_has_test_identity_but_no_production_coverage(self) -> None:
        """Creating and dropping Futures cannot satisfy a production execution gate."""

        self.assert_coverage_rejected(
            {"HTTP_SQL": {"EXACT_TEST_IDENTITY"}},
            "FAMILY_FACTORY_FUNCTION",
        )

    def test_early_process_exit_cannot_skip_trace_validation_success_coverage(
        self,
    ) -> None:
        """An exit-zero path after operations still lacks the typed validation-success region."""

        requirement_ids = getattr(verifier, "COVERAGE_REQUIREMENT_IDS", ())
        active = set(requirement_ids).difference({"TRACE_VALIDATION_SUCCESS"})
        self.assert_coverage_rejected(
            {"HTTP_SQL": active},
            "TRACE_VALIDATION_SUCCESS",
        )

    def test_coverage_from_another_exact_test_or_family_is_rejected(self) -> None:
        """A LIBSQL exact run cannot be credited to the HTTP SQL manifest row."""

        requirement_ids = set(getattr(verifier, "COVERAGE_REQUIREMENT_IDS", ()))
        self.assert_coverage_rejected(
            {"LIBSQL": requirement_ids},
            "another exact test|coverage requirement",
        )

    def test_combined_or_stale_coverage_profile_is_rejected(self) -> None:
        """One report may contain one exact evidence identity and must be fresh to its run."""

        validator = getattr(verifier, "validate_compiler_coverage", None)
        loader = getattr(verifier, "load_fresh_coverage_report", None)
        self.assertIsNotNone(validator, "compiler coverage validator is missing")
        self.assertIsNotNone(loader, "fresh compiler coverage loader is missing")
        if validator is None or loader is None:
            return
        requirement_ids = set(verifier.COVERAGE_REQUIREMENT_IDS)
        expected, family_specs, report = self.synthetic_coverage_fixture(
            {"HTTP_SQL": requirement_ids, "LIBSQL": requirement_ids}
        )
        with self.assertRaisesRegex(VerificationError, "combined"):
            validator(self.root, expected, family_specs, report)

        run_directory = self.root / "coverage-run"
        run_directory.mkdir()
        report_path = run_directory / "coverage.json"
        report_path.write_text(json.dumps(report), encoding="utf-8")
        old_time = time.time() - 60
        os.utime(report_path, (old_time, old_time))
        with self.assertRaisesRegex(VerificationError, "stale"):
            loader(
                report_path,
                run_directory=run_directory,
                started_at_ns=time.time_ns(),
            )

    def test_missing_common_stage_callsite_coverage_is_rejected(self) -> None:
        """A zero-count Statement update stage makes the exact family evidence incomplete."""

        requirement_ids = set(getattr(verifier, "COVERAGE_REQUIREMENT_IDS", ()))
        requirement_ids.discard("STAGE_STATEMENT_UPDATE")
        self.assert_coverage_rejected(
            {"HTTP_SQL": requirement_ids},
            "STAGE_STATEMENT_UPDATE",
        )

    def test_missing_trace_validation_success_region_is_rejected(self) -> None:
        """All operation stages without post-validation success remain non-evidence."""

        requirement_ids = set(getattr(verifier, "COVERAGE_REQUIREMENT_IDS", ()))
        requirement_ids.discard("TRACE_VALIDATION_SUCCESS")
        self.assert_coverage_rejected(
            {"HTTP_SQL": requirement_ids},
            "TRACE_VALIDATION_SUCCESS",
        )

    def test_wrong_family_factory_manifest_mapping_is_rejected(self) -> None:
        """A family cannot point its factory requirement at another adapter family."""

        repository = Path(__file__).resolve().parents[1]
        loader = getattr(verifier, "load_coverage_manifest", None)
        manifest_validator = getattr(verifier, "validate_coverage_manifest", None)
        self.assertIsNotNone(loader, "coverage manifest loader is missing")
        self.assertIsNotNone(
            manifest_validator, "coverage manifest validator is missing"
        )
        if loader is None or manifest_validator is None:
            return
        family_specs = loader(repository)
        http_index = next(
            index
            for index, spec in enumerate(family_specs)
            if spec.family == "HTTP_SQL"
        )
        http = family_specs[http_index]
        requirements = list(http.requirements)
        factory_index = next(
            index
            for index, requirement in enumerate(requirements)
            if requirement.requirement_id == "FAMILY_FACTORY_FUNCTION"
        )
        libsql_factory = repository / verifier.FAMILY_FACTORY_FILES["LIBSQL"]
        requirements[factory_index] = replace(
            requirements[factory_index],
            source_path=Path(verifier.FAMILY_FACTORY_FILES["LIBSQL"]),
            source_sha256=hashlib.sha256(libsql_factory.read_bytes()).hexdigest(),
        )
        mutated = list(family_specs)
        mutated[http_index] = replace(http, requirements=tuple(requirements))
        with self.assertRaisesRegex(VerificationError, "factory|source"):
            manifest_validator(repository, tuple(mutated))

    def test_duplicate_or_reordered_coverage_manifest_mapping_is_rejected(self) -> None:
        """Manifest family and requirement arrays are canonical ordered denominators."""

        repository = Path(__file__).resolve().parents[1]
        loader = getattr(verifier, "load_coverage_manifest", None)
        manifest_validator = getattr(verifier, "validate_coverage_manifest", None)
        self.assertIsNotNone(loader, "coverage manifest loader is missing")
        self.assertIsNotNone(
            manifest_validator, "coverage manifest validator is missing"
        )
        if loader is None or manifest_validator is None:
            return
        family_specs = loader(repository)
        with self.assertRaisesRegex(VerificationError, "duplicate|order"):
            manifest_validator(repository, (*family_specs, family_specs[0]))
        first = family_specs[0]
        reordered = replace(
            first,
            requirements=tuple(reversed(first.requirements)),
        )
        with self.assertRaisesRegex(VerificationError, "order"):
            manifest_validator(repository, (reordered, *family_specs[1:]))

    def test_wrong_coverage_file_or_function_identity_is_rejected(self) -> None:
        """Coverage identity uses exact source components and exact function region."""

        validator = getattr(verifier, "validate_compiler_coverage", None)
        self.assertIsNotNone(validator, "compiler coverage validator is missing")
        if validator is None:
            return
        requirement_ids = set(getattr(verifier, "COVERAGE_REQUIREMENT_IDS", ()))
        expected, family_specs, report = self.synthetic_coverage_fixture(
            {"HTTP_SQL": requirement_ids}
        )
        data = report["data"]
        self.assertIsInstance(data, list)
        if not isinstance(data, list):
            return
        functions = data[0]["functions"]
        self.assertIsInstance(functions, list)
        if not isinstance(functions, list):
            return
        functions[0]["filenames"] = [str(self.root / "wrong-prefix/not-the-source.rs")]
        with self.assertRaisesRegex(
            VerificationError, "source|function|coverage requirement|another exact test"
        ):
            validator(self.root, expected, family_specs, report)

    def test_runtime_trace_source_hygiene_rejects_fake_marker_and_recording(
        self,
    ) -> None:
        fixtures = {
            "fake-marker": 'fn runs() { eprintln!("DRUID_TASK4_RUNTIME_TRACE\\t{}"); }',
            "fake-record": "fn send(trace: Trace) { trace.task4_trace_result_next(); }",
            "fake-event": "struct TraceEvent { stage: TraceStage }",
        }
        for label, source in fixtures.items():
            with self.subTest(label=label):
                with self.assertRaises(VerificationError):
                    verify_runtime_trace_source_hygiene(Path(f"{label}.rs"), source)

    def test_exit_zero_without_marker_rejects_all_reviewer_execution_bypasses(
        self,
    ) -> None:
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
                "verify_wrapper_standard_impls.run_bounded_process",
                return_value=completed,
            ):
                with self.assertRaisesRegex(
                    VerificationError, "coverage manifest|NOT_RUN/ERROR"
                ):
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
                "verify_wrapper_standard_impls.run_bounded_process",
                return_value=completed,
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

    def test_duplicate_trace_calls_are_pinned_to_exact_mutually_exclusive_branches(self) -> None:
        """2/2/2 callsites cannot move, duplicate, disappear, or both execute。"""

        repository = Path(__file__).resolve().parents[1]
        paths = (
            "crates/druid-wrapper/src/standard_adapter_statement.rs",
            "crates/druid-wrapper/src/standard_adapter_prepared_statement.rs",
            "crates/druid-wrapper/src/toasty/toasty_connection_adapter.rs",
            "crates/druid-wrapper/src/sqlx/sqlx_prepared_statement.rs",
        )

        def seed() -> None:
            for relative in paths:
                target = self.root / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes((repository / relative).read_bytes())

        seed()
        verifier.verify_trace_branch_callsites(self.root)
        mutations = (
            (
                paths[0],
                "executor.task4_trace_statement_update();",
                "",
            ),
            (
                paths[1],
                "executor.task4_trace_prepared_query();",
                "",
            ),
            (
                paths[2],
                "} else if let Some(trace) = &self.runtime_trace {",
                "} if let Some(trace) = &self.runtime_trace {",
            ),
        )
        for relative, old, new in mutations:
            with self.subTest(relative=relative, mutation=old):
                seed()
                target = self.root / relative
                source = target.read_text(encoding="utf-8")
                self.assertIn(old, source)
                target.write_text(source.replace(old, new, 1), encoding="utf-8")
                with self.assertRaisesRegex(
                    VerificationError, "multiplicity|mutually exclusive"
                ):
                    verifier.verify_trace_branch_callsites(self.root)

    def test_parameter_trace_is_pinned_to_both_successful_setter_paths(self) -> None:
        """两个合法绑定入口各记录一次；相同文件总数不能掩盖跨方法搬移。"""

        repository_source = Path(__file__).resolve().parents[1] / "crates/druid-wrapper/src"
        temporary_source = self.root / "crates/druid-wrapper/src"
        shutil.copytree(repository_source, temporary_source, dirs_exist_ok=True)
        verify_trace_record_callsites(self.root)
        for relative, receiver in (
            ("standard_adapter_prepared_statement.rs", "executor"),
            ("sqlx/sqlx_prepared_statement.rs", "actor"),
        ):
            path = temporary_source / relative
            original = path.read_text(encoding="utf-8")
            call = f"{receiver}.task4_trace_parameter_set();"
            self.assertEqual(original.count(call), 2)
            # 删除第一个入口的事件，再在第二个入口重复记录；文件总数仍为二。
            moved = original.replace(call, "", 1).replace(call, call + "\n" + call, 1)
            path.write_text(moved, encoding="utf-8")
            with self.subTest(relative=relative):
                with self.assertRaisesRegex(VerificationError, "branch callsite multiplicity"):
                    verify_trace_record_callsites(self.root)
            path.write_text(original, encoding="utf-8")

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
        report = self.root / "coverage.json"
        self.assertEqual(
            cargo_evidence_command(spec, report),
            [
                "cargo",
                "llvm-cov",
                "test",
                "--json",
                "--output-path",
                str(report),
                "--offline",
                "--locked",
                "--color",
                "never",
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

    def test_complete_source_closure_rejects_helper_marker_fake_summary_and_exit(
        self,
    ) -> None:
        """A sibling helper cannot forge marker/completion output outside the root test file."""

        checker = getattr(verifier, "verify_evidence_source_closure", None)
        self.assertIsNotNone(
            checker, "complete Rust evidence source-closure gate is missing"
        )
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
            ' let prefix = ["DRUID_TASK4", "_RUNTIME_TRACE"].concat();\n'
            " let mut stderr = std::io::stderr();\n"
            ' writeln!(stderr, "{}\\t{}", prefix, "{}").unwrap();\n'
            ' println!("test result: ok. 1 passed; 0 failed; 0 ignored;");\n'
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

    def test_record_provenance_rejects_allowed_file_helper_and_unapproved_ufcs(
        self,
    ) -> None:
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
            original + "\nfn helper(trace: &RuntimeTraceEmitter) {\n"
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

    def test_stage_and_validation_anchor_source_must_be_unique(self) -> None:
        """Compiler-visible anchor definitions and calls cannot disappear or duplicate."""

        repository_source = (
            Path(__file__).resolve().parents[1] / "crates/druid-wrapper/src"
        )
        temporary_source = self.root / "crates/druid-wrapper/src"
        shutil.copytree(repository_source, temporary_source, dirs_exist_ok=True)
        trace_path = temporary_source / "task4_runtime_trace.rs"
        original = trace_path.read_text(encoding="utf-8")
        stage_call = "self.task4_coverage_anchor_statement_update();"
        success_call = "task4_coverage_anchor_validation_success();"
        fixtures = {
            "missing-stage": original.replace(stage_call, "", 1),
            "duplicate-stage": original.replace(
                stage_call,
                f"{stage_call}\n        {stage_call}",
                1,
            ),
            "missing-success": original.replace(success_call, "", 1),
            "duplicate-success": original.replace(
                success_call,
                f"{success_call}\n    {success_call}",
                1,
            ),
        }
        for label, source in fixtures.items():
            with self.subTest(label=label):
                trace_path.write_text(source, encoding="utf-8")
                with self.assertRaisesRegex(
                    VerificationError,
                    "coverage anchor.*missing|missing.*coverage anchor|bypassable",
                ):
                    verify_trace_record_callsites(self.root)
        trace_path.write_text(original, encoding="utf-8")

    def test_runtime_manifest_rejects_http_libsql_family_binding_swap_before_dedup(
        self,
    ) -> None:
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
                else http if "libsql/" in row.source_path.as_posix() else None
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

    def test_runner_is_offline_locked_bounded_and_kills_hanging_process_group(
        self,
    ) -> None:
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
        command = cargo_evidence_command(spec, self.root / "coverage.json")
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
        self.assertFalse(
            child_alive, f"evidence child process survived group kill: {child_pid}"
        )
        with self.assertRaisesRegex(VerificationError, "output limit"):
            runner(
                ["python3", "-c", "print('x' * 10000)"],
                cwd=self.root,
                timeout_seconds=2.0,
                output_limit=1024,
            )

    @unittest.skipUnless(os.name == "posix", "process groups require POSIX")
    def test_runner_kills_captured_process_group_after_direct_parent_exits(
        self,
    ) -> None:
        """Timeout and overflow cleanup use the start-time PGID after cargo has exited."""

        runner = getattr(verifier, "run_bounded_process", None)
        self.assertIsNotNone(runner, "bounded process-group runner is missing")
        if runner is None:
            return
        for label, child_body, expected_error in (
            ("timeout", "import time; time.sleep(10)", "timed out"),
            (
                "overflow",
                "import sys,time; sys.stdout.write('x'*10000); sys.stdout.flush(); time.sleep(10)",
                "output limit",
            ),
        ):
            with self.subTest(label=label):
                child_pid_file = self.root / f"{label}-orphan.pid"
                parent_script = (
                    "import subprocess; "
                    f"child=subprocess.Popen(['python3','-c',{child_body!r}]); "
                    f"open({str(child_pid_file)!r},'w').write(str(child.pid))"
                )
                child_pid: int | None = None
                try:
                    with self.assertRaisesRegex(VerificationError, expected_error):
                        runner(
                            ["python3", "-c", parent_script],
                            cwd=self.root,
                            timeout_seconds=0.2 if label == "timeout" else 2.0,
                            output_limit=4096 if label == "timeout" else 1024,
                        )
                    child_pid = int(child_pid_file.read_text(encoding="utf-8"))
                    child_alive = True
                    for _ in range(20):
                        try:
                            os.kill(child_pid, 0)
                        except ProcessLookupError:
                            child_alive = False
                            break
                        time.sleep(0.05)
                    self.assertFalse(
                        child_alive,
                        f"evidence descendant survived captured PGID cleanup: {child_pid}",
                    )
                finally:
                    if child_pid is not None:
                        try:
                            os.kill(child_pid, signal.SIGKILL)
                        except ProcessLookupError:
                            pass

        detached_pid_file = self.root / "successful-parent-detached-child.pid"
        detached_child = "import time; time.sleep(10)"
        successful_parent = (
            "import subprocess; "
            "child=subprocess.Popen("
            f"['python3','-c',{detached_child!r}],"
            "stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL); "
            f"open({str(detached_pid_file)!r},'w').write(str(child.pid))"
        )
        detached_pid: int | None = None
        try:
            completed = runner(
                ["python3", "-c", successful_parent],
                cwd=self.root,
                timeout_seconds=2.0,
                output_limit=4096,
            )
            self.assertEqual(completed.returncode, 0)
            detached_pid = int(detached_pid_file.read_text(encoding="utf-8"))
            detached_alive = True
            for _ in range(20):
                try:
                    os.kill(detached_pid, 0)
                except ProcessLookupError:
                    detached_alive = False
                    break
                time.sleep(0.05)
            self.assertFalse(
                detached_alive,
                "successful direct parent left a detached process-group descendant",
            )
        finally:
            if detached_pid is not None:
                try:
                    os.kill(detached_pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass

    def test_runtime_marker_parser_is_diagnostic_not_the_coverage_anchor(self) -> None:
        """Diagnostic marker parsing remains exact but coverage accepts no output text."""

        parameters = inspect.signature(validate_runtime_trace_output).parameters
        self.assertIn("expected_challenge", parameters)
        coverage_parameters = inspect.signature(
            verifier.validate_compiler_coverage
        ).parameters
        self.assertNotIn("output", coverage_parameters)
        self.assertNotIn("challenge", coverage_parameters)
        self.assertIsNotNone(
            getattr(verifier, "TRACE_CALLSITE_RULES", None),
            "typed caller provenance rules are missing",
        )

    def test_caller_line_must_remain_inside_the_exact_production_function_region(
        self,
    ) -> None:
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
                destination.write_text(
                    source.read_text(encoding="utf-8"), encoding="utf-8"
                )
                copied.add(relative)
            method = verifier.TRACE_STAGE_METHODS[event["stage"]]
            call_lines = [
                index
                for index, line in enumerate(
                    destination.read_text(encoding="utf-8").splitlines(), start=1
                )
                if f".{method}(" in line
            ]
            self.assertTrue(
                call_lines, f"missing canonical callsite {relative}:{method}"
            )
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
