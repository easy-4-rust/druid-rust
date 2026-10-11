# Runtime Production Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 Pool、Metrics 和 Admin 建立有界队列、受监管任务、确定 shutdown、容量预算、TTL 和安全运行边界，使过载、取消和服务实例 churn 可观测且可恢复。

**Architecture:** Pool close 使用有界命令队列并限制慢关闭积压，refill 使用 latest-target 语义；Metrics 所有任务归一个 supervisor；Admin repository 只保留有 TTL/容量的最新状态。所有服务从配置中得到明确预算并暴露自监控指标。

**Tech Stack:** Tokio mpsc/watch/JoinSet、CancellationToken、parking_lot、Prometheus、Axum/Tower、Tonic

**Spec:** `docs/superpowers/specs/2026-08-21-rdbc-first-five-crate-architecture.md`

## Global Constraints

- 前置：单一 Metrics/Admin 协议完成；RDBC resource cancellation 设计稳定。
- 禁止无界数据队列；控制队列若保留无界必须证明消息总量存在硬上限。
- 所有 task 有 owner、panic/failure 传播和 join deadline。
- shutdown 顺序：stop admission → close producers → drain/flush → cancel → join。
- SQL 热路径不等待 Metrics 队列、网络或磁盘。
- Admin 默认 loopback；非 loopback 使用 TLS+认证。

---

### Task 1: Pool close/refill 队列容量与慢关闭治理

**Files:**
- Modify: `crates/druid/src/pool/config.rs`
- Modify: `crates/druid/src/pool/druid_pool.rs`
- Modify: `crates/druid/src/pool/pool_inner.rs`
- Modify: `crates/druid/src/pool/connection_close_worker.rs`
- Modify: `crates/druid/src/pool/connection_create_worker.rs`
- Test: `crates/druid/tests/pool_close_backpressure_test.rs`
- Test: `crates/druid/tests/pool_maintenance_semantics_test.rs`

**Interfaces:**
- Produces config:

```rust
pub(crate) struct PoolWorkerBudget {
    close_queue_capacity: usize,
    close_timeout: Duration,
    shutdown_timeout: Duration,
}
```

- [ ] **Step 1: 写 close storm RED test**

Use a factory whose close waits on a semaphore. Repeatedly discard/refill more than
`max_open + close_queue_capacity`, then assert producer behavior is bounded and queue depth never
exceeds capacity. Release closes and assert shutdown joins all work.

- [ ] **Step 2: Run RED**

```bash
cargo test -p druid --test pool_close_backpressure_test -- --nocapture
```

Expected: FAIL because close/refill channels are currently unbounded and expose no queue depth.

- [ ] **Step 3: 实现 bounded close 和 latest refill target**

Use bounded mpsc for physical close commands. QueueFull policy is: attempt a deadline-bounded close
in the caller's explicit async close path; Drop path marks overflow and relies on owned connection
drop without spawning. Replace repeated refill commands with `watch::Sender<usize>` or atomic max
target plus Notify.

- [ ] **Step 4: Add self metrics**

Expose `close_queue_depth`, `close_queue_rejected_total`, `close_timeout_total`,
`refill_target`, and worker termination status in the pool snapshot.

- [ ] **Step 5: Run GREEN**

```bash
cargo test -p druid --test pool_close_backpressure_test
cargo test -p druid --test pool_maintenance_semantics_test
cargo test -p druid --test recycle_semantics_test
```

Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/druid/src/pool crates/druid/tests
git commit -m "fix(pool): bound worker queues and close latency"
```

### Task 2: Metrics 单一 Supervisor 和确定 shutdown

**Files:**
- Modify: `crates/druid-metrics/src/runtime.rs`
- Modify: `crates/druid-metrics/src/registry.rs`
- Modify: `crates/druid-metrics/src/self_metrics.rs`
- Create: `crates/druid-metrics/src/supervisor.rs`
- Test: `crates/druid-metrics/tests/task_supervision_test.rs`
- Test: `crates/druid-metrics/tests/shutdown_test.rs`

**Interfaces:**
- Produces:

```rust
pub(crate) struct MetricsSupervisor {
    tasks: tokio::task::JoinSet<Result<(), MetricsError>>,
    cancel: CancellationToken,
}
```

- [ ] **Step 1: 写 orphan-task RED test**

Keep a `RegistrationGuard` alive beyond runtime shutdown, then assert all runtime task counters reach
zero and the registry Arc can be dropped. Inject a sampler panic/error and assert shutdown returns a
structured supervisor error.

- [ ] **Step 2: Run RED**

```bash
cargo test -p druid-metrics --test task_supervision_test
```

Expected: FAIL because unregister listener is detached and task failures are discarded.

- [ ] **Step 3: 将所有任务纳入 JoinSet**

Supervisor owns sampler, aggregator, exporter/client and unregister listener. Registration admission
closes first; producers are dropped; aggregator flushes; exporter sends pending/goodbye; cancellation
then stops idle tasks; JoinSet is drained within configured deadline.

- [ ] **Step 4: Consume every public config field**

Add tests proving `batch_size`, `coalesce_window`, `max_sql_statements`, `sql_text_policy`, and
`shutdown_timeout` change observable behavior. Remove any public field that still has no consumer.

- [ ] **Step 5: Run GREEN**

```bash
cargo test -p druid-metrics --test task_supervision_test
cargo test -p druid-metrics --test shutdown_test
cargo test -p druid-metrics --test queue_semantics_test
```

Expected: all PASS and zero orphan tasks.

- [ ] **Step 6: Commit**

```bash
git add crates/druid-metrics
git commit -m "fix(metrics): supervise every task and flush shutdown"
```

### Task 3: Admin Repository TTL、容量和 session identity

**Files:**
- Modify: `crates/druid-admin/src/repository/metrics_repository.rs`
- Create: `crates/druid-admin/src/repository/repository_config.rs`
- Create: `crates/druid-admin/src/repository/eviction_report.rs`
- Modify: `crates/druid-admin/src/model/dto/data_source_result.rs`
- Modify: `crates/druid-admin/src/model/dto/sql_list_result.rs`
- Modify: `crates/druid-admin/src/model/dto/wall_result.rs`
- Test: `crates/druid-admin/tests/repository_capacity_test.rs`
- Test: `crates/druid-admin/tests/ingest_test.rs`

**Interfaces:**
- Produces:

```rust
pub(crate) struct RepositoryConfig {
    entry_ttl: Duration,
    max_services: usize,
    max_datasources: usize,
    max_sql_per_datasource: usize,
}
```

- [ ] **Step 1: 写 pod churn RED test**

Insert more than `max_services` distinct boot/service identities, advance time beyond TTL, invoke
maintenance, and assert memory-visible counts stay within limits. Verify newest sequence wins and
expired identities disappear from REST/Prometheus.

- [ ] **Step 2: Run RED**

```bash
cargo test -p druid-admin --test repository_capacity_test
```

Expected: FAIL because current HashMap has no TTL/capacity eviction.

- [ ] **Step 3: 实现 bounded repository**

Key entries by complete stream/session identity plus datasource ID. Apply SQL cap before insertion,
evict expired entries before capacity eviction, and record reasons `ttl/service_cap/datasource_cap/
sql_cap`. Prometheus remains long-term history; repository stores latest state only.

- [ ] **Step 4: Run GREEN**

```bash
cargo test -p druid-admin --test repository_capacity_test
cargo test -p druid-admin --test ingest_test
cargo test -p druid-admin --test prometheus_test
```

Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/druid-admin
git commit -m "fix(admin): bound metrics repository lifecycle"
```

### Task 4: HTTP/gRPC 认证、授权、TLS 和脱敏统一

**Files:**
- Create: `crates/druid-admin/src/security/admin_security_config.rs`
- Create: `crates/druid-admin/src/security/authenticator.rs`
- Create: `crates/druid-admin/src/security/redaction.rs`
- Modify: `crates/druid-admin/src/lib.rs`
- Modify: `crates/druid-admin/src/ingest/ingest_service.rs`
- Modify: `crates/druid-admin/src/bin/druid-admin.rs`
- Test: `crates/druid-admin/tests/security_e2e_test.rs`
- Modify: `docs/security/druid-admin-threat-model.md`

**Interfaces:**
- Produces one security policy consumed by both Tower HTTP middleware and Tonic interceptor

- [ ] **Step 1: 写 unauthenticated/TLS RED matrix**

Assert protected HTTP and gRPC requests fail without credentials, invalid credentials fail, valid
credentials succeed, loopback dev mode remains explicit, and non-loopback startup fails without TLS.

- [ ] **Step 2: Run RED**

```bash
cargo test -p druid-admin --test security_e2e_test -- --nocapture
```

Expected: FAIL because standalone HTTP and gRPC do not share security.

- [ ] **Step 3: 实现共享 policy**

Authenticator receives a redacted credential token and returns an authorization principal. HTTP and
gRPC attach the same principal/role model. Response redaction hides URL credentials, usernames when
configured, raw SQL and slow parameters according to policy.

- [ ] **Step 4: Run GREEN/security regression**

```bash
cargo test -p druid-admin --test security_e2e_test
cargo test -p druid-admin --test servlet_test
cargo test -p druid-admin --test grpc_network_e2e_test
```

Expected: all PASS; no token/password appears in captured logs.

- [ ] **Step 5: Commit**

```bash
git add crates/druid-admin docs/security/druid-admin-threat-model.md
git commit -m "feat(admin): unify management plane security"
```

### Task 5: 故障注入、运行时预算和报告

**Files:**
- Create: `crates/druid-bench/benches/runtime.rs`
- Create: `docs/verification/rdbc-first/runtime-hardening.md`
- Modify: `.github/workflows/driver-matrix.yml`

**Interfaces:**
- Produces: queue/latency/RSS/shutdown budgets and CI fault scenarios

- [ ] **Step 1: 运行故障矩阵**

Scenarios: slow driver close, saturated metrics queue, Admin unavailable/restart, duplicate batch,
sequence gap, expired repository entries, invalid auth, SIGTERM during pending ACK, and a JDBC
Agent that never sends ready/handshake (startup must fail within the configured request timeout).
An Agent sending an oversized frame without a newline must fail at the configured frame limit,
without waiting for EOF or allocating the entire frame.
Malformed Agent ready/handshake payloads must not be echoed into error messages or logs.

- [ ] **Step 2: 运行性能和 shutdown 测量**

```bash
cargo bench -p druid-bench --bench runtime -- --noplot
cargo test -p druid-metrics --test shutdown_test -- --nocapture
cargo test -p druid-admin --test grpc_network_e2e_test -- --nocapture
```

Record p50/p95/p99, max queue depth, dropped/coalesced count, peak RSS and shutdown time. Targets are
accepted only after baseline measurement and must be written as numeric budgets in the report.

- [ ] **Step 3: Add CI gates**

Add deterministic failure tests to normal CI; keep long performance measurements scheduled/manual
if hosted-runner variance prevents a stable threshold.

- [ ] **Step 4: Commit**

```bash
git add crates/druid-bench docs/verification/rdbc-first/runtime-hardening.md \
  .github/workflows/driver-matrix.yml
git commit -m "test(runtime): gate overload shutdown and security behavior"
```
