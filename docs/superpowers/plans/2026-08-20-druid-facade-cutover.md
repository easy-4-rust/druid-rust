# Druid Facade Features and Cutover Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让最终用户既可使用纯 Core，也可只声明一个 `druid` 依赖并通过可选 feature 启用 Metrics 或 Wrapper Adapter。

**Architecture:** Facade 默认只依赖 Core。Metrics/Wrapper 均依赖 Core，Facade 以 optional dependencies 向上聚合，禁止 Metrics 与 Wrapper 相互依赖。

**Tech Stack:** Cargo features、cargo-public-api、cargo-semver-checks、Rust examples

**Spec:** `docs/superpowers/plans/2026-08-20-druid-core-facade-split.md`

## Global Constraints

- `default = []`。
- 禁用 feature 时不得编译对应 dependency 或启动后台 task。
- `druid::sql::*` 等稳定路径不变。
- 具体 driver 类型只在 Wrapper feature 下可见。

---

### Task 1: 定义 feature matrix

**Files:**
- Modify: `crates/druid/Cargo.toml`
- Modify: `crates/druid/src/lib.rs`
- Create: `crates/druid/tests/feature_surface_test.rs`

**Interfaces:**
- Consumes: Core, Metrics, Wrapper crates
- Produces: facade feature forwarding

- [ ] **Step 1: 写 feature RED 脚本**

```bash
cargo check -p druid --no-default-features
cargo check -p druid --no-default-features --features metrics
cargo check -p druid --no-default-features --features sqlx
cargo check -p druid --no-default-features --features toasty-sqlite
cargo check -p druid --all-features
```

Expected before implementation: feature names are missing.

- [ ] **Step 2: 实现精确 feature**

```toml
[features]
default = []
metrics = ["dep:druid-metrics", "druid-metrics/client"]
wrapper = ["dep:druid-wrapper"]
sqlx = ["wrapper", "druid-wrapper/sqlx"]
rbdc = ["wrapper", "druid-wrapper/rbdc"]
toasty-sqlite = ["wrapper", "druid-wrapper/toasty-sqlite"]
toasty-postgresql = ["wrapper", "druid-wrapper/toasty-postgresql"]
toasty-mysql = ["wrapper", "druid-wrapper/toasty-mysql"]
toasty-turso = ["wrapper", "druid-wrapper/toasty-turso"]
```

- [ ] **Step 3: 实现条件重导出**

Metrics 重导出位于 `druid::metrics`，Wrapper 位于 `druid::wrapper`；Core 路径无条件存在。

- [ ] **Step 4: 运行 GREEN**

Run the Step 1 matrix. Expected: all commands exit 0.

### Task 2: 用户级集成 API 与示例

**Files:**
- Create: `crates/druid/examples/core_only.rs`
- Create: `crates/druid/examples/sqlx_pool.rs`
- Create: `crates/druid/examples/toasty_sqlite.rs`
- Create: `crates/druid/examples/metrics_sqlx.rs`
- Test: `crates/druid/tests/examples_compile_test.rs`

**Interfaces:**
- Produces: canonical application construction patterns

```rust
let metrics = DruidMetricsRuntime::start(config).await?;
let data_source = Arc::new(build_data_source()?);
let _registration = metrics.register(data_source.monitorable());
```

- [ ] **Step 1: 写 examples RED 测试**

编译四个 example，分别断言 feature requirements；core-only 的 dependency tree 不能含 Tonic/SQLx/Toasty。

- [ ] **Step 2: 实现 examples**

每个 example 必须显式 close pool/runtime；禁止 `unwrap()` 于非测试路径。

- [ ] **Step 3: 运行 GREEN**

```bash
cargo check -p druid --examples --all-features
cargo tree -p druid --no-default-features | rg "tonic|sqlx|toasty" && exit 1 || true
```

### Task 3: Public API 与迁移指南

**Files:**
- Create: `docs/migration/five-crate-cutover.md`
- Modify: `README.md`
- Modify: `README.zh-CN.md`
- Test: public API snapshots

- [ ] **Step 1: 写 API diff allowlist**

允许的破坏项仅包括 `druid::toasty` 移除和已批准的 metrics/admin 路径迁移；`druid::sql`、Pool、Filter、Wall、error 变化均阻断。

- [ ] **Step 2: 编写迁移映射**

```text
druid::toasty::* -> druid::wrapper::toasty::*
druid::stats::DruidStatService -> druid-admin REST/query service
druid::stats::DruidStatManagerFacade -> druid::metrics runtime/repository API
```

- [ ] **Step 3: 执行 API 门禁**

```bash
cargo public-api -p druid -sss --all-features > api/druid-public-api.five-crate.txt
baseline_rev=$(cat target/five-crate-baseline-rev.txt)
cargo semver-checks check-release -p druid --baseline-rev "$baseline_rev"
```

Expected: only approved allowlist entries require acknowledgement.

**Suggested commit:** `feat(facade): expose optional metrics and driver integrations`
