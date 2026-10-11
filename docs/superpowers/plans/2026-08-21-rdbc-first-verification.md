# RDBC-First Verification and Release Readiness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 对 ADR-CRATE-002 执行依赖、API、语义、驱动、协议、并发、安全、性能、文档和发布门禁，并为每个要求给出新鲜证据或明确 NOT_VERIFIED。

**Architecture:** 本计划不新增生产功能；它只消费前六份计划的交付物，运行全范围门禁并生成最终 verdict。任何失败项都阻止 `COMPLETE`，但不允许为了报告变绿而缩小规格范围。

**Tech Stack:** Cargo、CodeGraph、cargo-public-api、cargo-semver-checks、cargo-hack、cargo-deny、cargo-audit、Criterion、Markdown validation

**Spec:** `docs/superpowers/specs/2026-08-21-rdbc-first-five-crate-architecture.md`

## Global Constraints

- 不以文件存在、编译成功、HTTP 200、测试数量或计划 checkbox 代替行为完成证据。
- default suite、all-features、MSRV、真实数据库和真实网络证据分开报告。
- 任何未执行命令标记 `NOT_RUN`，外部环境缺失标记 `NOT_VERIFIED`。
- 不发布、不 push；只有用户另行授权后才能执行 crates.io/GitHub 发布。
- 报告必须记录命令、时间、平台、SHA、exit code 和关键计数。

---

### Task 1: Workspace、DAG 和 forbidden dependency

**Files:**
- Create: `docs/verification/rdbc-first/final-dependency-report.md`
- Modify: topology/dependency scripts

**Interfaces:**
- Produces: exact package set and recursive dependency evidence

- [ ] **Step 1: 验证 package 集合**

```bash
cargo metadata --format-version 1 --no-deps \
  | jq -r '.packages[].name' | sort
```

Expected exactly:

```text
druid
druid-admin
druid-bench
druid-metrics
druid-rdbc
druid-wrapper
```

- [ ] **Step 2: 验证依赖方向**

```bash
python3 scripts/verify_rdbc_dependency_boundary.py
python3 scripts/verify_feature_dependency_boundaries.py
python3 scripts/verify_package_ownership.py
cargo tree -p druid-rdbc -e normal
cargo tree -p druid -e normal --depth 2
cargo tree -p druid-admin -e normal --depth 2
```

Expected: `druid-rdbc` has no forbidden dependency; Admin reaches Metrics but not direct Druid/
Wrapper; every package is `publish=false`.

- [ ] **Step 3: 写报告**

Include the exact Cargo package IDs, edges, rejected edges and script exit codes.

- [ ] **Step 4: Commit report**

```bash
git add docs/verification/rdbc-first/final-dependency-report.md scripts
git commit -m "test(architecture): verify rdbc-first dependency dag"
```

### Task 2: Format、build、feature、MSRV、doc 和 public API

**Files:**
- Create: `docs/verification/rdbc-first/final-quality-report.md`
- Create: `api/druid-rdbc-public-api.txt`
- Create: `api/druid-public-api.rdbc-first.final.txt`

**Interfaces:**
- Produces: quality/API/semver verdict

- [ ] **Step 1: Run format/check/clippy**

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- \
  -D warnings -A clippy::pedantic
```

Expected: all exit 0.

- [ ] **Step 2: Run feature powerset**

```bash
cargo hack check --workspace --feature-powerset --no-dev-deps
```

Expected: exit 0. If cargo-hack is unavailable, record missing tool and use the explicit feature
matrix from the driver-feature plan; do not silently skip.

- [ ] **Step 3: Run MSRV**

```bash
cargo +1.95 check --workspace --all-targets
```

Expected: exit 0.

- [ ] **Step 4: Run tests/docs**

```bash
cargo test --workspace
cargo test --workspace --all-features
cargo doc --workspace --all-features --no-deps
```

Expected: all exit 0; report passed/failed/ignored counts separately.

- [ ] **Step 5: Capture public API**

```bash
cargo public-api -p druid-rdbc -sss --all-features \
  > api/druid-rdbc-public-api.txt
cargo public-api -p druid -sss --all-features \
  > api/druid-public-api.rdbc-first.final.txt
cargo semver-checks check-release -p druid-rdbc
cargo semver-checks check-release -p druid
```

Expected: API files non-empty; old `druid::sql::*` external fixture PASS; semver differences either
PASS or listed as explicitly approved pre-release changes.

- [ ] **Step 6: Commit report**

```bash
git add docs/verification/rdbc-first/final-quality-report.md api
git commit -m "test(quality): verify rdbc-first api and toolchains"
```

### Task 3: RDBC、External Pool 和 Java 语义合同

**Files:**
- Create: `docs/verification/rdbc-first/final-semantic-report.md`

**Interfaces:**
- Produces: conformance matrix with Java oracle/Rust contract/real DB columns

- [ ] **Step 1: Run core RDBC contracts**

```bash
cargo test -p druid-rdbc --tests
cargo test -p druid --test task6_standard_connection_contract_parity
cargo test -p druid --test rdbc_resource_context_test
cargo test -p druid --test task6_buffered_result_set_canonical_matrix_test
cargo test -p druid --test task7_pooled_async_result_set_test
```

Expected: all PASS.

- [ ] **Step 2: Run adapter parity**

```bash
cargo test -p druid-wrapper --test task6_sqlx_canonical_matrix_test
cargo test -p druid-wrapper --test task6_sqlx_method_parity_test
cargo test -p druid-wrapper --test sqlx_bb8_pool_test
cargo test -p druid-wrapper --test sqlx_deadpool_pool_test
cargo test -p druid-wrapper --test rbdc_connection_adapter_test
cargo test -p druid-wrapper --test toasty_connection_adapter_test
```

Expected: all configured SQLite/local contracts PASS; capability matrix contains no declaration/
behavior mismatch.

- [ ] **Step 3: Run Java/Rust differential suites**

Run the repository's frozen Java oracle commands and corresponding Rust differential tests for Pool,
Statement, PreparedStatement, ResultSet, Wall, exception sorter and lifecycle. Record Java JDK,
Druid SHA and exact case counts.

- [ ] **Step 4: Write matrix**

Every behavior row records `JAVA_ORACLE`, `RUST_CONTRACT`, `REAL_DB`, and `STATUS`; a missing column
prevents DONE.

- [ ] **Step 5: Commit report**

```bash
git add docs/verification/rdbc-first/final-semantic-report.md
git commit -m "test(semantics): verify rdbc and java parity contracts"
```

### Task 4: Metrics/Admin 可靠性与安全

**Files:**
- Create: `docs/verification/rdbc-first/final-runtime-security-report.md`

**Interfaces:**
- Produces: single-protocol, fault, shutdown and management security verdict

- [ ] **Step 1: Verify one protocol**

```bash
find crates -path '*/proto/*.proto' -print
cargo test -p druid-metrics --features client,server --test protocol_roundtrip_test
cargo test -p druid-admin --test shared_protocol_compile_test
```

Expected: one Metrics proto and tests PASS.

- [ ] **Step 2: Run real network/fault tests**

```bash
cargo test -p druid-admin --test grpc_network_e2e_test -- --nocapture
cargo test -p druid-metrics --test push_worker_test
cargo test -p druid-metrics --test shutdown_test
cargo test -p druid-admin --test repository_capacity_test
```

Expected: payload non-empty, byte-identical resend, duplicate ACK/gap handling, full snapshot,
reset, shutdown flush and TTL/cap PASS.

- [ ] **Step 3: Run security tests**

```bash
cargo test -p druid-admin --test security_e2e_test -- --nocapture
```

Expected: loopback default; non-loopback without TLS/auth rejected; protected HTTP/gRPC requires
credentials; captured logs contain no token/password/bind parameter.

- [ ] **Step 4: Commit report**

```bash
git add docs/verification/rdbc-first/final-runtime-security-report.md
git commit -m "test(runtime): verify metrics admin reliability and security"
```

### Task 5: Supply chain、真实数据库和性能

**Files:**
- Create: `docs/verification/rdbc-first/final-release-report.md`

**Interfaces:**
- Produces: final RELEASE_READY / NOT_READY verdict

- [ ] **Step 1: Run supply-chain gates**

```bash
cargo audit --ignore RUSTSEC-2023-0071
cargo deny check
```

Expected: exit 0 with only the documented narrow advisory exception.
Before the ignored audit, verify from locked Cargo dependency metadata that
`rsa@0.9.10` is still the sole waived RSA package and its only direct consumers
are `druid` and `sqlx-mysql`. Fail the gate if the waiver becomes stale or its
dependency scope expands.

- [ ] **Step 2: Run real database certification**

Run configured SQLite and DuckDB contract matrices plus any available PostgreSQL/MySQL/JDBC Agent
environments. Each report records target triple, Rust version, database version, artifact SHA and
test cases. Unconfigured products remain NOT_VERIFIED and support count is not increased.

- [ ] **Step 3: Run benchmarks**

```bash
cargo bench -p druid-bench --bench pool -- --noplot
cargo bench -p druid-bench --bench filter_chain -- --noplot
cargo bench -p druid-bench --bench wall -- --noplot
cargo bench -p druid-bench --bench adapter -- --noplot
cargo bench -p druid-bench --bench runtime -- --noplot
```

Expected: commands complete; report p50/p95/p99 where available, throughput, allocation/RSS, cursor
first-row/full-consume and shutdown values against accepted numeric budgets.

- [ ] **Step 4: Produce requirement-by-requirement verdict**

Copy all ten completion gates from the architecture spec §10 into the report. For each gate link
the exact report/command. Verdict is `RELEASE_READY` only when every gate is PASS; otherwise
`NOT_READY` with remaining actions.

- [ ] **Step 5: Validate docs and diff**

```bash
rg -n 'ADR-CRATE-001|druid-core.*facade|当前源码仍为三' \
  README.md README.zh-CN.md docs
git diff --check
git status --short --branch
```

Expected: old target only in historical/superseded records; diff check PASS; no unintended files.

- [ ] **Step 6: Commit final evidence**

```bash
git add docs/verification/rdbc-first README.md README.zh-CN.md docs/superpowers \
  docs/druid-rust-Architecture.zh_CN.md
git diff --cached --check
git commit -m "docs(release): record rdbc-first readiness verdict"
```
