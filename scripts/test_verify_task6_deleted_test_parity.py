#!/usr/bin/env python3
"""Adversarial self-tests for the Task 6 deleted-test parity verifier."""

from __future__ import annotations

import csv
import io
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path

from verify_task6_deleted_test_parity import (
    APPROVED_REMOVAL,
    EXACT,
    LEDGER_HEADER,
    PURE_IDENTITY,
    BaseTest,
    CurrentTest,
    LedgerRow,
    VerificationError,
    executed_functions,
    expanded_target_source,
    identity_only,
    missing_target_functions,
    missing_parameterized_case_digests,
    parameterized_case_body,
    parse_ledger,
    parse_test_functions,
    current_tests,
    target_package,
    verify_ledger,
    render_method_call_index,
    run_git,
)


class DeletedTestParityVerifierTest(unittest.TestCase):
    """Prove the verifier rejects denominator and execution shortcuts."""

    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)

    def tearDown(self) -> None:
        self.temporary.cleanup()

    @staticmethod
    def source(function: str = "preserves_behavior", body: str | None = None) -> str:
        body = body or "{ assert_eq!(adapter.commit(), 7); }"
        return (
            "#[test]\n"
            f"fn {function}() {body}\n"
            '\n#[tokio::test(flavor = "current_thread")]\n'
            "async fn async_case() { assert!(pool.recycle().await.is_ok()); }\n"
            "\nfn helper_is_not_a_test() {}\n"
        )

    @staticmethod
    def row(test: BaseTest, **changes: str) -> LedgerRow:
        values = {
            "base_source_path": test.source_path,
            "base_function": test.function,
            "base_identity": test.identity,
            "base_body_sha256": test.body_sha256,
            "behavioral_theme": test.behavioral_theme,
            "key_assertions": test.key_assertions,
            "key_calls": test.key_calls,
            "disposition": EXACT,
            "target_source_path": "crates/druid/tests/canonical_parity_test.rs",
            "target_function": test.function,
            "case_id": "-",
        }
        values.update(changes)
        return LedgerRow(**values)

    def write_target(self, source: str) -> None:
        path = self.root / "crates/druid/tests/canonical_parity_test.rs"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(source, encoding="utf-8")

    def test_parser_extracts_sync_and_tokio_tests_with_observations(self) -> None:
        tests = parse_test_functions("crates/druid/tests/base.rs", self.source())
        self.assertEqual(
            ["preserves_behavior", "async_case"], [test.function for test in tests]
        )
        self.assertEqual("assert_eq:1", tests[0].key_assertions)
        self.assertIn("commit:1", tests[0].key_calls)
        self.assertEqual("assert:1", tests[1].key_assertions)
        self.assertIn("recycle:1", tests[1].key_calls)

    def test_rejects_missing_duplicate_and_fabricated_base_rows(self) -> None:
        base = parse_test_functions("crates/druid/tests/base.rs", self.source())
        self.write_target(self.source())
        with self.assertRaisesRegex(VerificationError, "denominator mismatch"):
            verify_ledger(base, (self.row(base[0]),), self.root)
        with self.assertRaisesRegex(VerificationError, "duplicate"):
            verify_ledger(base, (self.row(base[0]), self.row(base[0])), self.root)
        fabricated = replace(
            self.row(base[0]),
            base_identity="crates/druid/tests/base.rs#fabricated",
            base_function="fabricated",
        )
        with self.assertRaisesRegex(VerificationError, "denominator mismatch"):
            verify_ledger(base, (fabricated, self.row(base[1])), self.root)

    def test_rejects_non_test_target_duplicate_target_and_retired_identity(
        self,
    ) -> None:
        base = (parse_test_functions("crates/druid/tests/base.rs", self.source())[0],)
        self.write_target("fn preserves_behavior() {}\n")
        with self.assertRaisesRegex(VerificationError, "not an executable"):
            verify_ledger(base, (self.row(base[0]),), self.root)

        self.write_target(self.source(function="one"))
        duplicated_base = (
            replace(base[0], function="base_one"),
            replace(base[0], function="base_two", body_sha256="b" * 64),
        )
        rows = (
            self.row(duplicated_base[0], target_function="one"),
            self.row(duplicated_base[1], target_function="one"),
        )
        with self.assertRaisesRegex(VerificationError, "exact targets are reused"):
            verify_ledger(duplicated_base, rows, self.root)

        self.write_target(
            "#[test]\nfn preserves_behavior() { let _: Option<PhysicalConnection> = None; }\n"
        )
        with self.assertRaisesRegex(VerificationError, "retired identity"):
            verify_ledger(base, (self.row(base[0]),), self.root)

        self.write_target("#[test]\nfn preserves_behavior() { assert!(true); }\n")
        with self.assertRaisesRegex(VerificationError, "weakens BASE"):
            verify_ledger(base, (self.row(base[0]),), self.root)

    def test_only_removed_identity_bodies_may_be_waived(self) -> None:
        identity = parse_test_functions(
            "crates/druid/tests/base.rs",
            "#[test]\nfn identity() { "
            "assert_type_eq_all!(dyn druid_rdbc::spi::PhysicalConnection, "
            "dyn druid::core::PhysicalConnection); }\n",
        )[0]
        behavioral = parse_test_functions(
            "crates/druid/tests/base.rs",
            "#[test]\nfn behavior() { "
            "assert_eq!(std::any::TypeId::of::<PhysicalConnection>(), expected); }\n",
        )[0]
        self.assertTrue(identity_only(identity))
        self.assertFalse(identity_only(behavioral))
        for disposition in (PURE_IDENTITY, APPROVED_REMOVAL):
            waived = self.row(
                behavioral,
                disposition=disposition,
                target_source_path="-",
                target_function="-",
            )
            with self.assertRaisesRegex(VerificationError, "cannot be removed"):
                verify_ledger((behavioral,), (waived,), self.root)

    def test_ledger_parser_rejects_header_drift_and_empty_fields(self) -> None:
        path = self.root / "ledger.tsv"
        path.write_text("wrong\theader\n", encoding="utf-8", newline="\n")
        with self.assertRaisesRegex(VerificationError, "header drift"):
            parse_ledger(path)

        buffer = io.StringIO()
        writer = csv.writer(buffer, delimiter="\t", lineterminator="\n")
        writer.writerow(LEDGER_HEADER)
        writer.writerow(["value"] * (len(LEDGER_HEADER) - 1) + [""])
        path.write_text(buffer.getvalue(), encoding="utf-8", newline="\n")
        with self.assertRaisesRegex(VerificationError, "empty"):
            parse_ledger(path)

    def test_git_baseline_decodes_chinese_source_as_utf8(self) -> None:
        root = Path(__file__).resolve().parent.parent
        source = run_git(
            root,
            ["show", "HEAD:docs/superpowers/specs/2026-08-24-rdbc-single-standard-trait-tree.md"],
        )
        self.assertIn("单一 JDBC-like Standard Trait Tree", source)

    def test_execution_parser_accepts_only_successful_libtest_functions(self) -> None:
        output = (
            "test cases::canonical_ok ... ok\n"
            "test cases::ignored ... ignored\n"
            "test cases::failed ... FAILED\n"
        )
        self.assertEqual({"cases::canonical_ok"}, executed_functions(output))

    def test_runtime_proof_keeps_success_marker_separate_from_case_stdout(self) -> None:
        """成功案例 stdout 必须可观察，且不能打断 libtest 的精确成功行。"""
        import subprocess
        from unittest.mock import patch
        import verify_task6_deleted_test_parity as parity

        base = parse_test_functions("crates/druid/tests/base.rs", self.source())[0]
        row = self.row(base, disposition="MERGED_PARAMETERIZED(onlycase)",
            target_function="matrix", case_id="onlycase")

        def actual_harness_shape(command, **kwargs):
            # 对应真实单线程 --nocapture 输出：digest 插入测试名和 ok 之间。
            output = "test matrix ... TASK6_CASE_DIGEST=matrix:onlycase\nok\n"
            if "--show-output" in command:
                output = ("test matrix ... ok\n\nsuccesses:\n\n---- matrix stdout ----\n"
                    "TASK6_CASE_DIGEST=matrix:onlycase\n")
            return subprocess.CompletedProcess(command, 0, stdout=output, stderr="")

        with patch("subprocess.run", side_effect=actual_harness_shape):
            parity.execute_targets(self.root, (row,), ())

    def test_same_name_in_another_target_cannot_satisfy_claimed_target(self) -> None:
        expected = {
            "claimed_target": {"same_name"},
            "other_target": {"same_name"},
        }
        outputs = {
            "claimed_target": "test same_name ... FAILED\n",
            "other_target": "test same_name ... ok\n",
        }
        self.assertEqual(
            ["claimed_target#same_name"],
            missing_target_functions(expected, outputs),
        )

    def test_method_call_index_uses_path_function_and_case_selectors(self) -> None:
        base = parse_test_functions("crates/druid/tests/base.rs", self.source())
        exact = self.row(base[0])
        parameterized = self.row(
            base[1],
            disposition="MERGED_PARAMETERIZED(async_case)",
            target_function="matrix",
            case_id="async_case",
        )
        rendered = render_method_call_index((exact, parameterized))
        self.assertIn(
            "crates/druid/tests/canonical_parity_test.rs#preserves_behavior",
            rendered,
        )
        self.assertIn(
            "crates/druid/tests/canonical_parity_test.rs#matrix::async_case",
            rendered,
        )

    def test_declared_but_uniterated_parameterized_case_is_rejected(self) -> None:
        expected = {"matrix": ["iterated", "dead_case"]}
        output = "TASK6_CASE_DIGEST=matrix:iterated\n"
        self.assertEqual(
            ["matrix::dead_case", "matrix::iterated"],
            missing_parameterized_case_digests(expected, output),
        )
        complete = "TASK6_CASE_DIGEST=matrix:iterated|dead_case\n"
        self.assertEqual(
            [], missing_parameterized_case_digests(expected, complete)
        )

    def test_parameterized_case_requires_one_explicit_match_arm(self) -> None:
        generic = CurrentTest(
            "crates/druid/tests/canonical_parity_test.rs",
            "matrix",
            '{ let cases = ["named_savepoint"]; for case_id in cases { assert!(case_id.len() > 0); } }',
            "",
        )
        with self.assertRaisesRegex(VerificationError, "explicit case arm"):
            parameterized_case_body(generic, "named_savepoint")

        explicit = CurrentTest(
            "crates/druid/tests/canonical_parity_test.rs",
            "matrix",
            '{ match case_id { "named_savepoint" => { assert!(Connection::begin(&mut connection).await.is_ok()); }, _ => unreachable!(), } }',
            "",
        )
        body = parameterized_case_body(explicit, "named_savepoint")
        self.assertIn("Connection::begin", body)

    def test_parameterized_case_rejects_tautological_result_masking(self) -> None:
        target = CurrentTest(
            "crates/druid/tests/canonical_parity_test.rs",
            "matrix",
            '{ match case_id { "update" => { let result = update(); assert!(result.is_err() || result.is_ok()); }, _ => unreachable!(), } }',
            "",
        )
        with self.assertRaisesRegex(VerificationError, "tautological"):
            parameterized_case_body(target, "update")
    def test_literal_include_closure_is_expanded_and_cannot_escape(self) -> None:
        target = self.root / "crates/druid/tests/canonical_parity_test.rs"
        part = self.root / "crates/druid/tests/generated/part.rs"
        part.parent.mkdir(parents=True, exist_ok=True)
        target.write_text('include!("generated/part.rs");\n', encoding="utf-8")
        part.write_text(
            "#[test]\nfn included_case() { assert!(true); }\n", encoding="utf-8"
        )
        expanded = expanded_target_source(self.root, target)
        self.assertIn("fn included_case", expanded)

        target.write_text('include!("../../../outside.rs");\n', encoding="utf-8")
        with self.assertRaisesRegex(VerificationError, "escapes owned test roots"):
            expanded_target_source(self.root, target)

    def test_crate_private_metadata_target_keeps_qualified_lib_identity(self) -> None:
        source_path = "crates/druid/src/core/result_set_meta_data_proxy.rs"
        target = self.root / source_path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(
            "#[cfg(test)]\nmod tests {\n"
            "    #[test]\n"
            "    fn result_set_meta_data_proxy_attributes() { assert!(true); }\n"
            "}\n",
            encoding="utf-8",
        )
        tests = current_tests(self.root, (source_path,))
        identity = (
            f"{source_path}#core::result_set_meta_data_proxy::tests::"
            "result_set_meta_data_proxy_attributes"
        )
        self.assertIn(identity, tests)
        self.assertEqual(("druid", "__lib__"), target_package(source_path))


    def test_result_set_diagnostics_keep_qualified_lib_identity(self) -> None:
        source_path = "crates/druid/src/core/druid_pooled_result_set/diagnostic_tests.rs"
        target = self.root / source_path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(
            "#[tokio::test]\n"
            "async fn task6_close_count_zero_before_close() { assert_eq!(observed(), 0); }\n",
            encoding="utf-8",
        )
        tests = current_tests(self.root, (source_path,))
        identity = (
            f"{source_path}#core::druid_pooled_result_set::diagnostic_tests::"
            "task6_close_count_zero_before_close"
        )
        self.assertIn(identity, tests)
        self.assertEqual(("druid", "__lib__"), target_package(source_path))
        with self.assertRaisesRegex(VerificationError, "unknown target test package"):
            target_package("crates/druid/src/core/unregistered_diagnostics.rs")


    def test_wrapper_private_lib_target_keeps_own_package_and_qualified_identity(self) -> None:
        from unittest.mock import patch
        import verify_task6_deleted_test_parity as parity
        source_path = "crates/druid-wrapper/src/sqlx/external_pool_connection_control/lease_complete_tests.rs"
        function = "task6_lease_delegates_complete_spi"
        qualified = "sqlx::external_pool_connection_control::lease_complete_tests::" + function
        target = self.root / source_path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(self.source(function=function), encoding="utf-8")
        with patch.dict(parity.LIB_TEST_TARGETS, {source_path: {function: qualified}}):
            tests = parity.current_tests(self.root, (source_path,))
            self.assertIn(source_path + "#" + qualified, tests)
            self.assertEqual(("druid-wrapper", "__lib__"), parity.target_package(source_path))

    def test_unknown_registered_lib_package_cannot_route_to_druid(self) -> None:
        from unittest.mock import patch
        import verify_task6_deleted_test_parity as parity
        source_path = "crates/other/src/private_tests.rs"
        with patch.dict(parity.LIB_TEST_TARGETS, {source_path: {"test": "tests::test"}}):
            with self.assertRaisesRegex(VerificationError, "unknown target test package"):
                parity.target_package(source_path)


if __name__ == "__main__":
    unittest.main()
