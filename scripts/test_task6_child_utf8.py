#!/usr/bin/env python3
"""以真实子进程验证 C1 的 C2 调用不会丢失 UTF-8 模式。"""

import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from verify_task6_method_parity import (
    VerificationError, verify_c2_runtime_evidence, verify_c2_source_evidence,
)


class ChildUtf8Test(unittest.TestCase):
    def test_runtime_and_structural_children_override_legacy_mode(self):
        for verify in [verify_c2_runtime_evidence, verify_c2_source_evidence]:
            with self.subTest(verifier=verify.__name__), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                (root / "scripts").mkdir()
                (root / "scripts/verify_task6_deleted_test_parity.py").write_text(
                    "import sys\n"
                    "from pathlib import Path\n"
                    "Path('mode.txt').write_text(str(sys.flags.utf8_mode))\n"
                    "if not sys.flags.utf8_mode: sys.exit(3)\n"
                    "print('中文合同输出 — 保留 UTF-8')\n"
                    "mode = 'no' if '--no-execute' in sys.argv else 'yes'\n"
                    "print('PASS task6 deleted-test parity: files=67 base_tests=1283 migrated_exact=1146 parameterized=137 executed=' + mode)\n",
                    encoding="utf-8",
                )
                failure = None
                with patch.dict(os.environ, {"PYTHONUTF8": "0"}):
                    try:
                        verify(root)
                    except VerificationError as error:
                        failure = error
                self.assertEqual((root / "mode.txt").read_text(), "1")
                self.assertIsNone(failure)


if __name__ == "__main__":
    unittest.main()
