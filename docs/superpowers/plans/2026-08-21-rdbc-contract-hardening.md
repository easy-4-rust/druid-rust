# RDBC Contract Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在 `druid-rdbc` 抽取完成后统一错误、能力和资源状态机，并引入可背压的 async cursor，同时保留 buffered ResultSet 与 Java 兼容入口。

**Architecture:** Task 1 首先强制退出 P03 的 `compat::DruidError` waiver：`RdbcError` 成为完整 driver-facing SQL 错误，`DruidError` 的实际实现回到 `druid` 并聚合 Pool/Wall/HA/Dynamic 与显式 RDBC wrapper；后续才进入 capability、cancellation-safe resource cleanup 和 `BufferedRowSet`/`QueryCursor` 分型。P04 不得在完整 mixed enum 仍由 `druid-rdbc` 拥有时宣称完成。

**Tech Stack:** Rust、async-trait、Tokio、thiserror、bitflags、SQLx Stream、Loom（小状态机）

**Spec:** `docs/superpowers/specs/2026-08-21-rdbc-first-five-crate-architecture.md`

## Global Constraints

- 前置：`druid-rdbc` extraction 已完成，old/new API baseline 已记录。
- Java SQLState、vendor code、class name、next exception 和 fatal sorter 输入不得丢失。
- unsupported RDBC 能力统一 `SQLState=0A000`。
- async cursor 不得借用会在 Future 外失效的 connection reference。
- buffered ResultSet compatibility 必须保留；不能把所有查询强制改成流式。
- `async_trait` 动态派发是否优化只由 benchmark 决定。
- P03 的 `druid_rdbc::compat::DruidError` 仅为 `TRANSITIONAL_UNTIL_P04`；Task 1 是所有后续 P04 tasks 的强制前置且必须满足六项 waiver-exit gates。
- Task 1 会产生有意的 public API 变化：旧 `DruidError` module-qualified 类型路径保留，但 driver-facing variants 迁移到 `RdbcError`，必须用 public API diff 和迁移文档明确记录。

---

### Task 1: 强制退出 transitional waiver，分层 `RdbcError` 与 `DruidError`

**Files — error ownership and migration evidence:**
- Create: `crates/druid-rdbc/src/error/rdbc_error_kind.rs`
- Modify: `crates/druid-rdbc/src/error/rdbc_error.rs`
- Modify: `crates/druid-rdbc/src/error/sql_exception.rs`
- Modify: `crates/druid-rdbc/src/error/mod.rs`
- Delete: `crates/druid-rdbc/src/compat/druid_error.rs`
- Modify: `crates/druid-rdbc/src/compat/mod.rs`
- Modify: `crates/druid/src/core/error.rs`
- Modify: `crates/druid/src/core/mod.rs`
- Create: `api/druid-error-transitional.before.txt`
- Create: `api/druid-error-p04.after.txt`
- Create: `docs/rdbc/error-migration.md`
- Create: `docs/verification/rdbc-first/rdbc-error-waiver-exit.md`
- Create: `docs/verification/rdbc-first/rdbc-driver-error-constructions.before.txt`

**Files — every moved RDBC signature that currently exposes transitional `DruidError`:**
- Modify: `crates/druid-rdbc/src/types/rdbc_string.rs`
- Modify: `crates/druid-rdbc/src/types/savepoint.rs`
- Modify: `crates/druid-rdbc/src/types/date.rs`
- Modify: `crates/druid-rdbc/src/types/time.rs`
- Modify: `crates/druid-rdbc/src/types/timestamp.rs`
- Modify: `crates/druid-rdbc/src/types/rdbc_calendar.rs`
- Modify: `crates/druid-rdbc/src/spi/callable_parameter.rs`
- Modify: `crates/druid-rdbc/src/spi/physical_callable_statement.rs`
- Modify: `crates/druid-rdbc/src/spi/physical_connection.rs`
- Modify: `crates/druid-rdbc/src/spi/physical_connection_factory.rs`
- Modify: `crates/druid-rdbc/src/spi/physical_database_meta_data.rs`
- Modify: `crates/druid-rdbc/src/spi/physical_prepared_statement.rs`
- Modify: `crates/druid-rdbc/src/spi/physical_result_set_meta_data.rs`
- Modify: `crates/druid-rdbc/src/spi/physical_statement.rs`
- Modify: `crates/druid-rdbc/src/spi/prepared_input_parameter.rs`
- Modify: `crates/druid-rdbc/src/spi/prepared_statement_key.rs`
- Modify: `crates/druid-rdbc/src/spi/rdbc_result_set.rs`
- Modify: `crates/druid-rdbc/src/spi/result_set_meta_data.rs`
- Modify: `crates/druid-rdbc/src/resource/rdbc_input_stream.rs`
- Modify: `crates/druid-rdbc/src/resource/rdbc_output_stream.rs`
- Modify: `crates/druid-rdbc/src/resource/rdbc_reader.rs`
- Modify: `crates/druid-rdbc/src/resource/rdbc_writer.rs`
- Modify: `crates/druid-rdbc/src/resource/rdbc_array_access.rs`
- Modify: `crates/druid-rdbc/src/resource/rdbc_blob_access.rs`
- Modify: `crates/druid-rdbc/src/resource/rdbc_clob_access.rs`
- Modify: `crates/druid-rdbc/src/resource/rdbc_n_clob_access.rs`
- Modify: `crates/druid-rdbc/src/resource/rdbc_ref_access.rs`
- Modify: `crates/druid-rdbc/src/resource/rdbc_resource_access.rs`
- Modify: `crates/druid-rdbc/src/resource/rdbc_resource_context.rs`
- Modify: `crates/druid-rdbc/src/resource/rdbc_resource_factory.rs`
- Modify: `crates/druid-rdbc/src/resource/rdbc_resource_owner.rs`
- Modify: `crates/druid-rdbc/src/resource/rdbc_sql_xml_access.rs`
- Modify: `crates/druid-rdbc/src/resource/array.rs`
- Modify: `crates/druid-rdbc/src/resource/blob.rs`
- Modify: `crates/druid-rdbc/src/resource/clob.rs`
- Modify: `crates/druid-rdbc/src/resource/n_clob.rs`
- Modify: `crates/druid-rdbc/src/resource/ref_value.rs`
- Modify: `crates/druid-rdbc/src/resource/sql_xml.rs`

**Files — direct adapter signatures and one-time native conversions:**
- Modify: `crates/druid-wrapper/src/prepared_parameter_materializer.rs`
- Modify: `crates/druid-wrapper/src/prepared_parameter_state.rs`
- Modify: `crates/druid-wrapper/src/duckdb/duckdb_connection_adapter.rs`
- Modify: `crates/druid-wrapper/src/duckdb/duckdb_connection_factory.rs`
- Modify: `crates/druid-wrapper/src/duckdb/duckdb_database_meta_data.rs`
- Modify: `crates/druid-wrapper/src/duckdb/duckdb_prepared_statement.rs`
- Modify: `crates/druid-wrapper/src/http_sql/http_sql_connection_adapter.rs`
- Modify: `crates/druid-wrapper/src/http_sql/http_sql_connection_factory.rs`
- Modify: `crates/druid-wrapper/src/http_sql/http_sql_database_meta_data.rs`
- Modify: `crates/druid-wrapper/src/http_sql/http_sql_prepared_statement.rs`
- Modify: `crates/druid-wrapper/src/jdbc_agent/agent_error.rs`
- Modify: `crates/druid-wrapper/src/jdbc_agent/agent_rpc_error.rs`
- Modify: `crates/druid-wrapper/src/jdbc_agent/agent_runtime.rs`
- Modify: `crates/druid-wrapper/src/jdbc_agent/agent_runtime_manager.rs`
- Modify: `crates/druid-wrapper/src/jdbc_agent/agent_value.rs`
- Modify: `crates/druid-wrapper/src/jdbc_agent/jdbc_agent_client.rs`
- Modify: `crates/druid-wrapper/src/jdbc_agent/jdbc_agent_connection.rs`
- Modify: `crates/druid-wrapper/src/jdbc_agent/jdbc_agent_connection_factory.rs`
- Modify: `crates/druid-wrapper/src/jdbc_agent/jdbc_agent_database_meta_data.rs`
- Modify: `crates/druid-wrapper/src/jdbc_agent/jdbc_agent_prepared_statement.rs`
- Modify: `crates/druid-wrapper/src/libsql/libsql_connection_adapter.rs`
- Modify: `crates/druid-wrapper/src/libsql/libsql_connection_factory.rs`
- Modify: `crates/druid-wrapper/src/libsql/libsql_database_meta_data.rs`
- Modify: `crates/druid-wrapper/src/libsql/libsql_prepared_statement.rs`
- Modify: `crates/druid-wrapper/src/rbdc/rbdc_connection_adapter.rs`
- Modify: `crates/druid-wrapper/src/rbdc/rbdc_connection_factory.rs`
- Modify: `crates/druid-wrapper/src/rbdc/rbdc_database_meta_data.rs`
- Modify: `crates/druid-wrapper/src/rbdc/rbdc_prepared_statement.rs`
- Modify: `crates/druid-wrapper/src/sqlx/sqlx_connection_adapter.rs`
- Modify: `crates/druid-wrapper/src/sqlx/sqlx_connection_factory.rs`
- Modify: `crates/druid-wrapper/src/sqlx/sqlx_database_meta_data.rs`
- Modify: `crates/druid-wrapper/src/sqlx/sqlx_prepared_statement.rs`
- Modify: `crates/druid-wrapper/src/sqlx/sqlx_streaming_result_set.rs`
- Modify: `crates/druid-wrapper/src/sqlx/bb8/sqlx_bb8_connection_manager.rs`
- Modify: `crates/druid-wrapper/src/sqlx/bb8/sqlx_bb8_pool.rs`
- Modify: `crates/druid-wrapper/src/sqlx/deadpool/sqlx_deadpool_connection_manager.rs`
- Modify: `crates/druid-wrapper/src/sqlx/deadpool/sqlx_deadpool_pool.rs`
- Modify: `crates/druid-wrapper/src/toasty/toasty_connection_adapter.rs`
- Modify: `crates/druid-wrapper/src/toasty/toasty_connection_factory.rs`
- Modify: `crates/druid-wrapper/src/toasty/toasty_database_meta_data.rs`
- Modify: `crates/druid-wrapper/src/toasty/toasty_prepared_statement.rs`

**Files — Druid product boundaries that currently construct/propagate driver-facing variants:**
- Modify: `crates/druid-wrapper/src/dbcp/basic_data_source_factory.rs`
- Modify: `crates/druid-wrapper/src/dbcp/managed_basic_data_source_factory.rs`
- Modify: `crates/druid-wrapper/src/dbcp2/basic_data_source_factory.rs`
- Modify: `crates/druid-wrapper/src/driver/extensions.rs`
- Modify: `crates/druid-wrapper/src/managed_wrapper_pool.rs`
- Modify: `crates/druid-wrapper/src/proxool/proxool_data_source.rs`
- Modify: `crates/druid-wrapper/src/rdbc/druid_rdbc_data_source.rs`
- Modify: `crates/druid-wrapper/src/rdbc/dynamic_rdbc_data_source.rs`
- Modify: `crates/druid-wrapper/src/wrapper_data_source_factory.rs`

**Tests:**
- Create: `crates/druid-rdbc/tests/error_contract_test.rs`
- Create: `crates/druid/tests/druid_error_transition_test.rs`
- Modify: `crates/druid/tests/druid_error_test.rs`
- Modify: `crates/druid/tests/exception_sorter_semantics_test.rs`
- Modify: `crates/druid-wrapper/tests/adapter_boundary_test.rs`
- Modify: `crates/druid-wrapper/tests/prepared_statement_semantics_test.rs`

**Interfaces:**
- Consumes: P03 `TRANSITIONAL_UNTIL_P04` 24-variant exact identity baseline
- Produces: the following concrete, non-exhaustive driver-facing starting contract. A Rust API
  review may rename a variant only if it updates compile fixtures/migration docs in the same task
  and preserves every field and behavior in the mandatory table:

```rust
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RdbcError {
    #[error("{exception}")]
    Sql { exception: Box<SqlException> },
    #[error("resource is closed: {resource}")]
    ResourceClosed {
        resource: &'static str,
        exception: Box<SqlException>,
    },
    #[error("invalid argument: {message}")]
    InvalidArgument { message: String },
    #[error("driver error: {message}")]
    Driver { message: String },
    #[error("connection has been discarded")]
    ConnectionDiscarded,
    #[error("batch update failed after {} result(s): {cause}", .update_counts.len())]
    BatchUpdate {
        update_counts: Vec<i32>,
        #[source]
        cause: Box<RdbcError>,
    },
    #[error("connection validation failed: {message}")]
    ValidationFailed { message: String },
    #[error("no driver extension registered for database type: {db_type}")]
    NoDriverExtension { db_type: String },
    #[error("{message}")]
    Other { message: String },
}
```

`RdbcError::feature_not_supported(operation)` produces `Sql` with the canonical 0A000 exception;
unsupported is a required taxonomy category but not a second competing representation.

| Contract category | Mandatory retained data/behavior |
| :--- | :--- |
| SQL | complete `SqlException`: message, SQLState, vendor code, class/assignable types, causes and next-exception chain |
| Resource closed | resource identity and current resource-closed SQL/class behavior |
| Invalid argument | complete message and current Display behavior |
| Driver | native-driver diagnostic message and any currently exposed source |
| Connection discarded | discard/fatal-classification input; never collapse into an unclassified string |
| Batch update | ordered update counts, large/int views where exposed, and typed cause/source |
| Validation failed | validation diagnostic message |
| Other | complete uncategorized driver diagnostic message |
| Unsupported | canonical `SqlExceptionKind::FeatureNotSupported`, SQLState `0A000`, operation name |
| No driver extension | `db_type` when registry absence remains below the Druid product boundary |

- Produces: actual `druid::core::error::DruidError` implementation in `druid`, retaining Pool,
  Wall, HA and Dynamic/product variants and adding `Rdbc(#[from] RdbcError)`. Old
  `druid::core::error::DruidError` and
  `druid::core::DruidError` paths remain; driver-facing variant construction intentionally migrates
  to `RdbcError`.
- Produces: every moved SPI/Access/platform fallible method returns `RdbcError`; product façades
  convert once into `DruidError`.

- [ ] **Step 1: Freeze all transitional constructors and the 24-variant identity before changing code**

Capture `cargo public-api -p druid -sss --all-features` as
`api/druid-error-transitional.before.txt`. Add a test fixture that constructs all 24 transitional
variants, proves bidirectional type identity between `druid::core::error::DruidError` and
`druid_rdbc::compat::DruidError`, and records, for each value, `Display`, `class_name`,
`sql_exception`, SQLState, vendor code, exception class/assignable types, next-exception chain,
`batch_update_counts`, and `Error::source` presence/value. Persist the expected matrix before
removing old constructors; after the cutover, update the test inputs to the documented
`DruidError`/`RdbcError` replacements while continuing to compare all 24 baseline rows. Separately
inventory every driver-facing construction:

```bash
rg -n 'DruidError::(SqlException|ValidationFailed|ConnectionDiscarded|DriverError|BatchUpdateException|InvalidArgument|UnsupportedOperation|NoDriverExtension|Other)' \
  crates/druid-rdbc crates/druid crates/druid-wrapper \
  > docs/verification/rdbc-first/rdbc-driver-error-constructions.before.txt
```

The inventory is a denominator: every line must map to an `RdbcError` construction or an explicit
Druid product-boundary conversion before this task exits.

- [ ] **Step 2: Write taxonomy/unsupported/batch/fatal RED tests**

Invoke unsupported metadata, LOB and savepoint operations and require
`SqlExceptionKind::FeatureNotSupported` plus `sql_state() == Some("0A000")`. Construct every
driver-facing category in the table, including chained SQL exceptions, non-zero vendor code,
class/assignable types, batch counts with a typed cause, validation/other strings, discarded/fatal
input, and optional no-driver-extension `db_type`. Compare against the Step 1 transitional fixtures.

```bash
cargo test -p druid-rdbc --test error_contract_test
cargo test -p druid --test druid_error_transition_test
cargo test -p druid --test exception_sorter_semantics_test
```

Expected: RED because the scaffold `RdbcError` lacks the complete taxonomy, unsupported paths are
not uniform, and the full mixed enum still lives under `druid-rdbc::compat`.

- [ ] **Step 3: Implement a behavior-complete `RdbcError`**

Use `#[non_exhaustive]` for `RdbcError` and implement the starting contract above. The table is a
hard gate: do not merge `SqlException`, discarded/fatal input, batch counts/cause, or no-driver
`db_type` into a lossy string. `feature_not_supported(operation)` must always expose the canonical
0A000 `SqlException`; `Display` and `Error::source` must be deterministic and covered by the
differential fixture.

- [ ] **Step 4: Switch every moved signature and restore Druid ownership atomically**

Change every fallible file listed under “every moved RDBC signature” from transitional
`DruidError` to `RdbcError`. Delete `compat/druid_error.rs` and its module export. Restore the actual
`DruidError` enum/`Display`/`Error` implementation in `druid/src/core/error.rs`: retain all Pool,
Wall, HA, Dynamic/data-source lifecycle, parser, timeout/leak and fatal-state behavior; replace
driver-owned constructions with the explicit RDBC wrapper/conversion. Preserve nested Druid causes
where they are product failures and preserve the existing local `XaStateTransitionError`
conversion.

This is an intentional public enum-construction change. Do not leave a complete mixed enum, type
alias, copied compatibility enum, or blanket conversion in `druid-rdbc::compat`.

- [ ] **Step 5: Convert native errors once at adapter boundaries**

Update every listed direct adapter/factory/metadata/prepared/result-set/helper implementation to
return `RdbcError` for moved traits. Preserve native diagnostics and structured SQL data at the
first adapter boundary. Druid logical pool/data-source code may convert `RdbcError` once through the
explicit wrapper; it must not make adapter code depend on Druid product variants again.

- [ ] **Step 6: Run GREEN differential, fatal sorter and public API gates**

```bash
cargo test -p druid-rdbc --test error_contract_test
cargo test -p druid --test druid_error_transition_test
cargo test -p druid --test druid_error_test
cargo test -p druid --test exception_sorter_semantics_test
cargo test -p druid-wrapper --test adapter_boundary_test
cargo test -p druid-wrapper --test prepared_statement_semantics_test
cargo test -p druid-wrapper --tests
cargo public-api -p druid -sss --all-features > api/druid-error-p04.after.txt
```

Expected: all PASS. Differential evidence covers all driver-facing baseline constructions, the full
24-variant transitional denominator, fatal sorter inputs/results, `Display`/`Error::source`,
SQLState, vendor code, exception class/assignable types, next-exception chains, and batch counts plus
cause. The public API diff contains only documented intentional error-contract changes.

- [ ] **Step 7: Prove all six waiver-exit gates and write migration docs**

`docs/rdbc/error-migration.md` gives old/new constructors, pattern-match changes, conversion
boundaries and examples for every intentionally changed public variant. The verification report
must record PASS evidence for each gate separately:

1. no full mixed `DruidError` definition remains in `druid-rdbc::compat` and
   `crates/druid-rdbc/src/compat/druid_error.rs` is deleted;
2. all driver-facing SPI/Access methods return `RdbcError`;
3. Pool/Wall/HA/Dynamic/product variants are implemented only in `druid`;
4. unsupported operations uniformly produce SQLState `0A000`;
5. batch counts, connection-discard/fatal input, SQL exception chains, `Display`, and
   `Error::source` have passing differential tests;
6. migration docs and the before/after public API diff explicitly record every intentional change.

Any failed gate blocks Task 2 and P04 completion; the full mixed enum is not an allowed residual
waiver.

- [ ] **Step 8: Commit**

```bash
git add crates/druid-rdbc crates/druid crates/druid-wrapper api docs
git diff --cached --check
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
- Consumes: Task 1 waiver-exit state; all capability operations and unsupported branches return the
  final `RdbcError`, never transitional `compat::DruidError`
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
- Consumes: Task 1 final `RdbcError`; driver-free and owner-release failures remain typed
  `RdbcError` values through the shared cleanup result
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
- Consumes: Task 1 final `RdbcError`; cursor/buffered operations do not reference the deleted
  transitional compat error
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
- Consumes: Task 1 waiver-exit evidence and final `RdbcError`/`DruidError::Rdbc` contract
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
