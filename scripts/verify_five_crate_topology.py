import argparse
import json
import subprocess

EXPECTED = ["druid", "druid-admin", "druid-core", "druid-metrics", "druid-wrapper"]
parser = argparse.ArgumentParser()
parser.add_argument("--require-binary", action="append", default=[])
args = parser.parse_args()
metadata = json.loads(subprocess.check_output(
    ["cargo", "metadata", "--format-version", "1", "--no-deps"], text=True
))
packages = sorted(package["name"] for package in metadata["packages"])
if packages != EXPECTED:
    raise SystemExit(f"workspace packages {packages!r} != {EXPECTED!r}")
for requirement in args.require_binary:
    binary, expected_package = requirement.split("=", 1)
    owners = [
        package["name"]
        for package in metadata["packages"]
        if any(binary == target["name"] and "bin" in target["kind"] for target in package["targets"])
    ]
    if owners != [expected_package]:
        raise SystemExit(f"binary {binary!r} owners {owners!r} != {[expected_package]!r}")
