"""Verify wrapper driver features own their optional dependency trees."""

import json
from pathlib import Path
import re
import subprocess
import sys


ROOT = Path(__file__).resolve().parents[1]
FORBIDDEN = {"sqlx", "toasty", "toasty-core", "rbdc", "rbs", "bb8", "deadpool"}
FEATURE_DEPS = {
    "sqlx": {"sqlx"},
    "sqlx-bb8": {"bb8"},
    "sqlx-deadpool": {"deadpool"},
    "toasty": {"toasty", "toasty-core"},
    "rbdc": {"rbdc", "rbs"},
}
ISOLATED_FEATURES = {
    "driver-catalog": set(),
    "sqlx": {"sqlx"},
    "toasty": {"toasty", "toasty-core"},
    "rbdc": {"rbdc", "rbs"},
    "sqlx-bb8": {"sqlx", "bb8"},
    "sqlx-deadpool": {"sqlx", "deadpool"},
}
# Toasty 0.9.0 internally depends on deadpool 0.13; this does not enable the
# wrapper's optional deadpool 0.12 adapter. Cargo's feature tree is checked
# alongside the direct optional-dependency ownership below.
ALLOWED_TRANSITIVE = {"toasty": {"deadpool"}}


def normal_dependencies(*features: str) -> tuple[set[str], str | None]:
    command = [
        "cargo", "tree", "-p", "druid-wrapper", "--no-default-features",
        "-e", "normal", "--prefix", "none", "--format", "{p}",
        "--offline", "--locked",
    ]
    if features:
        command.extend(["--features", ",".join(features)])
    result = subprocess.run(command, cwd=ROOT, text=True, capture_output=True)
    if result.returncode != 0:
        return set(), result.stderr.strip()
    names = {
        match.group(1)
        for line in result.stdout.splitlines()
        if (match := re.match(r"^([A-Za-z0-9_-]+) v\d", line))
    }
    return names, None


def main() -> int:
    package_names, tree_error = normal_dependencies()
    if tree_error:
        print(tree_error, file=sys.stderr)
        return 1
    unexpected = sorted(package_names & FORBIDDEN)
    errors = []
    if unexpected:
        errors.append("no-default-features includes forbidden driver dependencies: " + ", ".join(unexpected))

    metadata = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1", "--offline", "--locked"],
        cwd=ROOT,
        text=True,
        capture_output=True,
    )
    if metadata.returncode != 0:
        print(metadata.stderr, file=sys.stderr)
        return metadata.returncode
    packages = json.loads(metadata.stdout)["packages"]
    wrapper = next(package for package in packages if package["name"] == "druid-wrapper")
    features = wrapper["features"]
    dependencies = {dependency["name"]: dependency for dependency in wrapper["dependencies"]}
    for feature, owned_deps in FEATURE_DEPS.items():
        enabled = set(features.get(feature, []))
        if feature not in features:
            errors.append(f"missing driver feature: {feature}")
        for dependency_name in sorted(owned_deps):
            dependency = dependencies.get(dependency_name)
            if dependency is None or not dependency["optional"]:
                errors.append(f"{dependency_name} must be an optional dependency")
            if f"dep:{dependency_name}" not in enabled:
                errors.append(f"{feature} must own dep:{dependency_name}")
    if "toasty-sqlite" not in features:
        errors.append("missing driver feature: toasty-sqlite")
    for feature, expected in ISOLATED_FEATURES.items():
        if feature not in features:
            continue
        actual, tree_error = normal_dependencies(feature)
        if tree_error:
            errors.append(f"{feature} dependency tree failed: {tree_error}")
            continue
        missing = expected - actual
        leaked = (actual & FORBIDDEN) - expected - ALLOWED_TRANSITIVE.get(feature, set())
        if missing:
            errors.append(f"{feature} misses expected driver dependencies: {', '.join(sorted(missing))}")
        if leaked:
            errors.append(f"{feature} leaks unrelated driver dependencies: {', '.join(sorted(leaked))}")
    if errors:
        print("\n".join(errors), file=sys.stderr)
        return 1
    print("wrapper feature dependencies are optional and excluded without default features")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
