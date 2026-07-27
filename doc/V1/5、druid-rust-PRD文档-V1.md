# druid-rust PRD 文档（V1）

> **文档说明**：V1 版本的产品需求规格说明，包含目标、范围、跨模块规则
> 与统一验收口径；模块内部细节由 [V1/4、功能规划](4、druid-rust-功能与界面规划-V1.md)
> 与源码承担。
>
> **版本**：V1.0.0
> **最后更新**：2026-07-27

---

### 关联文档

| 文档 | 说明 |
| :--- | :--- |
| [V1/1、需求调研](1、druid-rust-需求调研文档-V1.md) | 需求调研 |
| [V1/2、需求分析](2、druid-rust-需求分析文档-V1.md) | 用户故事 |
| [V1/3、V1-Architecture](3、druid-rust-V1-Architecture.zh_CN.md) | 版本架构 |
| [V1/4、功能规划](4、druid-rust-功能与界面规划-V1.md) | 模块划分 |
| [V1/6、菜单](6、druid-rust-功能菜单与版本规划-V1.md) | 功能菜单 |

---

## 1. 文档信息

### 1.1 版本记录

| 版本号 | 修改日期 | 修改人 | 修改内容 | 备注 |
| :--- | :--- | :--- | :--- | :--- |
| V1.0.0 | 2026-07-27 | druid-rust maintainers | 初始版本 | 对应技术文档 V1.0.0 |

### 1.2 文档责任人

| 角色 | 姓名 | 职责 |
| :--- | :--- | :--- |
| 产品 | druid-rust maintainers | 需求输出、验收标准 |
| 开发 | druid-rust maintainers | 实现、code review |
| 测试 | druid-rust maintainers | 单元 + 集成测试 |

## 2. 产品概述

### 2.1 产品定位

druid-rust 是面向 Rust 后端服务的数据库连接治理中间件，借鉴阿里 Druid
(Java) 的设计思路。V1 阶段聚焦**最小闭环**：在 mock driver 上验证
trait 契约、池调度与 Wall 默认 deny 规则。

### 2.2 V1 目标用户

- **主要用户**：druid-rust 自身开发者（实现 V1）
- **次要用户**：早期关注者（阅读 V1 文档与代码）

V1 **不面向生产用户**——所有 crate 仍为 `publish = false`。

### 2.3 核心价值

1. 验证 `Connection` trait 是否能拦截全部 SQL。
2. 验证 Wall 默认 deny 规则的边界条件。
3. 验证池调度器在泄漏检测下的归还语义。

## 3. 功能需求（V1 范围）

### 3.1 `druid-core` trait 暴露

| ID | 需求 | 验收 |
| :--- | :--- | :--- |
| `FR-001` | 暴露 `Connection` trait，含 7 个方法 | trait 测试 |
| `FR-002` | 暴露 `Driver` trait | trait 测试 |
| `FR-003` | 暴露 `Pool` trait | trait 测试 |
| `FR-004` | 暴露 `ConnectionFactory` trait | trait 测试 |
| `FR-005` | 暴露 `BeforeFilter` / `AfterFilter` trait | trait 测试 |
| `FR-006` | 暴露统一 `Error` 枚举 | thiserror 派生 |

### 3.2 `druid-sql` AST + Wall

| ID | 需求 | 验收 |
| :--- | :--- | :--- |
| `FR-010` | 用 sqlparser-rs 解析 `SELECT 1` | 单元测试 |
| `FR-011` | 解析 `DROP TABLE users` 抛 `WallViolation` | 集成测试 |
| `FR-012` | 解析 `UPDATE users SET ...`（无 WHERE）抛 `WallViolation` | 集成测试 |
| `FR-013` | 解析 `DELETE FROM users`（无 WHERE）抛 `WallViolation` | 集成测试 |
| `FR-014` | 解析 `TRUNCATE users` 抛 `WallViolation` | 集成测试 |
| `FR-015` | 暴露 `WallConfig` builder | 单元测试 |

### 3.3 `druid-pool` 池调度

| ID | 需求 | 验收 |
| :--- | :--- | :--- |
| `FR-020` | 实现 `max_open` 上限 | 集成测试 |
| `FR-021` | 实现 `max_idle` 上限 | 集成测试 |
| `FR-022` | 实现 `acquire_timeout` 返回 | 单元测试 |
| `FR-023` | `PooledConnection::drop` 归还连接 | 10000 循环无泄漏 |
| `FR-024` | 装配 `FilterChain` | 集成测试 |

### 3.4 mock driver 端到端

| ID | 需求 | 验收 |
| :--- | :--- | :--- |
| `FR-030` | mock driver 实现 `ConnectionFactory` | 测试通过 |
| `FR-031` | `pool.get().fetch("SELECT 1", ...)` 返回 1 行 | 集成测试 |
| `FR-032` | mock driver 上跑通 `WallViolation` 路径 | 集成测试 |

## 4. 非功能需求

| ID | 需求 | 验收 |
| :--- | :--- | :--- |
| `NFR-001` | `cargo check --workspace` 通过 | CI |
| `NFR-002` | `cargo clippy -- -D warnings` 通过 | CI |
| `NFR-003` | `cargo test --workspace` 通过 | CI |
| `NFR-004` | MSRV 1.75 | `rust-toolchain.toml` |
| `NFR-005` | workspace lint `unsafe_code = forbid` | CI |

## 5. 接口约束

- 所有 trait 方法接受 `&mut self` 或 `&self`；不持有跨 await 的可变借用。
- 所有公开类型 `Send + Sync`。
- 不引入 driver / parser / async runtime 到 `druid-core`。

## 6. 范围外（V1 不做）

- 三个 adapter crate
- `druid-stats` SQL 合并统计
- `druid-dynamic` 多数据源
- `druid-admin` HTTP 端点
- Web UI
- SQL 注入正则检测
- 数据库 migration 工具

## 7. 验收口径

V1 退出 = 满足以下全部：

- [ ] FR-001 ~ FR-006（trait 测试通过）
- [ ] FR-010 ~ FR-015（sqlparser + Wall 测试通过）
- [ ] FR-020 ~ FR-024（池测试通过）
- [ ] FR-030 ~ FR-032（mock driver 端到端通过）
- [ ] NFR-001 ~ NFR-005（CI 通过）
- [ ] [架构文档 §23](../../druid-rust-Architecture.zh_CN.md) Phase 1 退出条件满足

## 8. 一致性自检清单

- [ ] FR ID 与 [V1/4、功能规划](4、druid-rust-功能与界面规划-V1.md) §2 一致。
- [ ] 验收 ID 与 [V1/2、需求分析](2、druid-rust-需求分析文档-V1.md) §4 一致。
- [ ] 范围外与 [架构文档 §5](../../druid-rust-Architecture.zh_CN.md) 一致。

---

**文档版本**：V1.0.0
**创建日期**：2026-07-27
**最后更新**：2026-07-27
**文档状态**：✅ 待评审