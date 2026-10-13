"""公开入口编译门禁的真实 rustc/CLI 合同测试。"""

import csv
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

from verify_jdbc42_semantic_inventory import SEMANTIC_HEADER
import verify_jdbc42_public_entries as verifier


SCRIPT = Path(__file__).with_name("verify_jdbc42_public_entries.py")
RUSTC = Path(os.environ.get(
    "RDBC_PROBE_TEST_RUSTC",
    "C:/Users/hnxyh/.rustup/toolchains/1.97.1-x86_64-pc-windows-msvc/bin/rustc.exe",
))


class PublicEntryCliTest(unittest.TestCase):
    """真实编译可区分trait、具体别名和缺失入口，不能只检查import。"""

    @classmethod
    def setUpClass(cls):
        cls.workspace = tempfile.TemporaryDirectory(prefix="rdbc_public_entry_")
        cls.root = Path(cls.workspace.name)
        source = cls.root / "fixture.rs"
        source.write_text(
            "pub trait ActualTrait {}\n"
            "pub struct Concrete;\n"
            "pub type Alias = Concrete;\n"
            "pub mod constants {pub const VALUE: i32 = 7;}\n"
            "pub mod kinds {\n"
            "    pub enum Kind { General }\n"
            "    impl Kind { pub const CanonicalAlias: Self = Self::General; }\n"
            "}\n",
            encoding="utf-8", newline="\n",
        )
        cls.artifact = cls.root / "libdruid_rdbc.rlib"
        result = subprocess.run(
            [str(RUSTC), "--edition=2021", "--crate-name", "druid_rdbc",
             "--crate-type=rlib", str(source), "-o", str(cls.artifact)],
            capture_output=True, timeout=60,
        )
        if result.returncode:
            raise AssertionError(result.stderr.decode("utf-8", errors="replace"))

    @classmethod
    def tearDownClass(cls):
        cls.workspace.cleanup()

    def invoke(self, entries):
        folder = Path(tempfile.mkdtemp(dir=self.root))
        semantic = folder / "semantic.tsv"
        with semantic.open("w", encoding="utf-8", newline="") as stream:
            writer = csv.writer(stream, delimiter="\t", lineterminator="\n")
            writer.writerow(SEMANTIC_HEADER.split("\t"))
            for index, (shape, target) in enumerate(entries):
                row = ["-"] * 19
                row[0] = f"java:test.Type{index}#TYPE"
                row[1] = f"test.Type{index}"
                row[3], row[5] = shape, target
                writer.writerow(row)
        output = folder / "output"
        command = [sys.executable, str(SCRIPT), "--semantic", str(semantic),
                   "--rustc", str(RUSTC), "--artifact", str(self.artifact),
                   "--dependency-dir", str(self.root), "--output-dir", str(output)]
        result = subprocess.run(command, capture_output=True, timeout=60)
        receipt = output / "summary.json"
        payload = json.loads(receipt.read_text(encoding="utf-8")) if receipt.exists() else None
        return result, payload, output, command

    def test_real_trait_bound_passes(self):
        result, summary, _, _ = self.invoke([("TRAIT", "druid_rdbc::ActualTrait")])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((summary["passed"], summary["failed"]), (1, 0))
        self.assertFalse(summary["full_shape_or_behavior_certified"])

    def test_concrete_alias_cannot_impersonate_trait(self):
        result, summary, output, _ = self.invoke([("TRAIT", "druid_rdbc::Alias")])
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertEqual(summary["failed"], 1)
        self.assertIn("E0404", (output / summary["results"][0]["log"]).read_text())

    def test_collects_failure_then_subsequent_success(self):
        result, summary, _, _ = self.invoke([
            ("DTO", "druid_rdbc::Missing"), ("DTO", "druid_rdbc::Concrete"),
        ])
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertEqual([x["status"] for x in summary["results"]], ["FAIL", "PASS"])

    def test_callback_requires_trait(self):
        result, summary, _, _ = self.invoke([("CALLBACK", "druid_rdbc::Concrete")])
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertEqual(summary["failed"], 1)

    def test_module_import_is_explicitly_resolution_only(self):
        result, summary, _, _ = self.invoke([("MODULE", "druid_rdbc::constants")])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(summary["results"][0]["scope"], "public import resolution only")

    def test_enum_type_level_kind_keeps_import_probe(self):
        result, summary, _, _ = self.invoke([("ENUM_KIND", "druid_rdbc::Concrete")])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(summary["results"][0]["scope"], "public import resolution only")

    def test_enum_member_variant_resolves_in_value_position(self):
        result, summary, _, _ = self.invoke([("ENUM_KIND", "druid_rdbc::kinds::Kind::General")])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((summary["passed"], summary["failed"]), (1, 0))
        self.assertEqual(
            summary["results"][0]["scope"],
            "public member value resolves; variants and associated consts",
        )

    def test_enum_associated_const_member_resolves_in_value_position(self):
        result, summary, _, _ = self.invoke([("ENUM_KIND", "druid_rdbc::kinds::Kind::CanonicalAlias")])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual((summary["passed"], summary["failed"]), (1, 0))

    def test_enum_missing_member_value_is_rejected(self):
        result, summary, output, _ = self.invoke([("ENUM_KIND", "druid_rdbc::kinds::Kind::Missing")])
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertEqual(summary["failed"], 1)
        # 类型可解析而成员缺失时，rustc 对枚举成员报 E0599。
        self.assertIn("E0599", (output / summary["results"][0]["log"]).read_text())

    def test_host_exclusion_is_not_rust_certification(self):
        result, summary, _, _ = self.invoke([("EXCLUDED_RUNTIME", "host::process_security_policy")])
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(summary["not_rust_target"], 1)
        self.assertEqual(summary["passed"], 0)

    def test_invalid_rust_target_is_rejected_before_compilation(self):
        result, summary, _, _ = self.invoke([("TRAIT", "druid_rdbc::ActualTrait; panic!()")])
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn(b"invalid canonical Rust entry", result.stderr)
        self.assertIsNone(summary)

    def test_unknown_shape_cannot_be_silently_skipped(self):
        result, summary, _, _ = self.invoke([("UNKNOWN", "druid_rdbc::ActualTrait")])
        self.assertEqual(result.returncode, 1, result.stderr)
        self.assertIn(b"unsupported rust_shape", result.stderr)
        self.assertIsNone(summary)

    def test_existing_evidence_directory_is_preserved(self):
        first, _, output, command = self.invoke([("TRAIT", "druid_rdbc::ActualTrait")])
        self.assertEqual(first.returncode, 0, first.stderr)
        before = (output / "summary.json").read_bytes()
        second = subprocess.run(command, capture_output=True, timeout=60)
        self.assertEqual(second.returncode, 1, second.stderr)
        self.assertIn(b"output directory already exists", second.stderr)
        self.assertEqual((output / "summary.json").read_bytes(), before)

    def test_inventory_changed_after_read_cannot_receive_pass_receipt(self):
        first, _, output, command = self.invoke([("TRAIT", "druid_rdbc::ActualTrait")])
        self.assertEqual(first.returncode, 0, first.stderr)
        semantic = Path(command[command.index("--semantic") + 1])
        original = semantic.read_bytes()
        parse = verifier.parse_semantic_payload

        def replace_after_actual_parse(payload):
            rows = parse(payload)
            semantic.write_bytes(original.replace(b"ActualTrait", b"Concrete"))
            return rows

        arguments = argparse.Namespace(
            semantic=semantic, artifact=self.artifact, rustc=RUSTC,
            dependency_dir=self.root, output_dir=output.with_name("race_output"),
        )
        # 只控制外部文件变化时机；解析器和rustc仍执行真实逻辑。
        with patch.object(verifier, "parse_semantic_payload", side_effect=replace_after_actual_parse):
            summary = verifier.verify(arguments)
        self.assertEqual(summary["passed"], 1)
        self.assertEqual(summary["status"], "FAIL_PUBLIC_ENTRIES")
        self.assertIn(str(semantic.resolve()), summary["changed_inputs"])
        self.assertEqual(summary["input_sha256"][str(semantic.resolve())], hashlib.sha256(original).hexdigest())


if __name__ == "__main__":
    unittest.main()
