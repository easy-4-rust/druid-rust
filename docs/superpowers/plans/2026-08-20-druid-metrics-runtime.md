# Druid Metrics Runtime Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 创建 `druid-metrics`，在 SQL 热路径之外完成 datasource 注册、非阻塞采样、单写者聚合、timeline 和 Prometheus 模型。

**Architecture:** Core 只维护累计状态和 typed snapshots。Metrics 持有 monitor weak reference，由 sampler 定时 `try_snapshot`；bounded queue 饱和时按 datasource 合并最新完整快照，永不反压 SQL。

**Tech Stack:** Tokio bounded mpsc、Arc/Weak、ArcSwap、Prometheus、Serde

**Spec:** `docs/superpowers/plans/2026-08-20-five-crate-architecture-spec.md`

## Global Constraints

- SQL 方法内不得调用 channel `send().await`、网络或磁盘。
- SQL 参数永不上报。
- 默认 `FingerprintOnly`。
- Metrics task 必须有 supervisor、shutdown 和 JoinHandle。
- V1 不落盘。

---

### Task 1: 创建 crate、配置与公共类型

**Files:**
- Create: `crates/druid-metrics/Cargo.toml`
- Create: `crates/druid-metrics/src/lib.rs`
- Create: `crates/druid-metrics/src/config.rs`
- Create: `crates/druid-metrics/src/error.rs`
- Test: `crates/druid-metrics/tests/config_test.rs`
- Modify: root `Cargo.toml`

**Interfaces:**
- Produces:

```rust
pub struct DruidMetricsRuntime;
pub struct DruidMetricsConfig;
pub struct RegistrationGuard;

pub enum SqlTextPolicy {
    Disabled,
    FingerprintOnly,
    RawWithoutParameters,
}
```

- [ ] **Step 1: 写配置 RED 测试**

断言默认值：15s、1024、64、500ms、256、FingerprintOnly、5s shutdown。

- [ ] **Step 2: 运行 RED**

```bash
cargo test -p druid-metrics --test config_test
```

Expected: FAIL because package/types do not exist.

- [ ] **Step 3: 实现配置和校验**

queue/batch/window 必须 >0；sample/flush/shutdown 必须非零；错误为结构化 `MetricsConfigError`。

- [ ] **Step 4: 运行 GREEN**

```bash
cargo test -p druid-metrics --test config_test
```

### Task 2: 实现 registry 与生命周期

**Files:**
- Create: `crates/druid-metrics/src/registry.rs`
- Create: `crates/druid-metrics/src/runtime.rs`
- Test: `crates/druid-metrics/tests/registration_lifecycle_test.rs`

**Interfaces:**
- Consumes: `Arc<dyn druid_core::stats::DataSourceMonitorable>`
- Produces:

```rust
impl DruidMetricsRuntime {
    pub async fn start(config: DruidMetricsConfig) -> Result<Self, MetricsError>;
    pub fn register(&self, source: Arc<dyn DataSourceMonitorable>) -> RegistrationGuard;
    pub async fn shutdown(self, deadline: Duration) -> Result<(), MetricsError>;
}
```

- [ ] **Step 1: 写 registration RED 测试**

验证 Runtime 只保存 Weak；guard Drop 注销；source Drop 后下一轮采样自动清理。

- [ ] **Step 2: 运行 RED**

```bash
cargo test -p druid-metrics --test registration_lifecycle_test
```

- [ ] **Step 3: 实现 registry 与 supervisor**

Runtime 明确保存 sampler/aggregator/exporter JoinHandle 和 cancellation token；禁止 detach。

- [ ] **Step 4: 运行 GREEN**

```bash
cargo test -p druid-metrics --test registration_lifecycle_test
```

### Task 3: bounded queue、采样与聚合

**Files:**
- Create: `crates/druid-metrics/src/sampler.rs`
- Create: `crates/druid-metrics/src/aggregator.rs`
- Create: `crates/druid-metrics/src/self_metrics.rs`
- Test: `crates/druid-metrics/tests/queue_semantics_test.rs`

**Interfaces:**
- Produces: `SnapshotBatch`, `PendingSnapshot`, runtime self-metrics

- [ ] **Step 1: 写饱和 RED 测试**

```rust
#[tokio::test]
async fn saturated_metrics_queue_never_blocks_datasource_operations() {
    // Fill a capacity-1 queue, keep producing datasource state, and assert
    // the producer returns immediately while coalesced_total increases.
}
```

- [ ] **Step 2: 运行 RED**

```bash
cargo test -p druid-metrics --test queue_semantics_test -- --nocapture
```

- [ ] **Step 3: 实现 sampler 与 coalescing**

`try_snapshot::Busy` 增加 `snapshot_busy_total`；queue/window 满时覆盖同 datasource 的 pending latest，不分配 sequence；成功入 pending window 后才分配 sequence。

- [ ] **Step 4: 运行 GREEN**

```bash
cargo test -p druid-metrics --test queue_semantics_test
```

### Task 4: Timeline 与 Prometheus model

**Files:**
- Create: `crates/druid-metrics/src/timeline.rs`
- Create: `crates/druid-metrics/src/prometheus.rs`
- Test: `crates/druid-metrics/tests/timeline_test.rs`
- Test: `crates/druid-metrics/tests/prometheus_cardinality_test.rs`

**Interfaces:**
- Produces: `TimelineSnapshot`, `PrometheusSnapshot`

- [ ] **Step 1: 写 timeline RED 测试**

精确断言 15s×180、60s×360、3600s×360，满后覆盖最旧值。

- [ ] **Step 2: 写 label RED 测试**

只允许 `service/instance/datasource/db_type/driver`；SQL text/fingerprint/request ID 出现在 label 时测试失败。

- [ ] **Step 3: 实现 timeline 与 encoder**

Prometheus counter 使用 boot 内累计值；SQL detail 保留在 repository payload，不导出为 label。

- [ ] **Step 4: 运行 GREEN**

```bash
cargo test -p druid-metrics --test timeline_test
cargo test -p druid-metrics --test prometheus_cardinality_test
```

### Task 5: SQL 隐私和 shutdown

**Files:**
- Create: `crates/druid-metrics/src/sanitizer.rs`
- Test: `crates/druid-metrics/tests/sql_privacy_test.rs`
- Test: `crates/druid-metrics/tests/shutdown_test.rs`

- [ ] **Step 1: 写参数泄漏 RED 测试**

fixture 同时包含原 SQL、parameterized SQL、fingerprint 和绑定参数；FingerprintOnly 输出只能包含 parameterized/fingerprint。

- [ ] **Step 2: 实现 sanitizer**

序列化前递归检查 payload；发现 bind values、password、token 字段返回 `MetricsError::SensitiveField`。

- [ ] **Step 3: 实现 shutdown 顺序**

停止采样→关闭 producer→flush aggregator→等待 exporter→join；deadline 后返回未 flush 数量。

- [ ] **Step 4: 验证 crate**

```bash
cargo test -p druid-metrics --all-targets
```

**Suggested commit:** `feat(metrics): add non-blocking local metrics runtime`
