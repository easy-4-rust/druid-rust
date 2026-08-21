# RDBC Contract Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 `druid-rdbc` 抽取完成后统一错误、能力和资源状态机，并引入可背压的 async cursor，同时保留 buffered ResultSet 与 Java 兼容入口。

**Architecture:** `RdbcError` 成为 driver-facing SQL 错误，`DruidError` 聚合 Pool/Wall/Dynamic 并包装它；最小必选 `PhysicalConnection` 配合少量 capability extension；资源释放使用 cancellation-safe cleanup；查询显式区分 `BufferedRowSet` 和 `QueryCursor`。

**Tech Stack:** Rust、async-trait、Tokio、thiserror、bitflags、SQLx Stream、Loom（小状态机）

**Spec:** `docs/superpowers/specs/2026-08-21-rdbc-first-five-crate-architecture.md`

## Global Constraints

- 前置：`druid-rdbc` extraction 已完成，old/new API baseline 已记录。
- Java SQLState、vendor code、class name、next exception 和 fatal sorter 输入不得丢失。
- unsupported RDBC 能力统一 `SQLState=0A000`。
- async cursor 不得借用会在 Future 外失效的 connection reference。
- buffered ResultSet compatibility 必须保留；不能把所有查询强制改成流式。
- `async_trait` 动态派发是否优化只由 benchmark 决定。

---

### Task 1: 分层 RdbcError 与 DruidError

**Files:**
- Create: `crates/druid-rdbc/src/error/rdbc_error_kind.rs`
- Modify: `crates/druid-rdbc/src/error/rdbc_error.rs`
- Modify: `crates/druid-rdbc/src/error/sql_exception.rs`
- Modify: `crates/druid/src/core/error.rs`
- Modify: `crates/druid-wrapper/src/duckdb/duckdb_connection_adapter.rs`
- Modify: `crates/druid-wrapper/src/http_sql/http_sql_connection_adapter.rs`
- Modify: `crates/druid-wrapper/src/jdbc_agent/jdbc_agent_connection.rs`
- Modify: `crates/druid-wrapper/src/libsql/libsql_connection_adapter.rs`
- Modify: `crates/druid-wrapper/src/rbdc/rbdc_connection_adapter.rs`
- Modify: `crates/druid-wrapper/src/sqlx/sqlx_connection_adapter.rs`
- Modify: `crates/druid-wrapper/src/toasty/toasty_connection_adapter.rs`
- Test: `crates/druid-rdbc/tests/error_contract_test.rs`
- Test: `crates/druid/tests/exception_sorter_semantics_test.rs`

**Interfaces:**
- Produces:

```rust
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RdbcError {
    #[error("{exception}")]
    Sql { exception: Box<SqlException> },
    #[error("resource is closed: {resource}")]
    ResourceClosed { resource: &'static str },
    #[error("invalid argument: {message}")]
    InvalidArgument { message: String },
}

impl RdbcError {
    pub fn feature_not_supported(operation: &'static str) -> Self {
        Self::Sql {
            exception: Box::new(SqlException::feature_not_supported(operation)),
        }
    }
}
```

- [ ] **Step 1: 写统一 unsupported RED 测试**

Invoke unsupported metadata, LOB and savepoint operations from sparse test adapters and assert every
error produces `SqlExceptionKind::FeatureNotSupported` and `sql_state() == Some("0A000")`.

- [ ] **Step 2: Run RED**

```bash
cargo test -p druid-rdbc --test error_contract_test
```

Expected: FAIL because ordinary `UnsupportedOperation` and SQL feature errors differ.

- [ ] **Step 3: 实现分层转换**

`RdbcError::feature_not_supported(operation)` builds the canonical `SqlException`; `DruidError`
adds `Rdbc(#[from] RdbcError)` while preserving existing Pool/Wall/HA variants and their Display.
Adapters convert native driver failures once at their boundary.

- [ ] **Step 4: Run GREEN/fatal regression**

```bash
cargo test -p druid-rdbc --test error_contract_test
cargo test -p druid --test druid_error_test
cargo test -p druid --test exception_sorter_semantics_test
cargo test -p druid-wrapper --tests
```

Expected: all PASS; sorter behavior unchanged.

- [ ] **Step 5: Commit**

```bash
git add crates/druid-rdbc crates/druid crates/druid-wrapper
git commit -m "refactor(rdbc): unify sql error contract"
```

### Task 2: 用可证明能力替代扁平 bool 漂移

**Files:**
- Modify: `crates/druid-rdbc/src/spi/physical_connection_capabilities.rs`
- Create: `crates/druid-rdbc/src/spi/prepared_capability.rs`
- Create: `crates/druid-rdbc/src/spi/metadata_capability.rs`
- Create: `crates/druid-rdbc/src/spi/lob_capability.rs`
- Create: `crates/druid-rdbc/src/spi/cursor_capability.rs`
- Modify: `crates/druid-rdbc/src/spi/physical_connection.rs`
- Modify: adapter implementations
- Test: `crates/druid-wrapper/tests/common/physical_connection_contract.rs`

**Interfaces:**
- Produces:

```rust
bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct ConnectionCapabilities: u64 {
        const TRANSACTIONS = 1 << 0;
        const SAVEPOINTS = 1 << 1;
        const PREPARED = 1 << 2;
        const CALLABLE = 1 << 3;
        const METADATA = 1 << 4;
        const LOB_READ = 1 << 5;
        const LOB_WRITE = 1 << 6;
        const ASYNC_CURSOR = 1 << 7;
    }
}
```

- [ ] **Step 1: 写 capability truth RED test**

For every adapter, enumerate declared capability bits. For each set bit call its canonical operation
and reject `FeatureNotSupported`; for each absent bit assert the operation returns 0A000 rather than
silent success.

- [ ] **Step 2: Run RED matrix**

```bash
cargo test -p druid-wrapper --test adapter_boundary_test capability_truth -- --exact
```

Expected: FAIL for any current declaration/behavior mismatch, including external lease paths.

- [ ] **Step 3: 引入最小 extension traits**

Keep `PhysicalConnection` responsible for execution, transaction, state and lifecycle. Prepared,
metadata, LOB and cursor traits contain only their own operations. A connection exposes an optional
trait object only when the matching capability bit is set; do not create one trait per JDBC method.

- [ ] **Step 4: Run GREEN matrix**

```bash
cargo test -p druid-wrapper --test adapter_boundary_test
cargo test -p druid-wrapper --test database_profile_live_contract_test
cargo test -p druid --test physical_connection_contract
```

Expected: all configured contracts PASS; unconfigured live databases remain skipped with explicit
reason.

- [ ] **Step 5: Commit**

```bash
git add crates/druid-rdbc crates/druid crates/druid-wrapper
git commit -m "refactor(rdbc): make driver capabilities executable"
```

### Task 3: 让资源释放 cancellation-safe

**Files:**
- Modify: `crates/druid-rdbc/src/resource/rdbc_resource_state.rs`
- Modify: `crates/druid-rdbc/src/resource/rdbc_resource_context.rs`
- Modify: `crates/druid-rdbc/src/resource/rdbc_resource_owner.rs`
- Create: `crates/druid-rdbc/tests/resource_cancellation_test.rs`
- Create: `crates/druid-rdbc/tests/resource_owner_failure_test.rs`

**Interfaces:**
- Produces states: `Open → Releasing → DriverReleased → OwnerReleased → Freed`

- [ ] **Step 1: 写 cancellation RED test**

Use a test access whose `free()` waits on a Notify. Spawn `context.free()`, wait until state is
`Releasing`, abort the task, release the notify, then call free again with timeout. The second call
must complete and driver free/owner release counts must each equal 1.

- [ ] **Step 2: 写 owner failure RED test**

Driver free succeeds, owner release returns an error once, then succeeds. Assert state is not exposed
as fully `Freed` until owner cleanup succeeds and all concurrent waiters observe the same final
result.

- [ ] **Step 3: Run RED**

```bash
cargo test -p druid-rdbc --test resource_cancellation_test
cargo test -p druid-rdbc --test resource_owner_failure_test
```

Expected: current design times out or returns inconsistent results.

- [ ] **Step 4: 实现受监管 cleanup**

The successful CAS owner installs a shared cleanup result and starts a cleanup future whose ownership
is not tied to the caller waiting future. All callers await the same completion. Driver failure may
return to Open only before driver release; owner failure remains `DriverReleased` and is retryable.

- [ ] **Step 5: Run GREEN and Loom model**

```bash
cargo test -p druid-rdbc --test resource_cancellation_test
cargo test -p druid-rdbc --test resource_owner_failure_test
cargo test -p druid-rdbc rdbc_resource_context
```

Expected: all PASS, counts exactly once.

- [ ] **Step 6: Commit**

```bash
git add crates/druid-rdbc
git commit -m "fix(rdbc): make resource release cancellation safe"
```

### Task 4: 引入 async QueryCursor 与显式 BufferedRowSet

**Files:**
- Create: `crates/druid-rdbc/src/spi/query_cursor.rs`
- Split: `crates/druid-rdbc/src/spi/rdbc_result_set.rs`
- Create: `crates/druid-rdbc/src/spi/physical_result_set.rs`
- Create: `crates/druid-rdbc/src/spi/buffered_row_set.rs`
- Modify: `crates/druid-rdbc/src/spi/physical_connection.rs`
- Modify: `crates/druid/src/core/druid_pooled_result_set.rs`
- Modify: `crates/druid-wrapper/src/sqlx/sqlx_connection_adapter.rs`
- Rename: `crates/druid-wrapper/src/sqlx/sqlx_streaming_result_set.rs` → `crates/druid-wrapper/src/sqlx/sqlx_buffered_row_set.rs`
- Test: `crates/druid-wrapper/tests/sqlx_cursor_test.rs`
- Test: `crates/druid/tests/druid_pooled_result_set_semantics_test.rs`
- Test: `crates/druid/tests/result_set_filter_semantics_test.rs`
- Test: `crates/druid-wrapper/tests/prepared_statement_semantics_test.rs`

**Interfaces:**
- Produces:

```rust
#[async_trait::async_trait]
pub trait QueryCursor: Send {
    fn metadata(&self) -> &ResultSetMetaData;
    async fn next(&mut self) -> Result<Option<Row>, RdbcError>;
    async fn close(&mut self) -> Result<(), RdbcError>;
}

pub enum QueryResult {
    Buffered(BufferedRowSet),
    Cursor(Box<dyn QueryCursor>),
}
```

- [ ] **Step 1: 写 bounded-memory RED test**

Create 10,000 SQLite rows, request cursor mode with fetch size 64, consume the first 65 rows, cancel,
and assert the adapter did not call `fetch_all` or materialize 10,000 rows. Use an injected SQLx
cursor probe/counter rather than RSS-only inference.

- [ ] **Step 2: Run RED**

```bash
cargo test -p druid-wrapper --test sqlx_cursor_test -- --nocapture
```

Expected: FAIL because current result set uses `fetch_all` and synchronous `next()`.

- [ ] **Step 3: 实现 cursor ownership**

The cursor must own the driver cursor/connection lease or communicate with a per-connection actor;
it must not borrow `&mut PhysicalConnection` past the method return. `fetch()` remains an explicit
collect convenience that produces `BufferedRowSet`.

- [ ] **Step 4: 保持 Java ResultSet facade**

`DruidPooledResultSet` wraps either variant. Existing synchronous buffered methods remain; cursor
methods use async entrypoints. Do not call a buffered type “streaming”.

- [ ] **Step 5: Run GREEN compatibility and cursor tests**

```bash
cargo test -p druid-wrapper --test sqlx_cursor_test
cargo test -p druid --test druid_pooled_result_set_semantics_test
cargo test -p druid --test result_set_filter_semantics_test
cargo test -p druid-wrapper --test prepared_statement_semantics_test
```

Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/druid-rdbc crates/druid crates/druid-wrapper
git commit -m "feat(rdbc): add async query cursor contract"
```

### Task 5: 完成合同文档和性能门禁

**Files:**
- Create: `docs/rdbc/contract-v1.md`
- Create: `docs/verification/rdbc-first/rdbc-contract-hardening.md`
- Modify: `crates/druid-bench/benches/adapter.rs`
- Modify: `crates/druid-bench/benches/pool.rs`

**Interfaces:**
- Produces: API/error/capability/resource/cursor normative contract and benchmark baseline

- [ ] **Step 1: 增加 measured dispatch/cursor benchmark**

Benchmark direct SQLite exec through dyn SPI, prepared exec, buffered 10k rows, cursor first-row and
cursor full-consume. Record median/p95 allocation and elapsed baselines; do not set optimization
targets without measured current values.

- [ ] **Step 2: Run full gates**

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo test --workspace
cargo bench -p druid-bench --bench adapter -- --noplot
```

Expected: format/check/test PASS; benchmark output captured in verification report.

- [ ] **Step 3: Commit**

```bash
git add docs/rdbc docs/verification/rdbc-first crates/druid-bench
git commit -m "docs(rdbc): publish v1 internal contract and baselines"
```
