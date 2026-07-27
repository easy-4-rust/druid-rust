# druid-rust V1 需求分析文档

> **文档说明**：将 V1 用户故事拆解为功能规则与验收标准，作为 PRD 与
> 架构细化的输入。
>
> **版本**：V1.0.0
> **最后更新**：2026-07-27

---

## 1. 文档信息

### 1.1 版本记录

| 版本 | 日期 | 作者 | 变更说明 |
| :--- | :--- | :--- | :--- |
| V1.0.0 | 2026-07-27 | druid-rust maintainers | 初始分析 |

### 1.2 关联文档

| 文档 | 关联说明 |
| :--- | :--- |
| [V1/1、需求调研](1、druid-rust-需求调研文档-V1.md) | 调研输入 |
| [V1/5、PRD](5、druid-rust-PRD文档-V1.md) | 需求规格 |
| [架构文档 §10](../../druid-rust-Architecture.zh_CN.md) | 主链参考 |

---

## 2. 用户故事细化

### US-001 启动一个连接池

| 字段 | 内容 |
| :--- | :--- |
| 角色 | Web 后端开发者 |
| 故事 | 作为开发者，我能用 `cargo add druid-core` 起一个连接池 |
| 验收 | V1 退出时存在 mock driver 上跑通 `SELECT 1` 的最小示例 |
| 依赖 | `druid-core` trait 暴露；`druid-pool` 实现 |

### US-002 Wall 默认拦截 `DROP TABLE`

| 字段 | 内容 |
| :--- | :--- |
| 角色 | Web 后端开发者 |
| 故事 | 我能让 Wall 默认拦截 `DROP TABLE` |
| 验收 | `WallConfig::default().deny_drop_table(true)` 在集成测试中抛 `WallViolation` |
| 依赖 | `druid-sql` sqlparser 适配；`druid-pool` 在 `before_execute` 短路 |

### US-003 `Drop` 自动归还

| 字段 | 内容 |
| :--- | :--- |
| 角色 | 后台 worker |
| 故事 | 我能在 `Drop` 时自动归还连接 |
| 验收 | 10000 acquire/release 循环后池内连接数 == 起始值 |
| 依赖 | `PooledConnection::drop` 实现 |

### US-004 `acquire_timeout`

| 字段 | 内容 |
| :--- | :--- |
| 角色 | 后台 worker |
| 故事 | 我能通过 `acquire_timeout` 避免无限等待 |
| 验收 | `pool.get_timeout(Duration::from_millis(100))` 在池满时返回 `Error::AcquireTimeout` |
| 依赖 | `druid-pool` 调度器 |

### US-005 热切换数据源

| 字段 | 内容 |
| :--- | :--- |
| 角色 | SaaS 平台工程师 |
| 故事 | 我能用 `ArcSwap` 热切换数据源 |
| 验收 | V3：切换期间 1000 RPS 错误率 < 1% |
| 依赖 | `druid-dynamic` |
| 备注 | V1 不实现；本故事作占位 |

### US-006 Prometheus 抓取

| 字段 | 内容 |
| :--- | :--- |
| 角色 | SRE |
| 故事 | 我能用 Prometheus 抓取 druid 指标 |
| 验收 | V2：`/metrics` 暴露 `druid_pool_*` 系列 |
| 依赖 | `druid-stats` |
| 备注 | V1 不实现 |

### US-007 JSON 端点查看活跃连接

| 字段 | 内容 |
| :--- | :--- |
| 角色 | DBA |
| 故事 | 我能用 JSON 端点查看当前活跃连接 |
| 验收 | V3：`/druid/api/active` 返回 `ConnectionInfo[]` |
| 依赖 | `druid-admin` |
| 备注 | V1 不实现 |

## 3. V1 范围内 / 外

### 3.1 范围内

- `druid-core` trait 暴露
- `druid-sql` sqlparser 适配
- `druid-sql::Wall` 默认 deny 规则
- `druid-pool` 调度器 + `PooledConnection`
- mock driver 端到端

### 3.2 不在 V1 范围

- `druid-stats`、`druid-dynamic`、`druid-admin`（V2/V3）
- 三个 adapter（V2）
- Prometheus 导出（V2）
- Web UI（V3 之后单独评估）
- SQL 注入正则（永不实现）

## 4. 验收汇总

| ID | 验收 | 测量方式 |
| :--- | :--- | :--- |
| `AC-001` | `cargo check --workspace` 通过 | CI |
| `AC-002` | mock driver `SELECT 1` 返回 1 行 | 集成测试 |
| `AC-003` | `DROP TABLE` 抛 `WallViolation` | 集成测试 |
| `AC-004` | `UPDATE users` 抛 `WallViolation`（无 WHERE） | 集成测试 |
| `AC-005` | `DELETE users` 抛 `WallViolation`（无 WHERE） | 集成测试 |
| `AC-006` | 10000 acquire/release 循环无泄漏 | 单元测试 |
| `AC-007` | `acquire_timeout` 在池满时返回错误 | 单元测试 |

## 5. 一致性自检清单

- [ ] 验收汇总与 [V1/5、PRD](5、druid-rust-PRD文档-V1.md) §3 一致。
- [ ] 范围内/外与 [V1/4、功能规划](4、druid-rust-功能与界面规划-V1.md) 一致。
- [ ] 用户故事 ID 与 [V1/1、需求调研](1、druid-rust-需求调研文档-V1.md) §5 一致。

---

**文档版本**：V1.0.0
**创建日期**：2026-07-27
**最后更新**：2026-07-27
**文档状态**：✅ 待评审