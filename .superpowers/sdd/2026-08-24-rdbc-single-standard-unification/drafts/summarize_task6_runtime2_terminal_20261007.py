"""保存C2真实失败终态，不猜测隐藏的成功目标或测试总计。"""
from pathlib import Path
import hashlib
import json

root = Path.cwd()
evidence = root / ".superpowers/sdd/2026-08-24-rdbc-single-standard-unification"
stem = "task6-complete-stat-close-error-runtime2-2026-10-07"
freeze = json.loads((evidence / "task6-complete-stat-close-error-runtime2-inputs-2026-10-07.json").read_text(encoding="utf-8"))
assert len(freeze["inputs"]) == 1090
assert all(hashlib.sha256((root / n).read_bytes()).hexdigest() == value for n, value in freeze["inputs"].items())
log = evidence / (stem + ".log")
output = log.read_text(encoding="utf-8-sig")
assert "failed to download `arrow v58.4.0`" in output and "--offline was specified" in output
result = {"state": "TERMINAL_FAILED", "exit_code": 1, "session_id": 33663,
          "scope": "full default C2 verifier, all-features locked offline exact function and case digest proof",
          "unchanged_frozen_inputs": 1090, "aggregate_test_counts": None,
          "failed_target": "druid-wrapper/__lib__", "failure": "locked dependency cache missing arrow 58.4.0",
          "log": log.relative_to(root).as_posix(), "log_sha256": hashlib.sha256(log.read_bytes()).hexdigest()}
destination = evidence / (stem + ".summary.json")
assert not destination.exists()
destination.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
print(json.dumps(result))
