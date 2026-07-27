# druid-rust V1 功能菜单与版本规划

> **文档说明**：V1 范围内精简版功能列表与版本分布。
>
> **版本**：V1.0.0
> **最后更新**：2026-07-27

---

## 1. 文档信息

| 项目 | 内容 |
| :--- | :--- |
| 文档类型 | V1 功能菜单与版本规划 |
| 产品 | druid-rust |
| 版本 | V1.0.0 |
| 状态 | ✅ 待评审 |

### 1.1 关联文档

| 文档 | 关联说明 |
| :--- | :--- |
| [10、root 功能菜单](../10、druid-rust-功能菜单与版本规划.md) | 全量菜单 |
| [V1/5、PRD](5、druid-rust-PRD文档-V1.md) | 需求规格 |
| [架构文档 §23](../../druid-rust-Architecture.zh_CN.md) | 实施路线 |

---

## 2. V1 功能清单

### 2.1 `druid-core`

| ID | 功能 | 优先级 | 验收 |
| :--- | :--- | :---: | :--- |
| V1-C-01 | `Connection` trait | P0 | trait 测试 |
| V1-C-02 | `Driver` trait | P0 | trait 测试 |
| V1-C-03 | `Pool` trait | P0 | trait 测试 |
| V1-C-04 | `ConnectionFactory` trait | P0 | trait 测试 |
| V1-C-05 | `BeforeFilter` / `AfterFilter` trait | P0 | trait 测试 |
| V1-C-06 | `Error` 统一枚举 | P0 | thiserror |

### 2.2 `druid-sql`

| ID | 功能 | 优先级 | 验收 |
| :--- | :--- | :---: | :--- |
| V1-S-01 | sqlparser-rs 解析 `SELECT 1` | P0 | 单元测试 |
| V1-S-02 | `WallConfig` builder | P0 | 单元测试 |
| V1-S-03 | 默认 deny `DROP TABLE` | P0 | 集成测试 |
| V1-S-04 | 默认 deny `TRUNCATE` | P0 | 集成测试 |
| V1-S-05 | `UPDATE` 必须带 WHERE | P0 | 集成测试 |
| V1-S-06 | `DELETE` 必须带 WHERE | P0 | 集成测试 |

### 2.3 `druid-pool`

| ID | 功能 | 优先级 | 验收 |
| :--- | :--- | :---: | :--- |
| V1-P-01 | `max_open` / `max_idle` | P0 | 单元测试 |
| V1-P-02 | `acquire_timeout` | P0 | 单元测试 |
| V1-P-03 | `PooledConnection::drop` 归还 | P0 | 10000 循环 |
| V1-P-04 | `FilterChain` 装配 | P0 | 集成测试 |

### 2.4 mock driver

| ID | 功能 | 优先级 | 验收 |
| :--- | :--- | :---: | :--- |
| V1-M-01 | mock `ConnectionFactory` | P0 | 测试通过 |
| V1-M-02 | `SELECT 1` 端到端 | P0 | 集成测试 |

## 3. V1 不做的功能（占位）

| ID | 功能 | 推迟到 |
| :--- | :--- | :--- |
| V1-N-01 | `druid-stats` SQL 合并 | V2 |
| V1-N-02 | Prometheus 导出 | V2 |
| V1-N-03 | `druid-rbdc` | V2 |
| V1-N-04 | `druid-sqlx-deadpool` | V2 |
| V1-N-05 | `druid-sqlx-bb8` | V2 |
| V1-N-06 | `druid-dynamic` | V3 |
| V1-N-07 | `druid-admin` | V3 |
| V1-N-08 | Web UI | V3+ 评估 |
| V1-N-09 | SQL 注入正则 | 永不 |

## 4. 一致性自检清单

- [ ] V1 功能 ID 与 [V1/5、PRD](5、druid-rust-PRD文档-V1.md) FR ID 对应。
- [ ] V1 不做的功能 ID 与 [10、root 功能菜单](../10、druid-rust-功能菜单与版本规划.md) §4 一致。
- [ ] 版本分布与 [6、版本规划](../6、druid-rust-产品与版本规划.md) §3 一致。

---

**文档版本**：V1.0.0
**创建日期**：2026-07-27
**最后更新**：2026-07-27
**文档状态**：✅ 待评审