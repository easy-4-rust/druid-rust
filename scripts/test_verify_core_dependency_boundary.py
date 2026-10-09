#!/usr/bin/env python3
"""当前核心 crate 依赖门禁的负向合同测试。"""

import contextlib
import io
import subprocess
import unittest
from pathlib import Path
from unittest.mock import patch

import verify_core_dependency_boundary as verifier


class CoreDependencyBoundaryTest(unittest.TestCase):
    def invoke(self, stdout, returncode=0, stderr=""):
        output = io.StringIO()
        errors = io.StringIO()
        result = subprocess.CompletedProcess([], returncode, stdout, stderr)
        with patch.object(verifier.subprocess, "run", return_value=result) as run:
            with contextlib.redirect_stdout(output), contextlib.redirect_stderr(errors):
                status = verifier.main()
        return status, output.getvalue(), errors.getvalue(), run

    def test_checks_actual_product_from_repository_root(self):
        status, _, _, run = self.invoke("druid v0.0.0-design\n└── druid-rdbc v0.0.0-design\n")
        self.assertEqual(status, 0)
        command = run.call_args.args[0]
        self.assertEqual(command[command.index("-p") + 1], "druid")
        self.assertEqual(command[command.index("-e") + 1], "normal")
        self.assertIn("--locked", command)
        self.assertEqual(Path(run.call_args.kwargs["cwd"]), Path(__file__).resolve().parent.parent)

    def test_rejects_each_forbidden_package_at_nested_depth(self):
        # 固定原架构禁止集合，避免实现通过缩减集合获得 GREEN。
        forbidden = {"toasty", "toasty-core", "sqlx", "rbdc", "duckdb", "libsql",
                     "bb8", "deadpool", "prometheus", "reqwest", "tonic", "axum", "topcoat"}
        self.assertEqual(verifier.FORBIDDEN, forbidden)
        for name in sorted(forbidden):
            with self.subTest(package=name):
                status, _, errors, _ = self.invoke(
                    f"druid v0.0.0-design\n├── druid-rdbc v0.0.0-design\n│   └── {name} v1.0.0\n")
                self.assertEqual(status, 1)
                self.assertIn(name, errors)

    def test_duplicate_forbidden_dependency_cannot_hide_violation(self):
        status, _, errors, _ = self.invoke(
            "druid v0.0.0-design\n├── sqlx v0.8.0\n└── sqlx v0.8.0 (*)\n")
        self.assertEqual(status, 1)
        self.assertIn("['sqlx']", errors)

    def test_cargo_failure_is_not_reported_as_clean_graph(self):
        status, output, errors, _ = self.invoke("", 101, "dependency resolution failed")
        self.assertEqual(status, 1)
        self.assertNotIn("PASS", output)
        self.assertIn("dependency resolution failed", errors)

    def test_empty_or_wrong_root_output_is_not_clean_graph_evidence(self):
        for tree in ["", "druid-wrapper v0.0.0-design\n└── sqlx v0.8.0\n",
                     "unrelated v1.0.0\n└── tokio v1.53.1\n"]:
            with self.subTest(tree=tree):
                status, output, _, _ = self.invoke(tree)
                self.assertEqual(status, 1)
                self.assertNotIn("PASS", output)


if __name__ == "__main__":
    unittest.main()
