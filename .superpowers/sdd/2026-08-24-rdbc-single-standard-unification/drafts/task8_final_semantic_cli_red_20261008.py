"""Task8 final语义CLI的RED合同；只调用真实脚本，不修改冻结输入。"""
import subprocess
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
SCRIPT = ROOT / "scripts/verify_jdbc42_semantic_inventory.py"

class FinalSemanticCliContractTest(unittest.TestCase):
    def run_cli(self, *arguments):
        return subprocess.run([sys.executable, "-X", "utf8", str(SCRIPT), *arguments], cwd=ROOT, capture_output=True, text=True, encoding="utf-8")

    def test_help_exposes_required_final_evidence_mode(self):
        result = self.run_cli("--help")
        self.assertEqual(0, result.returncode)
        self.assertIn("--require-final-evidence", result.stdout)

    def test_current_planned_inventory_is_semantically_rejected_not_usage_error(self):
        result = self.run_cli("--require-final-evidence")
        self.assertEqual(1, result.returncode, result.stdout + result.stderr)
        self.assertIn("ERROR:", result.stdout + result.stderr)
        self.assertIn("planned", (result.stdout + result.stderr).lower())

    def test_planned_phase_cannot_bypass_required_final_evidence(self):
        """显式 planned 不能降级 final 合同，当前账本应被语义拒绝。"""
        result = self.run_cli("--require-final-evidence", "--phase", "planned")
        self.assertEqual(1, result.returncode, result.stdout + result.stderr)
        self.assertIn("ERROR:", result.stderr)
        self.assertIn("planned", result.stderr.lower())

    def test_adapter_phase_cannot_bypass_required_final_evidence(self):
        """Task4 阶段不能绕过 Task8 的计划态禁令。"""
        result = self.run_cli("--require-final-evidence", "--phase", "adapter-live")
        self.assertEqual(1, result.returncode, result.stdout + result.stderr)
        self.assertIn("ERROR:", result.stderr)
        self.assertIn("planned", result.stderr.lower())

    def test_logical_phase_cannot_bypass_required_final_evidence(self):
        """Task5 阶段不能替代完整最终验收。"""
        result = self.run_cli("--require-final-evidence", "--phase", "logical-live")
        self.assertEqual(1, result.returncode, result.stdout + result.stderr)
        self.assertIn("ERROR:", result.stderr)
        self.assertIn("planned", result.stderr.lower())

if __name__ == "__main__":
    unittest.main()
