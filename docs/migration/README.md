# druid-rust 迁移文档

> **2026-11-02 当前状态：** OpenSpec `migration-completion-roadmap` 已完成 24/68；默认全量测试 7/7 crate 通过，`--all-features` 仅 gRPC 目标受 Windows 回环阻塞；全仓 SOURCE_PARITY 4759 仍为 `MISSING/NOT_RUN`，最终判定 **NOT_READY**。当前证据索引见 [执行进展](../verification/migration-completion-20261102/progress.md)。

后续推进：[ExceptionSorter 逐方法语义核验](../verification/method-semantics-20261101/README.md)（12 对象、30 个显式签名）。已修复 Unicode 错误码解析真实偏差；复用源测试和 live differential，保留日志、宿主配置与全量验收未完成项。

本目录是 druid-rust 的**迁移事实与验收**入口。先读 [2026-11-01 当前代码核对](../verification/current-code-audit-20261101/README.md)：已区分文档滞后、已有实现缺验证、真实偏差，并保留待核实队列。

四份主文档顶部的当前核对摘要优先；下方旧自动生成内容是历史快照。旧 `objects.tsv` 是名称启发式台账，不能作为功能完成/缺失的唯一状态源。尤其 `MISSING` 不表示相应行为不存在，`semantic_test=yes` 也不表示测试已执行。

| 文档 | 作用 |
|---|---|
| [迁移验收规范](迁移验收规范.md) | canonical 验收口径：状态词表、证据等级、红线、门禁 |
| [迁移路线图](迁移路线图.md) | 阶段（P0–P6）、依赖、风险、ADR 冲突与决策 |
| [对象级对照表](对象级对照表.md) | 1 Java 对象 = 1 行；路径、状态、证据 |
| [语义迁移对照表](语义迁移对照表.md) | 调用链与语义域；六类可观察语义 |
| [对象名称一致性检查](对象名称一致性检查.md) | 名称/文件/目录/主类型对齐与缺口 |
| `inventory/` | 机器台账：`objects.tsv`、`methods.tsv`、`rust_extra.tsv`、`redlines.tsv`、`summary.json` |

## 更新候选证据清单

```bash
python scripts/reconcile_migration_evidence.py \
  --java-root E:/workspaces/workspace-github/druid \
  --rust-root . --output docs/verification/<本次审计标识>/inventory
```

## 阅读顺序

1. 先读当前代码核对报告，确认基线、运行范围及未核实边界；
2. 读验收规范，区分名称候选、语义状态与证据等级；
3. 结合对象表、语义表和名称检查定位实现，不用旧数量推导完成率；
4. 仅对确认的缺口沿既有 OpenSpec / ADR / 计划继续实施。

新清单是全模块文件普查，不是嵌套对象/方法全量验收账本。手工结论必须引用源码或运行证据。旧 `migration_inventory.py` 与模板保留作历史复现；`render_migration_docs.py` 遇到已核对文档会在写入前拒绝覆盖，不应移除保护标记来强行生成。
