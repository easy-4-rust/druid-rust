#!/usr/bin/env python3
"""Regression tests for the strict PR SemVer scan."""

from __future__ import annotations

import subprocess
import unittest

from verify_pr_semver import CRATES, check_pr_semver

BASELINE = "a" * 40


class PrSemverTest(unittest.TestCase):
    def test_all_crates_pass_against_exact_base(self) -> None:
        calls = []

        def runner(command, *, check):
            calls.append((command, check))
            return subprocess.CompletedProcess(command, 0)

        self.assertEqual(check_pr_semver(BASELINE, runner), 0)
        self.assertEqual([call[0][3] for call in calls], list(CRATES))
        for command, check in calls:
            self.assertFalse(check)
            self.assertEqual(command[command.index("--baseline-rev") + 1], BASELINE)
            self.assertIn("--release-type", command)
            self.assertIn("patch", command)
            self.assertIn("--all-features", command)

    def test_new_breaking_api_fails(self) -> None:
        calls = []

        def runner(command, *, check):
            calls.append(command)
            return subprocess.CompletedProcess(command, 100 if len(calls) == 2 else 0)

        self.assertEqual(check_pr_semver(BASELINE, runner), 1)
        self.assertEqual(len(calls), 3)

    def test_execution_failure_fails(self) -> None:
        calls = []

        def runner(command, *, check):
            calls.append(command)
            return subprocess.CompletedProcess(command, 101)

        self.assertEqual(check_pr_semver(BASELINE, runner), 1)
        self.assertEqual(len(calls), 3)

    def test_missing_executable_fails(self) -> None:
        def runner(command, *, check):
            raise OSError("cargo unavailable")

        self.assertEqual(check_pr_semver(BASELINE, runner), 1)

    def test_baseline_must_be_immutable_sha(self) -> None:
        self.assertEqual(check_pr_semver("origin/main"), 1)


if __name__ == "__main__":
    unittest.main()
