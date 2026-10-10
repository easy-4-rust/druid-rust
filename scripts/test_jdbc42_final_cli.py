"""最终语义入口的来源、状态和不可跳过证据合同；合成控制不认证产品。"""

import contextlib
import io
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import verify_jdbc42_semantic_inventory as gate


class FinalCliTest(unittest.TestCase):
    def setUp(self):
        self.root = Path(__file__).resolve().parents[1]
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.semantic = Path(self.temp.name) / "semantic.tsv"
        self.rows = gate.parse_semantic_payload((self.root / gate.DEFAULT_SEMANTIC).read_bytes())

    def complete_state_control(self):
        # 只构造状态组件的正向控制；运行组件另行验证，不能据此签发产品完成结论。
        for row in self.rows:
            row[6] = "DECLARED"
            row[12:18] = ["-", "NOT_APPLICABLE", "-", "NOT_APPLICABLE", "V0_STATIC", "NOT_APPLICABLE"]

    def verify(self, **kwargs):
        self.semantic.write_bytes(gate.canonical_semantic_payload(self.rows))
        return gate.verify_inventory(self.root, self.semantic, gate.DEFAULT_ORACLE,
                                     require_final_evidence=True, **kwargs)

    def test_cli_recognizes_final_flag(self):
        with patch.object(sys, "argv", ["gate", "--require-final-evidence"]):
            self.assertTrue(gate.parse_arguments().require_final_evidence)

    def test_final_rejects_current_planned_inventory(self):
        with self.assertRaisesRegex(gate.VerificationError, "planned|unresolved"):
            self.verify()

    def test_final_source_binding_is_not_replaced_by_promoted_digest(self):
        self.complete_state_control()
        self.rows[0][7] += " FORGED"
        with self.assertRaisesRegex(gate.VerificationError, "source declaration drift"):
            self.verify()

    def test_state_and_source_control_reaches_required_runtime_component(self):
        self.complete_state_control()
        with patch.object(gate, "verify_final_runtime_evidence", create=True) as runtime:
            summary = self.verify()
            runtime.assert_called_once_with(self.root, self.rows)
        self.assertEqual((52, 17, 0), (summary.java_sql, summary.javax_sql, summary.unknown))

    def test_runtime_failure_cannot_be_downgraded_to_static_pass(self):
        self.complete_state_control()
        with patch.object(gate, "verify_final_runtime_evidence", create=True,
                          side_effect=gate.VerificationError("current compiler evidence missing")):
            with self.assertRaisesRegex(gate.VerificationError, "compiler evidence missing"):
                self.verify()

    def test_final_rejects_semantic_mutation_during_runtime(self):
        self.complete_state_control()
        def mutate(*args):
            self.semantic.write_bytes(self.semantic.read_bytes() + b"\n")
        with patch.object(gate, "verify_final_runtime_evidence", create=True, side_effect=mutate):
            with self.assertRaisesRegex(gate.VerificationError, "inventory changed"):
                self.verify()

    def test_final_rejects_oracle_mutation_during_runtime(self):
        self.complete_state_control()
        source = self.root / gate.DEFAULT_ORACLE
        oracle = Path(self.temp.name) / "oracle.json"
        oracle.write_bytes(source.read_bytes())
        self.semantic.write_bytes(gate.canonical_semantic_payload(self.rows))
        def mutate(*args):
            oracle.write_bytes(oracle.read_bytes() + b"\n")
        with patch.object(gate, "verify_final_runtime_evidence", side_effect=mutate):
            with self.assertRaisesRegex(gate.VerificationError, "oracle changed"):
                gate.verify_inventory(self.root, self.semantic, oracle, require_final_evidence=True)

    def test_final_rejects_oracle_swap_during_source_loading(self):
        self.complete_state_control()
        oracle = Path(self.temp.name) / "oracle.json"
        oracle.write_bytes((self.root / gate.DEFAULT_ORACLE).read_bytes())
        self.semantic.write_bytes(gate.canonical_semantic_payload(self.rows))
        original = gate.load_oracle
        def swap(*args):
            records = original(*args)
            oracle.write_bytes(oracle.read_bytes() + b"\n")
            return records
        with patch.object(gate, "load_oracle", side_effect=swap), \
             patch.object(gate, "verify_final_runtime_evidence"):
            with self.assertRaisesRegex(gate.VerificationError, "oracle changed"):
                gate.verify_inventory(self.root, self.semantic, oracle, require_final_evidence=True)

    def test_final_retains_explicit_standard_entry_requirement(self):
        self.complete_state_control()
        row = next(row for row in self.rows if row[1] in gate.TASK3_STANDARD_ENTRIES)
        row[5] = "druid_rdbc::Fake"
        with self.assertRaisesRegex(gate.VerificationError, "canonical entry drift"):
            self.verify(require_standard_entries=True)

    def test_final_disallows_both_legacy_evidence_skip_switches(self):
        self.complete_state_control()
        for flag in ("require_adapter_evidence", "require_logical_evidence"):
            with self.subTest(flag=flag), self.assertRaisesRegex(gate.VerificationError, "cannot skip"):
                self.verify(**{flag: False})

    def test_final_rejects_unknown_shape_and_disposition(self):
        self.complete_state_control()
        for index, value in ((3, "FAKE_SHAPE"), (4, "SILENT_SKIP")):
            original = self.rows[0][index]
            self.rows[0][index] = value
            with self.subTest(index=index), self.assertRaisesRegex(gate.VerificationError, "shape|disposition"):
                self.verify()
            self.rows[0][index] = original

    def test_live_cli_fails_semantically_instead_of_unknown_argument(self):
        result = subprocess.run([sys.executable, "-X", "utf8", str(self.root / "scripts/verify_jdbc42_semantic_inventory.py"),
                                 "--require-final-evidence"], capture_output=True, text=True, encoding="utf-8")
        self.assertEqual(1, result.returncode, result.stderr)
        self.assertRegex(result.stderr, "planned|unresolved")
        self.assertNotIn("JAVA_SQL=", result.stdout)

    def api_shapes(self, text):
        import verify_jdbc42_final_runtime as runtime
        return runtime.verify_api_shapes(self.rows, text)

    def test_api_shape_accepts_declared_kinds_and_resolved_aliases(self):
        self.rows = [["java:test#TYPE", "java.sql.Test", "Test", "DTO", "MATCH", "druid_rdbc::Alias"],
                     ["java:trait#TYPE", "java.sql.Trait", "Trait", "TRAIT", "MATCH", "druid_rdbc::Trait"],
                     ["java:enum#TYPE", "java.sql.Enum", "Enum", "ENUM_KIND", "MATCH", "druid_rdbc::Kind"],
                     ["java:mod#TYPE", "java.sql.Mod", "Mod", "MODULE", "MATCH", "druid_rdbc::module"]]
        self.api_shapes("pub type druid_rdbc::Alias = druid_rdbc::Value\npub struct druid_rdbc::Value\n"
                        "pub trait druid_rdbc::Trait\npub enum druid_rdbc::Kind\npub mod druid_rdbc::module\n")

    def test_api_import_success_does_not_certify_enum_shape(self):
        self.rows = [["java:test#TYPE", "java.sql.Test", "Test", "ENUM_KIND", "MATCH", "druid_rdbc::Kind"]]
        with self.assertRaisesRegex(gate.VerificationError, "kind mismatch"):
            self.api_shapes("pub struct druid_rdbc::Kind\n")

    def test_api_alias_cycle_and_missing_target_fail_closed(self):
        self.rows = [["java:test#TYPE", "java.sql.Test", "Test", "DTO", "MATCH", "druid_rdbc::A"]]
        for text in ("pub type druid_rdbc::A = druid_rdbc::B\npub type druid_rdbc::B = druid_rdbc::A\n",
                     "pub type druid_rdbc::A = druid_rdbc::Missing\n"):
            with self.subTest(text=text), self.assertRaises(gate.VerificationError):
                self.api_shapes(text)

    def test_api_host_exclusion_must_have_exact_disposition_and_target(self):
        self.rows = [["java:test#TYPE", "java.sql.Test", "Test", "EXCLUDED_RUNTIME", "MATCH", "host::process_security_policy"]]
        with self.assertRaisesRegex(gate.VerificationError, "host"):
            self.api_shapes("")

    def test_final_runtime_rejects_pending_resource_evidence(self):
        import verify_jdbc42_final_runtime as runtime
        from types import SimpleNamespace
        with self.assertRaisesRegex(gate.VerificationError, "Task7.*pending"):
            runtime.require_closed_druid_evidence(SimpleNamespace(task6_pending_rows=0, task7_pending_rows=108))

    def test_final_runtime_rejects_pending_compatibility_evidence(self):
        import verify_jdbc42_final_runtime as runtime
        from types import SimpleNamespace
        with self.assertRaisesRegex(gate.VerificationError, "Task6.*pending"):
            runtime.require_closed_druid_evidence(SimpleNamespace(task6_pending_rows=75, task7_pending_rows=0))


class RuntimeOrchestrationTest(unittest.TestCase):
    """验证委托顺序与失败关闭；受控组件输出不构成真实编译/运行认证。"""

    def setUp(self):
        import verify_jdbc42_final_runtime as runtime
        self.runtime = runtime
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "Cargo.toml").write_bytes(b"source-before\n")
        self.rows = [["java:test#TYPE", "java.sql.Test", "Test", "DTO", "MATCH", "druid_rdbc::Value"]]

    def run_control(self, *, api_code=0, pending=0, mutate=False):
        from types import SimpleNamespace
        calls = []
        def cargo(command, **kwargs):
            self.assertEqual(["cargo", "public-api", "-p", "druid-rdbc", "-sss", "--all-features"], command)
            self.assertEqual("0", kwargs["env"]["RUSTUP_AUTO_INSTALL"])
            kwargs["stdout"].write(b"pub struct druid_rdbc::Value\n")
            calls.append("api")
            return SimpleNamespace(returncode=api_code)
        def wrapper(root):
            calls.append("wrapper")
            if mutate:
                (root / "Cargo.toml").write_bytes(b"source-after\n")
        with patch.object(self.runtime.subprocess, "run", side_effect=cargo), \
             patch("verify_no_physical_standard.verify_repository", side_effect=lambda root: calls.append("c1-c2")), \
             patch("verify_rdbc_single_standard_purity.verify_tree", side_effect=lambda root, phase: calls.append(phase)), \
             patch("verify_wrapper_standard_impls.verify_repository", side_effect=wrapper), \
             patch("verify_druid_single_standard.verify_repository", return_value=SimpleNamespace(task6_pending_rows=0, task7_pending_rows=pending)):
            self.runtime.verify_runtime(self.root, self.rows)
        return calls

    def receipt(self):
        import json
        paths = list(self.root.rglob("summary.json"))
        self.assertEqual(1, len(paths))
        return json.loads(paths[0].read_text(encoding="utf-8"))

    def test_required_gate_order_and_success_receipt_control(self):
        self.assertEqual(["api", "final", "c1-c2", "wrapper"], self.run_control())
        self.assertEqual("PASS_FINAL_SEMANTIC_EVIDENCE", self.receipt()["status"])

    def test_compiler_failure_retains_log_and_does_not_run_later_gates(self):
        with self.assertRaisesRegex(gate.VerificationError, "compiler gate failed"):
            self.run_control(api_code=101)
        self.assertEqual([], self.receipt()["completed_gates"])
        self.assertEqual(101, self.receipt()["public_api_exit_code"])

    def test_pending_resource_gate_prevents_success_receipt(self):
        with self.assertRaisesRegex(gate.VerificationError, "Task7.*pending"):
            self.run_control(pending=108)
        self.assertEqual("FAIL_FINAL_SEMANTIC_EVIDENCE", self.receipt()["status"])

    def test_current_input_mutation_prevents_success_receipt(self):
        with self.assertRaisesRegex(gate.VerificationError, "inputs changed"):
            self.run_control(mutate=True)
        self.assertEqual("FAIL_FINAL_SEMANTIC_EVIDENCE", self.receipt()["status"])

    def test_input_freeze_includes_protocol_and_api_evidence(self):
        for relative in ("crates/druid-metrics/proto/metrics.proto", "api/current.txt",
                         "scripts/fixtures/control.json", ".cargo/config.toml"):
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b"input\n")
        frozen = self.runtime.input_hashes(self.root)
        for relative in ("crates/druid-metrics/proto/metrics.proto", "api/current.txt",
                         "scripts/fixtures/control.json", ".cargo/config.toml"):
            self.assertIn(relative, {path.replace("\\", "/") for path in frozen})


if __name__ == "__main__":
    unittest.main()
