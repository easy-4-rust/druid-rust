#!/usr/bin/env python3
"""Compare Rust and Java golden observations for the P2 dialect matrix."""
from __future__ import annotations

import argparse
import json
import re
from collections import Counter
from pathlib import Path


def normalize(value: str) -> str:
    value = value.strip().rstrip(";")
    # 空白与大小写不参与语义比较：Druid 与 sqlparser 的排版差异不应算 DIFF。
    value = re.sub(r"\s+", "", value)
    return value.lower()


def read_tsv(path: Path) -> dict[tuple[str, str], dict[str, str]]:
    rows: dict[tuple[str, str], dict[str, str]] = {}
    lines = path.read_text(encoding="utf-8-sig").splitlines()
    header = lines[0].split("\t")
    for line in lines[1:]:
        if not line.strip():
            continue
        parts = line.split("\t")
        record = dict(zip(header, parts))
        key = (record["dialect"].strip().lower(), record["case"].strip())
        rows[key] = record
    return rows


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--java", required=True)
    parser.add_argument("--rust", required=True)
    parser.add_argument("--out", required=True)
    parser.add_argument("--summary", required=True)
    args = parser.parse_args()

    java = read_tsv(Path(args.java))
    rust = read_tsv(Path(args.rust))
    keys = sorted(set(java) | set(rust))

    out_lines = ["dialect\tcase\tparse_match\tparameterized_match\treparse_match\tjava_parse\trust_parse\tjava_parameterized\trust_parameterized"]
    per_dialect: Counter[tuple[str, str]] = Counter()
    summary = {"total": 0, "parse_match": 0, "parameterized_match": 0, "reparse_match": 0, "per_dialect": {}}

    for key in keys:
        j = java.get(key)
        r = rust.get(key)
        if j is None or r is None:
            continue
        parse_match = j["parse"] == r["parse"]
        param_match = normalize(j["parameterized"]) == normalize(r["parameterized"])
        reparse_match = j.get("reparse", "n/a") == r.get("reparse", "n/a")
        out_lines.append(
            "\t".join(
                [
                    key[0],
                    key[1],
                    str(parse_match).lower(),
                    str(param_match).lower(),
                    str(reparse_match).lower(),
                    j["parse"],
                    r["parse"],
                    j["parameterized"],
                    r["parameterized"],
                ]
            )
        )
        summary["total"] += 1
        summary["parse_match"] += int(parse_match)
        summary["parameterized_match"] += int(param_match)
        summary["reparse_match"] += int(reparse_match)
        per_dialect[key[0], "total"] += 1
        per_dialect[key[0], "parse"] += int(parse_match)
        per_dialect[key[0], "parameterized"] += int(param_match)

    summary["per_dialect"] = {
        dialect: {
            "total": per_dialect[dialect, "total"],
            "parse_match": per_dialect[dialect, "parse"],
            "parameterized_match": per_dialect[dialect, "parameterized"],
        }
        for dialect in sorted({d for d, _ in per_dialect})
    }

    Path(args.out).write_text("\n".join(out_lines) + "\n", encoding="utf-8")
    Path(args.summary).write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(summary, ensure_ascii=False))
    return 0 if summary["total"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
