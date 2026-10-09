#!/usr/bin/env python3
"""Task 6 方法级迁移验证器的 fail-closed 自测。"""

from __future__ import annotations

import csv
import io
import subprocess
import sys
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path
from unittest.mock import patch


SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS))

from verify_task6_method_parity import (  # noqa: E402
    BehaviorSelector,
    METHOD_HEADER,
    MethodCallEvidence,
    MethodRow,
    VerificationError,
    behavior_candidates,
    execute_behavior_evidence,
    parse_behavior_selector,
    parse_direct_trait_methods,
    parse_rows,
    parse_target_method,
    render_rows,
    verify_c2_runtime_evidence,
    verify_generated_behavior_matrix,
    verify_rows,
    _verify_selector_contract,
    _verify_selector_source,
    _summary,
)
import render_task6_method_default_evidence as evidence_renderer  # noqa: E402


TRAIT_SOURCE = r'''
#[async_trait::async_trait]
pub trait PhysicalExample: Send {
    /// 必须实现的方法。
    async fn required(
        &mut self,
        value: Vec<(String, i32)>,
    ) -> Result<Vec<i32>, Error>;

    fn unsupported(&self, _value: &str) -> Result<(), Error> {
        Err(Error::UnsupportedOperation { operation: "example_unsupported" })
    }

    fn constant(&self) -> bool {
        false
    }
}

impl PhysicalExample for Example {
    async fn required(&mut self, value: Vec<(String, i32)>) -> Result<Vec<i32>, Error> {
        Ok(value.into_iter().map(|(_, value)| value).collect())
    }
}
'''


TARGET_SOURCE = r'''
pub trait Example {
    async fn required(&mut self, value: Vec<(String, i32)>) -> Result<Vec<i32>, Error>;
    fn unsupported(&self, value: &str) -> Result<(), Error>;
    fn constant(&self) -> bool;
}
'''


EVIDENCE_SOURCE = r'''
#[test]
fn required_behavior() {
    let actual = Example::required(&mut example, vec![]);
    assert_eq!(actual, Ok(vec![]));
}

#[test]
fn unsupported_behavior() {
    let actual = Example::unsupported(&example, "value");
    assert!(matches!(actual, Err(Error::UnsupportedOperation { operation: "example_unsupported" })));
}

#[test]
fn constant_behavior() {
    assert_eq!(Example::constant(&example), false);
}
'''


class Task6MethodParityVerifierTest(unittest.TestCase):
    """验证 tokenized denominator、TSV schema 和 target 绑定。"""

    def test_tokenizes_only_direct_trait_methods_and_normalizes_signatures(self) -> None:
        """impl 方法不得污染 direct public method denominator。"""

        methods = parse_direct_trait_methods(TRAIT_SOURCE, "PhysicalExample")
        self.assertEqual(["required", "unsupported", "constant"], [m.name for m in methods])
        self.assertEqual("REQUIRED", methods[0].default_kind)
        self.assertEqual("DEFAULT_UNSUPPORTED", methods[1].default_kind)
        self.assertEqual("DEFAULT_CONSTANT", methods[2].default_kind)
        self.assertIn("Vec<(String,i32)>", methods[0].signature)

    def test_rejects_missing_duplicate_and_extra_base_rows(self) -> None:
        """每个 BASE identity/signature 必须恰好出现一次。"""

        rows = self.rows()
        self.verify(rows)
        for invalid in (rows[:-1], rows + (rows[0],), rows + (replace(rows[0], method="fake"),)):
            with self.assertRaises(VerificationError):
                self.verify(invalid)

    def test_rejects_signature_hash_and_base_evidence_drift(self) -> None:
        """签名、default 分类、override/call paths 都不能手工伪造。"""

        rows = self.rows()
        mutations = (
            replace(rows[0], source_signature="fn required(&self)"),
            replace(rows[1], source_signature_sha256="0" * 64),
            replace(rows[1], base_default_kind="DEFAULT_NOOP"),
            replace(rows[0], base_override_paths="-"),
            replace(rows[0], base_call_paths="src/fake.rs"),
        )
        for mutation in mutations:
            candidate = tuple(mutation if row.method == mutation.method else row for row in rows)
            with self.assertRaises(VerificationError):
                self.verify(candidate)

    def test_rejects_unknown_disposition_and_nonexistent_target(self) -> None:
        """Disposition 必须闭集，target path/symbol 必须真实存在。"""

        rows = self.rows()
        for mutation in (
            replace(rows[0], disposition="REMOVED"),
            replace(rows[0], target_path="src/missing.rs"),
            replace(rows[0], target_symbol="Example::missing"),
        ):
            candidate = tuple(mutation if row.method == mutation.method else row for row in rows)
            with self.assertRaises(VerificationError):
                self.verify(candidate)

    def test_approved_break_requires_explicit_ruling_and_no_behavior(self) -> None:
        """APPROVED_BREAK 不能用于 required/overridden/called method。"""

        rows = self.rows()
        required_break = replace(
            rows[0],
            disposition="APPROVED_BREAK",
            approval_ref="spec:approved-break",
            target_entry="-",
            target_path="-",
            target_symbol="-",
        )
        with self.assertRaises(VerificationError):
            self.verify((required_break, *rows[1:]))

        unsupported_break = replace(
            rows[1],
            disposition="APPROVED_BREAK",
            approval_ref="spec:approved-break",
            target_entry="-",
            target_path="-",
            target_symbol="-",
            target_trait="-",
            target_signature="-",
            target_signature_sha256="-",
            target_ufcs="-",
        )
        self.verify((rows[0], unsupported_break, rows[2]))
        with self.assertRaises(VerificationError):
            self.verify((rows[0], replace(unsupported_break, approval_ref="-"), rows[2]))

    def test_rejects_target_trait_signature_hash_and_ufcs_drift(self) -> None:
        """Canonical target identity 必须从真实 trait source 精确重算。"""

        rows = self.rows()
        mutations = (
            replace(rows[0], target_trait="WrongExample"),
            replace(rows[0], target_signature="fn required(&self)"),
            replace(rows[0], target_signature_sha256="0" * 64),
            replace(rows[0], target_ufcs="canonical::ResultSet::required"),
        )
        for mutation in mutations:
            candidate = tuple(
                mutation if row.method == mutation.method else row for row in rows
            )
            with self.assertRaises(VerificationError):
                self.verify(candidate)

    def test_tsv_round_trip_is_canonical_and_rejects_schema_drift(self) -> None:
        """Renderer/hash 只接受固定 header、排序和 LF bytes。"""

        rows = self.rows()
        payload = render_rows(rows)
        self.assertEqual(tuple(sorted(rows)), parse_rows(payload))
        self.assertTrue(payload.endswith(b"\n"))

        fields = list(csv.reader(io.StringIO(payload.decode()), delimiter="\t"))
        fields[0][-1] = "unexpected"
        broken = "\n".join("\t".join(row) for row in fields).encode() + b"\n"
        with self.assertRaisesRegex(VerificationError, "header"):
            parse_rows(broken)

    def test_rejects_missing_duplicate_fabricated_and_wrong_file_evidence(self) -> None:
        """Evidence 必须是 C2-derived path-qualified executable selector。"""

        rows = self.rows()
        selector = rows[0].behavior_evidence
        mutations = (
            replace(rows[0], behavior_evidence="-"),
            replace(rows[0], behavior_evidence=f"{selector};{selector}"),
            replace(
                rows[0],
                behavior_evidence="crates/druid-rdbc/tests/evidence.rs#fabricated",
            ),
            replace(
                rows[0],
                behavior_evidence="crates/druid-rdbc/tests/wrong.rs#required_behavior",
            ),
            replace(
                rows[0],
                behavior_evidence="docs/verification/rdbc-first/task6-deleted-test-migration.tsv",
            ),
        )
        for mutation in mutations:
            candidate = tuple(mutation if row.method == mutation.method else row for row in rows)
            with self.assertRaises(VerificationError):
                self.verify(candidate)

    def test_rejects_same_leaf_evidence_from_the_wrong_canonical_receiver(self) -> None:
        """Connection::warnings 不能被 ResultSet::warnings 的同名测试冒充。"""

        base_source = r'''
pub trait ConnectionExt {
    async fn get_warnings(&mut self) -> Result<Option<SqlWarning>, Error>;
}
'''
        target_source = r'''
pub trait Connection {
    async fn warnings(&mut self) -> Result<Option<SqlWarning>, Error>;
}
'''
        wrong_receiver_evidence = r'''
#[test]
fn result_set_warnings_behavior() {
    let result = ResultSet::warnings(&mut result_set);
    assert_eq!(result, Ok(None));
}
'''
        method = parse_direct_trait_methods(base_source, "ConnectionExt")[0]
        target_method = parse_target_method(target_source, "Connection", "warnings")
        row = MethodRow(
            source_base="base-sha",
            source_object="ConnectionExt",
            source_trait_path="src/connection_ext.rs",
            method="get_warnings",
            source_signature=method.signature,
            source_signature_sha256=method.signature_sha256,
            base_default_kind=method.default_kind,
            base_override_paths="-",
            base_call_paths="-",
            disposition="MERGED_TYPED",
            target_entry="canonical::Connection::warnings",
            target_path="src/connection.rs",
            target_symbol="Connection::warnings",
            conversion="TYPED_OPTIONS+RESULT_FAIL_CLOSED",
            approval_ref="-",
            behavior_evidence=(
                "crates/druid-rdbc/tests/evidence.rs#result_set_warnings_behavior"
            ),
            notes="wrong-receiver regression",
            target_trait="Connection",
            target_signature=target_method.signature,
            target_signature_sha256=target_method.signature_sha256,
            target_ufcs="canonical::Connection::warnings",
        )
        call_index = (
            MethodCallEvidence(
                base_identity="tests/connection_test.rs#warnings",
                method_call="warnings",
                base_call_count=1,
                replacement_selector=row.behavior_evidence,
                disposition="MIGRATED_EXACT",
                behavioral_theme="warning_error",
            ),
        )

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "src").mkdir()
            evidence = root / "crates/druid-rdbc/tests"
            evidence.mkdir(parents=True)
            (root / "src/connection_ext.rs").write_text(
                base_source, encoding="utf-8"
            )
            (root / "src/connection.rs").write_text(
                target_source, encoding="utf-8"
            )
            (evidence / "evidence.rs").write_text(
                wrong_receiver_evidence, encoding="utf-8"
            )
            with self.assertRaisesRegex(VerificationError, "receiver"):
                verify_rows(
                    (row,),
                    repository_root=root,
                    source_base="base-sha",
                    base_sources={"src/connection_ext.rs": base_source},
                    trait_specs={"ConnectionExt": "src/connection_ext.rs"},
                    method_call_index=call_index,
                )

    def test_fallback_case_is_independent_receiver_exact_and_assertion_exact(self) -> None:
        """Fallback case 不得退化成 generic loop、case digest 或弱结果判断。"""

        row = replace(
            self.rows()[0],
            source_object="ConnectionExt",
            method="get_warnings",
            target_entry="canonical::Connection::warnings",
            target_symbol="Connection::warnings",
            target_trait="Connection",
            target_signature=(
                "async fn warnings(&mut self)->Result<Option<SqlWarning>,RdbcError>"
            ),
            target_signature_sha256=(
                "d4e066e3ad3e98ea7535001c0c62b59b1e1004836f932864e61ca4f7f9a92952"
            ),
            target_ufcs="canonical::Connection::warnings",
        )
        rendered = evidence_renderer.render_behavior_case(row)

        self.assertIn(
            "async fn task6_method_behavior_connection_ext_get_warnings()",
            rendered,
        )
        self.assertIn("Connection::warnings(&mut target).await", rendered)
        self.assertIn(
            'assert_trace(actual, "canonical::Connection::warnings")', rendered
        )
        for forbidden in (
            "for case_id",
            "TASK6_CASE_DIGEST",
            "is_ok() ||",
            "is_err()",
        ):
            self.assertNotIn(forbidden, rendered)

    def test_rejects_missing_or_tautological_observable_assertion(self) -> None:
        """Exact UFCS 后仍必须有 linked exact assertion，弱 mask 一律拒绝。"""

        row = self.rows()[0]
        selector = BehaviorSelector(
            "crates/druid-rdbc/tests/evidence.rs", "required_behavior"
        )
        bodies = (
            r'''
#[test]
fn required_behavior() {
    let actual = Example::required(&mut example, vec![]);
}
''',
            r'''
#[test]
fn required_behavior() {
    let actual = Example::required(&mut example, vec![]);
    assert!(actual.is_ok() || actual.is_err());
}
''',
            r'''
#[test]
fn required_behavior() {
    let actual = Example::required(&mut example, vec![]);
    assert!(matches!(actual, Err(_)));
}
''',
        )
        for body in bodies:
            with tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                target = root / selector.source_path
                target.parent.mkdir(parents=True)
                target.write_text(body, encoding="utf-8")
                with self.assertRaisesRegex(VerificationError, "observable assertion"):
                    _verify_selector_contract(root, selector, row)

    def test_c2_rebinding_preserves_independent_default_execution_control(self) -> None:
        """C2 主证据重新绑定后，原独立 default 控制必须保留且不得重复。"""

        default = replace(
            self.rows()[1],
            source_object="PhysicalConnection",
            method="execute",
            target_trait="Statement",
            target_symbol="Statement::execute",
            target_signature="async fn execute(&mut self,sql:&str)->Result<bool,RdbcError>",
            target_ufcs="canonical::Statement::execute",
            behavior_evidence=(
                "crates/druid-rdbc/tests/task6_method_default_evidence_test.rs#"
                "task6_method_behavior_physical_connection_execute"
            ),
        )
        rebound = replace(
            default,
            behavior_evidence=(
                "crates/druid-wrapper/src/sqlx/external_pool_connection_control/"
                "lease_complete_tests.rs#sqlx::external_pool_connection_control::"
                "lease_complete_tests::task6_lease_delegates_complete_spi"
            ),
        )
        unrelated = replace(rebound, source_object="AnotherConnection")
        with (
            patch.object(evidence_renderer, "_method_lookup", return_value={}),
            patch.object(evidence_renderer, "_render_trace_impl", return_value=""),
            patch.object(evidence_renderer, "_rustfmt", side_effect=lambda source: source.encode()),
        ):
            before = evidence_renderer.render_files(Path("/tmp/method-render"), (default,))
            after = evidence_renderer.render_files(Path("/tmp/method-render"), (rebound,))
            self.assertEqual(before, after)
            statement = after[evidence_renderer.GENERATED_DIRECTORY / "statement.inc.rs"]
            self.assertEqual(1, statement.count(b"fn task6_method_behavior_physical_connection_execute("))
            other = evidence_renderer.render_files(Path("/tmp/method-render"), (unrelated,))
            self.assertNotIn(
                b"task6_method_behavior_another_connection_execute",
                b"".join(other.values()),
            )

    def test_rejects_case_id_list_without_literal_arm_and_reachable_helper(self) -> None:
        """Case-ID list/digest 不能替代唯一 arm 到 receiver-exact helper 的路径。"""

        row = self.rows()[0]
        selector = BehaviorSelector(
            "crates/druid-rdbc/tests/evidence.rs",
            "required_behavior",
            "case_one",
        )
        source = r'''
#[test]
fn required_behavior() {
    let cases = ["case_one"];
    assert_eq!(cases.len(), 1);
}
'''
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / selector.source_path
            target.parent.mkdir(parents=True)
            target.write_text(source, encoding="utf-8")
            with self.assertRaisesRegex(VerificationError, "literal match arm"):
                _verify_selector_contract(root, selector, row)

    def test_rejects_typed_operation_trace_that_compares_the_same_value(self) -> None:
        """Explicit typed trace 必须比较独立 actual/expected，而非同值自证。"""

        row = replace(
            self.rows()[0],
            target_ufcs="canonical::ExternalPoolLeaseControl::poison",
            target_trait="ExternalPoolLeaseControl",
            target_symbol="ExternalPoolLeaseControl::poison",
        )
        selector = BehaviorSelector(
            "crates/druid-rdbc/tests/evidence.rs", "typed_trace_behavior"
        )
        source = r'''
fn task6_assert_typed_operation_trace<T: PartialEq>(
    _target: &'static str,
    actual: T,
    expected: T,
) {
    assert_eq!(actual, expected);
}

#[test]
fn typed_trace_behavior() {
    let actual = 1;
    task6_assert_typed_operation_trace(
        "canonical::ExternalPoolLeaseControl::poison",
        actual,
        actual,
    );
}
'''
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / selector.source_path
            target.parent.mkdir(parents=True)
            target.write_text(source, encoding="utf-8")
            with self.assertRaisesRegex(VerificationError, "observable assertion"):
                _verify_selector_contract(root, selector, row)

    def test_candidate_selection_ignores_filename_and_requires_target_receiver(self) -> None:
        """BASE filename family 不参与 C2 candidate selection。"""

        row = self.rows()[0]
        selector = row.behavior_evidence
        index = (
            MethodCallEvidence(
                base_identity="tests/result_set_wrong_family.rs#required",
                method_call=row.method,
                base_call_count=1,
                replacement_selector=selector,
                disposition="MIGRATED_EXACT",
                behavioral_theme="method_semantics",
            ),
        )
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            evidence = root / "crates/druid-rdbc/tests"
            evidence.mkdir(parents=True)
            (evidence / "evidence.rs").write_text(
                EVIDENCE_SOURCE, encoding="utf-8"
            )
            self.assertEqual(
                (parse_behavior_selector(selector),),
                behavior_candidates(row, index, root),
            )

    def test_rejects_non_executed_function_and_dead_parameter_case(self) -> None:
        """Source-listed function/case without runtime completion cannot satisfy evidence。"""

        root = Path("/tmp/task6-method-evidence-self-test")

        def completed(output: str) -> subprocess.CompletedProcess[str]:
            return subprocess.CompletedProcess([], 0, stdout=output, stderr="")

        exact = BehaviorSelector(
            "crates/druid-rdbc/tests/evidence.rs", "required_behavior"
        )
        with self.assertRaisesRegex(VerificationError, "not executed"):
            execute_behavior_evidence(
                root,
                (exact,),
                runner=lambda *args, **kwargs: completed("running 0 tests\n"),
            )

        case = BehaviorSelector(
            "crates/druid-rdbc/tests/evidence.rs", "required_behavior", "case_one"
        )
        function_only = "test required_behavior ... ok\n"
        with self.assertRaisesRegex(VerificationError, "not executed at runtime"):
            execute_behavior_evidence(
                root,
                (case,),
                runner=lambda *args, **kwargs: completed(function_only),
            )
        self.assertEqual(
            (1, 1, 1),
            execute_behavior_evidence(
                root,
                (case,),
                runner=lambda *args, **kwargs: completed(
                    function_only
                    + "TASK6_CASE_DIGEST=required_behavior:case_one\n"
                ),
            ),
        )

    def test_accepts_exact_crate_private_unit_test_module_and_rejects_wrong_module(self) -> None:
        """C2 `--lib` selector 必须同时绑定 source path 与完整 module test name。"""

        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source_path = root / "crates/druid/src/core/example.rs"
            source_path.parent.mkdir(parents=True)
            source_path.write_text(
                "#[cfg(test)]\nmod tests {\n#[test]\nfn exact_behavior() {}\n}\n",
                encoding="utf-8",
            )
            selector = parse_behavior_selector(
                "crates/druid/src/core/example.rs#"
                "core::example::tests::exact_behavior"
            )
            self.assertIsNone(selector.case_id)
            _verify_selector_source(root, selector)
            wrong = BehaviorSelector(
                selector.source_path,
                "core::other::tests::exact_behavior",
            )
            with self.assertRaisesRegex(VerificationError, "module"):
                _verify_selector_source(root, wrong)

    def test_runtime_digest_stdout_does_not_interrupt_success_marker(self) -> None:
        """使用真实 libtest 输出形态，同时严格证明成功函数与参数案例。"""

        selector = BehaviorSelector(
            "crates/druid-rdbc/tests/evidence.rs", "required_behavior", "case_one"
        )

        def runner(command: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
            self.assertIn("--all-features", command)
            self.assertIn("--locked", command)
            self.assertIn("--offline", command)
            if "--show-output" in command:
                output = (
                    "test required_behavior ... ok\n\nsuccesses:\n"
                    "---- required_behavior stdout ----\n"
                    "TASK6_CASE_DIGEST=required_behavior:case_one\n"
                )
            else:
                output = (
                    "test required_behavior ... "
                    "TASK6_CASE_DIGEST=required_behavior:case_one\nok\n"
                )
            return subprocess.CompletedProcess(command, 0, stdout=output, stderr="")

        self.assertEqual(
            (1, 1, 1),
            execute_behavior_evidence(Path("/tmp/method-runtime"), (selector,), runner=runner),
        )

    def test_runtime_routes_registered_unit_evidence_and_requires_exact_module(self) -> None:
        """已登记的两个包私有测试必须实际跑完整lib并证明精确模块身份。"""

        selectors = (
            BehaviorSelector(
                "crates/druid-wrapper/src/sqlx/external_pool_connection_control/lease_complete_tests.rs",
                "sqlx::external_pool_connection_control::lease_complete_tests::task6_lease_delegates_complete_spi",
            ),
            BehaviorSelector(
                "crates/druid/src/core/connection_defaults/complete_state_tests.rs",
                "core::connection_defaults::complete_state_tests::task6_connection_defaults_capture_compare_and_reset_complete_state",
            ),
        )
        for selector in selectors:
            with self.subTest(source_path=selector.source_path):
                def runner(command: list[str], **kwargs: object) -> subprocess.CompletedProcess[str]:
                    self.assertIn("--lib", command)
                    self.assertNotIn("--test", command)
                    self.assertIn("--all-features", command)
                    return subprocess.CompletedProcess(
                        command, 0, stdout=f"test {selector.function} ... ok\n", stderr=""
                    )

                self.assertEqual(
                    (1, 1, 1),
                    execute_behavior_evidence(Path("/tmp/unit-runtime"), (selector,), runner=runner),
                )
                for output in (
                    "running 0 tests\n",
                    f"test {selector.function.rsplit('::', 1)[-1]} ... ok\n",
                    f"test wrong::{selector.function} ... ok\n",
                ):
                    with self.assertRaisesRegex(VerificationError, "not executed"):
                        execute_behavior_evidence(
                            Path("/tmp/unit-runtime"), (selector,),
                            runner=lambda *args, _output=output, **kwargs: subprocess.CompletedProcess(
                                [], 0, stdout=_output, stderr=""
                            ),
                        )
                for invalid in (
                    replace(selector, source_path="crates/druid/src/core/not_registered.rs"),
                    replace(selector, function="wrong::" + selector.function),
                ):
                    with self.assertRaises(VerificationError):
                        execute_behavior_evidence(
                            Path("/tmp/unit-runtime"), (invalid,),
                            runner=lambda *args, **kwargs: self.fail("unregistered evidence must not run Cargo"),
                        )

    def test_summary_does_not_count_qualified_unit_modules_as_parameter_cases(self) -> None:
        """模块路径中的双冒号不能被统计为参数化案例。"""

        unit = replace(
            self.rows()[0],
            behavior_evidence=(
                "crates/druid-wrapper/src/sqlx/external_pool_connection_control/lease_complete_tests.rs#"
                "sqlx::external_pool_connection_control::lease_complete_tests::task6_lease_delegates_complete_spi"
            ),
        )
        parameterized = replace(
            self.rows()[1],
            behavior_evidence="crates/druid-rdbc/tests/evidence.rs#required_behavior::case_one",
        )
        selectors = tuple(parse_behavior_selector(row.behavior_evidence) for row in (unit, parameterized))
        summary = _summary((unit, parameterized), render_rows((unit, parameterized)), selectors, None)
        self.assertIn("TASK6_METHOD_EVIDENCE_PARAMETERIZED_ROWS=1\n", summary)
        self.assertIn("TASK6_METHOD_EVIDENCE_EXECUTED=no", summary)

    def test_rejects_c2_structural_only_or_failed_runtime_proof(self) -> None:
        """C2 selector 必须消费 authoritative 1283-row executed=yes 结果。"""

        def result(returncode: int, output: str) -> subprocess.CompletedProcess[str]:
            return subprocess.CompletedProcess([], returncode, stdout=output, stderr="")

        root = Path("/tmp/task6-c2-runtime-self-test")
        for completed in (
            result(1, "FAIL"),
            result(
                0,
                "PASS task6 deleted-test parity: files=79 base_tests=1283 "
                "migrated_exact=1065 parameterized=218 executed=no",
            ),
        ):
            with self.assertRaises(VerificationError):
                verify_c2_runtime_evidence(
                    root, runner=lambda *args, _result=completed, **kwargs: _result
                )
        verify_c2_runtime_evidence(
            root,
            runner=lambda *args, **kwargs: result(
                0,
                "PASS task6 deleted-test parity: files=79 base_tests=1283 "
                "migrated_exact=1065 parameterized=218 executed=yes",
            ),
        )

    def test_generated_behavior_matrix_check_is_authoritative(self) -> None:
        """Renderer drift/failure 不能被 method verifier 静默忽略。"""

        def result(returncode: int, output: str) -> subprocess.CompletedProcess[str]:
            return subprocess.CompletedProcess([], returncode, stdout=output, stderr="")

        root = Path("/tmp/task6-generated-method-matrix-self-test")
        with self.assertRaises(VerificationError):
            verify_generated_behavior_matrix(
                root, runner=lambda *args, **kwargs: result(1, "drift")
            )
        verify_generated_behavior_matrix(
            root,
            runner=lambda *args, **kwargs: result(
                0, "PASS generated method behavior matrix: files=14"
            ),
        )

    def rows(self) -> tuple[MethodRow, ...]:
        methods = parse_direct_trait_methods(TRAIT_SOURCE, "PhysicalExample")
        target_methods = {
            method.name: method
            for method in parse_direct_trait_methods(TARGET_SOURCE, "Example")
        }
        target_path = "src/example.rs"
        return tuple(
            MethodRow(
                source_base="base-sha",
                source_object="PhysicalExample",
                source_trait_path="src/physical_example.rs",
                method=method.name,
                source_signature=method.signature,
                source_signature_sha256=method.signature_sha256,
                base_default_kind=method.default_kind,
                base_override_paths="src/physical_example.rs" if method.name == "required" else "-",
                base_call_paths="-",
                disposition="MERGED_EXACT",
                target_entry=f"canonical::Example::{method.name}",
                target_path=target_path,
                target_symbol=f"Example::{method.name}",
                conversion="IDENTITY",
                approval_ref="-",
                behavior_evidence=(
                    "crates/druid-rdbc/tests/evidence.rs#"
                    f"{method.name}_behavior"
                ),
                notes="self-test",
                target_trait="Example",
                target_signature=target_methods[method.name].signature,
                target_signature_sha256=target_methods[method.name].signature_sha256,
                target_ufcs=f"canonical::Example::{method.name}",
            )
            for method in methods
        )

    def verify(self, rows: tuple[MethodRow, ...]) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "src"
            source.mkdir()
            evidence = root / "crates/druid-rdbc/tests"
            evidence.mkdir(parents=True)
            (source / "physical_example.rs").write_text(TRAIT_SOURCE, encoding="utf-8")
            (source / "example.rs").write_text(TARGET_SOURCE, encoding="utf-8")
            (evidence / "evidence.rs").write_text(EVIDENCE_SOURCE, encoding="utf-8")
            (evidence / "wrong.rs").write_text(EVIDENCE_SOURCE, encoding="utf-8")
            verify_rows(
                rows,
                repository_root=root,
                source_base="base-sha",
                base_sources={"src/physical_example.rs": TRAIT_SOURCE},
                trait_specs={"PhysicalExample": "src/physical_example.rs"},
                method_call_index=self.method_call_index(),
            )

    @staticmethod
    def method_call_index() -> tuple[MethodCallEvidence, ...]:
        return tuple(
            MethodCallEvidence(
                base_identity=f"tests/physical_example_test.rs#{method}",
                method_call=method,
                base_call_count=1,
                replacement_selector=(
                    "crates/druid-rdbc/tests/evidence.rs#" f"{method}_behavior"
                ),
                disposition="MIGRATED_EXACT",
                behavioral_theme="method_semantics",
            )
            for method in ("required", "unsupported", "constant")
        )


if __name__ == "__main__":
    unittest.main()
