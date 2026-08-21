# druid-rdbc No-Behavior Extraction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 从当前 `druid` 中无行为抽取独立 `druid-rdbc` crate，集中平台类型、结构化 SQL 错误、Physical driver SPI 和资源 Access SPI，同时保持 `druid::sql::*` 与现有 adapter 行为。

**Architecture:** 先移动无状态平台类型和错误，再移动 driver/resource SPI，最后把 `druid` 旧路径改为薄重导出。`druid` 继续拥有 pooled logical handles、Pool、Filter、Parser、Wall、Stat、Dynamic；本计划不引入 async cursor 或新错误行为。

**Tech Stack:** Cargo workspace、Rust 2021、cargo-public-api、cargo-semver-checks、CodeGraph

**Spec:** `docs/superpowers/specs/2026-08-21-rdbc-first-five-crate-architecture.md`

## Global Constraints

- 前置：External Pool RDBC Conformance 计划必须完成。
- 机械抽取阶段禁止修改方法语义、错误分类、默认值或线程模型。
- 每个 Java 对象继续独占一个 `.rs` 文件；禁止把对象合并进 `lib.rs`/`compat.rs`。
- `druid-rdbc` 不得依赖 `druid` 或任何具体数据库/池/HTTP/gRPC crate。
- 旧公共路径只做类型重导出，不复制实现。
- `druid::sql::*` external consumer 必须在每个 task 后通过。

---

### Task 1: 建立 crate 和依赖边界 RED 门禁

**Files:**
- Create: `crates/druid-rdbc/Cargo.toml`
- Create: `crates/druid-rdbc/src/lib.rs`
- Create: `scripts/verify_rdbc_dependency_boundary.py`
- Create: `scripts/verify_rdbc_public_paths.py`
- Create: `crates/druid/tests/rdbc_external_consumer_test.rs`
- Modify: root `Cargo.toml`

**Interfaces:**
- Produces: package `druid-rdbc`, Rust crate `druid_rdbc`, dependency/API boundary scripts

- [ ] **Step 1: 写目标 topology RED 脚本**

`verify_rdbc_dependency_boundary.py` must read `cargo metadata --format-version 1`, locate package
`druid-rdbc`, recursively traverse normal dependencies, and reject:

```python
FORBIDDEN = {
    "druid", "druid-wrapper", "druid-metrics", "druid-admin",
    "sqlx", "toasty", "toasty-core", "rbdc", "bb8", "deadpool",
    "axum", "tonic", "reqwest", "kube", "prometheus",
}
```

It exits 1 when the package is absent or any forbidden package is reachable.

- [ ] **Step 2: Run RED boundary**

```bash
python3 scripts/verify_rdbc_dependency_boundary.py
```

Expected: FAIL with `package druid-rdbc not found`.

- [ ] **Step 3: 创建最小 package**

Use:

```toml
[package]
name = "druid-rdbc"
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true
publish = false

[dependencies]
async-trait.workspace = true
bitflags.workspace = true
bigdecimal.workspace = true
chrono.workspace = true
jiff.workspace = true
num-bigint.workspace = true
parking_lot.workspace = true
thiserror.workspace = true
tokio = { workspace = true, features = ["sync"] }
url.workspace = true

[lints]
workspace = true
```

`src/lib.rs` initially declares empty `types`, `error`, `spi`, `resource`, and `compat` modules;
these modules contain only declarations and re-exports.

- [ ] **Step 4: 写 external consumer compile fixture**

The integration test creates a temporary Cargo package depending on `druid` by path and compiles:

```rust
use druid::sql::{Connection, DataSource, Date, ResultSet, SQLException, Statement};

fn accepts_connection(_: Option<Connection>) {}
fn accepts_result_set(_: Option<ResultSet>) {}

fn main() {
    accepts_connection(None);
    accepts_result_set(None);
    let _ = std::any::TypeId::of::<Date>();
}
```

- [ ] **Step 5: Run GREEN scaffold checks**

```bash
python3 scripts/verify_rdbc_dependency_boundary.py
cargo test -p druid --test rdbc_external_consumer_test
cargo check -p druid-rdbc --all-targets
```

Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock crates/druid-rdbc scripts \
  crates/druid/tests/rdbc_external_consumer_test.rs
git commit -m "build(rdbc): establish isolated contract crate"
```

### Task 2: 移动平台值类型和 SQL 类型

**Files:**
- Move: `crates/druid/src/core/value.rs` → `crates/druid-rdbc/src/types/value.rs`
- Move: `crates/druid/src/core/row.rs` → `crates/druid-rdbc/src/types/row.rs`
- Move: `crates/druid/src/core/savepoint.rs` → `crates/druid-rdbc/src/types/savepoint.rs`
- Move: `crates/druid/src/core/rdbc_string.rs` → `crates/druid-rdbc/src/types/rdbc_string.rs`
- Move: `crates/druid/src/core/rdbc_url.rs` → `crates/druid-rdbc/src/types/rdbc_url.rs`
- Move: `crates/druid/src/sql/date.rs` → `crates/druid-rdbc/src/types/date.rs`
- Move: `crates/druid/src/sql/time.rs` → `crates/druid-rdbc/src/types/time.rs`
- Move: `crates/druid/src/sql/timestamp.rs` → `crates/druid-rdbc/src/types/timestamp.rs`
- Move: `crates/druid/src/sql/types.rs` → `crates/druid-rdbc/src/types/types.rs`
- Move: `crates/druid/src/sql/rdbc_type.rs` → `crates/druid-rdbc/src/types/rdbc_type.rs`
- Move: `crates/druid/src/sql/rdbc_sql_type.rs` → `crates/druid-rdbc/src/types/rdbc_sql_type.rs`
- Modify: `crates/druid-rdbc/src/types/mod.rs`
- Modify: `crates/druid/src/core/mod.rs`
- Modify: `crates/druid/src/sql/mod.rs`
- Test: `crates/druid/tests/rdbc_standard_test.rs`
- Test: `crates/druid/tests/rdbc_extended_types_test.rs`
- Test: `crates/druid/tests/rdbc_url_extended_test.rs`

**Interfaces:**
- Produces: `druid_rdbc::types::{Value, Row, Savepoint, RdbcString, RdbcUrl, Date, Time, Timestamp, Types, RdbcType, SqlType}`

- [ ] **Step 1: 捕获类型身份 RED/GREEN 基线**

Before moving, run:

```bash
cargo test -p druid --test rdbc_standard_test
cargo test -p druid --test rdbc_extended_types_test
cargo test -p druid --test rdbc_url_extended_test
```

Expected: PASS. Save output in `docs/verification/rdbc-first/rdbc-types-before.txt`.

- [ ] **Step 2: 使用 git mv 逐对象移动**

Run one `git mv` per listed file. Update imports inside moved files to `crate::types::*`; do not combine
files or rename public types.

- [ ] **Step 3: 增加旧路径薄重导出**

Old modules contain only forms such as:

```rust
pub use druid_rdbc::types::Value;
```

or `core/mod.rs` directly re-exports the new crate type. `sql/mod.rs` continues exposing the same
names.

- [ ] **Step 4: Run type/API regression**

```bash
cargo test -p druid --test rdbc_standard_test
cargo test -p druid --test rdbc_extended_types_test
cargo test -p druid --test rdbc_url_extended_test
cargo test -p druid --test rdbc_external_consumer_test
cargo check --workspace --all-targets
```

Expected: all PASS; no semantic assertion changes.

- [ ] **Step 5: Commit**

```bash
git add crates/druid-rdbc crates/druid docs/verification/rdbc-first
git commit -m "refactor(rdbc): extract platform value types"
```

### Task 3: 移动结构化 SQL 错误

**Files:**
- Move: `crates/druid/src/core/sql_exception.rs` → `crates/druid-rdbc/src/error/sql_exception.rs`
- Move: `crates/druid/src/core/sql_warning.rs` → `crates/druid-rdbc/src/error/sql_warning.rs`
- Move: `crates/druid/src/sql/exceptions.rs` → `crates/druid-rdbc/src/compat/exceptions.rs`
- Create: `crates/druid-rdbc/src/error/rdbc_error.rs`
- Modify: `crates/druid-rdbc/src/error/mod.rs`
- Modify: `crates/druid/src/core/error.rs`
- Modify: `crates/druid/src/sql/mod.rs`
- Test: `crates/druid/tests/druid_error_test.rs`
- Test: `crates/druid/tests/exception_sorter_semantics_test.rs`

**Interfaces:**
- Produces: `SqlException`/`SqlWarning` in `druid-rdbc`; temporary `DruidError` wrapper remains in `druid`

- [ ] **Step 1: 写错误 identity test**

Assert `druid::sql::SQLException` and `druid_rdbc::error::SqlException` are the same type by assigning
one to the other without conversion. Repeat for `SQLWarning`.

- [ ] **Step 2: Run RED after moving before re-export**

```bash
cargo test -p druid --test rdbc_external_consumer_test error_identity -- --exact
```

Expected: compile FAIL until old paths re-export the new types.

- [ ] **Step 3: 完成重导出和 DruidError import**

`DruidError::SqlException(Box<SqlException>)` continues using the exact moved type; do not change
variants or Display messages in this task.

- [ ] **Step 4: Run GREEN**

```bash
cargo test -p druid --test druid_error_test
cargo test -p druid --test exception_sorter_semantics_test
cargo test -p druid --test rdbc_external_consumer_test
```

Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/druid-rdbc crates/druid
git commit -m "refactor(rdbc): extract sql exception values"
```

### Task 4: 移动 Physical Driver SPI

**Files:**
- Move: `crates/druid/src/core/physical_connection.rs` → `crates/druid-rdbc/src/spi/physical_connection.rs`
- Move: `crates/druid/src/core/physical_connection_capabilities.rs` → `crates/druid-rdbc/src/spi/physical_connection_capabilities.rs`
- Move: `crates/druid/src/core/physical_connection_factory.rs` → `crates/druid-rdbc/src/spi/physical_connection_factory.rs`
- Move: `crates/druid/src/core/physical_connection_info.rs` → `crates/druid-rdbc/src/spi/physical_connection_info.rs`
- Move: `crates/druid/src/core/physical_statement.rs` → `crates/druid-rdbc/src/spi/physical_statement.rs`
- Move: `crates/druid/src/core/physical_prepared_statement.rs` → `crates/druid-rdbc/src/spi/physical_prepared_statement.rs`
- Move: `crates/druid/src/core/physical_callable_statement.rs` → `crates/druid-rdbc/src/spi/physical_callable_statement.rs`
- Move: `crates/druid/src/core/physical_database_meta_data.rs` → `crates/druid-rdbc/src/spi/physical_database_meta_data.rs`
- Move: `crates/druid/src/core/physical_result_set_meta_data.rs` → `crates/druid-rdbc/src/spi/physical_result_set_meta_data.rs`
- Move: `crates/druid/src/core/rdbc_result_set.rs` → `crates/druid-rdbc/src/spi/rdbc_result_set.rs`
- Move: `crates/druid/src/core/prepared_input_parameter.rs` → `crates/druid-rdbc/src/spi/prepared_input_parameter.rs`
- Move: `crates/druid/src/core/prepared_statement_key.rs` → `crates/druid-rdbc/src/spi/prepared_statement_key.rs`
- Move: `crates/druid/src/core/result_set_meta_data.rs` → `crates/druid-rdbc/src/spi/result_set_meta_data.rs`
- Move: `crates/druid/src/core/result_set_column_meta.rs` → `crates/druid-rdbc/src/spi/result_set_column_meta.rs`
- Move: `crates/druid/src/core/result_set_column_type.rs` → `crates/druid-rdbc/src/spi/result_set_column_type.rs`
- Modify: `crates/druid-rdbc/src/spi/mod.rs`
- Modify: `crates/druid/src/core/mod.rs`
- Modify: files recorded by Step 1 in `target/rdbc-spi-imports.txt`
- Test: `crates/druid/tests/physical_connection_contract.rs`
- Test: `crates/druid/tests/physical_connection_lease_test.rs`
- Test: `crates/druid-wrapper/tests/adapter_boundary_test.rs`
- Test: `crates/druid-wrapper/tests/sqlx_connection_adapter_test.rs`
- Test: `crates/druid-wrapper/tests/rbdc_connection_adapter_test.rs`
- Test: `crates/druid-wrapper/tests/toasty_connection_adapter_test.rs`

**Interfaces:**
- Produces: `druid_rdbc::spi::{PhysicalConnection, PhysicalConnectionFactory, PhysicalPreparedStatement, PhysicalResultSet, PhysicalDatabaseMetaData, PreparedInputParameter, PreparedStatementKey}`

- [ ] **Step 1: 冻结 import 修改集合并写 adapter compile RED test**

Run:

```bash
rg -l 'druid::core::.*(Physical|PreparedInputParameter|PreparedStatementKey)|crate::core::.*(Physical|PreparedInputParameter|PreparedStatementKey)' \
  crates/druid-wrapper/src > target/rdbc-spi-imports.txt
test -s target/rdbc-spi-imports.txt
```

Review and record every path before editing; the file is evidence only and is not committed.

Add a test importing the SPI from `druid_rdbc::spi` and asserting a SQLx factory can be coerced to
`Arc<dyn PhysicalConnectionFactory>`.

- [ ] **Step 2: Move one dependency slice at a time**

Order:

```text
capabilities/options/key/value descriptors
→ statement/prepared/result-set traits
→ metadata traits
→ PhysicalConnection
→ PhysicalConnectionFactory/Info
```

After each slice run `cargo check -p druid-rdbc -p druid -p druid-wrapper`.

- [ ] **Step 3: 将 old core path 变为定向重导出**

`core/mod.rs` may re-export public compatibility names, but old source files must not contain a
second trait definition.

- [ ] **Step 4: Run complete SPI regression**

```bash
python3 scripts/verify_rdbc_dependency_boundary.py
python3 scripts/verify_physical_connection_lease_surface.py
cargo test -p druid --test physical_connection_contract
cargo test -p druid --test physical_connection_lease_test
cargo test -p druid-wrapper --tests
```

Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/druid-rdbc crates/druid crates/druid-wrapper
git commit -m "refactor(rdbc): extract physical driver spi"
```

### Task 5: 移动资源 Access SPI 和标准句柄

**Files:**
- Move: `crates/druid/src/spi/rdbc_array_access.rs` → `crates/druid-rdbc/src/resource/rdbc_array_access.rs`
- Move: `crates/druid/src/spi/rdbc_blob_access.rs` → `crates/druid-rdbc/src/resource/rdbc_blob_access.rs`
- Move: `crates/druid/src/spi/rdbc_clob_access.rs` → `crates/druid-rdbc/src/resource/rdbc_clob_access.rs`
- Move: `crates/druid/src/spi/rdbc_n_clob_access.rs` → `crates/druid-rdbc/src/resource/rdbc_n_clob_access.rs`
- Move: `crates/druid/src/spi/rdbc_ref_access.rs` → `crates/druid-rdbc/src/resource/rdbc_ref_access.rs`
- Move: `crates/druid/src/spi/rdbc_resource_access.rs` → `crates/druid-rdbc/src/resource/rdbc_resource_access.rs`
- Move: `crates/druid/src/spi/rdbc_resource_capabilities.rs` → `crates/druid-rdbc/src/resource/rdbc_resource_capabilities.rs`
- Move: `crates/druid/src/spi/rdbc_resource_context.rs` → `crates/druid-rdbc/src/resource/rdbc_resource_context.rs`
- Move: `crates/druid/src/spi/rdbc_resource_factory.rs` → `crates/druid-rdbc/src/resource/rdbc_resource_factory.rs`
- Move: `crates/druid/src/spi/rdbc_resource_id.rs` → `crates/druid-rdbc/src/resource/rdbc_resource_id.rs`
- Move: `crates/druid/src/spi/rdbc_resource_kind.rs` → `crates/druid-rdbc/src/resource/rdbc_resource_kind.rs`
- Move: `crates/druid/src/spi/rdbc_resource_owner.rs` → `crates/druid-rdbc/src/resource/rdbc_resource_owner.rs`
- Move: `crates/druid/src/spi/rdbc_resource_state.rs` → `crates/druid-rdbc/src/resource/rdbc_resource_state.rs`
- Move: `crates/druid/src/spi/rdbc_sql_xml_access.rs` → `crates/druid-rdbc/src/resource/rdbc_sql_xml_access.rs`
- Move: `crates/druid/src/sql/array.rs` → `crates/druid-rdbc/src/resource/array.rs`
- Move: `crates/druid/src/sql/blob.rs` → `crates/druid-rdbc/src/resource/blob.rs`
- Move: `crates/druid/src/sql/clob.rs` → `crates/druid-rdbc/src/resource/clob.rs`
- Move: `crates/druid/src/sql/n_clob.rs` → `crates/druid-rdbc/src/resource/n_clob.rs`
- Move: `crates/druid/src/sql/ref_value.rs` → `crates/druid-rdbc/src/resource/ref_value.rs`
- Move: `crates/druid/src/sql/sql_xml.rs` → `crates/druid-rdbc/src/resource/sql_xml.rs`
- Move: `crates/druid/src/core/rdbc_input_stream.rs` → `crates/druid-rdbc/src/resource/rdbc_input_stream.rs`
- Move: `crates/druid/src/core/rdbc_output_stream.rs` → `crates/druid-rdbc/src/resource/rdbc_output_stream.rs`
- Move: `crates/druid/src/core/rdbc_reader.rs` → `crates/druid-rdbc/src/resource/rdbc_reader.rs`
- Move: `crates/druid/src/core/rdbc_writer.rs` → `crates/druid-rdbc/src/resource/rdbc_writer.rs`
- Modify: `crates/druid-rdbc/src/resource/mod.rs`
- Modify: `crates/druid/src/spi/mod.rs`
- Modify: `crates/druid/src/sql/mod.rs`
- Test: `rdbc_blob_test.rs`, `rdbc_clob_test.rs`, `rdbc_resource_context_test.rs`, `rdbc_extended_types_test.rs`

**Interfaces:**
- Produces: single resource state/access/factory implementation in `druid-rdbc`

- [ ] **Step 1: 写 type identity/resource clone contract**

Assert old and new paths name the same Blob/Clob/SQLXML types and cloned handles share cursor/free
state.

- [ ] **Step 2: Move resources without state-machine changes**

Use `git mv`; adjust crate-local imports only. Do not fix cancellation behavior in this task—that is
owned by the contract-hardening plan.

- [ ] **Step 3: Run resource regression**

```bash
cargo test -p druid --test rdbc_blob_test
cargo test -p druid --test rdbc_clob_test
cargo test -p druid --test rdbc_resource_context_test
cargo test -p druid --test rdbc_extended_types_test
```

Expected: all PASS with unchanged assertions.

- [ ] **Step 4: Commit**

```bash
git add crates/druid-rdbc crates/druid
git commit -m "refactor(rdbc): extract resource handles and access spi"
```

### Task 6: 收口 public API、DAG 和文档

**Files:**
- Modify: `crates/druid-rdbc/src/lib.rs`
- Modify: `crates/druid/src/lib.rs`
- Modify: `crates/druid/src/sql/mod.rs`
- Modify: `Cargo.toml`
- Create: `docs/verification/rdbc-first/rdbc-extraction.md`
- Modify: `docs/druid-rust-Architecture.zh_CN.md`

**Interfaces:**
- Produces: final five-product-crate DAG and compatibility report

- [ ] **Step 1: 收紧 module visibility**

`druid-rdbc` exposes only `types`, `error`, `spi`, `resource`, `compat` and curated root re-exports.
Implementation modules are private. `druid` exposes logical handles and compatibility re-exports;
remove ambiguous `core::Connection` aliases from the new canonical documentation, retaining only
deprecated compatibility if external baseline proves they exist.

- [ ] **Step 2: Run topology/API checks**

```bash
python3 scripts/verify_rdbc_dependency_boundary.py
python3 scripts/verify_rdbc_public_paths.py
cargo metadata --format-version 1 --no-deps \
  | jq -r '.packages[].name' | sort
cargo test -p druid --test rdbc_external_consumer_test
cargo public-api -p druid -sss --all-features \
  > api/druid-public-api.rdbc-first.after.txt
```

Expected products: `druid-rdbc`, `druid`, `druid-wrapper`, `druid-metrics`, `druid-admin` plus
`druid-bench`; consumer test PASS. Public API differences must be only approved re-export/visibility
changes.

- [ ] **Step 3: Run workspace gates**

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo test --workspace
cargo doc --workspace --all-features --no-deps
```

Expected: all PASS.

- [ ] **Step 4: 写迁移报告**

Record every moved file, old/new public path, API hash, dependency graph and remaining hardening
work in `rdbc-extraction.md`.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock crates docs api scripts
git diff --cached --check
git commit -m "refactor(rdbc): complete isolated contract extraction"
```
