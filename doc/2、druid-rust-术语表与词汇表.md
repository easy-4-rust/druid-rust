# druid-rust 术语表与词汇表

> **文档说明**：统一 druid-rust 范围内使用的术语、缩写与同名词，作为
> 所有后续文档、产品讨论与代码注释的共同语言基础。
>
> **版本**：V1.0.0
> **最后更新**：2026-07-27

---

## 1. 文档信息

| 项目 | 内容 |
| :--- | :--- |
| 文档类型 | 术语表与词汇表 |
| 产品 | druid-rust |
| 版本 | V1.0.0 |
| 状态 | ✅ 待评审 |

### 1.1 关联文档

| 文档 | 关联说明 |
| :--- | :--- |
| [1、命名与品牌](1、druid-rust-命名与品牌说明.md) | 命名口径 |
| [7、领域模型](7、druid-rust-领域模型设计.md) | 聚合与事件名称 |
| [架构文档 §8](../druid-rust-Architecture.zh_CN.md) | 组件清单 |

---

## 2. 术语约定

### 2.1 产品级术语

| 术语 | 定义 | 备注 |
| :--- | :--- | :--- |
| druid-rust | 仓库与产品总称 | 不可省略连字符 |
| Cargo workspace | 仓库根 `Cargo.toml` 声明的多 crate 工作区 | 与"crate"区别 |
| adapter / 适配器 | `druid-rbdc / druid-sqlx-deadpool / druid-sqlx-bb8` 的统称 | 不含领域 crate |
| 横切层 / cross-cutting | Filter / Wall / Stat / Dynamic 的统称 | 通过 `Connection` 拦截 |
| 治理面 | `druid-admin` 暴露的 HTTP 端点 | 不参与请求路径 |

### 2.2 架构级术语

| 术语 | 定义 | 禁止混用 |
| :--- | :--- | :--- |
| `Connection` | `druid-core` 暴露的 trait，所有横切层的拦截点 | `sqlx::Connection`、`db::Connection` |
| `Driver` | 数据库驱动抽象 | `sqlx::Pool`、`deadpool::Pool` |
| `Pool` | 连接池 trait | `r2d2::Pool`、`bb8::Pool` |
| `ConnectionFactory` | 适配器实现，向 pool 提供新连接 | 不要叫 `Provider` |
| `PooledConnection` | RAII 句柄，`Drop` 自动归还 | 不要叫 `ConnHandle` |
| `BeforeFilter` / `AfterFilter` | 横切关注单元 trait | 不要叫 `Interceptor` |
| `FilterChain` | 一组 filter 的执行链 | 不要叫 `Pipeline` |
| `Wall` | SQL 防火墙 | 不要叫 `Guard` |
| `SqlMerger` | SQL 合并与指纹 | 不要叫 `Template` |
| `MergedSqlStat` | 按 SQL 指纹聚合的统计对象 | 不要叫 `RowCount` |
| `DynamicDataSource` | 多数据源入口 | 不要叫 `MultiDb` |
| `DataSourceGroup` | 主库 + 从库 + 负载均衡器的组合 | 不要叫 `Shard` |
| `SqlHint` | 路由选择（`Read`/`Write`/`Auto`） | 不要叫 `RoutingKey` |
| `LoadBalancer` | 从库选择策略 trait | 不要叫 `Picker` |
| `ArcSwap` | `arc-swap` crate 的 lock-free `Arc<T>` 替换 | 不要叫 `AtomicArc` |
| `ArcSwapGuard` | 通过 `ArcSwap::load()` 拿到的 `Arc<T>` 引用 | 不要叫 `Snapshot` |
| `MergedSqlStat` | `druid-stats` 内的合并统计 | 不要叫 `StatRow` |
| `Histogram` | 延迟直方图 | 不要叫 `Counter` |

### 2.3 平台术语

| 术语 | 定义 | 来源 |
| :--- | :--- | :--- |
| MSRV | Minimum Supported Rust Version | `rust-toolchain.toml` |
| Edition | Cargo edition（当前 2021） | `[workspace.package]` |
| Workspace Resolver | Cargo resolver 版本（当前 2） | `[workspace]` |
| Adapter layer | 适配层 | `druid-rust-Architecture.zh_CN.md` §7 |
| Cross-cutting | 横切 | 同上 |
| 横切关注 / cross-cutting concern | Filter / Wall / Stat 等 | 同上 |
| 控制面 | 控制 / 路由 / 治理 | `druid-dynamic` |
| 数据面 | 数据 / 执行 | `druid-pool` / driver |

### 2.4 缩写

| 缩写 | 全称 | 含义 |
| :--- | :--- | :--- |
| ADR | Architecture Decision Record | 架构决策记录 |
| AST | Abstract Syntax Tree | sqlparser-rs 输出的语法树 |
| CI | Continuous Integration | 持续集成 |
| E2E | End-to-End | 端到端测试 |
| FFI | Foreign Function Interface | 外部函数接口 |
| IDS | Intrusion Detection System | 入侵检测（不参与 druid-rust） |
| JSON | JavaScript Object Notation | 监控 API 响应 |
| LSP | Language Server Protocol | 不参与 |
| Mermaid | 文本图表 DSL | 文档图 |
| OOM | Out Of Memory | 内存不足 |
| ORM | Object-Relational Mapping | druid-rust 不是 ORM |
| PRD | Product Requirements Document | 产品需求文档 |
| RBAC | Role-Based Access Control | 鉴权（由宿主负责） |
| SLA | Service Level Agreement | 服务等级协议 |
| SLO | Service Level Objective | 服务等级目标 |
| SPI | Service Provider Interface | 扩展点 |
| TLS | Transport Layer Security | 传输加密（由 driver 负责） |
| WORM | Write Once Read Many | 审计存储（不参与） |

## 3. 同名词对照（Druid Java ↔ druid-rust）

| Druid Java | druid-rust | 备注 |
| :--- | :--- | :--- |
| `DruidDataSource` | `druid_core::Pool`（trait） | trait 而非类 |
| `Connection` | `Connection`（trait） | 语义一致 |
| `Filter` | `BeforeFilter` + `AfterFilter` | Rust 拆为两个 trait |
| `StatFilter` | `druid_stats::StatFilter` | 计划中 |
| `WallFilter` | `druid_sql::WallFilter` | 计划中 |
| `LogFilter` | `druid_stats::LogFilter` | 计划中 |
| `Slf4jLogFilter` | `tracing` 集成 | 由 `tracing-subscriber` 输出 |
| `SqlStat` | `MergedSqlStat` | 合并 + 直方图 |
| `SqlMerger` | `SqlMerger` | 算法思路一致 |
| `DruidStatService` | `druid_admin` | HTTP 端点 |
| `DynamicDataSource` | `druid_dynamic::DynamicDataSource` | 命名一致 |
| `DataSourceProxy` | 装饰器模式挂在 `Connection` 上 | 形态不同 |
| `WallProvider` | `druid_sql::WallConfig` | 配置结构而非 provider |
| `StatManager` | `druid_stats::StatsCollector` | 计划中 |

## 4. 用语规则

- "横切层" 与 "filter 链" 含义相同，文档统一使用"过滤器链"。
- "动态数据源" 与 "多数据源" 在产品文档中互用；架构文档统一用"多数据源"。
- "监控" 与 "可观测性" 在 §19 章节中分别使用；监控 = 指标，可观测性 = 指标 + 日志 + 追踪 + 审计。
- "适配器" 与 "adapter" 在文档中互用；crate 名固定为 `druid-<driver>` 形式。
- "SqlMerger" 与 "SQL 合并" 在文档中互用；指代参数化 + 指纹过程。

## 5. 一致性自检清单

- [ ] 所有文档使用本术语表的术语。
- [ ] `druid-core` 文档出现 `Connection` 时指向 `druid_core::Connection`。
- [ ] `druid-dynamic` 文档使用 `DynamicDataSource` 而非 `MultiDb`。
- [ ] `druid-stats` 文档使用 `SqlMerger` 与 `MergedSqlStat`。
- [ ] 缩写词在首次出现时给出全称。

---

**文档版本**：V1.0.0
**创建日期**：2026-07-27
**最后更新**：2026-07-27
**文档状态**：✅ 待评审