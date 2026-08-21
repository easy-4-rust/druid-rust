# druid-rust Superpowers 规格驱动开发（SDD）体系

> **建立日期**：2026-08-12
> **基线**：Alibaba Druid Java 1.2.28（commit `33824c3dec1612711f9bb4e409319bcab2e4cd0e`）+ druid-rust workspace 当前 HEAD
> **方法论**：Superpowers SDD（Spec-Driven Development）
> **迁移原则**：这是迁移，不是借鉴；允许 Rust 化实现，但不允许丢失功能语义

---

## 概述

本目录是 druid-rust 项目的规格驱动开发中心。所有迁移工作的设计依据（specs/）和执行清单（plans/）均在此管理。

### 如何使用

- **specs/** 是设计依据：迁移治理、连接架构、驱动矩阵、对象/语义对照、命名审计等横切关注点的权威规格。
- **plans/** 是执行清单：按模块分组的迁移实施计划，每个计划含 checkbox 跟踪的分阶段任务。
- **状态基于代码事实**：每处 DONE 标注引用具体 crate 路径或文件数或测试覆盖率；TODO 引用 Java 侧缺失对象清单。
- **架构文档**：[druid-rust-Architecture.zh_CN.md](../druid-rust-Architecture.zh_CN.md) 独立于 superpowers 的权威架构文档。

### 当前源码治理基线（2026-08-21）

| 模块 | Java 来源与职责 | Rust crate |
| :--- | :--- | :--- |
| `druid` | Java `/core` 语义迁移；Pool、Filter、SQL Parser、Wall、Stat、Dynamic、logical RDBC handles | `crates/druid/` |
| `druid-admin` | Java `/druid-admin` 管理、HTTP/UI、repository 与当前独立 binary | `crates/druid-admin/` |
| `druid-wrapper` | Java `/druid-wrapper` 与 Rust driver/外部池生态 | `crates/druid-wrapper/` |
| `druid-metrics` | typed snapshot、sampler、aggregator、timeline、协议组件 | `crates/druid-metrics/` |
| `druid-bench` | 合同、差分和性能验证成员；不计产品 crate | `crates/druid-bench/` |

### RDBC-First 五产品 Crate 目标（已批准，ADR-CRATE-002）

> `ADR-CRATE-002` 已取代 `ADR-CRATE-001`。保留 `druid` 核心产品身份，新增
> `druid-rdbc`；不再执行 `druid-core + druid facade` 拆分。

| 目标 Crate | 职责 | 依赖 |
| :--- | :--- | :--- |
| `druid-rdbc` | Types、Errors、Physical Driver SPI、Resources、Java compatibility | -- |
| `druid` | Pool、Filter、SQL Parser、Wall、Stat、Dynamic；保留 `druid::sql::*` | druid-rdbc |
| `druid-wrapper` | Toasty/SQLx/RBDC/DuckDB/libSQL/HTTP SQL/JDBC Agent、bb8/deadpool、driver tooling | druid-rdbc + druid |
| `druid-metrics` | registry、sampler、bounded aggregator、timeline、唯一 gRPC protocol/runtime | druid |
| `druid-admin` | ingest repository、REST、认证、兼容静态 UI、独立 binary | druid-metrics |

---

## 设计规格（specs/）

| 日期 | 文件 | 主题 | 状态 |
|---|---|---|---|
| 2026-08-12 | [migration-governance.md](specs/2026-08-12-migration-governance.md) | 迁移治理（状态定义、差分流水线、SEM-NFR-*、命名规则、门禁） | 设计完成 |
| 2026-08-12 | [connection-abstraction-design.md](specs/2026-08-12-connection-abstraction-design.md) | 连接抽象与驱动适配架构（PhysicalConnection SPI、ADR-CONN-*） | 设计完成 |
| 2026-08-12 | [toasty-builtin-datasource.md](specs/2026-08-12-toasty-builtin-datasource.md) | Toasty 内置数据源标准实现 | 设计完成 |
| 2026-08-12 | [driver-integration-matrix.md](specs/2026-08-12-driver-integration-matrix.md) | 数据库驱动集成矩阵（D0-D5 验证级别、80 产品目录） | 设计完成 |
| 2026-08-12 | [object-and-semantic-mapping.md](specs/2026-08-12-object-and-semantic-mapping.md) | 对象级与语义迁移对照聚合（3 模块 × 对象级 + 语义表） | 设计完成 |
| 2026-08-12 | [object-naming-audit.md](specs/2026-08-12-object-naming-audit.md) | 对象名称一致性审计聚合（3 模块） | 设计完成 |
| 2026-08-21 | [rdbc-first-five-crate-architecture.md](specs/2026-08-21-rdbc-first-five-crate-architecture.md) | ADR-CRATE-002、RDBC 合同、目标 DAG、运行时与完成门禁 | **ACCEPTED / 当前架构事实源** |

---

## 实施计划（plans/）

### 模块覆盖总表

| 模块分组 | plan 文件 | 覆盖范围 | 状态 |
|---|---|---|---|
| 跨模块总路线 | [master-cross-module-roadmap.md](plans/2026-08-12-master-cross-module-roadmap.md) | P0-P10 全局阶段、冻结基线、门禁 | 进行中 |
| 核心模块 | [druid-core-migration.md](plans/2026-08-12-druid-core-migration.md) | C0-C9 核心迁移阶段 | 进行中 |
| 管理端 | [druid-admin-migration.md](plans/2026-08-12-druid-admin-migration.md) | A0-A9 管理端阶段 | IMPLEMENTED_UNVERIFIED |
| 驱动封装 | [druid-wrapper-migration.md](plans/2026-08-12-druid-wrapper-migration.md) | W0-W9 驱动封装阶段 | 进行中 |

### 2026-08-21 RDBC-First 执行计划集

> 以下计划是当前唯一执行清单。计划文件存在不代表实现完成。

| 顺序 | plan 文件 | 交付物 | 状态 |
|---:|---|---|---|
| 00 | [RDBC-First Master Roadmap](plans/2026-08-21-rdbc-first-master-roadmap.md) | 跨计划顺序、依赖、状态和最终同步 | 待执行 |
| 01 | [External Pool RDBC Conformance](plans/2026-08-21-external-pool-rdbc-conformance.md) | direct SQLx / bb8 / deadpool 统一合同 | 待执行 |
| 02 | [Metrics/Admin Single Protocol](plans/2026-08-21-metrics-admin-single-protocol.md) | 唯一 proto、真实 payload/gRPC、安全默认值 | 待执行 |
| 03 | [druid-rdbc Extraction](plans/2026-08-21-druid-rdbc-extraction.md) | 无行为抽取低层合同 crate、保持旧路径 | 待执行 |
| 04 | [RDBC Contract Hardening](plans/2026-08-21-rdbc-contract-hardening.md) | 错误、能力、resource cancellation、async cursor | 待执行 |
| 05 | [Driver Registry and Feature Boundaries](plans/2026-08-21-driver-registry-feature-boundaries.md) | 单一 registry、真实 optional dependencies、发布边界 | 待执行 |
| 06 | [Runtime Production Hardening](plans/2026-08-21-runtime-production-hardening.md) | 有界队列、supervisor、shutdown、TTL、认证 | 待执行 |
| 07 | [RDBC-First Verification](plans/2026-08-21-rdbc-first-verification.md) | 依赖、API、语义、可靠性、安全、性能和发布门禁 | 待执行 |

### 2026-08-20 历史计划

原 `druid-core + druid facade` 八份计划均已在文件头标注 `SUPERSEDED` 或
`SUPERSEDED_FOR_REMAINING_WORK`。完成/失败证据继续保留，但不得从旧 checkbox 恢复执行。

### 按模块状态明细

| 模块 | Rust 文件 | Java 对象 | 测试 | 状态 | 所属 plan |
|---|---:|---:|---|---|---|
| druid/core | 325 | 1,644 | 540+ workspace 测试 | C0-C6 DONE, C7-C9 PARTIAL | 核心模块 |
| druid-admin | 49 | 13 | 待统一验证 | IMPLEMENTED_UNVERIFIED | 管理端 |
| druid-wrapper | 95 | 13+ | adapter 合同测试 | W0-W6 DONE, W7-W9 PARTIAL | 驱动封装 |

---

## 冻结基线

| 项目 | 冻结值 |
| :--- | :--- |
| Java 源仓库 | `/Users/wandl/workspaces/workspace-github/druid` |
| Java 版本 | `1.2.28`（tag `1.2.28`） |
| Java 提交 | `33824c3dec1612711f9bb4e409319bcab2e4cd0e` |
| Java 主源码 | core 1,644 个 `.java`；全仓 1,719 个 `.java` |
| Rust 源仓库 | `/Users/wandl/workspaces/workspace-github-easy-4-rust/druid-rust` |
| Rust 当前产品模块 | `druid`、`druid-admin`、`druid-wrapper`、`druid-metrics` |
| Rust 当前验证成员 | `druid-bench` |
| Rust 目标产品模块 | 新增 `druid-rdbc`，形成 ADR-CRATE-002 五产品 Crate |
| Rust 工具链 | MSRV 1.95；默认工具链 1.97.1 |

---

## 归并记录

> 2026-08-12 从原 `docs/{README.md, 迁移总路线图.md, 连接抽象与驱动适配架构.md}` 和 `docs/{druid,druid-admin,druid-wrapper}/` 目录归并为 superpowers 体系。

| 原文件 | canonical 承接位置 |
| :--- | :--- |
| `docs/README.md` | `specs/migration-governance.md` + 本 README 三模块治理段 |
| `docs/迁移总路线图.md` | `plans/master-cross-module-roadmap.md` |
| `docs/连接抽象与驱动适配架构.md` | `specs/connection-abstraction-design.md` |
| `docs/druid/迁移路线图.md` | `plans/druid-core-migration.md` |
| `docs/druid/对象级对照表.md` + `语义迁移对照表.md` | `specs/object-and-semantic-mapping.md`（druid 段） |
| `docs/druid/对象名称一致性检查.md` | `specs/object-naming-audit.md`（druid 段） |
| `docs/druid/Toasty-内置数据源标准实现.md` | `specs/toasty-builtin-datasource.md` |
| `docs/druid-admin/迁移路线图.md` | `plans/druid-admin-migration.md` |
| `docs/druid-admin/对象级对照表.md` + `语义迁移对照表.md` | `specs/object-and-semantic-mapping.md`（admin 段） |
| `docs/druid-admin/对象名称一致性检查.md` | `specs/object-naming-audit.md`（admin 段） |
| `docs/druid-wrapper/迁移路线图.md` | `plans/druid-wrapper-migration.md` |
| `docs/druid-wrapper/对象级对照表.md` + `语义迁移对照表.md` | `specs/object-and-semantic-mapping.md`（wrapper 段） |
| `docs/druid-wrapper/对象名称一致性检查.md` | `specs/object-naming-audit.md`（wrapper 段） |
| `docs/druid-wrapper/数据库驱动集成矩阵.md` | `specs/driver-integration-matrix.md` |
