# Druid → druid-rust 迁移文档

本目录定义 Druid 1.2.28 到 druid-rust 的功能语义完整迁移基线。阅读顺序：

1. [迁移路线图](./1、迁移路线图.md)：范围、阶段、门禁和目标架构。
2. [对象级对照表](./2、对象级对照表.md)：Java 对象到 Rust 落点的迁移账本。
3. [语义迁移对照表](./3、语义迁移对照表.md)：可差分验收的行为契约。
4. [对象名称一致性检查](./4、对象名称一致性检查.md)：canonical 名称与非一对一映射规则。
5. [连接抽象与驱动适配架构](./5、连接抽象与驱动适配架构.md)：`DruidPooledConnection`、`PhysicalConnection`、native pool 与外部池 bridge 的统一设计。

统一口径：这是迁移，不是借鉴；允许 Rust 化实现，但不允许丢失功能语义。

## 三模块治理基线

产品、发布与依赖治理边界只有三个：

| 模块 | Java 来源与职责 |
| :--- | :--- |
| `druid` | Java `/core` 的完整语义迁移；内部包含 core、pool、SQL/Wall、Stat、Dynamic 和默认 Toasty 数据源实现 |
| `druid-admin` | Java `/druid-admin` 的管理、监控、认证与 API 语义迁移 |
| `druid-wrapper` | Java `/druid-wrapper` 及 Rust 数据库生态封装；内部包含 SQLx、RBDC、bb8、deadpool |

原 `druid-core`、`druid-pool`、`druid-sql`、`druid-stats`、`druid-dynamic`、
`druid-toasty`、`druid-sqlx`、`druid-rbdc`、`druid-sqlx-bb8`、
`druid-sqlx-deadpool` 已物理迁入 `druid/src/*` 或
`druid-wrapper/src/*`，独立目录和 workspace member 已删除。

## 文档维护原则

- 原文中的对象清单、字段映射、阶段目标文件、工作量、风险和覆盖率记录应保留。
- 发现失实内容时，增加“当前证据、修订目标、验收条件”，不直接删除有价值的明细。
- “目标文件”与“当前已实现”必须分列，计划不得使用完成状态。
- Java 生态能力使用 `ADAPTER` 或 `PROTOCOL` 迁移，不以运行时形态不同为由移出范围。
- 删除迁移条目必须有 ADR、替代语义和评审记录。
- 迁移文档中的模块归属只能使用三个产品模块；提到已删除的原 crate 时必须标注为历史证据。
