# Standalone Druid Admin Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 `druid-admin` 改成独立 Axum 服务，通过 gRPC push repository 提供静态 UI、Java 兼容 REST/JSON、Prometheus 和远程 reset。

**Architecture:** Admin V1 单实例，HTTP/UI 和 gRPC ingest 分端口运行，共享内存 repository。长期历史交给 Prometheus；当前 DiscoveryClient/K8s/远端 HTTP pull 被移除。

**Tech Stack:** Axum、Tower、Tonic server、Tokio、Prometheus、Serde

**Spec:** `docs/superpowers/plans/2026-08-20-druid-metrics-grpc.md`

## Global Constraints

- Admin 不依赖 `druid-wrapper`。
- HTTP 默认 8080，gRPC 默认 9090。
- heartbeat 10s，30s 无心跳标记 offline。
- 静态前端先兼容现有资源和 API。
- Admin 不持久化指标数据库。

---

### Task 1: 独立 binary、配置和 shutdown

**Files:**
- Create: `crates/druid-admin/src/main.rs`
- Create: `crates/druid-admin/src/config/admin_config.rs`
- Create: `crates/druid-admin/src/application.rs`
- Modify: Admin manifest/lib
- Test: `crates/druid-admin/tests/config_precedence_test.rs`
- Test: `crates/druid-admin/tests/shutdown_test.rs`

**Interfaces:**
- Produces: standalone `druid-admin` binary and `AdminApplication`

- [ ] **Step 1: 写配置 RED 测试**

断言 env > TOML > default；默认 HTTP/gRPC 地址精确为 `0.0.0.0:8080`/`:9090`。

- [ ] **Step 2: 运行 RED**

```bash
cargo test -p druid-admin --test config_precedence_test
```

- [ ] **Step 3: 实现 config 和 binary**

配置包含 bind、TLS、ingest token file、Web 用户名/密码、instance TTL 和 shutdown deadline。

- [ ] **Step 4: 实现统一 shutdown**

Ctrl-C/TERM 停止 admission，关闭 HTTP/gRPC，flush Metrics repository task，deadline 内 join。

- [ ] **Step 5: 运行 GREEN**

```bash
cargo test -p druid-admin --test config_precedence_test --test shutdown_test
```

### Task 2: MetricsRepository

**Files:**
- Create: `crates/druid-admin/src/repository/metrics_repository.rs`
- Create: `crates/druid-admin/src/repository/instance_state.rs`
- Create: `crates/druid-admin/src/repository/mod.rs`
- Test: `crates/druid-admin/tests/metrics_repository_test.rs`

**Interfaces:**
- Implements: `druid_metrics::grpc::MetricsIngestHandler`
- Produces: latest instance snapshots, timelines, sequence state and command registry

- [ ] **Step 1: 写 ingest RED 测试**

验证 full/delta snapshot、duplicate sequence、instance isolation、last ACK 和 SQL fingerprint detail。

- [ ] **Step 2: 写 TTL RED 测试**

注入固定时钟；29s online、30s offline；新 heartbeat 恢复 online。

- [ ] **Step 3: 实现 repository**

单写者更新，读侧使用 ArcSwap/immutable snapshot；SQL 参数字段一律拒绝。

- [ ] **Step 4: 运行 GREEN**

```bash
cargo test -p druid-admin --test metrics_repository_test
```

### Task 3: 重写 MonitorStatService

**Files:**
- Modify: `crates/druid-admin/src/service/monitor_stat_service.rs`
- Delete after replacement: discovery/HTTP pull modules
- Test: existing DTO/service tests plus `repository_query_test.rs`

**Interfaces:**
- Consumes: `Arc<MetricsRepository>`
- Produces: Java-compatible datasource/sql/wall/connection JSON

- [ ] **Step 1: 写 repository-backed RED 测试**

构造两个 instance snapshots，断言合并、排序、分页、空值、offline 状态和错误码。

- [ ] **Step 2: 运行 RED**

```bash
cargo test -p druid-admin --test repository_query_test
```

- [ ] **Step 3: 替换远端请求**

删除 DiscoveryClient/K8s/Reqwest 路径；业务查询只能读取 repository，不进行 handler 内网络 I/O。

- [ ] **Step 4: 运行 GREEN**

```bash
cargo test -p druid-admin --test repository_query_test
```

### Task 4: Axum API 与兼容静态资源

**Files:**
- Create: `crates/druid-admin/src/assets/static_assets.rs`
- Modify: router/servlet modules
- Test: `crates/druid-admin/tests/http_api_test.rs`
- Test: `crates/druid-admin/tests/static_assets_test.rs`

- [ ] **Step 1: 写路由 RED 测试**

覆盖 `/druid/*.json`、`/druid/api/*`、login/session、`/metrics`、`/health/live`、`/health/ready`。

- [ ] **Step 2: 建立 StaticAssets**

统一 MIME、ETag、Cache-Control、404；现有 HTML/CSS/JS 内容不重写。

- [ ] **Step 3: 直接使用 Axum/Tower**

移除 Topcoat 启动外壳；handler 状态只持有 repository/service/auth。

- [ ] **Step 4: 运行 GREEN**

```bash
cargo test -p druid-admin --test http_api_test --test static_assets_test
```

### Task 5: Prometheus、认证与 ResetStats

**Files:**
- Create: `crates/druid-admin/src/auth/ingest_auth.rs`
- Create: `crates/druid-admin/src/auth/web_session.rs`
- Create: `crates/druid-admin/src/metrics/admin_metrics.rs`
- Test: `crates/druid-admin/tests/security_and_reset_test.rs`

- [ ] **Step 1: 写安全 RED 测试**

验证 token scope、TLS policy、HttpOnly/SameSite cookie、敏感字段脱敏和 metric label 白名单。

- [ ] **Step 2: 写 reset RED 测试**

只有匹配 CommandAck 才返回成功；offline、timeout、NACK 均返回明确失败且不清除本地展示数据。

- [ ] **Step 3: 实现认证/metrics/reset**

`/metrics` 输出 Admin runtime、ingest 和低基数 datasource 指标；不得按 SQL fingerprint 建 label。

- [ ] **Step 4: 验证 Admin**

```bash
cargo test -p druid-admin --all-targets
cargo tree -p druid-admin | rg "druid-wrapper" && exit 1 || true
```

Expected: tests PASS and no Wrapper dependency match.

**Suggested commit:** `feat(admin): run standalone metrics control plane`
