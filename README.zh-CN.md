<a id="readme-top"></a>

<div align="center">

# druid-rust

**阿里 Druid (Java) 到 Rust 的规划式完整语义迁移**

[English](./README.md) | [简体中文](./README.zh-CN.md)

[定位与状态](#1-项目定位与状态) · [功能与成熟度](#2-功能与成熟度) ·
[Workspace 与 crate 架构](#4-workspace-与-crate-架构) ·
[设计草图](#6-设计草图-未来-api-当前不可运行) ·
[路线图](#11-路线图与阶段) ·
[贡献与许可证](#19-贡献安全与许可证)

</div>

---

> **项目状态：语义迁移进行中**
>
> 当前仓库是可构建、可测试的 Cargo workspace，已经包含 core、pool、
> SQL、统计、动态数据源和驱动桥接实现。它尚未完成全部 Druid 语义迁移，
> 也没有稳定公共 API；任何“完成”都必须有 Java oracle 差分证据或明确的
> Rust 生态契约测试。
>
> 目标是完整迁移 Druid 的功能语义。迁移不是 Java 源码逐行翻译，但也
> 不是只借鉴架构模式后自行取舍功能。

> **最后核验**：2026-07-28。

## 1. 项目定位与状态

**druid-rust 是一个把阿里 Druid 可观察语义迁移到 Rust 异步生态的
workspace。** 连接池、过滤器链、SQL 防火墙、统计、动态数据源与管理端
均进入对象总账和语义契约表，不以“只选择架构模式”的方式缩减范围。实现基于
(`tokio`、`sqlparser-rs`、`sqlx`、`deadpool`、`bb8`、`rbdc`、`axum`)
生态完成平台适配。

### 1.1 是什么

| 字段 | 值 |
| :--- | :--- |
| 核心契约 | `druid-core`；native pool 位于 `druid-pool` |
| 当前版本 | `0.0.0-design`（迁移期不稳定版本） |
| MSRV | `1.75`（在 `rust-toolchain.toml` 中固定） |
| Edition | `2021` |
| Workspace Resolver | `2` |
| unsafe 策略 | `forbid`（workspace lint） |
| 默认 features | 无 |
| 发布状态 | 未发布——每个 crate `publish = false` |
| 许可证 | Apache-2.0 |

### 1.2 不是什么

- **不是逐行、逐类布局复制**。Java 行为通过对象总账和语义契约迁移；
  调度器、所有权和驱动适配使用 Rust 机制实现，但不能因此删减外部语义。
- **不是 ORM**。druid-rust 不生成 SQL，只对宿主应用发出的 SQL 做池化、
  观测和治理。
- **不是数据库 migration 工具**。Schema 版本管理明确不在范围内。
- **尚未达到发布标准**。workspace 已可运行且有测试，但完整对象矩阵、
  Java golden 差分、稳定 API、CI 与基准仍未闭环。

### 1.3 状态证据

| 声明 | 当前值 | 证据 |
| :--- | :--- | :--- |
| workspace 可构建 | 是（当前需 stable 工具链） | `cargo +stable test --workspace --all-targets` |
| 连接 API | 已实现，尚不稳定 | `DruidPooledConnection` → `PhysicalConnection` |
| 测试 | 已实现 | core/pool/SQL/Stats/Dynamic/SQLx/RBDC/bb8/deadpool 测试 |
| 文档 | 迁移总账 + 架构 | `doc/migration/` |
| crates.io | 未发布 | 所有 crate `publish = false` |
| docs.rs | 未发布 | 所有 crate `publish = false` |
| CI | 未配置 | 无 `.github/workflows/` 目录 |
| 覆盖率 | 基线已测；当前变更需重审 | `cargo llvm-cov` 是出口门禁，不代替语义验收 |
| 基准 | 未测量 | 无 `benches/` 目录 |

## 2. 功能与成熟度

### 2.1 功能矩阵

| 功能 | 状态 | crate | 限制 | 验证 |
| :---: | :---: | :--- | :--- | :--- |
| 与 driver 解耦的 `PhysicalConnection` SPI | 🚧 部分 | `druid-core` | JDBC 广度未完成 | 迁移文档 §5 |
| Druid native 异步连接池 | 🚧 部分 | `druid-pool` | 完整生命周期未完成 | 语义迁移表 |
| 基于 sqlparser-rs 的 Wall 规则 | 🚧 部分 | `druid-sql` | Druid 规则矩阵未完成 | 语义迁移表 |
| SQL 合并统计 | 🚧 部分 | `druid-stats` | Java 分层统计未完成 | 语义迁移表 |
| 多数据源热切换 | 🚧 部分 | `druid-dynamic` | HA 健康/恢复未完成 | 语义迁移表 |
| `sqlx + deadpool` 外部池桥接 | 🧪 预览 | `druid-sqlx-deadpool` | 禁止嵌套 DruidPool | bridge 契约测试 |
| `sqlx + bb8` 外部池桥接 | 🧪 预览 | `druid-sqlx-bb8` | 禁止嵌套 DruidPool | bridge 契约测试 |
| SQLx direct adapter | 🧪 预览 | `druid-sqlx` | SQLite 已测，数据库矩阵未完成 | direct 契约测试 |
| RBDC direct adapter | 🚧 部分 | `druid-rbdc` | 真实 driver 矩阵未完成 | RBDC trait 契约测试 |
| `/druid/admin` HTTP 端点 | 🗓️ 计划 | `druid-admin` | V3 | `doc/9` |
| SQL 注入正则检测 | ⛔ 不移植 | — | 语义不安全 | ADR-005 |

### 2.2 状态定义

| 状态 | 定义 |
| :--- | :--- |
| ✅ 稳定 | 公共 API、测试、文档和兼容承诺齐全 |
| 🧪 预览 | 可用但 API 或行为可能变化 |
| 🚧 部分 | 只有明确列出的子集可用 |
| 🗓️ 计划 | 尚无可调用实现 |
| ⛔ 不移植 | 明确拒绝——见 ADR 理由 |

## 3. Rust 基线与平台支持

| 项目 | 值 | 来源 |
| :--- | :--- | :--- |
| MSRV | `1.75` | `rust-toolchain.toml`、`Cargo.toml` `rust-version` |
| Edition | `2021` | workspace `[workspace.package]` |
| Resolver | `2` | workspace `[workspace]` |
| rustfmt | stable | `rust-toolchain.toml` components |
| Clippy | 启用 workspace pedantic | `Cargo.toml` `[workspace.lints.clippy]` |

> `druid-core` 已暴露公共契约，但平台支持矩阵、`no_std` 与 WASM 尚未
> 决策，因此当前不作稳定支持承诺。

## 4. Workspace 与 crate 架构

### 4.1 一眼看懂

```text
[下游应用]
        │
        ▼
┌──────────────────────────────────────────────────────────────┐
│ druid-rust Cargo Workspace（语义迁移进行中）                 │
│                                                              │
│ druid-core         DruidPooledConnection / PhysicalConnection│
│ druid-sql          sqlparser-rs AST、Wall、参数化            │
│ druid-pool         Druid native 异步连接池                   │
│ druid-stats        SQL 合并 + 百分位 + Prometheus             │
│ druid-dynamic      ArcSwap 多数据源 + 读写分离                │
│                                                              │
│ druid-sqlx         SQLx direct PhysicalConnection adapter    │
│ druid-rbdc         RBDC direct PhysicalConnection adapter    │
│ druid-sqlx-deadpool deadpool 外部池 bridge                   │
│ druid-sqlx-bb8     bb8 外部池 bridge                         │
│                                                              │
│ druid-admin        axum 实现的 /druid/admin HTTP 端点         │
└──────────────────────────────────────────────────────────────┘
        │
        ▼
[数据库 driver 生态：sqlx、rbdc、deadpool、bb8、tokio-postgres、...]
```

连接边界固定为：

```text
DruidPooledConnection            对外池化连接
└── PhysicalConnection           druid-rust 内部最小 SPI
    ├── SqlxConnectionAdapter
    ├── RbdcConnectionAdapter
    └── 其他驱动 Adapter
```

bb8/deadpool 是 `Pool` Provider：它们通过 `PhysicalConnectionLease`
持有外部租约，不实现 `PhysicalConnectionFactory`，也不能再嵌套到
`DruidPool`。

### 4.2 crate 依赖图

```mermaid
flowchart TB
    APP["下游应用"] --> CORE["druid-core"]
    APP --> POOL["druid-pool"]
    APP --> DYNAMIC["druid-dynamic"]
    APP --> ADMIN["druid-admin"]

    SQL["druid-sql"] --> CORE
    POOL --> CORE
    STATS["druid-stats"] --> CORE
    STATS --> SQL
    DYNAMIC --> CORE
    DYNAMIC --> SQL
    DYNAMIC --> POOL

    RBDC["druid-rbdc"] --> CORE
    SQLX["druid-sqlx"] --> CORE
    SQLXDP["druid-sqlx-deadpool"] --> CORE
    SQLXDP --> SQLX
    SQLXB8["druid-sqlx-bb8"] --> CORE
    SQLXB8 --> SQLX

    ADMIN --> CORE
    ADMIN --> POOL
    ADMIN --> STATS
    ADMIN --> DYNAMIC
```

### 4.3 Crate Map

| Crate | 发布 | 默认启用 | 职责 | 计划关键依赖 |
| :---: | :---: | :---: | :--- | :--- |
| `druid-core` | ⛔ | — | `DruidPooledConnection`、`PhysicalConnection`、`Pool`、Filter 契约 | `async-trait` |
| `druid-sql` | ⛔ | — | sqlparser-rs 适配、Wall 规则、参数化、指纹 | `sqlparser` |
| `druid-pool` | ⛔ | — | HikariCP 风格异步池、空闲队列、驱逐调度 | `tokio`、`parking_lot` |
| `druid-stats` | ⛔ | — | SQL 合并、百分位直方图、Prometheus 导出 | `moka`、`prometheus` |
| `druid-dynamic` | ⛔ | — | `ArcSwap` 多数据源、读写分离、负载均衡 | `arc-swap`、`dashmap` |
| `druid-sqlx` | ⛔ | — | SQLx raw connection direct adapter | `sqlx` |
| `druid-rbdc` | ⛔ | — | RBDC raw connection direct adapter | `rbdc`、`rbs` |
| `druid-sqlx-deadpool` | ⛔ | — | SQLx + deadpool external bridge | `druid-sqlx`、`deadpool` |
| `druid-sqlx-bb8` | ⛔ | — | SQLx + bb8 external bridge | `druid-sqlx`、`bb8` |
| `druid-admin` | ⛔ | — | axum 实现的 `/druid/admin` HTTP 端点 | `axum`、`prometheus` |

### 4.4 依赖和可见性规则

- `druid-core` **不得**依赖任何 driver、parser、async runtime 或 TLS 后端，
  只暴露 trait 契约。
- 领域 crate（`druid-sql`、`druid-pool`、`druid-stats`、`druid-dynamic`）
  **不得**形成循环依赖，仅共享 `druid-core`。
- direct adapter 只依赖 `druid-core`；外部池 bridge 依赖 `druid-core`、
  `druid-sqlx` 和一个外部池生态。bridge **不得**依赖 `druid-pool`，
  否则会形成 pool-in-pool。
- `druid-admin` 是唯一依赖所有其他 crate 的 crate，且故意不在任何领域
  crate 的依赖闭包里。
- 所有 crate 在 Phase 1 完成前保持 `publish = false`。

## 5. 设计原则

| 原则 | 工程落地 | 验证手段 |
| :--- | :--- | :--- |
| 类型安全 | `PhysicalConnection` 通过 `Box<dyn PhysicalConnection>` 留在内部；应用拿到 `DruidPooledConnection` | 编译与契约测试 |
| 所有权清晰 | `DruidPooledConnection` 只有一个连接和一个 `FnOnce` 归还路径；外部池使用 `PhysicalConnectionLease` | exactly-once 测试 |
| 错误可组合 | 单一 `druid_core::Error` 枚举，使用 `thiserror`；所有层返回 `Result<T, Error>` | 未来的错误测试 |
| 默认安全 | Wall 默认拒绝 `DROP`/`TRUNCATE`；过滤器链默认 `StatFilter + WallFilter` | 未来的安全测试 |
| 零成本抽象 | 仅在边界使用 `async-trait` 动态分发；调度器内部用 `parking_lot::Mutex` 和 `tokio::sync::Notify` | 未来的基准 |
| 可演进 | 通过 feature flag 隐藏每个适配器；`druid-core` 的 trait 契约是跨升级的稳定合同 | 未来的 semver 测试 |

## 6. 历史设计草图

> **警告：以下片段是保留的历史设计草图。** crate 名、模块路径、trait 签名和配置键
> 均为**暂定**，在任何 crate 发布前都会调整。**请勿**复制到生产代码。
> 它们只用来传达意图，不作为 API 保证。

### 6.1 最小示例（计划）

```rust
// 伪代码——当前 workspace 不可编译
use druid_core::pool::DruidPool;
use druid_sqlx_bb8::SqlxBb8PoolBuilder;

let pool: DruidPool<_> = SqlxBb8PoolBuilder::new()
    .driver_name("postgres")
    .url("postgres://user:pwd@host/db")
    .max_open(20)
    .max_idle(4)
    .acquire_timeout(Duration::from_secs(3))
    .build()
    .await?;

let mut conn = pool.get().await?;
let rows = conn.fetch("SELECT id FROM users WHERE id = ?", vec![42.into()]).await?;
println!("rows = {rows:?}");
```

### 6.2 多数据源热切换（计划）

```rust
// 伪代码——当前 workspace 不可编译
use druid_dynamic::{DynamicDataSource, SqlHint};

let ds = DynamicDataSource::builder()
    .add("main", main_pool)
    .add("readonly", read_pool.clone())
    .build();

// 之后，零停机切换主库：
ds.switch("new_main").await?;

// 按 SQL 类型路由
let conn = ds.route(SqlHint::Read).await?;
```

### 6.3 Wall 规则（计划）

```rust
// 伪代码——当前 workspace 不可编译
use druid_sql::wall::WallConfig;

let wall = WallConfig::default()
    .deny_drop_table(true)
    .deny_truncate(true)
    .update_must_have_where(true)
    .delete_must_have_where(true)
    .max_sql_length(Some(64 * 1024))
    .build();
```

## 7. 计划的 Cargo Features

> **注意：所有 crate 当前 features 为零。** 本节是设计草图，会在 V1
> `druid-core` 暴露 trait 后填实。

| Crate | Feature | 默认 | 增加能力 | 计划关键依赖 |
| :---: | :---: | :--- | :--- | :--- |
| `druid-core` | `std` | ✅ | 使用 `std::error::Error` 与 `std::time::Duration` | 无 |
| `druid-sql` | `postgres-dialect` | ❌ | `PostgreSqlDialect` 参数化 | `sqlparser` |
| `druid-sql` | `mysql-dialect` | ❌ | `MySqlDialect` 参数化 | `sqlparser` |
| `druid-sqlx-deadpool` | `postgres` | ❌ | `sqlx::postgres` feature gate | `sqlx` |
| `druid-sqlx-deadpool` | `mysql` | ❌ | `sqlx::mysql` feature gate | `sqlx` |
| `druid-admin` | `tls` | ❌ | rustls 服务端配置 | `axum`、`rustls` |

## 8. 计划的核心 API 与用法

V1 计划的 trait 面如下；完整合同见 `druid-rust-Architecture.zh_CN.md` §8
与 `doc/7、druid-rust-领域模型设计.md`。

```rust
// 计划——尚未实现
#[async_trait::async_trait]
pub trait Connection: Send + Sync {
    async fn exec(&mut self, sql: &str, params: Vec<Value>) -> Result<ExecResult, Error>;
    async fn fetch(&mut self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, Error>;
    async fn begin(&mut self) -> Result<(), Error>;
    async fn commit(&mut self) -> Result<(), Error>;
    async fn rollback(&mut self) -> Result<(), Error>;
    async fn ping(&mut self) -> Result<(), Error>;
    async fn close(&mut self) -> Result<(), Error>;
}
```

## 9. 后端、格式与可选引擎

| 能力 | 后端 | 计划 feature | 边界 | 许可证 |
| :--- | :--- | :--- | :--- | :--- |
| Postgres | `tokio-postgres` via `sqlx` | `druid-sqlx-deadpool/postgres` 或 `druid-sqlx-bb8/postgres` | 必须参数绑定 | Apache-2.0 / MIT |
| MySQL | `sqlx::mysql` | `druid-sqlx-deadpool/mysql` | 通过 `druid-sql` 重写占位符 | Apache-2.0 / MIT |
| MSSQL | `tiberius`（计划） | `druid-sqlx-bb8/mssql`（计划） | 不在 V1/V2 范围 | Apache-2.0 |
| rbdc 生态 | `rbdc-pg`、`rbdc-mysql`、`rbdc-mssql` | `druid-rbdc/<dialect>` | 通过 `Box<dyn Connection>` 桥接 | Apache-2.0 |
| SQLite / DuckDB / Turso | n/a | n/a | 明确不在范围内 | — |

> 多引擎能力**不对等**。若某后端缺少某能力，`Pool::state()` 与
> `druid-admin` API 返回 `Unsupported`，而不是悄悄降级。

## 10. 并发、内存与资源模型

- `Send + Sync`：所有公共类型必须满足。`Box<dyn Connection>` 是 API 边界
  唯一出现的非 `Sized` 类型。
- 内存策略：连接池最多持有 `max_open` 条活连接加一个有界空闲队列。
  `druid-stats` 为每个独立 SQL 指纹保留一个直方图，TTL 通过
  `druid-stats::MergeConfig` 配置。
- 资源释放：`PooledConnection::drop` 是归还连接并发出泄漏告警的唯一位置。
- 取消：每个 `async` 方法接收 `&mut self`，接受调用方驱动的
  `tokio::select!` 竞争；不在调用方上下文中派生内部 spawn。
- 背压：`pool.get()` 在 `in_use == max_open` 时阻塞；可选 `acquire_timeout`
  返回 `Error::AcquireTimeout` 而不是无限等待。

## 11. 路线图与阶段

以下阶段与 `doc/5、druid-rust-技术方案与路线.md` 和
`druid-rust-Architecture.zh_CN.md` §23 对齐。

| 阶段 | 交付物 | 退出条件 | 依赖 | 风险 |
| :---: | :--- | :--- | :--- | :--- |
| Phase 0 | workspace 骨架 + 设计文档（当前） | `cargo check --workspace` 通过 | toolchain | 依赖漂移 |
| Phase 1 | `druid-core` + `druid-sql` + `druid-pool` + mock driver | `SELECT 1` 端到端；Wall 拦截 `DROP TABLE` | Phase 0 | API 漂移 |
| Phase 2 | `druid-rbdc` + `druid-sqlx-deadpool` + `druid-sqlx-bb8` + `druid-stats` | Prometheus 导出可用；任一 driver 适配器冒烟测试 | Phase 1 | 上游 `sqlx` / `rbdc` 变更 |
| Phase 3 | `druid-dynamic` + `druid-admin` | 热切换 demo；`/druid/admin` JSON 端点 | Phase 2 | ArcSwap 语义验证 |

> 故意省略时间估计。引用到发布说明前，先替换为可验证的退出条件。

## 12. 计划的文档集合

| 文档 | 作用 |
| :--- | :--- |
| `druid-rust-Architecture.zh_CN.md` | workspace、crate、关键决策（架构基线唯一来源） |
| `doc/`（root） | 10 篇产品级文档，符合 `full-stack-doc` v3 标准 |
| `doc/V1/` | Phase 1 里程碑对应的 7 篇版本级文档 |
| `LICENSE` | Apache-2.0 |

## 13. 计划的质量门禁

以下门禁是**目标**，不是现状。CI 里今天**一个都没有**。

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo check --workspace --all-features
cargo test --workspace
cargo test --workspace --no-default-features
cargo test --workspace --all-features
cargo doc --workspace --all-features --no-deps
cargo llvm-cov --workspace --all-features
cargo audit
cargo deny check
```

## 14. 计划的基准矩阵

| 场景 | 目标 | 状态 |
| :--- | :--- | :--- |
| `pool_acquire` | 热池中位数 < 200ns | 未测量 |
| `sql_merge` | 参数化 + 指纹 < 50µs | 未测量 |
| `wall_check` | 完整 Wall 检查 < 100µs | 未测量 |
| `dynamic_switch` | `ArcSwap::store` 后 `load` < 50ns | 未测量 |

> 基准声明只在与可复现脚本、固定硬件、`git rev-parse HEAD` 引用一同发布
> 时才会被引用。

## 15. 计划的兼容与迁移

| 主题 | 目标 |
| :--- | :--- |
| SemVer | crate 在取消 `publish = false` 后遵循 SemVer |
| MSRV 策略 | Phase 1 后，MSRV 升级需要 minor 版本号变更 |
| 默认 features | 改变默认 feature 是破坏性变更 |
| 源码兼容性 | 与 `druid-rust-Architecture.zh_CN.md` §6 的 ADR 对齐 |

## 16. 故障排查

占位 workspace 暂时没有故障排查表。如果阅读设计文档时遇到与预期不符
的描述，请使用 `design-question` 标签在 issue 中反馈，并附上相关章节
链接。

## 17. 计划的 crates.io 发布

当前**没有** crate 发布。发布前置条件：

- [ ] `druid-core` 导出完整 trait 面并有单元测试覆盖。
- [ ] `druid-pool` 针对 `deadpool` 和 `bb8` 完成热获取路径基准。
- [ ] 每个取消 `publish = false` 的 crate 通过 `cargo publish --dry-run`。
- [ ] CI 在每个 PR 上运行 `cargo fmt`、`cargo clippy -D warnings`、
      `cargo test` 与 `cargo audit`。
- [ ] docs.rs 在已发布的 feature 矩阵下构建成功。

## 18. 计划的贡献流程

- 新增 crate 或 ADR 前先开 issue 或 discussion。
- 提审前运行 §13 列出的计划门禁。
- 新公共 API 必须附带文档、示例、测试，以及在 PR 描述中说明 SemVer /
  MSRV 影响。

## 19. 贡献、安全与许可证

druid-rust 使用 [Apache-2.0](LICENSE) 许可证。

漏洞披露渠道将在首次发布前建立。报告前请勿公开未修复的漏洞。

---

<div align="center">

[返回顶部](#readme-top) · [架构文档](druid-rust-Architecture.zh_CN.md) ·
[产品文档](doc/) · [Issues](https://github.com/easy-4-rust/druid-rust/issues)

</div>
