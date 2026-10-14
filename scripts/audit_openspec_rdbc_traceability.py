#!/usr/bin/env python3
"""Generate a bounded scope and scenario traceability report for the RDBC OpenSpec change."""

from __future__ import annotations

import argparse
import csv
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[1]
CHANGE = ROOT / "openspec/changes/archive/2026-10-14-close-rdbc-jdbc42-contract-gaps"
TASKS = CHANGE / "tasks.md"
SPECS = CHANGE / "specs"
EVIDENCE = CHANGE / "evidence"
CONFIG = {
    "rdbc-driver-negotiation": ("DN", ["2.1", "2.2", "2.3", "2.4"], ["driver-negotiation.md"]),
    "rdbc-jdbc-temporal-values": ("TV", [f"3.{n}" for n in range(1, 6)], ["temporal-differential.md"]),
    "rdbc-statement-cancellation": ("SC", [f"4.{n}" for n in range(1, 8)], ["cancellation-contract.md", "sqlx-cancellation-capability-matrix.md"]),
    "rdbc-pooled-connection-events": ("PE", [f"5.{n}" for n in range(1, 6)], ["pooled-connection-events.md", "pooled-connection-owner.md"]),
    "rdbc-xa-resource-contract": ("XA", ["6.1", "6.2", "6.3", "6.4", "6.5.1", "6.5.2", "6.5.3", "6.5.4", "6.5.5", "6.5.5a", "6.5.5b", "6.5.5c", "6.5.5d", "6.5.5e"], ["xa-standard-contract.md", "xa-agent-protocol.md", "xa-backend-recovery-probe.md", "xa-rust-client.md"]),
    "rdbc-common-data-source": ("CD", [f"7.{n}" for n in range(1, 5)], ["common-data-source-timeout.md", "common-data-source-logging.md", "common-data-source-diagnostics.md", "common-data-source-migration.md"]),
    "rdbc-jdbc-method-conformance": ("MC", ["1.1", "1.2", "1.3", "1.4", "1.5", *[f"8.{n}" for n in range(1, 9)], *[f"9.{n}" for n in range(1, 5)]], ["preparation.md", "java8-api-method-inventory.md", "method-evidence-ledger.md", "method-evidence-freshness.md"]),
}
SCENARIO_RE = re.compile(r"^#### Scenario:\s*(.+?)\s*$", re.MULTILINE)
REQUIREMENT_RE = re.compile(r"^### Requirement:\s*([A-Z]{2}-\d+)\s+(.+?)\s*$", re.MULTILINE)


class AuditError(ValueError):
    """The specification/task/evidence crosswalk is incomplete or malformed."""


def read_task_states() -> dict[str, bool]:
    text = TASKS.read_text(encoding="utf-8")
    return {match.group(2): match.group(1) == "x" for match in re.finditer(r"^\s*- \[([ x])\] (\d+(?:\.\d+){1,2}[a-z]?)\b", text, re.MULTILINE)}


def parse_scenarios(path: Path) -> list[tuple[str, str]]:
    text = path.read_text(encoding="utf-8")
    headings = list(re.finditer(r"^### Requirement:\s*([A-Z]{2}-\d+)\s+(.+?)\s*$", text, re.MULTILINE))
    if not headings:
        raise AuditError(f"no requirement heading with identifier in {path}")
    results = []
    for index, requirement in enumerate(headings):
        end = headings[index + 1].start() if index + 1 < len(headings) else len(text)
        section = text[requirement.end():end]
        scenarios = SCENARIO_RE.findall(section)
        if not scenarios:
            raise AuditError(f"requirement {requirement.group(1)} has no scenario")
        results.extend((requirement.group(1), scenario) for scenario in scenarios)
    return results


def audit(output_dir: Path) -> tuple[int, int]:
    task_states = read_task_states()
    spec_paths = sorted(SPECS.glob("*/spec.md"))
    if len(spec_paths) != 7 or set(path.parent.name for path in spec_paths) != set(CONFIG):
        raise AuditError(f"expected the seven configured capability specs, found {len(spec_paths)}")
    scenario_rows = []
    scope_rows = []
    for spec in spec_paths:
        code, task_ids, evidence_names = CONFIG[spec.parent.name]
        missing_tasks = [task for task in task_ids if task not in task_states]
        missing_evidence = [name for name in evidence_names if not (EVIDENCE / name).is_file()]
        if missing_tasks or missing_evidence:
            raise AuditError(f"{spec.parent.name}: missing tasks={missing_tasks}, evidence={missing_evidence}")
        state = "TASKS_COMPLETE" if all(task_states[task] for task in task_ids) else "PENDING_TASKS"
        if code != "MC":
            scope_rows.append({"gap_code": code, "capability": spec.parent.name,
                               "tasks": ",".join(task_ids), "evidence": ";".join(f"evidence/{name}" for name in evidence_names),
                               "task_status": state, "completion_claim": "NOT_CLAIMED"})
        scenarios = parse_scenarios(spec)
        for requirement, scenario in scenarios:
            scenario_rows.append({"capability": spec.parent.name, "requirement": requirement,
                                  "scenario": scenario, "tasks": ",".join(task_ids),
                                  "evidence": ";".join(f"evidence/{name}" for name in evidence_names),
                                  "traceability": "TRACEABLE_TO_PENDING_TASKS" if state == "PENDING_TASKS"
                                  else "TRACEABLE_TO_COMPLETED_TASKS_NOT_SCENARIO_PASS"})
    if len({(row["capability"], row["requirement"], row["scenario"]) for row in scenario_rows}) != len(scenario_rows):
        raise AuditError("duplicate capability/requirement/scenario identity")
    output_dir.mkdir(parents=True, exist_ok=True)
    inventory_path = ROOT / "docs/verification/rdbc-first/java8-public-method-inventory.tsv"
    method_ledger_path = ROOT / "docs/verification/rdbc-first/java8-method-evidence.tsv"
    with inventory_path.open(encoding="utf-8") as source:
        inventory_rows = list(csv.DictReader((line for line in source if not line.startswith("#")), delimiter="\t"))
    with method_ledger_path.open(encoding="utf-8") as source:
        ledger_rows = list(csv.DictReader((line for line in source if not line.startswith("#")), delimiter="\t"))
    method_ids = {row["identity"] for row in inventory_rows if row["kind"] in {"method", "inherited_method"}}
    method_count = len(method_ids)
    linked_methods = sum(row["identity"] in method_ids and row["capability"] != "UNASSESSED"
                         for row in ledger_rows)
    mc_tasks = CONFIG["rdbc-jdbc-method-conformance"][1]
    global_rows = [
        {"remainder": "METHOD_LEVEL_API_COVERAGE", "scope": "全量 JDBC Java 方法行为/适配器/实库证据",
         "observed": f"{method_count-linked_methods} of {method_count} method identities remain unassessed",
         "impact": "当前清单是分母；未评估项不得算作支持或精确不支持",
         "evidence": "docs/verification/rdbc-first/java8-method-evidence.tsv; evidence/method-evidence-ledger.md",
         "status": "OUTSIDE_CURRENT_BOUNDED_SCENARIO_SET", "added_to_tasks": "NO"},
        {"remainder": "MC06_RESOURCES_AND_RESULT_SETS", "scope": "Array/LOB/SQLXML/SQLData/RowSet及流式/缓冲结果集有限验收",
         "observed": ",".join(task for task in mc_tasks if task.startswith(("8.6", "8.7", "8.8"))),
         "impact": "资源/游标/owner关闭及known_row_count边界尚未通过本批完成门禁",
         "evidence": "openspec/changes/close-rdbc-jdbc42-contract-gaps/tasks.md; evidence/method-evidence-ledger.md",
         "status": "PENDING_IN_THIS_CHANGE", "added_to_tasks": "NO_NEW_TASKS"},
        {"remainder": "P04_P07_PROGRAM_COMPLETION", "scope": "跨项目原长期任务和生产就绪总验收",
         "observed": "本变更仅覆盖提案定义的七项增量；不包含原计划全部 P04-P07 关闭条件",
         "impact": "不能据本变更报告推断 18 项历史任务或全项目生产就绪已关闭",
         "evidence": "openspec/changes/close-rdbc-jdbc42-contract-gaps/proposal.md#规格归属和范围",
         "status": "OUTSIDE_THIS_CHANGE", "added_to_tasks": "NO"},
        {"remainder": "JDBC_TCK_AND_ALL_BACKENDS", "scope": "第三方 JDBC TCK/全部数据库适配器认证",
         "observed": "无 TCK 完成声明；不同后端需按方法能力分别验证",
         "impact": "具体数据库和可选能力不可由标准类型声明推断",
         "evidence": "openspec/changes/close-rdbc-jdbc42-contract-gaps/evidence/method-evidence-ledger.md",
         "status": "NOT_CERTIFIED", "added_to_tasks": "NO"},
    ]
    for filename, rows in (("spec-scenario-traceability.tsv", scenario_rows), ("scope-gap-status.tsv", scope_rows),
                           ("global-remainder.tsv", global_rows)):
        path = output_dir / filename
        fields = list(rows[0])
        with path.open("w", encoding="utf-8", newline="") as target:
            writer = csv.DictWriter(target, fieldnames=fields, delimiter="\t", lineterminator="\n")
            writer.writeheader()
            writer.writerows(rows)
    return len(scenario_rows), len(scope_rows)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output-dir", type=Path, default=EVIDENCE)
    arguments = parser.parse_args()
    try:
        scenarios, capabilities = audit(arguments.output_dir)
    except (AuditError, OSError) as error:
        print(f"TRACEABILITY_FAIL: {error}", file=sys.stderr)
        return 1
    print(f"TRACEABILITY_PASS capabilities={capabilities} scenarios={scenarios} output={arguments.output_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
