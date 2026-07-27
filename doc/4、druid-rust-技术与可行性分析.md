# druid-rust 技术与可行性分析

> **文档说明**：评估 druid-rust 各核心能力的可行性，给出可验证结论与
> 风险登记，作为技术方案与路线的输入。
>
> **版本**：V1.0.0
> **最后更新**：2026-07-27

---

## 1. 文档信息

| 项目 | 内容 |
| :--- | :--- |
| 文档类型 | 技术与可行性分析 |
| 产品 | druid-rust |
| 版本 | V1.0.0 |
| 状态 | ✅ 待评审 |

### 1.1 关联文档

| 文档 | 关联说明 |
| :--- | :--- |
| [3、市场分析](3、druid-rust-市场与商业分析.md) | 机会输入 |
| [5、技术方案](5、druid-rust-技术方案与路线.md) | 选型输入 |
| [架构文档](../druid-rust-Architecture.zh_CN.md) | 架构合同 |

---

## 2. 可行性评级

| ID | 能力 | 评级 | 关键依据 |
| :--- | :--- | :---: | :--- |
| `F-001` | `druid-core` 零外部依赖 | ✅ 高可行 | Rust workspace 隔离 |
| `F-002` | sqlparser-rs AST 适配 | ✅ 中可行 | sqlparser 0.52 已成熟 |
| `F-003` | HikariCP 风格池 | ✅ 高可行 | deadpool / bb8 已验证模式 |
| `F-004` | Filter 链装饰器 | ✅ 高可行 | Rust trait object + RAII |
| `F-005` | `ArcSwap` 多数据源 | ✅ 高可行 | arc-swap 1.x 已稳定 |
| `F-006` | Prometheus 导出 | ✅ 高可行 | prometheus 0.13 成熟 |
| `F-007` | axum 治理面 | ✅ 高可行 | axum 0.7 + tokio |
| `F-008` | `rbdc` 适配 | ⚠️ 中可行 | rbdc 维护节奏不确定 |
| `F-009` | `sqlx` + `deadpool` 适配 | ✅ 高可行 | 两者均成熟 |
| `F-010` | `sqlx` + `bb8` 适配 | ✅ 高可行 | bb8 成熟 |
| `F-011` | 不实现 Druid Java 注入正则 | ⛔ 不做 | ADR-005 |
| `F-012` | 自实现数据库协议 | ⛔ 不做 | ADR-001 |

## 3. 关键技术验证

### 3.1 sqlparser-rs AST 适配

- **目标**：用 `sqlparser-rs` 把任意 SQL 解析为 `Statement`，然后
  `druid-sql` 把它转换为内部 `ParsedStmt`。
- **可行性**：✅ 中可行。
- **关键风险**：
  - sqlparser-rs 大版本之间 AST 类型会变；锁版本（`sqlparser = "0.52"`）。
  - `?` 占位符到方言的映射需要 dialect 区分。
- **验证计划**：Phase 1 端到端跑 `SELECT 1` 与 `DROP TABLE`。

### 3.2 HikariCP 风格池

- **目标**：`druid-pool` 提供 `max_open / max_idle / acquire_timeout /
  test_while_idle / max_lifetime` 等可配置项。
- **可行性**：✅ 高可行。
- **关键风险**：泄漏检测需要 RAII；`Drop` 实现必须可重入。
- **验证计划**：Phase 1 用 mock factory 跑 10000 acquire/release 循环。

### 3.3 Filter 链装饰器

- **目标**：每个 SQL 在 `exec` 前经过 `BeforeFilter`，在 `exec` 后经过
  `AfterFilter`；任一 before 拒绝即短路。
- **可行性**：✅ 高可行。
- **关键风险**：filter panic 必须 `catch_unwind`。
- **验证计划**：Phase 1 写一个 `LogFilter` 与 `WallFilter` 跑通。

### 3.4 ArcSwap 多数据源

- **目标**：`druid-dynamic::DynamicDataSource` 用 `ArcSwap<DataSourceGroup>`
  提供 lock-free 热切换。
- **可行性**：✅ 高可行。
- **关键风险**：切换期间持有旧引用的请求可能跨版本，需排空策略。
- **验证计划**：Phase 3 跑 1000 RPS 下 1 秒 1 次切换。

### 3.5 Prometheus 导出

- **目标**：`/metrics` 暴露 `druid_pool_*`、`druid_sql_*`、
  `druid_filter_*` 系列指标。
- **可行性**：✅ 高可行。
- **关键风险**：标签基数不可控；用 `moka` 容量约束。
- **验证计划**：Phase 2 跑一次 Prometheus 抓取。

### 3.6 axum 治理面

- **目标**：`druid-admin` 提供 `/druid/api/*` JSON 端点 + `/metrics`。
- **可行性**：✅ 高可行。
- **关键风险**：端口暴露与鉴权；鉴权由宿主反向代理负责。
- **验证计划**：Phase 3 跑 axum 集成测试 + curl smoke。

### 3.7 `rbdc` 适配

- **目标**：`druid-rbdc` 把 `rbdc::Connection` 包装为 `druid_core::Connection`。
- **可行性**：⚠️ 中可行。
- **关键风险**：`rbdc` 维护节奏不确定；保持可选。
- **验证计划**：Phase 2 与 `rbdc` 同步发布。

## 4. 风险登记

| ID | 风险 | 概率 | 影响 | 缓解 |
| :--- | :--- | :---: | :---: | :--- |
| `R-001` | sqlparser-rs 大版本破坏性变更 | 中 | 高 | 锁版本 + 适配层 |
| `R-002` | 上游 sqlx / deadpool / bb8 重大变更 | 中 | 中 | workspace 锁定 |
| `R-003` | `rbdc` 维护停滞 | 中 | 中 | adapter 可选 |
| `R-004` | MSRV 受 sqlparser / sqlx 限制 | 低 | 中 | 文档声明 |
| `R-005` | arc-swap 锁语义误用 | 低 | 中 | 集成测试 |
| `R-006` | Prometheus 标签基数爆炸 | 低 | 中 | 容量 + 白名单 |

## 5. 非目标

- ⛔ 不实现 SQL 注入正则检测（ADR-005）
- ⛔ 不自实现数据库协议（ADR-001）
- ⛔ 不内置 Web UI（仅 JSON）
- ⛔ 不实现数据库 migration 工具
- ⛔ 不实现 ORM 抽象
- ⛔ 不内置鉴权

## 6. 一致性自检清单

- [ ] 每条评级有"关键依据"列。
- [ ] 风险登记与 [5、技术方案](5、druid-rust-技术方案与路线.md) ADR 一致。
- [ ] 非目标与 [架构文档](../druid-rust-Architecture.zh_CN.md) §2 一致。

---

**文档版本**：V1.0.0
**创建日期**：2026-07-27
**最后更新**：2026-07-27
**文档状态**：✅ 待评审