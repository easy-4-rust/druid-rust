#!/usr/bin/env python3
"""Render the P2 dialect fidelity matrix from the Rust observation TSV."""
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TSV = ROOT / "docs" / "migration" / "dialect-fidelity" / "rust-observations.tsv"
OUT = ROOT / "docs" / "migration" / "dialect-fidelity" / "README.md"


def clip(text: str, limit: int = 72) -> str:
    text = text.replace("|", "\\|").replace("\n", " ")
    return text if len(text) <= limit else text[: limit - 1] + "..."


rows = []
for line in TSV.read_text(encoding="utf-8").splitlines()[1:]:
    parts = line.split("\t")
    if len(parts) < 8:
        continue
    rows.append(
        {
            "dialect": parts[0],
            "case": parts[1],
            "parse": parts[2],
            "format": parts[3],
            "reparse": parts[4],
            "param": parts[5],
            "wall": parts[6],
            "sql": parts[7],
        }
    )

by_dialect = defaultdict(list)
for row in rows:
    by_dialect[row["dialect"]].append(row)

lines = [
    "# P2 方言保真矩阵（Oracle / ODPS / Hive / Doris）",
    "",
    "> 状态：**STARTED**。Rust 观测已完成；Java golden 列仍为 PENDING（需要固定版本 jar 生成）。",
    "> 本文件由 scripts/dialect_fidelity_report.py 从 rust-observations.tsv 生成，禁止手改统计。",
    "",
    "## 方法",
    "",
    "| 列 | 含义 | 生成方式 |",
    "|---|---|---|",
    "| parse | SqlUtils::parse_statements 是否接受 | cargo run -p druid-bench --example dialect_fidelity_matrix |",
    "| format | SqlUtils::to_sql_string 输出；parse 失败记为 ERR: | 同上 |",
    "| reparse | 对 format 输出再次 parse 是否成功 | 同上 |",
    "| parameterized | SqlUtils::parameterize 的字面量替换结果 | 同上 |",
    "| wall_violations | 默认 WallConfig 下 WallProvider::check 的违规数 | 同上 |",
    "| Java golden | Java Druid 1.2.28 的同四列输出 | PENDING：需 mvn -pl core -DskipTests package 后用固定 jar 运行 |",
    "",
    "## 汇总",
    "",
    "| 方言 | 用例 | parse ok | format-reparse ok | wall 违规 | 主要缺口 |",
    "|---|---:|---:|---:|---:|---|",
]
notes = {
    "Oracle": "RETURNING ... INTO 未支持",
    "Odps": "无",
    "Hive": "TABLESAMPLE (n PERCENT) 未支持",
    "Doris": "UNIQUE KEY / DISTRIBUTED ... BUCKETS DDL 未支持",
}
for dialect in ("Oracle", "Odps", "Hive", "Doris"):
    group = by_dialect.get(dialect, [])
    ok = sum(1 for r in group if r["parse"] == "ok")
    re_ok = sum(1 for r in group if r["reparse"] == "ok")
    wall = sum(1 for r in group if r["wall"].isdigit() and int(r["wall"]) > 0)
    lines.append("| %s | %d | %d | %d | %d | %s |" % (dialect, len(group), ok, re_ok, wall, notes[dialect]))

lines += ["", "## 逐用例观测", ""]
for dialect in ("Oracle", "Odps", "Hive", "Doris"):
    lines += ["### " + dialect, "", "| 用例 | parse | reparse | parameterized | wall | Java golden |", "|---|---|---|---|---:|---|"]
    for r in by_dialect.get(dialect, []):
        lines.append("| %s | %s | %s | %s | %s | PENDING |" % (r["case"], r["parse"], r["reparse"], clip(r["param"], 64), r["wall"]))
    lines.append("")

lines += [
    "## 失败用例（Rust 侧）",
    "",
    "| 方言 | 用例 | 原始 SQL | 错误 |",
    "|---|---|---|---|",
]
for r in rows:
    if r["parse"] != "ok":
        lines.append("| %s | %s | %s | %s |" % (r["dialect"], r["case"], clip(r["sql"], 90), clip(r["format"], 110)))

lines += [
    "",
    "## 结论与下一步",
    "",
    "1. **ODPS/Hive 的查询面**已由 sqlparser 覆盖到可解析；缺口集中在 Hive 采样语法。",
    "2. **Doris 的 DDL 扩展**（UNIQUE KEY / BUCKETS）不被 GenericDialect 接受，属 P2 方案 C（Druid 侧扩展）的候选。",
    "3. **Oracle RETURNING INTO** 是 Druid Wall/解析常见语料，需在 P2 决定是补 sqlparser 扩展还是走预处理。",
    "4. Java golden 必须来自固定 commit 的 jar，不得用「同版本号」代替；生成后本文件的 Java 列由 MATCH/MISMATCH 替换 PENDING。",
    "5. 在 golden 完成前，不得把本矩阵当作「方言保真通过」的证据。",
]
OUT.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
print("rendered %s (%d lines)" % (OUT.relative_to(ROOT), len(lines)))
