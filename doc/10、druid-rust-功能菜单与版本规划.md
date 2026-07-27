# druid-rust 功能菜单与版本规划

> **文档说明**：冻结 druid-rust 的导航、功能清单、优先级与版本分布。
>
> **版本**：V1.0.0
> **最后更新**：2026-07-27

---

## 1. 文档信息

| 项目 | 内容 |
| :--- | :--- |
| 文档类型 | 功能菜单与版本规划 |
| 产品 | druid-rust |
| 版本 | V1.0.0 |
| 状态 | ✅ 待评审 |

### 1.1 关联文档

| 文档 | 关联说明 |
| :--- | :--- |
| [6、版本规划](6、druid-rust-产品与版本规划.md) | 版本矩阵 |
| [9、视觉 DNA](9、druid-rust-视觉与交互DNA规范.md) | 端点形状 |
| [架构文档 §19](../druid-rust-Architecture.zh_CN.md) | 运维端点 |

---

## 2. 一级菜单（API Surface）

druid-rust 没有 GUI；以下菜单是 API surface + Crate surface 的导航。

```mermaid
mindmap
  root(("druid-rust"))
    Crate 表面
      druid-core
      druid-sql
      druid-pool
      druid-stats
      druid-dynamic
      druid-rbdc
      druid-sqlx-deadpool
      druid-sqlx-bb8
      druid-admin
    HTTP 端点（druid-admin）
      数据源
        /druid/api/datasources
        /druid/api/datasources/{name}
      SQL
        /druid/api/sql/top
        /druid/api/sql/slow
      Wall
        /druid/api/wall
      活跃连接
        /druid/api/active
      监控
        /metrics
```

## 3. 优先级与版本标注

| 标注 | 含义 |
| :---: | :--- |
| P0 | 必须实现，阻塞发布 |
| P1 | 重要，影响核心体验 |
| P2 | 期望，提升用户体验 |
| P3 | 可选，低优先级增强 |
| Phase 0 | 已交付（占位骨架） |
| V1 | 核心闭环 |
| V2 | 适配 + 统计 |
| V3 | 动态 + 治理 |

## 4. 功能清单

### 4.1 Core / trait surface

| 功能 | 优先级 | 版本 | 验收 |
| :--- | :---: | :--- | :--- |
| `Connection` trait | P0 | V1 | trait 测试 |
| `Driver` trait | P0 | V1 | trait 测试 |
| `Pool` trait | P0 | V1 | trait 测试 |
| `ConnectionFactory` trait | P0 | V1 | trait 测试 |
| `BeforeFilter` / `AfterFilter` trait | P0 | V1 | trait 测试 |
| `LoadBalancer` trait | P1 | V3 | trait 测试 |
| `ExecContext` / `FilterEvent` | P1 | V1 | 文档化 |
| `Error` 统一枚举 | P0 | V1 | thiserror |

### 4.2 SQL

| 功能 | 优先级 | 版本 | 验收 |
| :--- | :---: | :--- | :--- |
| sqlparser-rs AST 适配 | P0 | V1 | `SELECT 1` 端到端 |
| `WallConfig` 配置结构 | P0 | V1 | 单元测试 |
| 默认 deny `DROP` / `TRUNCATE` | P0 | V1 | 集成测试 |
| `update_where = required` | P0 | V1 | 集成测试 |
| `delete_where = required` | P0 | V1 | 集成测试 |
| 参数化 + 指纹 | P0 | V2 | 单元测试 |
| 函数黑名单 | P1 | V2 | 集成测试 |
| 表白名单模式 | P2 | V2 | 集成测试 |
| 子查询深度限制 | P1 | V2 | 集成测试 |

### 4.3 Pool

| 功能 | 优先级 | 版本 | 验收 |
| :--- | :---: | :--- | :--- |
| 空闲队列 + 调度器 | P0 | V1 | 单元测试 |
| `max_open` / `max_idle` | P0 | V1 | trait setter |
| `acquire_timeout` | P0 | V1 | trait setter |
| `test_while_idle` | P1 | V2 | 集成测试 |
| `max_lifetime` | P1 | V2 | 集成测试 |
| 泄漏检测（持有时长阈值） | P1 | V2 | 压测 |
| 驱逐任务（idle 回收） | P2 | V2 | 单元测试 |
| `warmup`（预热） | P2 | V2 | 集成测试 |

### 4.4 Stats

| 功能 | 优先级 | 版本 | 验收 |
| :--- | :---: | :--- | :--- |
| `MergedSqlStat` 聚合 | P0 | V2 | 单元测试 |
| `SqlMerger` 指纹 | P0 | V2 | 合并率测试 |
| 延迟直方图 | P0 | V2 | 单元测试 |
| Prometheus 导出 | P0 | V2 | 抓取测试 |
| 慢 SQL 阈值 | P1 | V2 | 集成测试 |
| Top SQL 视图 | P1 | V3 | admin |
| 错误率告警 | P2 | V3 | admin |

### 4.5 Dynamic

| 功能 | 优先级 | 版本 | 验收 |
| :--- | :---: | :--- | :--- |
| `ArcSwap<DataSourceGroup>` | P0 | V3 | 切换测试 |
| `SqlHint` 路由 | P0 | V3 | 集成测试 |
| `RoundRobinLoadBalancer` | P1 | V3 | 单元测试 |
| `RandomLoadBalancer` | P2 | V3 | 单元测试 |
| `WeightedLoadBalancer` | P2 | V3 | 单元测试 |
| 读写分离 | P1 | V3 | 集成测试 |
| 切换期间不丢请求 | P0 | V3 | 压测 |

### 4.6 Adapter

| 功能 | 优先级 | 版本 | 验收 |
| :--- | :---: | :--- | :--- |
| `druid-rbdc` 适配 | P1 | V2 | Postgres 冒烟 |
| `druid-sqlx-deadpool` 适配 | P0 | V2 | Postgres / MySQL 冒烟 |
| `druid-sqlx-bb8` 适配 | P1 | V2 | Postgres / MySQL 冒烟 |
| MSSQL（`tiberius`） | P3 | V2+ | 计划 |
| SQLite / DuckDB / Turso | ⛔ | 不做 | 显式不在范围 |

### 4.7 Admin

| 功能 | 优先级 | 版本 | 验收 |
| :--- | :---: | :--- | :--- |
| axum router 装配 | P0 | V3 | 单元测试 |
| `/druid/api/datasources` | P0 | V3 | curl smoke |
| `/druid/api/sql/top` | P0 | V3 | curl smoke |
| `/druid/api/sql/slow` | P1 | V3 | curl smoke |
| `/druid/api/wall` | P1 | V3 | curl smoke |
| `/druid/api/active` | P1 | V3 | curl smoke |
| `/metrics` | P0 | V3 | Prometheus 抓取 |
| 优雅停机 | P1 | V3 | axum graceful shutdown |

## 5. 一致性自检清单

- [ ] 所有 P0 功能在 V1 / V2 / V3 内有明确退出条件。
- [ ] 功能与 [6、版本规划](6、druid-rust-产品与版本规划.md) §3 版本矩阵一致。
- [ ] 端点形状与 [9、视觉 DNA](9、druid-rust-视觉与交互DNA规范.md) §4 一致。
- [ ] 不在范围功能（SQLite / DuckDB）显式标 ⛔。

---

**文档版本**：V1.0.0
**创建日期**：2026-07-27
**最后更新**：2026-07-27
**文档状态**：✅ 待评审