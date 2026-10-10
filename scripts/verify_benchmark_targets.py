"""Verify that release-performance benchmark commands name real Cargo targets."""

import json
from pathlib import Path
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
REQUIRED = {"pool", "filter_chain", "wall", "adapter", "runtime"}


def main() -> int:
    result = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1", "--offline", "--locked"],
        cwd=ROOT,
        text=True,
        capture_output=True,
    )
    if result.returncode != 0:
        print(result.stderr, file=sys.stderr)
        return result.returncode

    packages = json.loads(result.stdout)["packages"]
    benchmark = next((package for package in packages if package["name"] == "druid-bench"), None)
    if benchmark is None:
        print("workspace has no druid-bench package", file=sys.stderr)
        return 1
    targets = {
        target["name"] for target in benchmark["targets"] if "bench" in target["kind"]
    }
    missing = sorted(REQUIRED - targets)
    if missing:
        print("missing release benchmark targets: " + ", ".join(missing), file=sys.stderr)
        return 1
    print(f"release benchmark targets verified: {', '.join(sorted(REQUIRED))}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
