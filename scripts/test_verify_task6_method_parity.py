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


SCRIPTS = Path(__file__).resolve().parent
sys.path.insert(0, str(SCRIPTS))

from verify_task6_method_parity import (  # noqa: E402
    BehaviorSelector,
    METHOD_HEADER,
    MethodCallEvidence,
    MethodRow,
    VerificationError,
    execute_behavior_evidence,
    parse_behavior_selector,
    parse_direct_trait_methods,
    parse_rows,
    render_rows,
    verify_c2_runtime_evidence,
    verify_rows,
    _verify_selector_source,
)


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
fn required_behavior() {}

#[test]
fn unsupported_behavior() {}

#[test]
fn constant_behavior() {}
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
        )
        self.verify((rows[0], unsupported_break, rows[2]))
        with self.assertRaises(VerificationError):
            self.verify((rows[0], replace(unsupported_break, approval_ref="-"), rows[2]))

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

    def rows(self) -> tuple[MethodRow, ...]:
        methods = parse_direct_trait_methods(TRAIT_SOURCE, "PhysicalExample")
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
