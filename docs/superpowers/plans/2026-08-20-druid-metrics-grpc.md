# Druid Metrics gRPC Delivery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 为 `druid-metrics` 实现 gRPC 双向流、sequence/ACK、会话内至少一次、重连重发和远程 reset 命令。

**Architecture:** 客户端只重试已进入有界 ACK window 的 batch；进入 window 前的快照可合并。服务端按 stream identity 去重，Admin 重启时要求客户端重新发送 full snapshot。V1 不使用磁盘 WAL。

**Tech Stack:** Tonic、Prost、Tokio stream、rustls、vendored protoc

**Spec:** `docs/superpowers/plans/2026-08-20-druid-metrics-runtime.md`

## Global Constraints

- `protocol_version = 1`。
- 未 ACK window 容量默认 256。
- 退避 250ms 起、30s 封顶、带 jitter。
- 非 loopback 生产监听必须启用 TLS。
- Token 与 SQL 参数不得进入日志或 Debug。

---

### Task 1: 冻结 Proto 与生成流程

**Files:**
- Create: `crates/druid-metrics/proto/druid_metrics_v1.proto`
- Create: `crates/druid-metrics/build.rs`
- Create: `crates/druid-metrics/src/protocol.rs`
- Modify: Metrics manifest/features
- Test: `crates/druid-metrics/tests/protocol_roundtrip_test.rs`

**Interfaces:**
- Produces: `DruidMetricsIngest::Connect(stream ClientFrame) -> stream ServerFrame`

- [ ] **Step 1: 写 round-trip RED 测试**

构造 Hello、SnapshotBatch、Heartbeat、CommandAck、Goodbye 和全部 ServerFrame variant，经 Prost encode/decode 后逐字段相等。

- [ ] **Step 2: 运行 RED**

```bash
cargo test -p druid-metrics --features client,server --test protocol_roundtrip_test
```

Expected: FAIL because proto/generated module is absent.

- [ ] **Step 3: 写 Proto**

公共字段固定为：

```text
protocol_version, service_name, instance_id, boot_id,
stream_epoch, sequence, emitted_at_unix_ms
```

Client oneof：Hello、SnapshotBatch、Heartbeat、CommandAck、Goodbye。Server oneof：HelloAck、BatchAck、ResyncRequired、Command、Error。

- [ ] **Step 4: 使用 vendored protoc**

`build.rs` 从 `protoc-bin-vendored` 解析可执行路径，再调用 `tonic-build`；CI 不读取系统 `protoc`。

- [ ] **Step 5: 运行 GREEN**

Run the Step 2 command. Expected: PASS.

### Task 2: 客户端 session 状态机

**Files:**
- Create: `crates/druid-metrics/src/grpc/client.rs`
- Create: `crates/druid-metrics/src/grpc/session.rs`
- Create: `crates/druid-metrics/src/grpc/backoff.rs`
- Test: `crates/druid-metrics/tests/client_session_test.rs`

**Interfaces:**
- Consumes: `SnapshotBatch` from Metrics runtime
- Produces: `GrpcMetricsClient`, `ClientSessionState`

- [ ] **Step 1: 写 sequence RED 测试**

验证 sequence 仅在 batch 进入 pending window 后递增；duplicate ACK 无副作用；越界 ACK 返回 `ProtocolError::UnexpectedAck`。

- [ ] **Step 2: 写重连 RED 测试**

断线后按原 sequence 顺序重发全部未 ACK batch，不为重发分配新 sequence。

- [ ] **Step 3: 实现状态机**

状态：Disconnected→Connecting→Streaming→Backoff/Resync→Streaming→Closing。所有 spawned task 必须由 `GrpcMetricsClient` 持有 JoinHandle。

- [ ] **Step 4: 实现退避**

250ms、500ms、1s…30s；成功 HelloAck 后重置；jitter 范围 ±20%。

- [ ] **Step 5: 运行 GREEN**

```bash
cargo test -p druid-metrics --features client --test client_session_test
```

### Task 3: 服务端去重、Resync 与命令

**Files:**
- Create: `crates/druid-metrics/src/grpc/server.rs`
- Create: `crates/druid-metrics/src/grpc/server_session.rs`
- Test: `crates/druid-metrics/tests/server_session_test.rs`

**Interfaces:**
- Produces: `MetricsIngestService`, `MetricsIngestHandler`

```rust
pub trait MetricsIngestHandler: Send + Sync {
    async fn ingest(&self, batch: SnapshotBatch) -> Result<(), MetricsError>;
    async fn reset(&self, target: ResetTarget) -> Result<(), MetricsError>;
}
```

- [ ] **Step 1: 写去重 RED 测试**

相同 identity/sequence 两次到达只调用 handler 一次，两次均返回 BatchAck。

- [ ] **Step 2: 写乱序/重启 RED 测试**

sequence 大于 expected 返回 ResyncRequired；server session 空状态收到旧 epoch 时要求 full snapshot seq=1。

- [ ] **Step 3: 实现 session map**

key 精确为 `(service_name, instance_id, boot_id, stream_epoch)`；每个 session 保存 last applied/acked sequence 和 heartbeat。

- [ ] **Step 4: 实现命令 ACK**

`RequestFullSnapshot` 和 `ResetStats` 带 command_id；只有匹配的 CommandAck 才关闭 pending command。

- [ ] **Step 5: 运行 GREEN**

```bash
cargo test -p druid-metrics --features server --test server_session_test
```

### Task 4: 安全、故障与端到端验证

**Files:**
- Create: `crates/druid-metrics/src/grpc/auth.rs`
- Create: `crates/druid-metrics/src/grpc/tls.rs`
- Test: `crates/druid-metrics/tests/grpc_delivery_test.rs`

- [ ] **Step 1: 写 auth/TLS RED 测试**

缺 token、错误 scope、非 loopback 无 TLS 都拒绝；loopback test profile 可显式允许明文。

- [ ] **Step 2: 实现 bearer scope**

scope 至少为 ingest/control；使用 constant-time token 比较；Debug 只显示 `***`。

- [ ] **Step 3: 实现 in-process gRPC 故障测试**

覆盖 ACK 丢失、服务端中断、重连、duplicate、Resync、ResetStats、shutdown flush。

- [ ] **Step 4: 运行全协议门禁**

```bash
cargo test -p druid-metrics --features client,server --test grpc_delivery_test
cargo check -p druid-metrics --all-targets --all-features
```

Expected: both commands exit 0.

**Suggested commit:** `feat(metrics): add acknowledged grpc delivery`
