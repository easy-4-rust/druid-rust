# Five-Crate Verification and Release Readiness Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 对五 Crate 架构执行依赖、功能、可靠性、性能、安全、文档和发布门禁，只把有新鲜证据的状态标记为 DONE。

**Architecture:** 本计划不新增生产功能；它汇总前七份计划的验证证据，并对失败项生成明确的 remaining-gates 记录。

**Tech Stack:** Cargo、CodeGraph、cargo-public-api、cargo-semver-checks、benchmark harness、Markdown checks

**Spec:** `docs/superpowers/plans/2026-08-20-five-crate-architecture-spec.md`

## Global Constraints

- 禁止以文件存在、编译成功或 partial test 代替完整完成证据。
- 不修复无关 lint 债务；记录基线和本次新增差分。
- 不发布、不 push、不自动 commit。
- 真实数据库和生产 transport 未执行时必须标记未验证。

---

### Task 1: Workspace 与 dependency DAG

**Files:**
- Create: `scripts/verify_five_crate_dependencies.py`
- Create: `docs/verification/five-crate-dependency-report.md`

- [ ] **Step 1: 验证五 package**

```bash
cargo metadata --format-version 1 --no-deps | jq -r '.packages[].name' | sort
```

Expected exactly:

```text
druid
druid-admin
druid-core
druid-metrics
druid-wrapper
```

- [ ] **Step 2: 验证 Core 禁止依赖**

```bash
cargo tree -p druid-core -e normal
```

Fail on Toasty/SQLx/RBDC/DuckDB/libSQL/bb8/deadpool/Prometheus/Reqwest/Tonic/Axum/Topcoat.

- [ ] **Step 3: 验证 Admin/横向依赖**

Admin tree 不得含 Wrapper；Metrics 与 Wrapper 不得互相依赖；所有反向查询通过 `cargo tree --invert` 保存到报告。

### Task 2: Build、test、doc 和 API

**Files:**
- Create: `docs/verification/five-crate-quality-report.md`

- [ ] **Step 1: 执行格式和 diff hygiene**

```bash
cargo fmt --all -- --check
git diff --check
```

- [ ] **Step 2: 执行全 feature build/test**

```bash
cargo check --workspace --all-targets --all-features
cargo test --workspace
cargo doc --workspace --no-deps --all-features
```

- [ ] **Step 3: 执行 Clippy 并区分基线**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

若失败，报告既有错误数、本次新增文件错误数和阻断状态；不得写“Clippy 通过”。

- [ ] **Step 4: Public API/semver**

检查 facade API 和批准 allowlist；Core/Wrapper/Metrics 的公开类型全部有 Debug 与错误文档。

### Task 3: gRPC、Admin 与安全场景

**Files:**
- Create: `docs/verification/five-crate-runtime-report.md`

- [ ] **Step 1: gRPC 故障矩阵**

运行 ACK 丢失、duplicate、乱序、断线重连、Admin 重启、Resync、ResetStats、shutdown。

- [ ] **Step 2: Admin 场景**

验证静态资源、兼容 JSON、排序分页、login/session、TLS/token、online/offline、Prometheus、readiness。

- [ ] **Step 3: 敏感数据扫描**

```bash
rg -n "password|token|bind.*param|raw_parameters" target/test-output docs/verification
```

所有 match 必须是字段白名单说明或脱敏值；真实 secret/SQL 参数为阻断失败。

### Task 4: 性能和非阻塞门禁

**Files:**
- Create: `benches/metrics_overhead.rs`
- Create: `docs/verification/five-crate-performance-report.md`

- [ ] **Step 1: 建立 disabled/enabled 对照**

同一机器、数据库、pool 配置、SQL corpus、warmup 和采样数；至少报告 throughput、P50、P95、P99、RSS。

- [ ] **Step 2: 运行正常和故障模式**

```text
metrics disabled
metrics enabled + healthy Admin
Admin disconnected
queue saturated
ACK window saturated
```

- [ ] **Step 3: 应用门禁**

吞吐下降 ≤2%，P99 增幅 ≤3%；SQL trace 内不得出现网络、磁盘或 channel await。未达标则保持性能状态 OPEN。

### Task 5: 文档、CodeGraph 和完成状态

**Files:**
- Modify: architecture/README/Superpowers status documents
- Create: `docs/verification/five-crate-final-report.md`

- [ ] **Step 1: 同步 CodeGraph**

```bash
codegraph sync
codegraph status
```

- [ ] **Step 2: 查询关键调用链**

验证 App→Facade→Core、Wrapper→SPI、Core snapshot→Metrics→gRPC→Admin repository→REST/Prometheus。

- [ ] **Step 3: 状态对照**

每个计划 Task 映射到源码、测试和命令输出；缺少任一项则不能勾选完成。

- [ ] **Step 4: 最终报告**

报告分为：已实现、未完成、测试证据、真实数据库边界、性能、安全、兼容性、部署剩余门禁。

**Suggested commit:** `test(architecture): verify five-crate release gates`
