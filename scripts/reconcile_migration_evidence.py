"""只生成候选与证据清单；名称缺失绝不自动推导成功能缺失。"""
from __future__ import annotations

import argparse
import csv
import hashlib
import json
import re
import subprocess
from collections import Counter, defaultdict
from pathlib import Path

MODULES = ("core", "druid-admin", "druid-wrapper")
PACKAGES = ("druid", "druid-rdbc", "druid-wrapper", "druid-metrics", "druid-admin")
TYPE = re.compile(r"\b(?:struct|enum|trait|type)\s+([A-Za-z_]\w*)")
WORDS = re.compile(r"\b[A-Za-z_]\w*\b")


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def git(root: Path, *args: str) -> str:
    return subprocess.check_output(["git", "-C", str(root), *args], text=True).strip()


def normalized(name: str) -> str:
    return name.replace("_", "").lower()


def classify_candidates(name: str, declared: dict, mentioned: dict) -> tuple[str, list[str]]:
    """仅定位候选，不将声明、注释、文件名或依赖存在升级为语义完成。"""
    key = normalized(name)
    if key in declared:
        return "DECLARATION_CANDIDATE", sorted(declared[key])
    if key in mentioned:
        return "REFERENCE_CANDIDATE", sorted(mentioned[key])
    return "UNRESOLVED_MAPPING", []


def write_csv(path: Path, rows: list[dict], fields: list[str]) -> None:
    with path.open("w", encoding="utf-8", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=fields)
        writer.writeheader()
        writer.writerows(rows)


def inventory(java: Path, rust: Path, output: Path) -> dict:
    output.mkdir(parents=True, exist_ok=True)
    baseline = {"java_sha": git(java, "rev-parse", "HEAD"),
                "rust_sha": git(rust, "rev-parse", "HEAD"),
                "dirty_before": git(rust, "status", "--short"),
                "claim": "V0_STATIC; candidate links are not semantic verification"}
    declared, mentioned = defaultdict(set), defaultdict(set)
    rust_files, test_files, assets, sources = [], [], [], []
    for package in PACKAGES:
        for path in sorted((rust / "crates" / package / "src").rglob("*.rs")):
            rel = path.relative_to(rust).as_posix()
            text = path.read_text(encoding="utf-8-sig")
            rust_files.append({"path": rel, "sha256": digest(path), "lines": len(text.splitlines())})
            for name in TYPE.findall(text):
                declared[normalized(name)].add(rel)
            for name in set(WORDS.findall(text)):
                mentioned[normalized(name)].add(rel)
    # Current Cargo package tests only: omit .superpowers drafts, wip and target.
    for package in sorted((rust / "crates").iterdir()):
        if not (package / "Cargo.toml").exists():
            continue
        for path in sorted((package / "tests").rglob("*.rs")):
            text = path.read_text(encoding="utf-8-sig")
            test_files.append({"path": path.relative_to(rust).as_posix(),
                               "sha256": digest(path),
                               "test_attributes": len(re.findall(r"#\[(?:tokio::)?test(?:\([^]]*\))?\]", text))})
    for module in MODULES:
        base = java / module
        for path in sorted((base / "src/main/java").rglob("*.java")):
            text = path.read_text(encoding="utf-8-sig")
            package = re.search(r"^package\s+([\w.]+);", text, re.M)
            kind, candidates = classify_candidates(path.stem, declared, mentioned)
            sources.append({"module": module, "source": path.relative_to(java).as_posix(),
                            "java_name": (package.group(1) + "." if package else "") + path.stem,
                            "sha256": digest(path), "mapping_signal": kind,
                            "rust_candidates": ";".join(candidates),
                            "semantic_disposition": "REVIEW_REQUIRED"})
        for path in sorted((base / "src/test").rglob("*")):
            if path.is_file():
                assets.append({"module": module, "path": path.relative_to(java).as_posix(),
                               "kind": "JAVA_TEST_SOURCE" if path.suffix == ".java" else "TEST_ASSET",
                               "sha256": digest(path)})
    write_csv(output / "source-file-candidates.csv", sources,
              ["module", "source", "java_name", "sha256", "mapping_signal", "rust_candidates", "semantic_disposition"])
    write_csv(output / "rust-production-files.csv", rust_files, ["path", "sha256", "lines"])
    write_csv(output / "rust-test-files.csv", test_files, ["path", "sha256", "test_attributes"])
    write_csv(output / "java-test-inputs.csv", assets, ["module", "path", "kind", "sha256"])
    baseline["source_files"] = dict(Counter(row["module"] for row in sources))
    baseline["mapping_signals"] = dict(Counter(row["mapping_signal"] for row in sources))
    baseline["rust_production_files"] = len(rust_files)
    baseline["rust_test_files"] = len(test_files)
    baseline["java_test_inputs"] = dict(Counter(row["kind"] for row in assets))
    baseline["limitations"] = ["File census, not complete nested-object inventory",
        "Regex declarations/references are candidates, never proof of implementation",
        "No parameterized/dynamic test expansion; no asset copy parity claim",
        "Only conventional src/test roots inventoried; other roots need runner review",
        "No derived percentage of functional completion"]
    baseline["dirty_after"] = git(rust, "status", "--short")
    baseline["rust_sha_after"] = git(rust, "rev-parse", "HEAD")
    (output / "baseline.json").write_text(json.dumps(baseline, ensure_ascii=False, indent=2), encoding="utf-8")
    return baseline


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--java-root", required=True, type=Path)
    parser.add_argument("--rust-root", default=".", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    result = inventory(args.java_root.resolve(), args.rust_root.resolve(), args.output.resolve())
    print(json.dumps({k: result[k] for k in ("source_files", "mapping_signals", "rust_production_files", "rust_test_files", "java_test_inputs")}, ensure_ascii=False))
