"""防止静态匹配再次被误报为真实缺失或验收通过。"""
import unittest
from reconcile_migration_evidence import classify_candidates


class CandidateEvidenceTests(unittest.TestCase):
    def test_absent_name_is_unresolved_not_missing_functionality(self):
        self.assertEqual(classify_candidates("SQLExpr", {}, {}), ("UNRESOLVED_MAPPING", []))

    def test_declaration_is_only_a_candidate(self):
        self.assertEqual(classify_candidates("SQLUtils", {"sqlutils": {"sql_utils.rs"}}, {}),
                         ("DECLARATION_CANDIDATE", ["sql_utils.rs"]))

    def test_reference_is_not_implementation(self):
        self.assertEqual(classify_candidates("Filter", {}, {"filter": {"comment.rs"}}),
                         ("REFERENCE_CANDIDATE", ["comment.rs"]))

    def test_ambiguous_candidates_are_all_preserved(self):
        self.assertEqual(classify_candidates("BasicDataSource", {"basicdatasource": {"dbcp.rs", "dbcp2.rs"}}, {}),
                         ("DECLARATION_CANDIDATE", ["dbcp.rs", "dbcp2.rs"]))


if __name__ == "__main__":
    unittest.main()
