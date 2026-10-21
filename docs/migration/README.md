# druid-rust 迁移文档

本目录是 druid-rust 的**迁移事实与验收**入口。四份主文档由脚本从源码生成事实区，禁止手改统计与对象行。

| 文档 | 作用 |
|---|---|
| [迁移验收规范](迁移验收规范.md) | canonical 验收口径：状态词表、证据等级、红线、门禁 |
| [迁移路线图](迁移路线图.md) | 阶段（P0–P6）、依赖、风险、ADR 冲突与决策 |
| [对象级对照表](对象级对照表.md) | 1 Java 对象 = 1 行；路径、状态、证据 |
| [语义迁移对照表](语义迁移对照表.md) | 调用链与语义域；六类可观察语义 |
| [对象名称一致性检查](对象名称一致性检查.md) | 名称/文件/目录/主类型对齐与缺口 |
| `inventory/` | 机器台账：`objects.tsv`、`methods.tsv`、`rust_extra.tsv`、`redlines.tsv`、`summary.json` |

## 重新生成

```bash
python scripts/migration_inventory.py \
  --java-root E:/workspaces/workspace-github/druid/core/src/main/java \
  --java-repo E:/workspaces/workspace-github/druid \
  --java-test-root E:/workspaces/workspace-github/druid/core/src/test/java \
  --rust-root . --out-dir docs/migration
python scripts/render_migration_docs.py
```

## 阅读顺序

1. 先读验收规范，确认状态词表与证据等级；
2. 看对象级对照表的“汇总 + 结构红线”，判断分母与债务；
3. 看语义表确认能力缺口；
4. 看名称检查确认命名/路径问题；
5. 最后按路线图 P0–P6 执行。
