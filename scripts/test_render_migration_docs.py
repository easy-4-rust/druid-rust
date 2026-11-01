"""已核对的迁移结论不得被旧名称匹配台账覆盖。"""
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import render_migration_docs as renderer


class ReviewedDocumentProtectionTests(unittest.TestCase):
    def test_refuses_before_any_output_when_a_reviewed_document_exists(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            templates = root / "_templates"
            templates.mkdir()
            first = root / "a.md"
            reviewed = root / "z.md"
            first.write_text("existing", encoding="utf-8")
            reviewed.write_text("<!-- current-code-audit-start -->\nreviewed", encoding="utf-8")
            (templates / "a.md").write_text("replacement", encoding="utf-8")
            (templates / "z.md").write_text("replacement", encoding="utf-8")
            with patch.object(renderer, "DOCS", root), patch.object(renderer, "TPL", templates):
                with self.assertRaisesRegex(SystemExit, "reviewed"):
                    renderer.main()
            self.assertEqual(first.read_text(encoding="utf-8"), "existing")
            self.assertIn("reviewed", reviewed.read_text(encoding="utf-8"))


if __name__ == "__main__":
    unittest.main()
