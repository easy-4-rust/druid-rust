"""Regression tests for the single RSA advisory exception."""

import unittest

from verify_rsa_waiver_scope import waiver_scope_errors


RSA = "registry+https://github.com/rust-lang/crates.io-index#rsa@0.9.10"
DRUID = "path+file:///workspace/crates/druid#0.0.0-design"
SQLX = "registry+https://github.com/rust-lang/crates.io-index#sqlx-mysql@0.8.6"


def metadata(*, extra_consumer=False, rsa_version="0.9.10"):
    rsa = RSA.replace("0.9.10", rsa_version)
    packages = [
        {"id": rsa, "name": "rsa", "version": rsa_version},
        {"id": DRUID, "name": "druid", "version": "0.0.0-design"},
        {"id": SQLX, "name": "sqlx-mysql", "version": "0.8.6"},
    ]
    nodes = [
        {"id": DRUID, "deps": [{"name": "rsa", "pkg": rsa}]},
        {"id": SQLX, "deps": [{"name": "rsa", "pkg": rsa}]},
    ]
    if extra_consumer:
        packages.append({"id": "new-consumer", "name": "new-consumer", "version": "1.0.0"})
        nodes.append({"id": "new-consumer", "deps": [{"name": "rsa", "pkg": rsa}]})
    return {"packages": packages, "resolve": {"nodes": nodes}}


class RsaWaiverScopeTests(unittest.TestCase):
    def test_expected_scope_passes(self):
        self.assertEqual([], waiver_scope_errors(metadata()))

    def test_new_consumer_fails(self):
        self.assertIn("new-consumer", " ".join(waiver_scope_errors(metadata(extra_consumer=True))))

    def test_version_change_fails(self):
        self.assertIn("0.9.11", " ".join(waiver_scope_errors(metadata(rsa_version="0.9.11"))))

    def test_missing_rsa_fails_as_stale_waiver(self):
        self.assertTrue(waiver_scope_errors({"packages": [], "resolve": {"nodes": []}}))


if __name__ == "__main__":
    unittest.main()
