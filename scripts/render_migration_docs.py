#!/usr/bin/env python3
"""Render the four migration documents from the generated inventory.

Templates live in docs/migration/_templates/*.md and contain {{MARKERS}} that
this script replaces with facts derived from docs/migration/inventory/*.tsv.
Never edit generated regions by hand.
"""
from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DOCS = ROOT / "docs" / "migration"
INV = DOCS / "inventory"
TPL = DOCS / "_templates"
TICK = chr(96)


def code(value: str) -> str:
    return TICK + value + TICK


def tsv(name):
    path = INV / name
    if not path.exists():
        return []
    lines = path.read_text(encoding="utf-8").splitlines()
    if not lines:
        return []
    header = lines[0].split("\t")
    return [dict(zip(header, line.split("\t"))) for line in lines[1:] if line]


def esc(value) -> str:
    text = "" if value is None else str(value)
    return text.replace("|", "\\|").replace("\n", " ")


def table(header, rows, align_right=()):
    out = ["| " + " | ".join(header) + " |",
           "|" + "|".join("---:" if i in align_right else "---" for i in range(len(header))) + "|"]
    out += ["| " + " | ".join(esc(c) for c in row) + " |" for row in rows]
    return "\n".join(out)


def main():
    # 旧台账只有名称候选，不得覆盖已经人工核对的语义证据；先检查全部输出。
    protected = [
        path.name for path in (DOCS / tpl.name for tpl in TPL.glob("*.md"))
        if path.exists() and "<!-- current-code-audit-start -->" in path.read_text(encoding="utf-8")
    ]
    if protected:
        raise SystemExit(
            "refusing to overwrite reviewed migration documents: " + ", ".join(sorted(protected))
            + "; use scripts/reconcile_migration_evidence.py for candidate inventory"
        )
    summary = json.loads((INV / "summary.json").read_text(encoding="utf-8"))
    objects = tsv("objects.tsv")
    methods = tsv("methods.tsv")
    extras = tsv("rust_extra.tsv")
    redlines = tsv("redlines.tsv")

    java_objects = summary["java_objects"]
    matched = summary["matched_by_name"]
    missing = summary["missing"]
    replaced = summary["replaced_family_objects"]
    not_replaced = missing - replaced

    contract = [
        ("模块", "Java Druid core（%d 个对象）→ Rust workspace crates/" % java_objects),
        ("来源基线", code(summary["java_baseline"]) + "（E:/workspaces/workspace-github/druid）"),
        ("Rust 基线", code(summary["rust_baseline"]) + "（druid-rust workspace）"),
        ("当前对象边界", "%d 个 class/interface/enum/record/annotation；package-info 不计入" % java_objects),
        ("目标根", "crates/druid/src、crates/druid-rdbc/src、crates/druid-wrapper/src、crates/druid-metrics/src、crates/druid-admin/src"),
        ("目录和文件", "保留 Java 包末 2 段；文件名 acronym-aware snake_case；仓库既有 core/ 前缀与 sql/ 归并登记为偏差"),
        ("类型和方法", "Rust 类型 PascalCase；方法 snake_case；Java %d 个公开方法按语义族而非 1:1 重载映射" % summary["java_methods"]),
        ("模块文件", "lib.rs / mod.rs 只允许文档、声明与显式重导出"),
        ("完成口径", "仅 IMPLEMENTED / DEPENDENCY_REUSED / PLATFORM_NA 计入完成；本文档不自行升级状态"),
        ("红线", "禁止 stub、禁止以兼容重导出充数、禁止无证据的依赖替代、禁止跨 await 持有 parking_lot 锁"),
        ("注释和测试", "每个对象保留中文 Java 来源注释；行为测试必须有可观察断言，差分需记录 MATCH"),
    ]
    contract_md = table(["规范项", "本模块当前要求"], contract)

    snapshot_md = table(["状态", "数量"], [
        ("IMPLEMENTED", 0),
        ("DEPENDENCY_REUSED", 0),
        ("PLATFORM_NA", 0),
        ("UNVERIFIED（同名命中，语义未审计）", matched),
        ("MISSING", missing),
        ("MISSING（解析器族，已登记替换策略）", replaced),
        ("MISSING（待实现）", not_replaced),
        ("MISPLACED", 0),
        ("STUB", 0),
        ("合计", java_objects),
    ], align_right=(1,))

    area_rows = []
    for area, stat in summary["areas"].items():
        total = stat["java"]
        rate = (stat["matched"] / total * 100) if total else 0.0
        area_rows.append((area, total, stat["matched"], stat["missing"], stat["replaced"], "%.1f%%" % rate))
    area_md = table(["Java 包域", "对象", "同名命中", "缺失", "解析器族替换", "命中率"], area_rows, align_right=(1, 2, 3, 4, 5))

    groups = {}
    for row in objects:
        groups.setdefault(row["area"], []).append(row)
    sections = []
    for area in sorted(groups, key=lambda a: (-len(groups[a]), a)):
        rows = sorted(groups[area], key=lambda r: r["java_type"].lower())
        body = []
        for row in rows:
            if row["replacement"]:
                evidence = row["replacement"]
            elif row["object_status"] == "UNVERIFIED":
                evidence = "同名公开类型存在；语义与测试证据待审计"
            else:
                evidence = "未找到同名 Rust 对象文件"
            body.append((
                code(row["java_package"] + "." + row["java_type"]),
                code(row["java_file"]),
                code(row["expected_rust_path"]),
                code(row["rust_path"]) if row["rust_path"] else "—",
                code(row["object_status"]),
                evidence,
            ))
        sections.append("### " + area + "（" + str(len(rows)) + "）\n\n" + table(
            ["Java FQN", "Java 相对路径", "预期 Rust 路径", "当前 Rust 路径", "状态", "证据"], body))
    object_table_md = "\n\n".join(sections)

    matched_md = table(["Java 对象", "Rust 路径", "映射形态"], [
        (code(r["java_type"]), code(r["rust_path"]), r["mapping_form"])
        for r in sorted(objects, key=lambda x: x["java_type"].lower()) if r["rust_path"]
    ])

    critical = [r for r in objects if r["object_status"] == "MISSING" and r["mapping_form"] != "REPLACED_DEPENDENCY"]
    critical.sort(key=lambda r: (r["area"], r["java_type"].lower()))
    missing_md = table(["Java 对象", "所属域", "预期 Rust 路径", "处置"], [
        (code(r["java_type"]), r["area"], code(r["expected_rust_path"]), "待实现") for r in critical[:400]
    ])

    red_md = table(["类型", "路径", "细节"], [
        (r["kind"], code(r["path"]), r["detail"]) for r in redlines[:120]
    ])

    method_stats = {}
    for row in methods:
        stat = method_stats.setdefault(row["area"], {"java": 0, "matched": 0})
        stat["java"] += 1
        if row["semantic_status"] == "CANDIDATE":
            stat["matched"] += 1
    method_md = table(["Java 包域", "公开方法", "同名 snake_case 候选", "候选率"], [
        (area, stat["java"], stat["matched"], "%.1f%%" % ((stat["matched"] / stat["java"] * 100) if stat["java"] else 0))
        for area, stat in sorted(method_stats.items(), key=lambda kv: -kv[1]["java"])
    ], align_right=(1, 2, 3))

    extra_md = table(["Rust 类型", "种类", "路径"], [
        (code(r["rust_type"]), r["rust_kind"], code(r["rust_path"])) for r in extras[:250]
    ])

    test_md = table(["维度", "Java", "Rust", "说明"], [
        ("测试文件", summary["java_test_files"], summary["rust_test_files"], "Java 含 bvt 全量；Rust 为 tests/ 下的文件"),
        ("测试用例/属性", summary["java_test_methods"], summary["rust_test_attrs"], "Java 以 @Test 计数；Rust 以 #[test]/#[tokio::test]/#[rstest] 计数"),
        ("产品源码文件", "—", summary["rust_src_files"], "crates/*/src 下的 .rs"),
        ("产品源码行数", "—", summary["rust_src_loc"], "不含 tests/ 与 vendor/"),
        ("超 800 行文件", "—", summary["files_over_800"], "技能定义的 authored-file 阻断线"),
        ("501–800 行文件", "—", summary["files_501_800"], "需记录 cohesion review"),
        ("单文件多公开对象", "—", summary["multi_object_files"], "一 Java 对象一 Rust 对象的红线"),
    ], align_right=(1, 2))

    values = {
        "CONTRACT_TABLE": contract_md, "SNAPSHOT_TABLE": snapshot_md, "AREA_TABLE": area_md,
        "OBJECT_TABLE": object_table_md, "MATCHED_TABLE": matched_md, "MISSING_TABLE": missing_md,
        "REDLINE_TABLE": red_md, "METHOD_SUMMARY_TABLE": method_md, "EXTRA_TABLE": extra_md,
        "TEST_STATS_TABLE": test_md,
        "JAVA_BASELINE": summary["java_baseline"], "RUST_BASELINE": summary["rust_baseline"],
        "JAVA_OBJECTS": str(java_objects), "MATCHED": str(matched), "MISSING": str(missing),
        "REPLACED": str(replaced), "NOT_REPLACED_MISSING": str(not_replaced),
        "JAVA_METHODS": str(summary["java_methods"]), "METHOD_CANDIDATES": str(summary["method_candidates"]),
        "RUST_SRC_FILES": str(summary["rust_src_files"]), "RUST_SRC_LOC": "{:,}".format(summary["rust_src_loc"]),
        "FILES_OVER_800": str(summary["files_over_800"]), "FILES_501_800": str(summary["files_501_800"]),
        "MULTI_OBJECT": str(summary["multi_object_files"]),
        "JAVA_TESTS": str(summary["java_test_files"]), "JAVA_TEST_METHODS": str(summary["java_test_methods"]),
        "RUST_TESTS": str(summary["rust_test_files"]), "RUST_TEST_ATTRS": str(summary["rust_test_attrs"]),
        "EXTRA_TABLE_COUNT": str(len(extras)),
    }

    for tpl_path in sorted(TPL.glob("*.md")):
        text = tpl_path.read_text(encoding="utf-8")
        for key, value in values.items():
            text = text.replace("{{" + key + "}}", value)
        left = re.findall(r"\{\{[A-Z_]+\}\}", text)
        if left:
            raise SystemExit("unresolved markers in " + tpl_path.name + ": " + str(left))
        (DOCS / tpl_path.name).write_text(text, encoding="utf-8", newline="\n")
        print("rendered " + tpl_path.name + " (" + str(len(text)) + " bytes)")


if __name__ == "__main__":
    main()
