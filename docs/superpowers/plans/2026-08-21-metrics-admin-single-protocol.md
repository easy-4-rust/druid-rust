# Metrics/Admin Single Protocol Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 用唯一 `druid_metrics.v1` 双向流协议接通 Metrics sampler/aggregator/client 与 Admin gRPC repository，并修复空 payload、假 ACK 和不安全默认监听。

**Architecture:** `druid-metrics` 独占 proto 和生成类型，通过 `client`/`server` feature 生成两端 stub；Admin 直接实现同一 server trait。ACK window 存储完整 protobuf frame，Admin 按 `(service_name, instance_id, boot_id, stream_epoch, sequence)` 去重。

**Tech Stack:** Tonic 0.14、Prost 0.14、Tokio、rustls、Axum、CancellationToken

**Spec:** `docs/superpowers/specs/2026-08-21-rdbc-first-five-crate-architecture.md`

## Global Constraints

- 唯一 proto package 为 `druid_metrics.v1`。
- 唯一 RPC 为 `Connect(stream ClientFrame) returns (stream ServerFrame)`。
- SQL 参数、token、密码不得进入 proto、日志或 Debug。
- ACK window 默认容量 256；相同 sequence 重发 payload 必须逐字节一致。
- 非 loopback HTTP/gRPC 必须同时启用 TLS 和认证。
- shutdown 必须发送 goodbye、flush pending，并在 deadline 内 join。

---

### Task 1: 冻结唯一 Proto 和生成合同

**Files:**
- Modify: `crates/druid-metrics/proto/druid_metrics_v1.proto`
- Modify: `crates/druid-metrics/build.rs`
- Modify: `crates/druid-metrics/Cargo.toml`
- Modify: `crates/druid-metrics/src/protocol.rs`
- Delete after migration: `crates/druid-admin/proto/ingest.proto`
- Delete after migration: `crates/druid-admin/build.rs`
- Test: `crates/druid-metrics/tests/protocol_roundtrip_test.rs`
- Create: `crates/druid-admin/tests/shared_protocol_compile_test.rs`

**Interfaces:**
- Produces:

```proto
service MetricsIngest {
  rpc Connect(stream ClientFrame) returns (stream ServerFrame);
}
```

- [ ] **Step 1: 写共享协议 RED 编译测试**

The Admin test must import exactly:

```rust
use druid_metrics::protocol::metrics_ingest_server::{
    MetricsIngest, MetricsIngestServer,
};
use druid_metrics::protocol::{ClientFrame, ServerFrame};
```

and instantiate `MetricsIngestServer::new(TestService)`. No `druid.admin.ingest` generated type
may appear.

- [ ] **Step 2: Run RED test**

```bash
cargo test -p druid-admin --test shared_protocol_compile_test
```

Expected: FAIL because Metrics currently generates neither client nor server stub.

- [ ] **Step 3: 启用 feature-gated stub 生成**

Change build generation to inspect Cargo's build-script feature environment:

```rust
let build_client = std::env::var_os("CARGO_FEATURE_CLIENT").is_some();
let build_server = std::env::var_os("CARGO_FEATURE_SERVER").is_some();
tonic_prost_build::configure()
    .build_client(build_client)
    .build_server(build_server)
    .compile_protos(&["proto/druid_metrics_v1.proto"], &["proto"])?;
```

Build tests must cover `--features client`, `--features server`, and `--features client,server`;
do not maintain two proto files.

Set dependencies:

```toml
# druid-metrics
[features]
default = []
client = ["dep:tonic"]
server = ["dep:tonic"]

# druid-admin
druid-metrics = { path = "../druid-metrics", version = "0.0.0-design", features = ["server"] }
```

- [ ] **Step 4: 迁移 Admin 并删除第二份 schema**

Replace Admin generated imports with `druid_metrics::protocol::*`, then delete Admin proto/build.rs
only after the shared compile test is GREEN.

- [ ] **Step 5: Verify GREEN**

```bash
cargo test -p druid-metrics --features client,server --test protocol_roundtrip_test
cargo test -p druid-admin --test shared_protocol_compile_test
test "$(find crates -path '*/proto/*' -type f \
  \( -name '*metrics*.proto' -o -name 'ingest.proto' \) | wc -l)" -eq 1
```

Expected: tests PASS and exactly one metrics/ingest proto remains.

- [ ] **Step 6: Commit**

```bash
git add crates/druid-metrics crates/druid-admin
git commit -m "refactor(metrics): share one ingest protocol with admin"
```

### Task 2: 让 Aggregator 输出真实 SnapshotBatch

**Files:**
- Modify: `crates/druid-metrics/src/aggregator.rs`
- Modify: `crates/druid-metrics/src/runtime.rs`
- Modify: `crates/druid-metrics/src/protocol.rs`
- Create: `crates/druid-metrics/src/snapshot_encoder.rs`
- Test: `crates/druid-metrics/tests/aggregator_output_test.rs`

**Interfaces:**
- Produces:

```rust
pub struct AggregatedBatch {
    pub entries: Vec<protocol::SnapshotEntry>,
    pub is_full_snapshot: bool,
}

pub(crate) async fn run_aggregator(
    snapshot_rx: mpsc::Receiver<PendingSnapshot>,
    batch_tx: mpsc::Sender<AggregatedBatch>,
    command_rx: mpsc::Receiver<AggregatorCommand>,
    cancel: CancellationToken,
    config: DruidMetricsConfig,
    self_metrics: Arc<RuntimeSelfMetrics>,
);
```

- [ ] **Step 1: 写 RED 行为测试**

Use paused Tokio time. Send two snapshots for the same datasource inside `coalesce_window`, advance
time, then assert one output entry containing the newer sequence. Send `batch_size + 1` distinct
datasources and assert two batches.

- [ ] **Step 2: Run RED test**

```bash
cargo test -p druid-metrics --test aggregator_output_test
```

Expected: FAIL because current aggregator has no output channel and ignores config fields.

- [ ] **Step 3: 实现 typed encoder 和 bounded output**

`snapshot_encoder.rs` converts `DruidTelemetrySnapshot` to `SnapshotEntry`, enforces
`max_sql_statements`, and applies `SqlTextPolicy`. The aggregator uses a bounded channel sized from
config and flushes on batch size, window expiry, explicit flush, and shutdown.

- [ ] **Step 4: Run GREEN tests**

```bash
cargo test -p druid-metrics --test aggregator_output_test
cargo test -p druid-metrics --test queue_semantics_test
cargo test -p druid-metrics --test sql_privacy_test
```

Expected: PASS; SQL hot path still never awaits channel send.

- [ ] **Step 5: Commit**

```bash
git add crates/druid-metrics/src crates/druid-metrics/tests
git commit -m "feat(metrics): emit bounded typed snapshot batches"
```

### Task 3: 修复 ACK Window、Payload 和远程命令

**Files:**
- Modify: `crates/druid-metrics/src/push_worker.rs`
- Modify: `crates/druid-metrics/src/sequence_window.rs`
- Modify: `crates/druid-metrics/src/registry.rs`
- Test: `crates/druid-metrics/tests/push_worker_test.rs`
- Test: `crates/druid-metrics/tests/remote_reset_test.rs`
- Test: `crates/druid-metrics/tests/sequence_window_test.rs`

**Interfaces:**
- Consumes: `AggregatedBatch`
- Produces:

```rust
pub struct PendingFrame {
    pub sequence: u64,
    pub encoded_frame: Vec<u8>,
}
```

- [ ] **Step 1: 写 payload identity RED 测试**

Push a batch with one non-empty entry, simulate disconnect, reconnect, and assert:

```rust
assert!(!first.snapshot_batch().entries.is_empty());
assert_eq!(first.encode_to_vec(), resent.encode_to_vec());
```

- [ ] **Step 2: 写 command truthfulness RED 测试**

Register a datasource probe whose `reset_stat()` increments an AtomicUsize. Send ResetStats and
assert success ACK only after count becomes 1. Send RequestFullSnapshot and assert a full batch is
sent before command success ACK.

- [ ] **Step 3: Run RED tests**

```bash
cargo test -p druid-metrics --test push_worker_test
cargo test -p druid-metrics --test remote_reset_test
```

Expected: FAIL because current entries are empty and commands are no-op success.

- [ ] **Step 4: 实现 immutable frame window**

Encode the complete `ClientFrame` once after sequence assignment, store those bytes, and decode/send
the same bytes on retry. Connect command handling to registry datasource IDs and aggregator full
snapshot command. WindowFull must pause batch admission until ACK or cancellation; it must not
terminate the worker.

- [ ] **Step 5: Run GREEN tests**

```bash
cargo test -p druid-metrics --test sequence_window_test
cargo test -p druid-metrics --test push_worker_test
cargo test -p druid-metrics --test remote_reset_test
```

Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/druid-metrics/src crates/druid-metrics/tests
git commit -m "fix(metrics): preserve payloads and execute remote commands"
```

### Task 4: 启动真实 Client 和 Admin Server

**Files:**
- Create: `crates/druid-metrics/src/grpc_client.rs`
- Modify: `crates/druid-metrics/src/runtime.rs`
- Modify: `crates/druid-admin/src/ingest/ingest_service.rs`
- Modify: `crates/druid-admin/src/bin/druid-admin.rs`
- Test: `crates/druid-admin/tests/grpc_network_e2e_test.rs`

**Interfaces:**
- Produces:

```rust
pub struct GrpcMetricsClient { /* owned task handles and cancellation */ }

#[derive(Debug, Clone)]
pub struct GrpcClientConfig {
    pub endpoint: tonic::transport::Endpoint,
    pub service_name: String,
    pub instance_id: String,
    pub boot_id: String,
    pub ack_window_capacity: usize,
    pub shutdown_timeout: Duration,
}

impl GrpcMetricsClient {
    pub async fn connect(config: GrpcClientConfig) -> Result<Self, MetricsError>;
    pub async fn shutdown(self, deadline: Duration) -> Result<(), MetricsError>;
}
```

- [ ] **Step 1: 写真实网络 RED E2E**

Bind Admin gRPC on `127.0.0.1:0`, start a Metrics client, push a snapshot, then query the shared
repository and assert the datasource/SQL/wall values. No direct invocation of service methods is
allowed in this test.

- [ ] **Step 2: Run RED test**

```bash
cargo test -p druid-admin --test grpc_network_e2e_test -- --nocapture
```

Expected: FAIL because the binary/runtime do not serve/connect gRPC.

- [ ] **Step 3: 实现 server serve 和 client supervisor**

Admin uses:

```rust
tonic::transport::Server::builder()
    .add_service(ingest_service.into_server())
    .serve_with_shutdown(grpc_addr, grpc_shutdown.cancelled())
    .await
```

Metrics client owns transport/push/reconnect handles, applies bounded exponential backoff from
250ms to 30s with jitter, and resends only unacked frames.

- [ ] **Step 4: Run GREEN and failure tests**

```bash
cargo test -p druid-admin --test grpc_network_e2e_test
cargo test -p druid-metrics --test push_worker_test
cargo test -p druid-metrics --test shutdown_test
```

Expected: all PASS, including reconnect and shutdown flush cases.

- [ ] **Step 5: Commit**

```bash
git add crates/druid-metrics crates/druid-admin
git commit -m "feat(metrics): connect runtime to standalone admin over grpc"
```

### Task 5: 收紧 Admin 默认安全边界

**Files:**
- Modify: `crates/druid-admin/src/config/monitor_properties.rs`
- Modify: `crates/druid-admin/src/bin/druid-admin.rs`
- Modify: `crates/druid-admin/src/lib.rs`
- Modify: `crates/druid-admin/src/servlet/monitor_view_servlet.rs`
- Test: `crates/druid-admin/tests/admin_binary_test.rs`
- Test: `crates/druid-admin/tests/servlet_test.rs`
- Create: `docs/security/druid-admin-threat-model.md`

**Interfaces:**
- Produces: loopback default, explicit remote-listen gate, shared HTTP/gRPC auth/TLS config

- [ ] **Step 1: 写 secure-default RED 测试**

Without environment variables, assert HTTP and gRPC addresses parse to loopback. Configure
`0.0.0.0` without TLS/auth and assert startup returns a configuration error.

- [ ] **Step 2: Run RED tests**

```bash
cargo test -p druid-admin --test admin_binary_test secure_defaults -- --exact
```

Expected: FAIL because defaults are `0.0.0.0` and standalone router has no auth.

- [ ] **Step 3: 实现统一应用装配**

Default to `127.0.0.1:8080` and `127.0.0.1:9090`. Build the production router through one
authenticated application path; remove behavioral duplication between `standalone_router` and
`MonitorViewServlet::router`. Remote bind validation requires both TLS material and an auth mode.

- [ ] **Step 4: Run security regression**

```bash
cargo test -p druid-admin --test admin_binary_test
cargo test -p druid-admin --test servlet_test
cargo test -p druid-admin --test grpc_network_e2e_test
```

Expected: unauthenticated protected routes rejected; health/login public; loopback default PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/druid-admin docs/security/druid-admin-threat-model.md
git commit -m "fix(admin): require secure remote management defaults"
```
