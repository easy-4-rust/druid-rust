# 8.4：变更范围、规范场景与剩余缺口报告

## 本次六类功能缺口

`scope-gap-status.tsv` 分别列出 DN 驱动协商、TV 时间值、SC 取消控制、PE 池事件、XA 资源和 CD 公共数据源六类功能范围。其对应任务组均已打勾，且各有定向证据文档。`completion_claim=NOT_CLAIMED` 是刻意保留的边界：它表示任务清单状态，不代替 8.5–9.x 的最终场景、架构、资源矩阵和兼容性总验收。

第七份规范 MC（方法一致性）是上述六项的交叉证据规范，不属于“六类功能缺口”之一。`spec-scenario-traceability.tsv` 逐条收录七份规范的 96 个 `Requirement/Scenario`，每条都有对应任务组和存在的批次证据文件。状态只表示可追溯到已完成或待完成的任务；`TRACEABLE_TO_COMPLETED_TASKS_NOT_SCENARIO_PASS` 不声称场景已独立通过。MC 当前标为待完成，因为 8.4–8.8 与 9.x 仍未关闭。

## 全局剩余工作

`global-remainder.tsv` 将本 change 内部未完成项与全局范围分开：

- 冻结 Java 方法分母共有 1,573 个直接/继承方法身份；9 个已链接，另有 1,564 个保持 `UNASSESSED`。它们不是自动生成的支持/不支持结论。
- MC-06 的有限资源与结果集验收仍由 8.6–8.8 待执行；本报告没有替这些任务补造测试或状态。
- 原 P04–P07 多项目完成度不属于本 change。提案明确要求不得据此推断此前列出的长期任务完成或项目生产就绪。
- 第三方 JDBC TCK 与所有数据库/可选后端均未在本 change 中认证。

报告生成器只读取七份规范、现有任务、批次证据和 API 台账，并写出三份 TSV。测试断言场景数为 96、六类功能行数为 6、方法级未评估数为 1,564，并比较任务文件生成前后的字节，证明没有自动增加实施任务。

## 实际验证

| 命令 | 结果 |
|---|---|
| `python scripts/audit_openspec_rdbc_traceability.py` | `TRACEABILITY_PASS capabilities=6 scenarios=96` |
| `python -m unittest discover -s tools/java8-api-inventory -p 'test_*.py' -v` | 11 passed；含范围/场景交叉表审计 |
| `openspec validate close-rdbc-jdbc42-contract-gaps --strict --no-interactive` | valid |
| `git diff --check` | 退出码 0 |

## 报告文件 SHA-256

```text
scripts/audit_openspec_rdbc_traceability.py E58B1FDF148CD8C0ED1E7F42FD6F38C8292D33F613DFAB4499F6890B32E15E65
evidence/spec-scenario-traceability.tsv 45C16B7ECDDF81B1C8D636CFCF91BE847519D108A59D8595FCCC63C0499A2B69
evidence/scope-gap-status.tsv B9ABB61EA6CC4E7442465E6278DF6479C1EDE48B21EDBCC164D86175597374E7
evidence/global-remainder.tsv D9A350D5404D0042CF1048873CCA27CBAD0C52EB0B2678E4DF880FE897C34F96
```
