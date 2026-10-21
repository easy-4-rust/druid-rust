#!/usr/bin/env python3
"""Generate the migration fact tables for druid-rust.

Reads the frozen Java Druid source tree and the current Rust workspace, then
emits machine-readable inventories plus the generated regions consumed by the
four migration documents under docs/migration/.

Usage:
  python scripts/migration_inventory.py \
    --java-root E:/workspaces/workspace-github/druid/core/src/main/java \
    --rust-root . \
    --out-dir docs/migration
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
from pathlib import Path

JAVA_PKG_ROOT = Path("com/alibaba/druid")
TYPE_RE = re.compile(
    r"^\s*(?:@\w+(?:\([^)]*\))?\s*)*"
    r"(?:(public|protected|private)\s+)?"
    r"(?:(?:abstract|final|sealed|non-sealed|static|strictfp)\s+)*"
    r"(class|interface|enum|record|@interface)\s+([A-Za-z_$][\w$]*)",
    re.M,
)
METHOD_RE = re.compile(
    r"^\s{2,8}(public|protected)\s+"
    r"(?:(?:static|final|abstract|synchronized|native|default|strictfp)\s+)*"
    r"([\w<>\[\],.?\s$]+?)\s+([A-Za-z_$][\w$]*)\s*\(([^)]*)\)\s*(?:throws[^{;]*)?[{;]",
    re.M,
)
RUST_TYPE_RE = re.compile(r"^\s*pub\s+(?:\([^)]*\)\s+)?(struct|enum|trait|type)\s+([A-Za-z_]\w*)", re.M)
RUST_FN_RE = re.compile(r"^\s*pub(?:\([^)]*\))?\s+(?:const\s+|async\s+|unsafe\s+|extern\s+\"[^\"]*\"\s+)*fn\s+([A-Za-z_]\w*)")
RUST_IMPL_RE = re.compile(r"^impl(?:<[^>]*>)?\s+(?:[\w:<>,\s]+?\s+for\s+)?([A-Za-z_]\w*)")

# Java object -> Rust object when the migration intentionally renamed it.
ALIASES = {
    "DruidDataSource": ["DruidPool", "DruidDataSource"],
    "DruidAbstractDataSource": ["PoolInner"],
    "DruidPooledConnection": ["DruidPooledConnection"],
    "DruidConnectionHolder": ["DruidConnectionHolder"],
    "FilterChainImpl": ["FilterChainImpl"],
    "FilterEventAdapter": ["FilterEventAdapter"],
    "StatFilter": ["StatFilter"],
    "WallFilter": ["WallFilter"],
    "WallProvider": ["WallProvider"],
    "WallConfig": ["WallConfig"],
    "SQLUtils": ["SqlUtils"],
    "DbType": ["DbType"],
    "DruidStatService": ["DruidStatService"],
    "DruidStatManagerFacade": ["DruidStatManagerFacade"],
    "HighAvailableDataSource": ["HighAvailableDataSource"],
    "JdbcSqlStat": ["RdbcSqlStat"],
    "JdbcDataSourceStat": ["RdbcDataSourceStat"],
    "JdbcConnectionStat": ["RdbcConnectionStat"],
    "JdbcStatementStat": ["RdbcStatementStat"],
    "JdbcResultSetStat": ["RdbcResultSetStat"],
    "DruidDataSourceStatManager": ["DruidDataSourceStatManager"],
    "ExceptionSorter": ["ExceptionSorter"],
    "ValidConnectionChecker": ["ValidConnectionChecker"],
    "PreparedStatementPool": ["StandardPreparedStatementCache"],
}

# Families intentionally replaced by a platform crates; recorded, never counted
# as implemented by name alone.
REPLACED_FAMILIES = {
    "sql/ast": "sqlparser 0.52 AST (adapter: SqlUtils::parse_statements)",
    "sql/dialect": "sqlparser 0.52 dialect mapping + druid wall visitors",
    "sql/visitor": "sqlparser visitor + druid stats/wall visitors",
    "sql/parser": "sqlparser 0.52 parser (Druid Lexer kept as compatibility shell)",
}

# Java object whose responsibilities are intentionally split across several Rust types.
SPLITS = {
    "Filter": ["BeforeFilter", "AfterFilter", "ResultSetFilter"],
}

SKILL_RETAIN_SEGMENTS = 2


def snake(name: str) -> str:
    s = re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1_\2", name)
    s = re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", s)
    return s.replace("__", "_").lower()


def expected_path(pkg_segments: list[str], type_name: str) -> str:
    """Deterministic path per the migration skill: strip org/module root, then
    retain the final two package segments (one/zero when fewer remain)."""
    segs = pkg_segments[-SKILL_RETAIN_SEGMENTS:] if pkg_segments else []
    stem = type_name.lstrip("@")
    return "/".join(segs + [snake(stem) + ".rs"])


def read_text(path: Path) -> str:
    for enc in ("utf-8", "utf-8-sig", "latin-1"):
        try:
            return path.read_text(encoding=enc)
        except UnicodeDecodeError:
            continue
    return path.read_text(encoding="utf-8", errors="replace")


def java_objects(java_root: Path):
    pkg_root = java_root / JAVA_PKG_ROOT
    objects, methods = [], []
    for path in sorted(pkg_root.rglob("*.java")):
        rel = path.relative_to(pkg_root)
        text = read_text(path)
        pkg_match = re.search(r"^package\s+([\w.]+);", text, re.M)
        package = pkg_match.group(1) if pkg_match else ""
        pkg_segments = package.split(".")[: -1] if package else []
        if package.endswith(".druid"):
            pkg_segments = []
        elif package.startswith("com.alibaba.druid."):
            pkg_segments = package[len("com.alibaba.druid.") :].split(".")
        else:
            pkg_segments = package.split(".")[-2:]
        for m in TYPE_RE.finditer(text):
            visibility = m.group(1) or "package"
            if visibility not in ("public", "protected"):
                continue
            kind, name = m.group(2), m.group(3)
            if kind == "@interface":
                kind = "annotation"
            if name != path.stem:
                # nested type: recorded separately only when public
                nesting = "nested"
            else:
                nesting = "top-level"
            objects.append(
                {
                    "java_type": name,
                    "kind": kind,
                    "java_package": ".".join(["com", "alibaba", "druid"] + pkg_segments),
                    "java_file": str(rel).replace(os.sep, "/"),
                    "visibility": visibility,
                    "nesting": nesting,
                    "expected_rust_path": expected_path(pkg_segments, name),
                    "area": "/".join(pkg_segments[:2]) if pkg_segments else "(root)",
                }
            )
        for m in METHOD_RE.finditer(text):
            name = m.group(3)
            if name in ("if", "for", "while", "switch", "catch", "return", "new", "assert"):
                continue
            params = m.group(4).strip()
            arity = 0 if not params else params.count(",") + 1
            methods.append(
                {
                    "java_type": path.stem,
                    "method": name,
                    "arity": arity,
                    "visibility": m.group(1),
                    "area": "/".join(pkg_segments[:2]) if pkg_segments else "(root)",
                }
            )
    return objects, methods


def rust_objects(rust_root: Path):
    crates = rust_root / "crates"
    types, fns = {}, []
    for crate in sorted(p for p in crates.iterdir() if p.is_dir()):
        src = crate / "src"
        if not src.is_dir():
            continue
        for path in sorted(src.rglob("*.rs")):
            text = read_text(path)
            rel = path.relative_to(rust_root).as_posix()
            module = rel[len("crates/"):].replace("/src/", "::").replace(".rs", "").replace("/", "::")
            loc = text.count("\n") + (0 if text.endswith("\n") else 1)
            for m in RUST_TYPE_RE.finditer(text):
                kind, name = m.group(1), m.group(2)
                types.setdefault(name, []).append(
                    {"rust_type": name, "rust_kind": kind, "rust_path": rel, "rust_module": module, "loc": loc}
                )
            current = None
            for line in text.splitlines():
                im = RUST_IMPL_RE.match(line)
                if im:
                    current = im.group(1)
                fm = RUST_FN_RE.match(line)
                if fm and current:
                    fns.append({"rust_type": current, "fn": fm.group(1), "rust_path": rel})
    return types, fns



def structural_metrics(rust_root: Path, java_test_root: Path | None):
    rows, files = [], []
    crate_root = rust_root / "crates"
    for crate in sorted(p for p in crate_root.iterdir() if p.is_dir()):
        src = crate / "src"
        if not src.is_dir():
            continue
        for path in sorted(src.rglob("*.rs")):
            text = read_text(path)
            loc = text.count("\n") + (0 if text.endswith("\n") else 1)
            objs = len(RUST_TYPE_RE.findall(text))
            rel = path.relative_to(rust_root).as_posix()
            files.append({"loc": loc, "path": rel, "objects": objs})
    over = [f for f in files if f["loc"] > 800]
    mid = [f for f in files if 500 < f["loc"] <= 800]
    multi = [f for f in files if f["objects"] >= 2 and not f["path"].endswith(("lib.rs", "mod.rs"))]
    for f in sorted(over, key=lambda x: -x["loc"]):
        rows.append({"kind": "FILE_OVER_800", "path": f["path"], "detail": f"{f['loc']} lines"})
    for f in sorted(multi, key=lambda x: -x["objects"]):
        rows.append({"kind": "MULTI_PUBLIC_OBJECT", "path": f["path"], "detail": f"{f['objects']} public objects"})
    java_tests = java_test_methods = 0
    if java_test_root and java_test_root.is_dir():
        for path in java_test_root.rglob("*.java"):
            java_tests += 1
            java_test_methods += len(re.findall(r"^\s*@Test\b", read_text(path), re.M))
    rust_test_files = rust_test_attrs = 0
    for crate in sorted(p for p in crate_root.iterdir() if p.is_dir()):
        tests = crate / "tests"
        if tests.is_dir():
            for path in tests.rglob("*.rs"):
                rust_test_files += 1
                rust_test_attrs += len(re.findall(r"\#\[(?:tokio::)?test\]|\#\[rstest\]", read_text(path)))
        src = crate / "src"
        if src.is_dir():
            for path in src.rglob("*.rs"):
                rust_test_attrs += len(re.findall(r"\#\[(?:tokio::)?test\]|\#\[rstest\]", read_text(path)))
    return {
        "rust_src_files": len(files),
        "rust_src_loc": sum(f["loc"] for f in files),
        "files_over_800": over,
        "files_501_800": mid,
        "multi_object_files": multi,
        "redlines": rows,
        "java_test_files": java_tests,
        "java_test_methods": java_test_methods,
        "rust_test_files": rust_test_files,
        "rust_test_attrs": rust_test_attrs,
    }

def git_rev(path: Path) -> str:
    try:
        return subprocess.check_output(["git", "-C", str(path), "rev-parse", "HEAD"], text=True).strip()
    except Exception:
        return "UNKNOWN"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--java-root", required=True)
    ap.add_argument("--java-repo", required=True)
    ap.add_argument("--rust-root", default=".")
    ap.add_argument("--out-dir", default="docs/migration")
    ap.add_argument("--module", default="core")
    ap.add_argument("--java-test-root", default="")
    args = ap.parse_args()

    java_root = Path(args.java_root).resolve()
    rust_root = Path(args.rust_root).resolve()
    out = (rust_root / args.out_dir).resolve()
    (out / "inventory").mkdir(parents=True, exist_ok=True)

    objects, methods = java_objects(java_root)
    types, fns = rust_objects(rust_root)

    fn_by_type: dict[str, set[str]] = {}
    for row in fns:
        fn_by_type.setdefault(row["rust_type"], set()).add(row["fn"])

    rows = []
    for obj in objects:
        split_targets = SPLITS.get(obj["java_type"], [])
        if split_targets and all(t in types for t in split_targets):
            obj.update(
                {
                    "rust_type": " + ".join(split_targets),
                    "rust_path": ", ".join(types[t][0]["rust_path"] for t in split_targets),
                    "mapping_form": "SPLIT",
                    "object_status": "UNVERIFIED",
                    "replacement": "拆分为 " + " / ".join(split_targets) + "（接口隔离，保留链式语义）",
                    "semantic_test": "yes" if any(fn_by_type.get(t) for t in split_targets) else "no",
                    "area": obj["area"],
                }
            )
            rows.append(obj)
            continue
        candidates = [obj["java_type"]] + ALIASES.get(obj["java_type"], [])
        match_name, match = None, None
        for cand in candidates:
            if cand in types:
                match_name = cand
                match = types[cand][0]
                break
        area = obj["area"]
        replaced = next((v for k, v in REPLACED_FAMILIES.items() if area.startswith(k)), None)
        if match:
            status = "UNVERIFIED"
            form = "SAME" if match_name == obj["java_type"] else "RENAMED"
            actual = match["rust_path"]
        elif replaced:
            status = "MISSING"
            form = "REPLACED_DEPENDENCY"
            actual = ""
        else:
            status = "MISSING"
            form = "UNMAPPED"
            actual = ""
        obj.update(
            {
                "rust_type": match_name or "",
                "rust_path": actual,
                "mapping_form": form,
                "object_status": status,
                "replacement": replaced or "",
                "semantic_test": "yes" if match_name and fn_by_type.get(match_name) else "no",
            }
        )
        rows.append(obj)

    # Rust-only types are extensions unless they are alias targets.
    alias_targets = {t for v in ALIASES.values() for t in v}
    java_names = {o["java_type"] for o in objects}
    extras = []
    for name, defs in sorted(types.items()):
        if name in java_names or name in alias_targets:
            continue
        extras.append({"rust_type": name, "rust_kind": defs[0]["rust_kind"], "rust_path": defs[0]["rust_path"]})

    # Method-level candidates: same type + same snake_case name.
    method_rows = []
    by_type: dict[str, list[dict]] = {}
    for row in rows:
        if row["rust_type"]:
            by_type.setdefault(row["java_type"], []).append(row)
    for method in methods:
        owners = by_type.get(method["java_type"], [])
        rust_fns = set()
        for owner in owners:
            rust_fns |= fn_by_type.get(owner["rust_type"], set())
        candidate = snake(method["method"])
        matched = candidate in rust_fns
        method_rows.append({**method, "rust_fn": candidate if matched else "", "semantic_status": "CANDIDATE" if matched else "MISSING"})

    def write_tsv(path: Path, header: list[str], data: list[dict]):
        with path.open("w", encoding="utf-8", newline="\n") as fh:
            fh.write("\t".join(header) + "\n")
            for row in data:
                fh.write("\t".join(str(row.get(h, "")) for h in header) + "\n")

    write_tsv(
        out / "inventory" / "objects.tsv",
        ["java_type", "kind", "java_package", "java_file", "visibility", "nesting", "expected_rust_path", "rust_type", "rust_path", "mapping_form", "object_status", "replacement", "semantic_test", "area"],
        rows,
    )
    write_tsv(
        out / "inventory" / "methods.tsv",
        ["java_type", "method", "arity", "visibility", "area", "rust_fn", "semantic_status"],
        method_rows,
    )
    write_tsv(out / "inventory" / "rust_extra.tsv", ["rust_type", "rust_kind", "rust_path"], extras)

    areas: dict[str, dict] = {}
    for row in rows:
        a = areas.setdefault(row["area"], {"java": 0, "matched": 0, "missing": 0, "replaced": 0})
        a["java"] += 1
        if row["object_status"] == "UNVERIFIED":
            a["matched"] += 1
        else:
            a["missing"] += 1
        if row["mapping_form"] == "REPLACED_DEPENDENCY":
            a["replaced"] += 1

    summary = {
        "java_baseline": git_rev(Path(args.java_repo)),
        "rust_baseline": git_rev(rust_root),
        "module": args.module,
        "java_objects": len(rows),
        "matched_by_name": sum(1 for r in rows if r["object_status"] == "UNVERIFIED"),
        "missing": sum(1 for r in rows if r["object_status"] == "MISSING"),
        "replaced_family_objects": sum(1 for r in rows if r["mapping_form"] == "REPLACED_DEPENDENCY"),
        "java_methods": len(method_rows),
        "method_candidates": sum(1 for r in method_rows if r["semantic_status"] == "CANDIDATE"),
        "rust_types": sum(len(v) for v in types.values()),
        "rust_extra_types": len(extras),
        "areas": dict(sorted(areas.items(), key=lambda kv: -kv[1]["java"])),
    }
    structure = structural_metrics(rust_root, Path(args.java_test_root) if args.java_test_root else None)
    summary.update({k: structure[k] for k in ("rust_src_files", "rust_src_loc", "java_test_files", "java_test_methods", "rust_test_files", "rust_test_attrs")})
    summary["files_over_800"] = len(structure["files_over_800"])
    summary["files_501_800"] = len(structure["files_501_800"])
    summary["multi_object_files"] = len(structure["multi_object_files"])
    write_tsv(out / "inventory" / "redlines.tsv", ["kind", "path", "detail"], structure["redlines"])
    (out / "inventory" / "structure.json").write_text(json.dumps({
        "over_800": sorted(structure["files_over_800"], key=lambda x: -x["loc"])[:200],
        "mid": sorted(structure["files_501_800"], key=lambda x: -x["loc"])[:200],
        "multi": sorted(structure["multi_object_files"], key=lambda x: -x["objects"])[:200],
    }, ensure_ascii=False, indent=2), encoding="utf-8")
    (out / "inventory" / "summary.json").write_text(json.dumps(summary, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({k: v for k, v in summary.items() if k != "areas"}, ensure_ascii=False, indent=2))
    for area, stat in list(summary["areas"].items())[:14]:
        print(f"{area:28s} java={stat['java']:4d} matched={stat['matched']:4d} missing={stat['missing']:4d} replaced={stat['replaced']:4d}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
