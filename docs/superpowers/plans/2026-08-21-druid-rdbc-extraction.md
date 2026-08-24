# druid-rdbc No-Behavior Extraction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 保留 Tasks 1–5 已执行抽取的可恢复证据，冻结现有 `Physical*`/resource/adapter 分母，并把 rename/merge/delete 标准化交给 P04；本计划不把当前 `Physical*` 名称视为最终 API。

**Architecture:** Tasks 1–5 是已经执行的 transitional extraction：Physical/Resource/Wrapper SCC 被移入 `druid-rdbc` 并让 wrapper direct-import。ADR-RDBC-006 将这些实现视为唯一 standard trait tree 的迁移输入：JDBC 行为 rename/merge 到标准名称，Druid-only runtime internalize，concrete helpers move，最终删除 `Physical*`。Task 6 只冻结 denominator/disposition/handoff；P04 Task 1 先退出 error waiver。

**Tech Stack:** Cargo workspace、Rust 2021、cargo-public-api、cargo-semver-checks、CodeGraph

**Spec:** `docs/superpowers/specs/2026-08-24-rdbc-single-standard-trait-tree.md`

## Architecture correction — binding

- ADR-RDBC-006 取代 ADR-RDBC-005 与本计划原始 Physical-SPI 目标。
- Tasks 1–5 的历史任务文本保留为已执行证据，不再作为最终 owner 决策。
- 原 Task 6 已退休，禁止执行其“公开 `druid-rdbc::spi`”和“完成 isolated contract
  extraction”步骤。
- 当前 P03 完成态只能是 `TRANSITIONAL_PHYSICAL_SURFACE_PENDING_STANDARDIZATION`。
- 新 P04 的 Task 1 必须强制退出 `compat::DruidError` waiver；之后才允许 standard trait
  unification。

## Global Constraints

- 前置：External Pool RDBC Conformance 计划必须完成。
- 机械抽取阶段禁止修改方法语义、错误分类、默认值或线程模型。
- 每个 Java 对象继续独占一个 `.rs` 文件；禁止把对象合并进 `lib.rs`/`compat.rs`。
- `druid-rdbc` 不得依赖 `druid` 或任何具体数据库/池/HTTP/gRPC crate。
- 旧公共路径只做类型重导出，不复制实现。
- `druid::sql::*` external consumer 必须在每个 task 后通过。
- 用户已批准 P03 过渡豁免：`druid_rdbc::compat::DruidError` 标记为 `TRANSITIONAL_UNTIL_P04`，只服务于无环、无行为变化的机械抽取。
- P03 的 24 个 `DruidError` variants、字段、`Display`、`Error::source`、分类方法和转换实现不得增删或重分类；不得增加 `#[non_exhaustive]`，不得把任何 SPI 签名改为 `RdbcError`。
- `RdbcError` 在 P03 只建立 scaffold，不进入任何生产签名；P04 Task 1 是 P03 之后的强制前置，完整 mixed enum 不得成为 P04 完成态。
- public API baseline 暴露的每个旧 module-qualified path 都必须保留同名 `.rs` 文件并做定向 `pub use`；父模块/root re-export 不能替代模块 shim。
- 任何修改 `Cargo.toml` 或依赖解析的 task 必须同时审查并提交 `Cargo.lock`；Task 3/4 依赖集合应由 Task 1 预声明，若 manifest/lock 出现未计划 diff 必须 STOP 并先扩充该 task 的 Files/验证范围。
- wrapper import 与 public-path denominator 使用 tracked manifest 绑定实时源码集合；固定文件计数只能作为观察值，不能成为完成真值。

---

### Task 1: 建立 crate scaffold 和依赖/API 边界 RED 门禁

**Files:**
- Create: `crates/druid-rdbc/Cargo.toml`
- Create: `crates/druid-rdbc/src/lib.rs`
- Create: `crates/druid-rdbc/src/types/mod.rs`
- Create: `crates/druid-rdbc/src/error/mod.rs`
- Create: `crates/druid-rdbc/src/spi/mod.rs`
- Create: `crates/druid-rdbc/src/resource/mod.rs`
- Create: `crates/druid-rdbc/src/compat/mod.rs`
- Create: `scripts/verify_rdbc_dependency_boundary.py`
- Create: `scripts/verify_rdbc_public_paths.py`
- Create: `scripts/test_verify_rdbc_dependency_boundary.py`
- Create: `docs/verification/rdbc-first/rdbc-public-path-map.tsv`
- Create: `crates/druid/tests/rdbc_external_consumer_test.rs`
- Modify: root `Cargo.toml`

**Interfaces:**
- Produces: package `druid-rdbc`, Rust crate `druid_rdbc`, five declaration-only module facades, dependency verifier, public-path/source-shim verifier, external-consumer fixture
- Produces: the tracked `rdbc-public-path-map.tsv` denominator and `python3 scripts/verify_rdbc_public_paths.py --source-only` for checking declared new modules and exact old-module shim targets before the SCC can compile
- Produces: the five-column mapping schema
  `old_path<TAB>new_path<TAB>compile_mode<TAB>old_module_file<TAB>source_mode`.
  `source_mode` is `PURE_SHIM` for a whole-file exact compatibility shim or
  `FACADE_MEMBERSHIP` for a parent/secondary facade that retains unrelated baseline items while
  requiring every mapped alias to bind uniquely and directly to its canonical target.

- [ ] **Step 1: 写 topology RED 脚本**

`verify_rdbc_dependency_boundary.py` reads `cargo metadata --format-version 1`, requires exactly one
`workspace_members` package named `druid-rdbc` whose resolved `manifest_path` is exactly
`crates/druid-rdbc/Cargo.toml`, recursively traverses normal dependencies by package ID, and rejects:

```python
FORBIDDEN = {
    "druid", "druid-wrapper", "druid-metrics", "druid-admin",
    "sqlx", "toasty", "toasty-core", "rbdc", "bb8", "deadpool",
    "axum", "tonic", "reqwest", "kube", "prometheus",
}
```

It exits 1 when the unique local workspace member is absent, multiple, mismatched, or reaches a
forbidden package. The public-path script must use one fail-closed Rust tokenizer. For
`PURE_SHIM`, after allowed documentation/comments are removed, the whole old file is exactly its
mapped named `pub use` set; wildcards and every additional item/syntax fail. For
`FACADE_MEMBERSHIP`, every mapped alias has exactly one unrestricted-public direct target, no
same-name competing binding or wildcard exists, and unrelated baseline modules/exports/items remain
allowed. Domain facades allow only private module declarations and targeted re-exports.

- [ ] **Step 2: Run RED boundary**

```bash
python3 scripts/verify_rdbc_dependency_boundary.py
```

Expected: FAIL with `package druid-rdbc not found`.

- [ ] **Step 3: 创建最小 package 和五个 module 文件**

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
serde_json.workspace = true
thiserror.workspace = true
tokio = { workspace = true, features = ["sync"] }
url.workspace = true
uuid.workspace = true

[lints]
workspace = true
```

`lib.rs` declares `pub mod types;`, `pub mod error;`, `pub mod spi;`, `pub mod resource;`, and
`pub mod compat;`. Create all five listed `mod.rs` files now; each contains declarations/re-exports
only and no migrated object implementation.

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
python3 scripts/test_verify_rdbc_dependency_boundary.py
python3 scripts/verify_rdbc_public_paths.py --self-test
python3 scripts/verify_rdbc_public_paths.py --source-only --allow-empty-scaffold
! python3 scripts/verify_rdbc_public_paths.py --source-only
cargo test -p druid --test rdbc_external_consumer_test
cargo check -p druid-rdbc --all-targets
```

Expected: all positive commands PASS; the final default source-only command exits 1 because the
Task 1 manifest has the required header and zero rows. `--allow-empty-scaffold` is only valid for
this scaffold source-only check and is mutually exclusive with compile/API verifier modes.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock crates/druid-rdbc scripts \
  docs/verification/rdbc-first/rdbc-public-path-map.tsv \
  crates/druid/tests/rdbc_external_consumer_test.rs
git commit -m "build(rdbc): establish isolated contract crate"
```

### Task 2: 只移动无 `DruidError` 平台值和结构化 SQL 异常

**Files:**
- Move and recreate shim: `crates/druid/src/core/value.rs` → `crates/druid-rdbc/src/types/value.rs`
- Move and recreate shim: `crates/druid/src/core/row.rs` → `crates/druid-rdbc/src/types/row.rs`
- Move and recreate shim: `crates/druid/src/core/rdbc_url.rs` → `crates/druid-rdbc/src/types/rdbc_url.rs`
- Move and recreate shim: `crates/druid/src/sql/types.rs` → `crates/druid-rdbc/src/types/types.rs`
- Move and recreate shim: `crates/druid/src/sql/rdbc_type.rs` → `crates/druid-rdbc/src/types/rdbc_type.rs`
- Move and recreate shim: `crates/druid/src/sql/rdbc_sql_type.rs` → `crates/druid-rdbc/src/types/rdbc_sql_type.rs`
- Move and recreate shim: `crates/druid/src/core/sql_exception.rs` → `crates/druid-rdbc/src/error/sql_exception.rs`
- Move and recreate shim: `crates/druid/src/core/sql_warning.rs` → `crates/druid-rdbc/src/error/sql_warning.rs`
- Modify: `crates/druid-rdbc/src/types/mod.rs`
- Modify: `crates/druid-rdbc/src/error/mod.rs`
- Modify: `crates/druid/Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `docs/verification/rdbc-first/rdbc-public-path-map.tsv`
- Modify: `crates/druid/src/core/mod.rs`
- Modify: `crates/druid/src/sql/mod.rs`
- Test: `crates/druid/tests/rdbc_standard_test.rs`
- Test: `crates/druid/tests/rdbc_extended_types_test.rs`
- Test: `crates/druid/tests/rdbc_url_extended_test.rs`
- Test: `crates/druid/tests/druid_error_test.rs`
- Test: `crates/druid/tests/exception_sorter_semantics_test.rs`
- Test: `crates/druid/tests/rdbc_external_consumer_test.rs`

**Interfaces:**
- Consumes: Task 1 module facades and public-path verifier
- Produces: `druid_rdbc::types::{Value, Row, RdbcUrl, Types, RdbcType, SqlType}`
- Produces: `druid_rdbc::error::{SqlException, SqlExceptionCause, SqlExceptionKind, SqlWarning}`
- Preserves: old module-qualified paths such as `druid::core::value::Value`,
  `druid::core::sql_exception::SqlException`, and `druid::sql::rdbc_type::RdbcType` as exact type
  identities through per-file directional `pub use` shims

- [ ] **Step 1: 捕获行为与 module-qualified identity 基线**

```bash
cargo test -p druid --test rdbc_standard_test
cargo test -p druid --test rdbc_extended_types_test
cargo test -p druid --test rdbc_url_extended_test
cargo test -p druid --test druid_error_test
cargo test -p druid --test exception_sorter_semantics_test
```

Expected: PASS. Save output in `docs/verification/rdbc-first/rdbc-types-before.txt`. Extend the
external consumer with bidirectional assignments between every old module-qualified path and its
new `druid_rdbc` path; before the move those new imports are RED.

- [ ] **Step 2: 证明本切片不依赖 `DruidError`**

```bash
rg -n 'DruidError' \
  crates/druid/src/core/{value,row,rdbc_url,sql_exception,sql_warning}.rs \
  crates/druid/src/sql/{types,rdbc_type,rdbc_sql_type}.rs
```

Expected: no matches. `Savepoint`, `RdbcString`, `Date`, `Time`, and `Timestamp` are deliberately
excluded because their current public methods return `DruidError`; they move atomically in Task 3.

- [ ] **Step 3: Move implementations and recreate every old module file**

Run one `git mv` per listed object, then recreate the old source path with only a directed re-export,
for example:

```rust
pub use druid_rdbc::types::Value;
```

Update `core/mod.rs`/`sql/mod.rs` root exports without removing a baseline name. Do not replace a
module shim with only a parent re-export. In the same commit update the cumulative TSV denominator
with exactly one row for every moved public definition; the new path, compile mode, old module
file, and source mode must describe the compatibility source before the default nonempty source-only
gate runs. Moved leaf files use `PURE_SHIM`; `core/mod.rs`, `sql/mod.rs`, and any secondary facade
containing unrelated baseline exports use `FACADE_MEMBERSHIP`.
Rows are keyed by old public path, not by implementation file: one old module file may therefore
have several rows. The verifier groups those rows and applies their one consistent `source_mode`:
whole-file exact re-export equality for `PURE_SHIM`, or unique mapped-name membership for
`FACADE_MEMBERSHIP`. A direct new path has the form `druid_rdbc::{types,error,spi,resource,compat}::Item`
and is validated through that public domain facade, never by guessing a private implementation file.
If an old leaf name differs from the new leaf, the shim must use `as OldLeaf`; unchanged leaf names
must remain unaliased.
When `source_mode` is `FACADE_MEMBERSHIP`, scan the parent or secondary facade's top-level Rust
bindings after lexically skipping nested comments plus normal/byte/raw strings and character
literals. For every mapped old leaf, the facade must contain exactly one unrestricted-public
use-tree binding to the canonical mapped new path.
Any second private/restricted/public use binding, module, type/trait/union, value (`fn`/`const`/`static`/
foreign item), extern-crate alias, or macro declaration with that canonical name fails. Grouped/nested
use trees, aliases, and `self` members are resolved; any wildcard use fails. Unrelated baseline
attributes, `pub(crate)` modules, grouped named uses, functions, impls, and other complete items remain
allowed. `PURE_SHIM` leaf files remain exact grouped mapped-re-export sets with no additional item.

- [ ] **Step 4: Run GREEN type/error regression**

```bash
python3 scripts/verify_rdbc_public_paths.py --source-only
cargo test -p druid --test rdbc_standard_test
cargo test -p druid --test rdbc_extended_types_test
cargo test -p druid --test rdbc_url_extended_test
cargo test -p druid --test druid_error_test
cargo test -p druid --test exception_sorter_semantics_test
cargo test -p druid --test rdbc_external_consumer_test
cargo test -p druid --test rdbc_external_consumer_test error_identity -- --exact
cargo check --workspace --all-targets
```

Expected: all PASS; no semantic assertion, error classification, or type identity changes.

- [ ] **Step 5: Commit**

```bash
git add Cargo.lock crates/druid-rdbc crates/druid docs/verification/rdbc-first
git commit -m "refactor(rdbc): extract independent types and sql exceptions"
```

### Task 3: 原子下沉 `RdbcString`、依赖错误的平台值和 transitional `DruidError`

**Files:**
- Move and recreate shim: `crates/druid/src/core/rdbc_string.rs` → `crates/druid-rdbc/src/types/rdbc_string.rs`
- Move and recreate shim: `crates/druid/src/core/savepoint.rs` → `crates/druid-rdbc/src/types/savepoint.rs`
- Move and recreate shim: `crates/druid/src/sql/date.rs` → `crates/druid-rdbc/src/types/date.rs`
- Move and recreate shim: `crates/druid/src/sql/time.rs` → `crates/druid-rdbc/src/types/time.rs`
- Move and recreate shim: `crates/druid/src/sql/timestamp.rs` → `crates/druid-rdbc/src/types/timestamp.rs`
- Move and recreate shim: `crates/druid/src/core/error.rs` → `crates/druid-rdbc/src/compat/druid_error.rs`
- Move and recreate shim: `crates/druid/src/sql/exceptions.rs` → `crates/druid-rdbc/src/compat/exceptions.rs`
- Create: `crates/druid-rdbc/src/error/rdbc_error.rs`
- Create: `crates/druid/tests/druid_error_transition_identity_test.rs`
- Modify: `crates/druid-rdbc/src/types/mod.rs`
- Modify: `crates/druid-rdbc/src/error/mod.rs`
- Modify: `crates/druid-rdbc/src/compat/mod.rs`
- Modify: `crates/druid/src/core/mod.rs`
- Modify: `crates/druid/src/core/xa.rs`
- Modify: `crates/druid/src/sql/mod.rs`
- Modify: `docs/verification/rdbc-first/rdbc-public-path-map.tsv`
- Test: `crates/druid/tests/druid_error_test.rs`
- Test: `crates/druid/tests/exception_sorter_semantics_test.rs`
- Test: `crates/druid/tests/rdbc_standard_test.rs`
- Test: `crates/druid/tests/rdbc_extended_types_test.rs`
- Test: `crates/druid/tests/rdbc_external_consumer_test.rs`

**Interfaces:**
- Consumes: Task 2 `SqlException`/`SqlWarning` and independent types
- Produces: exact type `druid_rdbc::compat::DruidError`; both
  `druid::core::error::DruidError` and `druid::core::DruidError` are directional `pub use` aliases
- Produces: `druid_rdbc::types::{RdbcString, Savepoint, Date, Time, Timestamp}` using that same
  transitional error in their unchanged signatures
- Produces: scaffold-only `druid_rdbc::error::RdbcError`; no P03 SPI or adapter signature may use it

- [ ] **Step 1: 写 24-variant exhaustive/identity RED tests**

The new transition test constructs all 24 variants exactly once:

```text
PoolClosed, DataSourceClosed, AcquireTimeout, GetConnectionTimeout, LoginTimeout,
PoolExhausted, MaxWaitThreadCountExceeded, ValidationFailed, ConnectionLeaked,
ConnectionDiscarded, DriverError, SqlException, BatchUpdateException, SqlParseError,
WallViolation, DataSourceNotFound, DataSourceDisabled, ActiveConnectionsPreventRestart,
OnFatalError, DataSourceNotAvailable, InvalidArgument, UnsupportedOperation,
NoDriverExtension, Other
```

For every value, assign old → new and new → old without `From`/`Into`; exhaustively match without a
wildcard; compare `Display`, `class_name()`, `sql_exception()`, `batch_update_counts()`, and
`Error::source()` shape. Include nested causes and `OnFatalError.last_sql`. Before the move, importing
`druid_rdbc::compat::DruidError` is RED.

- [ ] **Step 2: Move the complete implementation as one compatibility slice**

Move the full enum, its 24 variants, fields, `Display`, `Error`, classification helpers, and existing
`From<String>`/`From<&str>` implementations to `compat/druid_error.rs`. Add the exact marker
`TRANSITIONAL_UNTIL_P04` to its module docs. Do **not** add `#[non_exhaustive]`, a new variant, a new
conversion, or a wrapper type. Keep the existing Druid-local
`From<XaStateTransitionError>` coherence impl in `core/xa.rs`, where it continues to compile against
the re-exported `DruidError`; its observable conversion must remain unchanged. `core/error.rs` is a
pure exact directed re-export shim and contains no `impl`.

Move `RdbcString`, `Savepoint`, `Date`, `Time`, and `Timestamp` in the same commit and update their
imports to `crate::compat::DruidError`. `RdbcError` contains only the smallest compilable scaffold
and is not referenced by those types or any SPI.

- [ ] **Step 3: Recreate all old paths as exact shims**

Recreate `core/error.rs`, `core/rdbc_string.rs`, `core/savepoint.rs`, `sql/date.rs`, `sql/time.rs`,
`sql/timestamp.rs`, and `sql/exceptions.rs` as exact directed re-export modules. In particular,
`core/error.rs` contains no coherence implementation; `core/xa.rs` retains the
`From<XaStateTransitionError>` implementation. Preserve both root and module-qualified API
identities shown by `api/druid-public-api.rdbc-first.before.txt`. In the same commit update the
cumulative TSV denominator with one exact row for every moved public definition and its
`PURE_SHIM` or `FACADE_MEMBERSHIP` source mode; then run the default nonempty source-only verifier.

- [ ] **Step 4: Run GREEN error/identity regression**

```bash
python3 scripts/verify_rdbc_dependency_boundary.py
python3 scripts/verify_rdbc_public_paths.py --source-only
cargo test -p druid --test druid_error_transition_identity_test
cargo test -p druid --test druid_error_test
cargo test -p druid --test exception_sorter_semantics_test
cargo test -p druid --test rdbc_standard_test
cargo test -p druid --test rdbc_extended_types_test
cargo test -p druid --test rdbc_external_consumer_test
cargo check --workspace --all-targets
```

Expected: all PASS; the 24 variants and all observable error behaviors are unchanged, and
`RdbcError` appears in no public production signature.

Before commit, run `git diff --quiet -- Cargo.toml crates/druid-rdbc/Cargo.toml Cargo.lock`.
Expected: PASS because Task 1 predeclared Task 3 dependencies. Any manifest/lock diff is an
unplanned dependency change: STOP, add both the owning `Cargo.toml` and `Cargo.lock` to Task 3 Files,
then rerun dependency-boundary and workspace checks before staging.

- [ ] **Step 5: Commit**

```bash
git add crates/druid-rdbc crates/druid \
  docs/verification/rdbc-first/rdbc-public-path-map.tsv
git commit -m "refactor(rdbc): establish transitional error seam"
```

### Task 4: 原子移动 Physical/Resource/Wrapper 完整 SCC

Task 4 merges the original Physical Task 4 and Resource Task 5. The files below are one strongly
connected move unit; do not split them into commits or claim a full `cargo check` before all target
modules and old shims are internally closed.

**Files — common descriptors and wrapper:**
- Move and recreate shim: `crates/druid/src/core/exec_result.rs` → `crates/druid-rdbc/src/spi/exec_result.rs`
- Move and recreate shim: `crates/druid/src/core/wrapper.rs` → `crates/druid-rdbc/src/spi/wrapper.rs`
- Modify shim: `crates/druid/src/sql/wrapper.rs`
- Move and recreate shim: `crates/druid/src/core/database_meta_data_row_id_lifetime.rs` → `crates/druid-rdbc/src/spi/database_meta_data_row_id_lifetime.rs`
- Move and recreate shim: `crates/druid/src/core/result_set_nullability.rs` → `crates/druid-rdbc/src/spi/result_set_nullability.rs`
- Move and recreate shim: `crates/druid/src/core/callable_input_parameter.rs` → `crates/druid-rdbc/src/spi/callable_input_parameter.rs`
- Move and recreate shim: `crates/druid/src/core/callable_out_parameter.rs` → `crates/druid-rdbc/src/spi/callable_out_parameter.rs`
- Move and recreate shim: `crates/druid/src/core/callable_parameter.rs` → `crates/druid-rdbc/src/spi/callable_parameter.rs`
- Move and recreate shim: `crates/druid/src/core/result_set_update.rs` → `crates/druid-rdbc/src/spi/result_set_update.rs`

**Files — RDBC value/parameter/XML closure:**
- Move and recreate shim: `crates/druid/src/core/rdbc_calendar.rs` → `crates/druid-rdbc/src/types/rdbc_calendar.rs`
- Move and recreate shim: `crates/druid/src/core/rdbc_object.rs` → `crates/druid-rdbc/src/types/rdbc_object.rs`
- Move and recreate shim: `crates/druid/src/core/rdbc_opaque_object.rs` → `crates/druid-rdbc/src/types/rdbc_opaque_object.rs`
- Move and recreate shim: `crates/druid/src/core/rdbc_row_id.rs` → `crates/druid-rdbc/src/types/rdbc_row_id.rs`
- Move and recreate shim: `crates/druid/src/core/rdbc_target_type.rs` → `crates/druid-rdbc/src/types/rdbc_target_type.rs`
- Move and recreate shim: `crates/druid/src/core/rdbc_type_map.rs` → `crates/druid-rdbc/src/types/rdbc_type_map.rs`
- Move and recreate shim: `crates/druid/src/core/rdbc_parameter.rs` → `crates/druid-rdbc/src/types/rdbc_parameter.rs`
- Move and recreate shim: `crates/druid/src/core/rdbc_parameter_date.rs` → `crates/druid-rdbc/src/types/rdbc_parameter_date.rs`
- Move and recreate shim: `crates/druid/src/core/rdbc_parameter_decimal.rs` → `crates/druid-rdbc/src/types/rdbc_parameter_decimal.rs`
- Move and recreate shim: `crates/druid/src/core/rdbc_parameter_impl.rs` → `crates/druid-rdbc/src/types/rdbc_parameter_impl.rs`
- Move and recreate shim: `crates/druid/src/core/rdbc_parameter_int.rs` → `crates/druid-rdbc/src/types/rdbc_parameter_int.rs`
- Move and recreate shim: `crates/druid/src/core/rdbc_parameter_long.rs` → `crates/druid-rdbc/src/types/rdbc_parameter_long.rs`
- Move and recreate shim: `crates/druid/src/core/rdbc_parameter_null.rs` → `crates/druid-rdbc/src/types/rdbc_parameter_null.rs`
- Move and recreate shim: `crates/druid/src/core/rdbc_parameter_string.rs` → `crates/druid-rdbc/src/types/rdbc_parameter_string.rs`
- Move and recreate shim: `crates/druid/src/core/rdbc_parameter_timestamp.rs` → `crates/druid-rdbc/src/types/rdbc_parameter_timestamp.rs`
- Move and recreate shim: `crates/druid/src/core/rdbc_xml_representation_type.rs` → `crates/druid-rdbc/src/resource/rdbc_xml_representation_type.rs`
- Move and recreate shim: `crates/druid/src/core/rdbc_xml_result.rs` → `crates/druid-rdbc/src/resource/rdbc_xml_result.rs`
- Move and recreate shim: `crates/druid/src/core/rdbc_xml_source.rs` → `crates/druid-rdbc/src/resource/rdbc_xml_source.rs`

**Files — Physical statement/result-set/metadata/connection SPI:**
- Move and recreate shim: `crates/druid/src/core/physical_connection.rs` → `crates/druid-rdbc/src/spi/physical_connection.rs`
- Move and recreate shim: `crates/druid/src/core/physical_connection_capabilities.rs` → `crates/druid-rdbc/src/spi/physical_connection_capabilities.rs`
- Move and recreate shim: `crates/druid/src/core/physical_connection_factory.rs` → `crates/druid-rdbc/src/spi/physical_connection_factory.rs`
- Move and recreate shim: `crates/druid/src/core/physical_connection_info.rs` → `crates/druid-rdbc/src/spi/physical_connection_info.rs`
- Move and recreate shim: `crates/druid/src/core/physical_statement.rs` → `crates/druid-rdbc/src/spi/physical_statement.rs`
- Move and recreate shim: `crates/druid/src/core/physical_prepared_statement.rs` → `crates/druid-rdbc/src/spi/physical_prepared_statement.rs`
- Move and recreate shim: `crates/druid/src/core/physical_callable_statement.rs` → `crates/druid-rdbc/src/spi/physical_callable_statement.rs`
- Move and recreate shim: `crates/druid/src/core/physical_database_meta_data.rs` → `crates/druid-rdbc/src/spi/physical_database_meta_data.rs`
- Move and recreate shim: `crates/druid/src/core/physical_result_set_meta_data.rs` → `crates/druid-rdbc/src/spi/physical_result_set_meta_data.rs`
- Move and recreate shim: `crates/druid/src/core/rdbc_result_set.rs` → `crates/druid-rdbc/src/spi/rdbc_result_set.rs`
- Move and recreate shim: `crates/druid/src/core/prepared_input_parameter.rs` → `crates/druid-rdbc/src/spi/prepared_input_parameter.rs`
- Move and recreate shim: `crates/druid/src/core/prepared_statement_key.rs` → `crates/druid-rdbc/src/spi/prepared_statement_key.rs`
- Move and recreate shim: `crates/druid/src/core/result_set_meta_data.rs` → `crates/druid-rdbc/src/spi/result_set_meta_data.rs`
- Move and recreate shim: `crates/druid/src/core/result_set_column_meta.rs` → `crates/druid-rdbc/src/spi/result_set_column_meta.rs`
- Move and recreate shim: `crates/druid/src/core/result_set_column_type.rs` → `crates/druid-rdbc/src/spi/result_set_column_type.rs`

**Files — Resource Access/Context/Owner/Factory/state and standard handles:**
- Move and recreate shim: `crates/druid/src/spi/rdbc_array_access.rs` → `crates/druid-rdbc/src/resource/rdbc_array_access.rs`
- Move and recreate shim: `crates/druid/src/spi/rdbc_blob_access.rs` → `crates/druid-rdbc/src/resource/rdbc_blob_access.rs`
- Move and recreate shim: `crates/druid/src/spi/rdbc_clob_access.rs` → `crates/druid-rdbc/src/resource/rdbc_clob_access.rs`
- Move and recreate shim: `crates/druid/src/spi/rdbc_n_clob_access.rs` → `crates/druid-rdbc/src/resource/rdbc_n_clob_access.rs`
- Move and recreate shim: `crates/druid/src/spi/rdbc_ref_access.rs` → `crates/druid-rdbc/src/resource/rdbc_ref_access.rs`
- Move and recreate shim: `crates/druid/src/spi/rdbc_resource_access.rs` → `crates/druid-rdbc/src/resource/rdbc_resource_access.rs`
- Move and recreate shim: `crates/druid/src/spi/rdbc_resource_capabilities.rs` → `crates/druid-rdbc/src/resource/rdbc_resource_capabilities.rs`
- Move and recreate shim: `crates/druid/src/spi/rdbc_resource_context.rs` → `crates/druid-rdbc/src/resource/rdbc_resource_context.rs`
- Move and recreate shim: `crates/druid/src/spi/rdbc_resource_factory.rs` → `crates/druid-rdbc/src/resource/rdbc_resource_factory.rs`
- Move and recreate shim: `crates/druid/src/spi/rdbc_resource_id.rs` → `crates/druid-rdbc/src/resource/rdbc_resource_id.rs`
- Move and recreate shim: `crates/druid/src/spi/rdbc_resource_kind.rs` → `crates/druid-rdbc/src/resource/rdbc_resource_kind.rs`
- Move and recreate shim: `crates/druid/src/spi/rdbc_resource_owner.rs` → `crates/druid-rdbc/src/resource/rdbc_resource_owner.rs`
- Move and recreate shim: `crates/druid/src/spi/rdbc_resource_state.rs` → `crates/druid-rdbc/src/resource/rdbc_resource_state.rs`
- Move and recreate shim: `crates/druid/src/spi/rdbc_sql_xml_access.rs` → `crates/druid-rdbc/src/resource/rdbc_sql_xml_access.rs`
- Move and recreate shim: `crates/druid/src/sql/array.rs` → `crates/druid-rdbc/src/resource/array.rs`
- Move and recreate shim: `crates/druid/src/sql/blob.rs` → `crates/druid-rdbc/src/resource/blob.rs`
- Move and recreate shim: `crates/druid/src/sql/clob.rs` → `crates/druid-rdbc/src/resource/clob.rs`
- Move and recreate shim: `crates/druid/src/sql/n_clob.rs` → `crates/druid-rdbc/src/resource/n_clob.rs`
- Move and recreate shim: `crates/druid/src/sql/ref_value.rs` → `crates/druid-rdbc/src/resource/ref_value.rs`
- Move and recreate shim: `crates/druid/src/sql/sql_xml.rs` → `crates/druid-rdbc/src/resource/sql_xml.rs`
- Move and recreate shim: `crates/druid/src/core/rdbc_input_stream.rs` → `crates/druid-rdbc/src/resource/rdbc_input_stream.rs`
- Move and recreate shim: `crates/druid/src/core/rdbc_output_stream.rs` → `crates/druid-rdbc/src/resource/rdbc_output_stream.rs`
- Move and recreate shim: `crates/druid/src/core/rdbc_reader.rs` → `crates/druid-rdbc/src/resource/rdbc_reader.rs`
- Move and recreate shim: `crates/druid/src/core/rdbc_writer.rs` → `crates/druid-rdbc/src/resource/rdbc_writer.rs`
- Modify: `crates/druid-rdbc/src/types/mod.rs`
- Modify: `crates/druid-rdbc/src/spi/mod.rs`
- Modify: `crates/druid-rdbc/src/resource/mod.rs`
- Modify: `crates/druid/src/core/mod.rs`
- Modify: `crates/druid/src/core/connection.rs`
- Modify: `crates/druid/src/core/connection_factory.rs`
- Modify: `crates/druid/src/spi/mod.rs`
- Modify: `crates/druid/src/sql/mod.rs`
- Modify: `crates/druid/src/sql/result_set_meta_data.rs`
- Modify: `crates/druid/src/sql/row_id.rs`
- Modify: `crates/druid/src/sql/row_id_lifetime.rs`
- Modify: `crates/druid/tests/rdbc_external_consumer_test.rs`
- Modify: `scripts/verify_rdbc_public_paths.py`
- Modify: `scripts/verify_physical_connection_lease_surface.py`
- Modify: `docs/verification/rdbc-first/rdbc-public-path-map.tsv`
- Create: `docs/verification/rdbc-first/rdbc-public-path-map.task4.sha256`
- Test: `crates/druid/tests/physical_connection_contract.rs`
- Test: `crates/druid/tests/physical_connection_lease_test.rs`
- Test: `crates/druid/tests/rdbc_blob_test.rs`
- Test: `crates/druid/tests/rdbc_clob_test.rs`
- Test: `crates/druid/tests/rdbc_resource_context_test.rs`
- Test: `crates/druid/tests/rdbc_extended_types_test.rs`
- Test: `crates/druid-wrapper/tests/adapter_boundary_test.rs`
- Test: `crates/druid-wrapper/tests/sqlx_connection_adapter_test.rs`
- Test: `crates/druid-wrapper/tests/rbdc_connection_adapter_test.rs`
- Test: `crates/druid-wrapper/tests/toasty_connection_adapter_test.rs`

**Interfaces:**
- Consumes: Task 3 transitional `DruidError`, platform values, SQL exceptions, and module shims
- Produces: one internally closed `druid_rdbc::{types, spi, resource}` SCC containing every listed
  Physical/Prepared/Callable/ResultSet/Metadata/Resource/Wrapper definition
- Preserves: every old root and module-qualified type/trait object as the exact same definition via
  directional `pub use`; no copied trait, enum, handle, or state machine
- Verifies: the lease-surface checker reads the canonical trait from `druid-rdbc` and separately
  rejects a missing, wrong-target, copied-definition, wildcard, or otherwise non-pure old
  `core::physical_connection` shim. This Task 4 scope expansion was approved after the original
  verifier's hard-coded old implementation path became unreachable by design.
- Verifies: the lease checker imports the existing public-path Rust tokenizer by script-relative
  module resolution, with no import-time CLI side effect, and obtains only top-level `fn` names
  from the canonical trait/lease-impl token blocks. Documentation, nested comments, normal/raw/byte
  strings, character literals, and nested default bodies containing braces cannot truncate the
  surface; the self-test places a missing future default after all of those adversarial forms.
- Visibility seam: `ResultSetMetaData::with_mysql8_datetime_compatibility` is the sole approved
  normalized non-import delta: `pub(crate)` becomes `pub` because its unchanged canonical
  implementation moved to `druid-rdbc` while the existing MySQL8 filter caller remains in
  `druid`. Its name, parameters, return type, body, state transition, and call site remain
  unchanged. Task 6 must record every exact generated `druid` re-export-path API addition for this
  one method in the additions-only allowlist; it may not use a wildcard or allow any other changed
  or removed API line.

- [ ] **Step 1: Freeze imports and write SCC identity RED tests**

```bash
rg -l 'druid::core::|druid::spi::|druid::sql::' crates/druid-wrapper/src \
  | sort > target/rdbc-scc-imports.before.txt
test -s target/rdbc-scc-imports.before.txt
```

Extend the `druid` external-consumer fixture with bidirectional assignments for old/new
`Arc<dyn PhysicalConnectionFactory>`, `Box<dyn PhysicalConnection>`, statement/result-set/metadata
trait objects, `Wrapper`/`Unwrapped`, resource Access trait objects, and standard handles. Keep the
wrapper adapter-boundary tests on old paths in this task; the wrapper crate does not receive its
direct `druid-rdbc` dependency until Task 5. New paths are RED in the consumer fixture before the
move.

- [ ] **Step 2: Move the SCC in internal source slices without false compile claims**

Within this single task/commit, move in this order: value/parameter/XML descriptors → resource
Access/state/handles → result-set/metadata → statement/prepared/callable → wrapper/connection/factory.
Update the cumulative public-path TSV denominator with one exact row for every moved public
definition before each default source-only invocation. Moved leaves and whole-file exact secondary
compatibility facades use `PURE_SHIM`; parents and secondary compatibility facades that retain
unrelated baseline items use `FACADE_MEMBERSHIP`. An empty, four-column, stale, or
source-mode-inconsistent denominator blocks the slice. Task 4 closes at 285 rows and 75 source
groups—71 `PURE_SHIM`, 4 `FACADE_MEMBERSHIP`—including the 11 reviewed secondary aliases in
`core::connection`, `core::connection_factory`, SQL RowId, RowIdLifetime, and ResultSetMetaData
facades.
After each small source slice run only:

```bash
python3 scripts/verify_rdbc_public_paths.py --source-only
rg -n 'pub (struct|enum|trait) ' crates/druid/src/core crates/druid/src/spi crates/druid/src/sql
```

Review matches against the moved-file ledger to prove old files are shims and imports point toward
`druid-rdbc`. Do not claim `cargo check` for an incomplete slice.

- [ ] **Step 3: Close all module declarations and old public shims**

Populate the three `druid-rdbc` facades with directed declarations/re-exports. Recreate every source
path named in Files as a thin `pub use` module, including `core::wrapper`, `sql::wrapper`, and all
resource modules. Parent/root exports remain compatibility additions, not replacements for shims.

After the final 285-row TSV bytes are stable, create the tracked Task 4 freeze record with Python
`hashlib` so the command is portable across macOS/Linux:

```bash
python3 - <<'PY'
import hashlib
from pathlib import Path
mapping = Path('docs/verification/rdbc-first/rdbc-public-path-map.tsv')
digest = hashlib.sha256(mapping.read_bytes()).hexdigest()
Path('docs/verification/rdbc-first/rdbc-public-path-map.task4.sha256').write_text(
    f'{digest}  {mapping.as_posix()}\n', encoding='utf-8'
)
PY
```

The hash file is exactly one newline-terminated line: 64 lowercase hexadecimal characters, two
ASCII spaces, and `docs/verification/rdbc-first/rdbc-public-path-map.tsv`. Default nonempty
`--source-only` validation verifies the five-column schema, exact 285-row denominator, required
secondary set, and this digest before it inspects source modes. Its self-test must pass the exact
bytes and fail after a one-byte mapping mutation.

- [ ] **Step 4: Run the first complete Cargo check and all merged regressions**

Only after the full SCC is closed, run:

```bash
python3 scripts/verify_physical_connection_lease_surface.py --self-test
python3 scripts/verify_rdbc_dependency_boundary.py
python3 scripts/verify_rdbc_public_paths.py --self-test
python3 scripts/verify_rdbc_public_paths.py --source-only
python3 scripts/verify_physical_connection_lease_surface.py
cargo check -p druid-rdbc -p druid -p druid-wrapper --all-targets
cargo test -p druid --test physical_connection_contract
cargo test -p druid --test physical_connection_lease_test
cargo test -p druid --test rdbc_blob_test
cargo test -p druid --test rdbc_clob_test
cargo test -p druid --test rdbc_resource_context_test
cargo test -p druid --test rdbc_extended_types_test
cargo test -p druid-wrapper --test adapter_boundary_test
cargo test -p druid-wrapper --test sqlx_connection_adapter_test
cargo test -p druid-wrapper --test rbdc_connection_adapter_test
cargo test -p druid-wrapper --test toasty_connection_adapter_test
```

Expected: all PASS with unchanged resource state, clone/free semantics, SPI signatures, defaults,
and lease behavior.

Before commit, run `git diff --quiet -- Cargo.toml crates/druid-rdbc/Cargo.toml Cargo.lock`.
Expected: PASS because Task 1 already declared `serde_json`, `uuid`, Tokio and all other SCC
dependencies. Any manifest/lock diff is an unplanned dependency change: STOP, expand Task 4 Files to
include the owning manifest plus `Cargo.lock`, and rerun the dependency and all-target checks.

- [ ] **Step 5: Commit**

```bash
git add crates/druid-rdbc crates/druid crates/druid-wrapper \
  scripts/verify_rdbc_public_paths.py \
  scripts/verify_physical_connection_lease_surface.py \
  docs/superpowers/plans/2026-08-21-druid-rdbc-extraction.md \
  docs/verification/rdbc-first/rdbc-public-path-map.tsv \
  docs/verification/rdbc-first/rdbc-public-path-map.task4.sha256
git commit -m "refactor(rdbc): extract physical resource wrapper scc"
```

### Task 5: Cut over concrete wrapper imports to `druid-rdbc`

**Files:**
- Modify: `crates/druid-wrapper/Cargo.toml`
- Modify: `Cargo.lock`
- Create: `scripts/verify_wrapper_rdbc_import_boundary.py`
- Create: `docs/verification/rdbc-first/wrapper-rdbc-import-boundary.before.tsv`
- Create: `docs/verification/rdbc-first/wrapper-rdbc-import-boundary.after.tsv`
- Read/verify unchanged: `docs/verification/rdbc-first/rdbc-public-path-map.tsv`
- Read/verify unchanged: `docs/verification/rdbc-first/rdbc-public-path-map.task4.sha256`
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
- Modify: every additional live production path classified `RDBC_DIRECT` in `docs/verification/rdbc-first/wrapper-rdbc-import-boundary.before.tsv`
- Test: `crates/druid-wrapper/tests/adapter_boundary_test.rs`
- Test: `crates/druid-wrapper/tests/sqlx_connection_adapter_test.rs`
- Test: `crates/druid-wrapper/tests/rbdc_connection_adapter_test.rs`
- Test: `crates/druid-wrapper/tests/toasty_connection_adapter_test.rs`
- Test: `crates/druid-wrapper/tests/prepared_statement_semantics_test.rs`

**Interfaces:**
- Consumes: Task 4 canonical `druid_rdbc::{types, spi, resource}` plus
  `druid_rdbc::compat::DruidError`
- Produces: concrete adapters that import driver-facing contracts directly from `druid-rdbc`
- Produces: a live-set verifier plus tracked before/after classification manifests; manifest rows
  are keyed by production file and moved symbol family, not by a fixed expected file count
- Preserves: the complete 285-row five-column public-path manifest unchanged; wrapper import
  cutover creates no new `druid` compatibility path and must not erase or rewrite `source_mode`
- Preserves: imports of `DruidPooledConnection`, logical pooled statements/result sets, `Pool`,
  `Filter`, driver catalog/product configuration, and Druid lifecycle from `druid`

- [ ] **Step 1: Write the live wrapper-import verifier and tracked denominator manifests**

Before any Task 5 edit, prove that the Task 4 denominator and freeze record are exact and untouched:

```bash
python3 scripts/verify_rdbc_public_paths.py --source-only
git diff --quiet -- docs/verification/rdbc-first/rdbc-public-path-map.tsv
git diff --cached --quiet -- docs/verification/rdbc-first/rdbc-public-path-map.tsv
git diff --quiet -- docs/verification/rdbc-first/rdbc-public-path-map.task4.sha256
git diff --cached --quiet -- docs/verification/rdbc-first/rdbc-public-path-map.task4.sha256
```

Any failure stops Task 5. Do not regenerate, stage, or commit either Task 4 denominator file.

`verify_wrapper_rdbc_import_boundary.py` scans every production `.rs` file below
`crates/druid-wrapper/src` and normalizes multiline/nested `use` trees plus fully-qualified paths. It
must discover any implementation or use of:

```text
Physical* traits/descriptors
Rdbc*Access / RdbcResource* contracts
all low-level types moved by Tasks 2–4
compat::DruidError used by those low-level contracts
```

The TSV schema is:

```text
path<TAB>symbol_family<TAB>classification<TAB>expected_source
```

Allowed classifications are `RDBC_DIRECT` and `DRUID_PRODUCT`. The before manifest records the live
post-Task-4 source set and its current source. The after manifest has the exact same `(path,
symbol_family)` keys: `RDBC_DIRECT` rows require `druid_rdbc`; `DRUID_PRODUCT` rows allow `druid` for
logical pooled handles, `Pool`, `Filter`, catalog/configuration, and lifecycle only.

Run the inventory mode and review every row before editing:

```bash
python3 scripts/verify_wrapper_rdbc_import_boundary.py --emit-live \
  > target/wrapper-rdbc-import-boundary.live.tsv
diff -u docs/verification/rdbc-first/wrapper-rdbc-import-boundary.before.tsv \
  target/wrapper-rdbc-import-boundary.live.tsv
```

Expected: exact match after the tracked before manifest is written. Missing/stale/extra manifest
rows, wildcard imports, or an unclassified symbol family are RED; do not accept an empty live set.

- [ ] **Step 2: Write old/new trait-object identity RED tests**

For SQLx, Toasty, RBDC, JDBC Agent, libSQL, HTTP SQL, and DuckDB, compile assignments in both
directions between the old `druid::core::*`/`druid::spi::*` trait object and the new
`druid_rdbc::spi::*`/`resource::*` object. Add direct imports of `Value`, prepared/callable
descriptors, and transitional `DruidError`. The test is RED while adapters still depend only on old
imports. This identity test is supplementary: only the live import verifier can prevent a zero-edit
thin-reexport false green.

- [ ] **Step 3: Cut over only RDBC-owned imports**

Add the direct `druid-rdbc` dependency. Concrete adapter, connection factory, prepared statement,
metadata, result-set, resource and native-error mapping files import `spi`, `types`, `resource`, and
`compat::DruidError` directly. Keep logical pooled handles, `Pool`, `Filter`, and product behavior on
`druid`; do not use `druid::core`, `druid::spi`, or `druid::sql` as an indirect facade for any moved
low-level name. Populate the tracked after manifest from the reviewed expected state, not by blindly
blessing verifier output.

- [ ] **Step 4: Run GREEN wrapper/identity regression**

```bash
python3 scripts/verify_rdbc_dependency_boundary.py
python3 scripts/verify_rdbc_public_paths.py --source-only
python3 scripts/verify_wrapper_rdbc_import_boundary.py \
  --before docs/verification/rdbc-first/wrapper-rdbc-import-boundary.before.tsv \
  --after docs/verification/rdbc-first/wrapper-rdbc-import-boundary.after.tsv
cargo check --workspace --all-targets --all-features
cargo test -p druid-wrapper --test adapter_boundary_test
cargo test -p druid-wrapper --test sqlx_connection_adapter_test
cargo test -p druid-wrapper --test rbdc_connection_adapter_test
cargo test -p druid-wrapper --test toasty_connection_adapter_test
cargo test -p druid-wrapper --test prepared_statement_semantics_test
cargo test -p druid-wrapper --tests
```

Expected: all PASS. The verifier recomputes the live set, requires exact key equality with both
tracked manifests, rejects every moved low-level import from `druid::{core,spi,sql}`, and requires a
direct `druid_rdbc` import for each `RDBC_DIRECT` row. Old/new trait objects remain identical and
Druid product imports remain explicit.

Immediately before staging, rerun the Task 4 freeze and index guards:

```bash
python3 scripts/verify_rdbc_public_paths.py --source-only
git diff --quiet -- docs/verification/rdbc-first/rdbc-public-path-map.tsv
git diff --cached --quiet -- docs/verification/rdbc-first/rdbc-public-path-map.tsv
git diff --quiet -- docs/verification/rdbc-first/rdbc-public-path-map.task4.sha256
git diff --cached --quiet -- docs/verification/rdbc-first/rdbc-public-path-map.task4.sha256
```

- [ ] **Step 5: Commit**

```bash
git add Cargo.lock crates/druid-wrapper scripts/verify_wrapper_rdbc_import_boundary.py \
  docs/verification/rdbc-first/wrapper-rdbc-import-boundary.before.tsv \
  docs/verification/rdbc-first/wrapper-rdbc-import-boundary.after.tsv
git commit -m "refactor(wrapper): consume rdbc contracts directly"
python3 scripts/verify_rdbc_public_paths.py --source-only
git diff --quiet -- docs/verification/rdbc-first/rdbc-public-path-map.tsv
git diff --cached --quiet -- docs/verification/rdbc-first/rdbc-public-path-map.tsv
git diff --quiet -- docs/verification/rdbc-first/rdbc-public-path-map.task4.sha256
git diff --cached --quiet -- docs/verification/rdbc-first/rdbc-public-path-map.task4.sha256
git diff --quiet HEAD^..HEAD -- docs/verification/rdbc-first/rdbc-public-path-map.tsv
git diff --quiet HEAD^..HEAD -- docs/verification/rdbc-first/rdbc-public-path-map.task4.sha256
```

The postcommit hook fails for a late working-tree mutation, an accidentally staged denominator,
or a Task 5 commit that changed either frozen file; source/hash validation runs before all six Git
diff guards.

### Historical Task 6 (RETIRED): 收口旧 public API、DAG 和文档

> **DO NOT EXECUTE.** 本节保留用于解释旧 Task 6 为什么曾被 `cargo-semver-checks`
> 阻塞。其目标 `druid-rdbc::spi::Physical*` 已被 ADR-RDBC-006 取代，由文件末尾的新
> Task 6 冻结为标准化输入。

**Files:**
- Modify: `crates/druid-rdbc/src/lib.rs`
- Modify: `crates/druid/src/lib.rs`
- Modify: `crates/druid/src/core/mod.rs`
- Modify: `crates/druid/src/spi/mod.rs`
- Modify: `crates/druid/src/sql/mod.rs`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `scripts/verify_rdbc_public_paths.py`
- Create: `api/druid-public-api.rdbc-first.after.txt`
- Create: `api/druid-public-api.rdbc-first.diff.txt`
- Create: `api/druid-public-api.rdbc-first.allowlist.txt`
- Modify: `docs/verification/rdbc-first/rdbc-public-path-map.tsv`
- Modify or replace: `docs/verification/rdbc-first/rdbc-public-path-map.task4.sha256`
- Create: `docs/verification/rdbc-first/rdbc-extraction.md`
- Modify: `docs/druid-rust-Architecture.zh_CN.md`

**Interfaces:**
- Consumes: Tasks 1–5 complete crate/API/import topology
- Produces: five-product-crate DAG, tracked complete old/new path mapping, generated external-consumer
  compile evidence, before/after API diff with an exact additions-only allowlist, and extraction report
- Produces: explicit P03 completion state `ERROR_OWNERSHIP_TRANSITIONAL_UNTIL_P04`; P04 Task 1 is the mandatory next plan

- [ ] **Step 1: Verify API tooling before changing Task 6 files**

Run:

```bash
cargo public-api --version
cargo public-api diff --help
cargo semver-checks --version
```

`cargo-public-api 0.52.0` is observed in the Task 0 environment, and its help accepts diff options
after `diff` followed by a single `BASE..HEAD` range. Re-check at execution time. If
`cargo semver-checks --version` is unavailable, **STOP Task 6**: do not install it, do not claim a
semver PASS, and do not weaken or remove the independent `cargo public-api diff` gate below. Follow
the repository tooling rule: inspect current official installation documentation, describe scope
and affected files, and request explicit user authorization before installation. Resume only after
the installed version/help and exact invocation are recorded in the verification report.

- [ ] **Step 2: 收紧 visibility and create the complete path/allowlist manifests**

`druid-rdbc` exposes only `types`, `error`, `spi`, `resource`, `compat` and curated root re-exports.
Implementation modules are private. `druid` exposes logical handles plus compatibility re-exports.
The P03 baseline comparison has a zero-deletion budget for every existing old module-qualified path;
do not remove an ambiguous public alias in P03 even if new canonical documentation stops recommending
it.

Complete and validate the cumulative tracked `rdbc-public-path-map.tsv`, which Tasks 2–5 update
from its Task 1 header-only scaffold, against this schema:

```text
old_path<TAB>new_path<TAB>compile_mode<TAB>old_module_file<TAB>source_mode
```

`old_path` is a legal absolute `druid::...::Item` path whose module components exactly match
`old_module_file` under `crates/druid/src` (including `mod.rs`). `new_path` is a legal direct
`druid_rdbc::{types,error,spi,resource,compat}::Item` public-facade path. `old_path` is globally
unique; repeated `old_module_file` rows use one consistent `source_mode`. A repeated `new_path` is
permitted for distinct legacy aliases only when all rows use the same compile mode.
Whitespace-only, trailing, malformed, or empty TSV fields fail.
Manifest identifiers use an explicit ASCII Rust 2021 policy: ordinary identifiers match
`[A-Za-z_][A-Za-z0-9_]*`, cannot be `_`, and cannot be strict or reserved keywords; contextual
`union` remains a valid ordinary path segment. Raw `r#ident` is accepted only to escape a Rust 2021
strict/reserved keyword. Redundant raw nonkeywords are rejected, and `r#crate`, `r#self`, `r#super`,
`r#Self`, `r#_`, and malformed raw identifiers fail. Old paths, new paths, exported aliases,
duplicate/mode keys, and module components are compared by canonical names with `r#` removed;
`old_module_file` uses the corresponding canonical filesystem component without `r#`.

`source_mode` is `PURE_SHIM` or `FACADE_MEMBERSHIP`. `PURE_SHIM` validates the entire old source
file as exactly the mapped named re-export set after lexical removal of allowed documentation and
comments; it rejects every extra item, modifier, non-documentation attribute, macro, or wildcard.
`FACADE_MEMBERSHIP` applies to parent and secondary facades that retain unrelated baseline
exports. It requires each mapped alias to have exactly one unrestricted-public binding directly to
the canonical new path, rejects a same-name private/restricted/public competing binding and every
wildcard, and otherwise permits unrelated complete baseline items. Parent `mod.rs` rows must use
`FACADE_MEMBERSHIP`; mixed source modes for one old file fail.

`compile_mode` is `OWNED`, `DYN_TRAIT`, or `PATH_ONLY`. The verifier derives the live baseline
denominator, and every moved public definition present in
`api/druid-public-api.rdbc-first.before.txt` must have exactly one row: no missing, duplicate, stale,
or extra mapping row is permitted. `OWNED` generates old→new and
new→old value assignments; `DYN_TRAIT` generates reference conversions in both directions.
`PATH_ONLY` covers non-dyn-compatible traits, module constants, and other items whose contract is
that both names compile but which cannot use owned or trait-object identity assignments. The Task 6
generator compiles non-dyn traits through both imported trait names and generic bounds, evaluates
both module-constant paths with locally inferred types, and emits distinct path-use checks for any
remaining path-only item. Source verification still proves the exact re-export or unique facade
membership, so `PATH_ONLY` cannot weaken an `OWNED` or `DYN_TRAIT` row. The five
`rdbc_parameter` module constants are explicitly validated `PATH_ONLY` rows. Missing, duplicate,
stale, source-mode-inconsistent, or wildcard mappings fail.

The post-Task-4 denominator is exactly 285 rows: the prior 274 rows plus 11 API-before-backed
secondary aliases for `ExecResult`, PhysicalConnection/Connection,
PhysicalConnectionFactory/ConnectionFactory, SQL RowId/RowIdLifetime, and ResultSetMetaData. Task 5
does not change this denominator. Task 6 derives the complete live baseline set and requires exact
equality with all 285 rows before compile/API comparison; a smaller denominator is never accepted.

The Task 4 SHA-256 record freezes only the exact Task 4 handoff and blocks Task 5 drift. Task 6 is
explicitly authorized to modify the TSV when its complete live/API comparison proves a required
change. In that case Task 6 must record the old/new hash-file diff and either regenerate the same
stable SHA-256 record for its final TSV bytes or replace it with a clearly named final-denominator
hash artifact and update the verifier atomically. Task 5's no-stage restriction does not prohibit
this Task 6-owned update; leaving a stale Task 4 digest is forbidden.

`druid-public-api.rdbc-first.allowlist.txt` contains exact API additions permitted in P03. Removed or
changed baseline lines can never be allowlisted.

The sole Task 4 visibility seam is an explicit Task 6 allowlist obligation:
`ResultSetMetaData::with_mysql8_datetime_compatibility` changed only from `pub(crate)` to `pub` so
the unchanged `druid` MySQL8 filter can call the implementation now owned by `druid-rdbc`. Task 6
must enumerate each exact addition emitted for that method through the existing `druid::core` and
`druid::sql` re-export paths. It must not allowlist a prefix, wildcard, removal, changed signature,
or any unrelated addition.

The source-only public-path gate is also the privacy and source-mode gate: before compile/API modes, run
`python3 scripts/verify_rdbc_public_paths.py --source-only`. It requires only `lib.rs` to expose the
five public domain modules; in each `druid-rdbc/src/{types,error,spi,resource,compat}/mod.rs`, every
implementation declaration must be private `mod name;`. `pub mod name;` and every
visibility-qualified implementation declaration fail, while targeted `pub use` remains allowed.
It then applies whole-file exact checking only to `PURE_SHIM` groups and unique direct-target
membership checking to every `FACADE_MEMBERSHIP` group, including non-`mod.rs` secondary facades.

- [ ] **Step 3: Generate/compile every mapping and compare the working-tree snapshot**

```bash
python3 scripts/verify_rdbc_dependency_boundary.py
python3 scripts/verify_rdbc_public_paths.py --source-only
python3 scripts/verify_rdbc_public_paths.py \
  --mapping docs/verification/rdbc-first/rdbc-public-path-map.tsv \
  --compile-all-pairs
cargo metadata --format-version 1 --no-deps \
  | jq -r '.packages[].name' | sort
cargo tree -p druid-rdbc
cargo tree --invert -p druid-rdbc
cargo test -p druid --test rdbc_external_consumer_test
cargo public-api -p druid -sss --all-features \
  > api/druid-public-api.rdbc-first.after.txt
diff -u api/druid-public-api.rdbc-first.before.txt \
  api/druid-public-api.rdbc-first.after.txt \
  > api/druid-public-api.rdbc-first.diff.txt || test "$?" -eq 1
python3 scripts/verify_rdbc_public_paths.py \
  --mapping docs/verification/rdbc-first/rdbc-public-path-map.tsv \
  --baseline api/druid-public-api.rdbc-first.before.txt \
  --after api/druid-public-api.rdbc-first.after.txt \
  --allowlist api/druid-public-api.rdbc-first.allowlist.txt \
  --api-diff api/druid-public-api.rdbc-first.diff.txt \
  --compile-all-pairs
```

Expected products: `druid-rdbc`, `druid`, `druid-wrapper`, `druid-metrics`, `druid-admin` plus
`druid-bench`; no forbidden reverse dependency; every moved object has one implementation; consumer
test PASS. The verifier derives the baseline moved-definition set, requires exact equality with the
mapping manifest, generates/compiles every module-qualified pair, rejects all removed/changed API
lines, and accepts only exact additions in the tracked allowlist.

- [ ] **Step 4: Run workspace gates**

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo test --workspace
cargo doc --workspace --all-features --no-deps
```

Expected: all PASS.

- [ ] **Step 5: Write the after snapshot/report and transitional handoff**

Record every moved file, old/new public path, before/after API hashes, dependency graph, unique
definition evidence, generated pair count, allowlist entries, tests, tool versions, and commands in
`rdbc-extraction.md`. State explicitly:

```text
P03 error ownership: TRANSITIONAL_UNTIL_P04
P04 mandatory prerequisite: Task 1 must remove the complete compat::DruidError definition,
switch all driver-facing SPI/Access signatures to RdbcError, and restore DruidError to druid.
```

Do not describe the P03 transitional error ownership as the accepted final ADR architecture.

- [ ] **Step 6: Commit the verified working-tree state**

```bash
git add Cargo.toml Cargo.lock crates docs api scripts
git diff --cached --check
git commit -m "refactor(rdbc): complete isolated contract extraction"
```

- [ ] **Step 7: Run the clean post-commit public API gate**

`cargo-public-api diff` performs temporary in-place commit checkouts according to its installed
help. Run it only with a clean checkout, never pass `--force`, and use the verified option order:

```bash
test -z "$(git status --porcelain)"
cargo public-api diff --deny=removed --deny=changed \
  -p druid -sss --all-features \
  75b5294eaa6ec886480bac4cec194a0abb485043..HEAD
```

Expected: PASS and restore the same branch/HEAD. This available gate is mandatory independently of
`cargo-semver-checks`. If it fails after the local commit, keep the commit recoverable, mark Task 6
incomplete, make a new corrective commit, and rerun; do not reset, amend, force, push, or describe
P03 as complete.

### Task 6: 冻结过渡态并交接 single-standard merge

**Files:**
- Modify: `docs/superpowers/plans/2026-08-21-druid-rdbc-extraction.md`（修正本 Task 的 verifier 参数）
- Modify: `.superpowers/sdd/2026-08-21-druid-rdbc-extraction/progress.md`
- Create: `docs/verification/rdbc-first/rdbc-transitional-standardization-handoff.md`
- Create: `docs/verification/rdbc-first/rdbc-physical-standardization.tsv`
- Create: `api/druid-rdbc-spi.transitional.txt`
- Modify: `docs/verification/rdbc-first/status.md`
- Modify: `crates/druid-rdbc/src/lib.rs`
- Test: `crates/druid/tests/rdbc_external_consumer_test.rs`

**Interfaces:**
- Consumes: Tasks 1–5 current HEAD, 285-row old/new path denominator, wrapper direct-import
  manifest and transitional `compat::DruidError`.
- Produces: recovery status `TRANSITIONAL_PHYSICAL_SURFACE_PENDING_STANDARDIZATION`、every public
  `druid_rdbc::spi/resource-runtime` item 的 merge/internalize/move/delete disposition、P04
  single-standard input。
- Does not produce: final standard API、Physical alias deletion、semver compatibility claim。

- [ ] **Step 1: Freeze current evidence without installing tools**

Run:

```bash
git status --short --branch
git rev-parse HEAD
python3 scripts/verify_rdbc_dependency_boundary.py
python3 scripts/verify_rdbc_public_paths.py --source-only
python3 scripts/verify_wrapper_rdbc_import_boundary.py \
  --before docs/verification/rdbc-first/wrapper-rdbc-import-boundary.before.tsv \
  --after docs/verification/rdbc-first/wrapper-rdbc-import-boundary.after.tsv
cargo check --workspace --all-targets --all-features
cargo test -p druid --test rdbc_external_consumer_test
```

Expected: current transitional state compiles and existing source/path/import gates pass. Do not
install `cargo-semver-checks`; unavailable semver tooling is recorded `NOT_RUN`, not PASS。

- [ ] **Step 2: Capture transitional public surface**

```bash
RUSTUP_TOOLCHAIN=nightly-2026-08-07-aarch64-apple-darwin \
  cargo public-api --manifest-path crates/druid-rdbc/Cargo.toml -sss --all-features \
  > api/druid-rdbc-spi.transitional.txt
test -s api/druid-rdbc-spi.transitional.txt
```

`rdbc-physical-standardization.tsv` uses exact columns：

```text
old_path<TAB>classification<TAB>target_path<TAB>compatibility<TAB>source_file<TAB>normalized_signature<TAB>evidence
```

Every current `druid_rdbc::spi::*` and runtime `resource::{Rdbc*Access,Context,Factory,Owner,
State,Id,Kind,Capabilities}` public item has exactly one row。`classification` is
`MERGE_STANDARD`、`RDBC_EXTENSION`、`DRUID_INTERNAL` or `CONCRETE_IMPL`。No
`UNKNOWN`/empty/duplicate row。

- [ ] **Step 3: Write executable P04 handoff**

`rdbc-transitional-standardization-handoff.md` records：

- exact HEAD and API snapshot SHA-256；
- Tasks 1–5 commits and review state；
- the three parked P03 verification issues；
- transitional `DruidError` waiver；
- current wrapper import denominator；
- the standardization inventory row count/hash；
- mandatory next plan
  `docs/superpowers/plans/2026-08-24-rdbc-single-standard-unification.md` Task 1。

It must state exactly：

```text
P03 status: TRANSITIONAL_PHYSICAL_SURFACE_PENDING_STANDARDIZATION
P03 is not the accepted final ADR-RDBC-006 architecture.
P04 Task 1 must exit the DruidError waiver before standard trait unification.
```

- [ ] **Step 4: Update docs without changing production ownership**

Update `druid-rdbc/src/lib.rs` crate docs to label `Physical*` and runtime resource access as
standardization inputs until P04。Do not rename traits、move files、change manifests or modify
behavior in P03。

- [ ] **Step 5: Verify the recovery handoff**

```bash
git diff --check
test -s docs/verification/rdbc-first/rdbc-transitional-standardization-handoff.md
test -s docs/verification/rdbc-first/rdbc-physical-standardization.tsv
rg -n 'TRANSITIONAL_PHYSICAL_SURFACE_PENDING_STANDARDIZATION' \
  docs/verification/rdbc-first/rdbc-transitional-standardization-handoff.md \
  docs/verification/rdbc-first/status.md crates/druid-rdbc/src/lib.rs
```

Expected: all checks PASS and no production `.rs` file except crate docs changed。

- [ ] **Step 6: Commit**

The SDD `progress.md` is deliberately Git-ignored controller state and is not staged；verify its
Ruling/status lines before staging, then stage only tracked recovery evidence。

```bash
rg -n 'Task 6 Ruling|TRANSITIONAL_PHYSICAL_SURFACE_PENDING_STANDARDIZATION' \
  .superpowers/sdd/2026-08-21-druid-rdbc-extraction/progress.md
git add api/druid-rdbc-spi.transitional.txt \
  docs/superpowers/plans/2026-08-21-druid-rdbc-extraction.md \
  docs/verification/rdbc-first/rdbc-transitional-standardization-handoff.md \
  docs/verification/rdbc-first/rdbc-physical-standardization.tsv \
  docs/verification/rdbc-first/status.md crates/druid-rdbc/src/lib.rs
git diff --cached --check
git commit -m "docs(rdbc): freeze transitional standardization handoff"
```
