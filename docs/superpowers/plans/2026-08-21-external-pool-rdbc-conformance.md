# External Pool RDBC Conformance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 direct SQLx、bb8 lease 和 deadpool lease 对统一 `DruidPooledConnection` 暴露等价的 PreparedStatement、Metadata、LOB、事务和资源关闭语义。

**Architecture:** 先建立可复用的 adapter conformance harness，再补齐 `PhysicalConnectionLease` 对所有非等价默认方法的透明委托。External pool 只改变 lease owner，不改变底层 driver capability 或错误类型。

**Tech Stack:** Rust、async-trait、SQLx SQLite、bb8、deadpool、Tokio tests

**Spec:** `docs/superpowers/specs/2026-08-21-rdbc-first-five-crate-architecture.md`

## Global Constraints

- 不允许把 external lease 放入 `DruidPool`，禁止 pool-in-pool。
- 每个 logical lease 只能归还一次；显式 close、Drop、错误淘汰必须分别测试。
- capability、成功值、错误类型和资源关闭行为必须与 direct SQLx 等价。
- 测试必须使用真实 SQLite，不以 mock-only 测试替代。
- 不改变 `druid::sql::*` 公共路径。

---

### Task 1: 建立统一连接合同 Harness

**Files:**
- Create: `crates/druid-wrapper/tests/common/mod.rs`
- Create: `crates/druid-wrapper/tests/common/physical_connection_contract.rs`
- Modify: `crates/druid-wrapper/tests/sqlx_connection_adapter_test.rs`
- Modify: `crates/druid-wrapper/tests/sqlx_bb8_pool_test.rs`
- Modify: `crates/druid-wrapper/tests/sqlx_deadpool_pool_test.rs`

**Interfaces:**
- Produces:

```rust
pub async fn assert_rdbc_connection_contract<F, Fut>(acquire: F)
    -> Result<(), DruidError>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<DruidPooledConnection, DruidError>>;
```

- [ ] **Step 1: 写 PreparedStatement/Metadata RED 合同**

Create a harness that executes exactly:

```rust
let mut connection = acquire().await?;
connection.exec("create table item(id integer primary key, name text)", vec![]).await?;
let mut prepared = connection.prepare_statement("insert into item(name) values (?)").await?;
let result = prepared
    .exec(&mut connection, vec![Value::String("alpha".to_owned())])
    .await?;
assert_eq!(result.rows_affected, 1);

let mut metadata = connection.get_meta_data()?;
assert_eq!(
    metadata.get_database_product_name()?.as_deref(),
    Some("SQLite")
);
connection.close().await?;
Ok(())
```

The harness must also assert a second close is idempotent and that the pool can reacquire a
usable connection.

- [ ] **Step 2: Run the three contracts to verify RED**

```bash
cargo test -p druid-wrapper --test sqlx_connection_adapter_test \
  rdbc_connection_contract -- --exact
cargo test -p druid-wrapper --test sqlx_bb8_pool_test \
  rdbc_connection_contract -- --exact
cargo test -p druid-wrapper --test sqlx_deadpool_pool_test \
  rdbc_connection_contract -- --exact
```

Expected: direct SQLx PASS；bb8/deadpool FAIL at `prepare_physical_statement` or metadata
because `PhysicalConnectionLease` falls back to Unsupported.

- [ ] **Step 3: 提取公共 harness，不复制测试逻辑**

Expose the harness from `tests/common/mod.rs` and call it from all three integration files with
their actual pool construction. Preserve each pool's existing lifecycle tests.

- [ ] **Step 4: Commit RED contract**

```bash
git add crates/druid-wrapper/tests
git commit -m "test(wrapper): define external pool rdbc conformance"
```

### Task 2: 补齐 PhysicalConnectionLease 透明委托

**Files:**
- Modify: `crates/druid/src/core/physical_connection_lease.rs`
- Test: `crates/druid/tests/physical_connection_lease_test.rs`
- Test: `crates/druid-wrapper/tests/common/physical_connection_contract.rs`

**Interfaces:**
- Consumes: every public method of `PhysicalConnection`
- Produces: transparent delegation to `self.physical_connection_mut()` or
  `self.physical_connection()`

- [ ] **Step 1: 写低层委托 RED 测试**

Extend `ContractConnection` with counters and assert delegation for:

```rust
create_physical_statement
prepare_physical_statement
prepare_physical_call
exec_prepared
fetch_prepared
database_meta_data
create_blob
create_clob
create_n_clob
```

Expected counts are exactly `1` per invoked operation; returned Arc/resource identity must match
the underlying object.

- [ ] **Step 2: Run RED test**

```bash
cargo test -p druid --test physical_connection_lease_test \
  lease_delegates_complete_spi -- --exact
```

Expected: FAIL for every currently missing override.

- [ ] **Step 3: 实现完整委托**

Add explicit overrides following this exact pattern:

```rust
async fn prepare_physical_statement(
    &mut self,
    key: &PreparedStatementKey,
) -> Result<Arc<dyn PhysicalPreparedStatement>, DruidError> {
    self.physical_connection_mut()
        .prepare_physical_statement(key)
        .await
}
```

Read-only methods delegate through `physical_connection()`; mutable/async methods delegate through
`physical_connection_mut()`. Do not emulate an operation with a weaker default implementation.

- [ ] **Step 4: Add an SPI surface guard**

Add `scripts/verify_physical_connection_lease_surface.py` that parses the method names in
`physical_connection.rs` and asserts that methods with non-equivalent defaults appear in the lease
impl. The required set is stored explicitly in the script and includes the methods listed above.

Run:

```bash
python3 scripts/verify_physical_connection_lease_surface.py
```

Expected: `PASS: PhysicalConnectionLease delegates all required methods`.

- [ ] **Step 5: Run GREEN tests**

```bash
cargo test -p druid --test physical_connection_lease_test
cargo test -p druid-wrapper --test sqlx_connection_adapter_test
cargo test -p druid-wrapper --test sqlx_bb8_pool_test
cargo test -p druid-wrapper --test sqlx_deadpool_pool_test
```

Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/druid/src/core/physical_connection_lease.rs \
  crates/druid/tests/physical_connection_lease_test.rs \
  crates/druid-wrapper/tests scripts/verify_physical_connection_lease_surface.py
git commit -m "fix(wrapper): preserve rdbc capabilities through external leases"
```

### Task 3: 增加资源与错误等价门禁

**Files:**
- Modify: `crates/druid-wrapper/tests/common/physical_connection_contract.rs`
- Modify: `crates/druid-wrapper/tests/sqlite_wrapper_semantics_test.rs`
- Create: `docs/verification/rdbc-first/external-pool-conformance.md`

**Interfaces:**
- Produces: per-adapter capability/result/error/release matrix

- [ ] **Step 1: 添加错误合同**

For each acquisition mode, execute invalid SQL and assert:

```rust
let error = connection
    .exec("select * from definitely_missing_table", vec![])
    .await
    .expect_err("invalid SQL must fail");
assert!(error.sql_exception().is_some());
```

Then close and reacquire; assert fatal classification/discard behavior matches the direct adapter.

- [ ] **Step 2: 添加 dirty transaction 合同**

Begin, insert without commit, explicitly close, reacquire, and assert the row is absent. Repeat via
Drop and assert the dirty lease is discarded/replaced rather than returned unreset.

- [ ] **Step 3: Run regression**

```bash
cargo test -p druid-wrapper --tests
cargo test -p druid --test recycle_semantics_test
cargo test -p druid --test prepared_statement_cache_test
```

Expected: all PASS, no nested pool, exactly-once return counters unchanged.

- [ ] **Step 4: 写验证报告**

Record command, exit code, tested backend, direct/bb8/deadpool result, and unverified non-SQLite
boundaries in `external-pool-conformance.md`.

- [ ] **Step 5: Commit**

```bash
git add crates/druid-wrapper/tests docs/verification/rdbc-first/external-pool-conformance.md
git commit -m "test(wrapper): certify external pool rdbc parity"
```
