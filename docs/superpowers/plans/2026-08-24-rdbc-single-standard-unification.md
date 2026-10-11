# druid-rdbc Single Standard Trait Tree Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 建立唯一 `druid-rdbc` JDBC-like standard trait tree，让所有 wrapper integrations 与 Druid pooled proxies 直接实现它，并在完成行为保持迁移后删除全部 `Physical*` canonical names。

**Architecture:** 先退出 transitional `DruidError` waiver并冻结 Java/Physical/adapter 分母；新增纯 standard traits 后，wrapper 与 Druid proxies 分阶段切入同一 standard。raw/logical 差异迁入 holder/lease policy，Druid-only cache/resource runtime 回到内部。最后删除 `Physical*` aliases/impls，强化统一 resource/capability/async ResultSet，并用 differential/real-DB/SemVer 收口。

**Tech Stack:** Rust 2021、async-trait、Tokio、thiserror、bitflags、OpenJDK 8 inventory、cargo-public-api、cargo-semver-checks、SQLx/Toasty/RBDC/bb8/deadpool

**Spec:** `docs/superpowers/specs/2026-08-24-rdbc-single-standard-trait-tree.md`

## Global Constraints

- Java oracle：OpenJDK `jdk8u502-b07` / `7373384725418fa645fb4508ec05a869bc7fc3e7` / SHA-256 `5f9c0657c534319a272751431a1f5919cfdcd4e0ac4e0fb9b7e181bc27fa489b`。
- Java Druid：`1.2.28` / `33824c3dec1612711f9bb4e409319bcab2e4cd0e`。
- MSRV `1.95`、default `1.97.1`、Edition `2021`、`unsafe_code=forbid`。
- 最终只有一套 canonical standard traits；`Physical*` merge 完成后删除。
- Canonical object traits include `Connection`、`Statement`、`PreparedStatement`、
  `CallableStatement`、`ResultSet`、`DatabaseMetaData`、`ResultSetMetaData`、
  `ParameterMetaData` and standard resources；Rust acquisition extensions include
  `ConnectionFactory` and `DriverRegistry`。
- 所有 wrapper adapters 与 Druid pooled proxies 直接实现 standard traits。
- raw/logical 用 holder/lease/close policy 表达；禁止新建 Raw/Physical marker trait。
- Java 52 + 17 objects 是 semantic denominator，不是必须创建 69 个 Rust public types。
- 每个实际迁移的 Java object 一个 `.rs`；`mod.rs` 只声明/定向重导出；生产禁止 wildcard import。
- `druid-rdbc <- druid <- druid-wrapper` normal DAG 无反边。
- 禁止 Git worktree、branch switch、reset/amend/force/push/publish。
- `cargo-semver-checks` 缺失时仅阻塞最终 SemVer step；不得静默安装或伪报 PASS。
- Entry state is P03 `TRANSITIONAL_PHYSICAL_SURFACE_PENDING_STANDARDIZATION` with exact
  Physical/resource/adapter denominator。

---

### Task 1: 强制退出 transitional `DruidError` waiver

**Files:**
- Create: `crates/druid-rdbc/src/error/batch_update_counts.rs`
- Create: `crates/druid-rdbc/src/error/rdbc_error_kind.rs`
- Modify: `crates/druid-rdbc/src/error/rdbc_error.rs`
- Modify: `crates/druid-rdbc/src/error/sql_exception.rs`
- Modify: `crates/druid-rdbc/src/error/mod.rs`
- Delete: `crates/druid-rdbc/src/compat/druid_error.rs`
- Modify: `crates/druid-rdbc/src/compat/mod.rs`
- Modify: `crates/druid/src/core/error.rs`
- Modify: `crates/druid/src/core/mod.rs`
- Create: `docs/verification/rdbc-first/rdbc-error-disposition.tsv`
- Create: `docs/verification/rdbc-first/rdbc-error-waiver-exit.md`
- Create: `scripts/verify_rdbc_error_waiver_exit.py`
- Create: `crates/druid-rdbc/tests/error_contract_test.rs`
- Create: `crates/druid/tests/druid_error_transition_test.rs`
- Modify: every live production row emitted by `verify_rdbc_error_waiver_exit.py --emit-live`。

**Interfaces:**
- Consumes: P03 24-variant identity/golden observables。
- Produces: final `RdbcError` for standard/adapters；single product `DruidError` in Druid；six
  exact waiver gates。

- [ ] **Step 1: Generate live RED disposition**

```bash
python3 scripts/verify_rdbc_error_waiver_exit.py --emit-live \
  docs/verification/rdbc-first/rdbc-error-disposition.tsv
python3 scripts/verify_rdbc_error_waiver_exit.py \
  --disposition docs/verification/rdbc-first/rdbc-error-disposition.tsv
```

Expected: RED while compat mixed enum、DruidError signatures/constructions remain。

- [ ] **Step 2: Write RED fidelity tests**

```rust
let counts = BatchUpdateCounts::from_large(vec![1, i64::from(i32::MAX) + 7, -2, -3]);
assert_eq!(counts.large()[1], i64::from(i32::MAX) + 7);
assert_eq!(counts.jdbc_i32()[1], (i64::from(i32::MAX) + 7) as i32);
assert!(counts.try_i32().is_err());
```

Also assert SQLState/vendor/cause/next/warning、client-info failed properties、DataTruncation and
distinct SQL-feature/default unsupported taxonomy。

```bash
cargo test -p druid-rdbc --test error_contract_test
cargo test -p druid --test druid_error_transition_test
```

Expected: RED because `RdbcError` is scaffold and batch counts are `i32`。

- [ ] **Step 3: Implement error ownership**

All `druid-rdbc` and wrapper fallible APIs return `RdbcError`。`DruidError` lives only in Druid and
wraps RDBC errors once。`BatchUpdateCounts` stores `Vec<i64>` and exposes large、Java-cast and
checked views。

- [ ] **Step 4: Prove canonical gates**

```text
WAIVER_EXIT_GATE_1_COMPAT_REMOVED
WAIVER_EXIT_GATE_2_STANDARD_CLEAN
WAIVER_EXIT_GATE_3_ADAPTERS_CLEAN
WAIVER_EXIT_GATE_4_PRODUCT_OWNER_UNIQUE
WAIVER_EXIT_GATE_5_BASELINE_DISPOSED
WAIVER_EXIT_GATE_6_COMPAT_VERIFIED
```

Gate 5 对已删除的 RDBC 旧文件只接受完整 Task 6 迁移权威表验证后、同一旧源码对应的每个迁移行均为 `TASK6_CLOSED` 的情况。仍为 `TASK7_PENDING`、缺少权威映射的 wrapper 文件，以及尚未证明标准错误委托关系的存续文件继续失败。

```bash
python3 scripts/verify_rdbc_error_waiver_exit.py \
  --disposition docs/verification/rdbc-first/rdbc-error-disposition.tsv \
  --exit-report docs/verification/rdbc-first/rdbc-error-waiver-exit.md
cargo test -p druid-rdbc --test error_contract_test
cargo test -p druid --test druid_error_transition_test
cargo test -p druid --test exception_sorter_semantics_test
cargo test -p druid-wrapper --test adapter_boundary_test
cargo check --workspace --all-targets --all-features
```

Expected: all six named gates and commands PASS。

- [ ] **Step 5: Commit**

```bash
git add crates/druid-rdbc crates/druid crates/druid-wrapper scripts \
  docs/verification/rdbc-first/rdbc-error-*.tsv \
  docs/verification/rdbc-first/rdbc-error-waiver-exit.md
git diff --cached --check
git commit -m "refactor(rdbc): establish unified error contract"
```

### Task 2: 冻结 JDBC semantic 与现有 integration 分母

**Files:**
- Create: `docs/verification/rdbc-first/jdbc42-semantic-inventory.tsv`
- Modify: `docs/verification/rdbc-first/rdbc-physical-standardization.tsv`
- Create: `docs/verification/rdbc-first/rdbc-adapter-implementations.tsv`
- Create: `docs/verification/rdbc-first/rdbc-production-consumers.tsv`
- Create: `docs/verification/rdbc-first/jdbc42-oracle.json`
- Create: `scripts/generate_jdbc42_semantic_inventory.py`
- Create: `scripts/verify_jdbc42_semantic_inventory.py`
- Create: `scripts/test_verify_jdbc42_semantic_inventory.py`
- Create: `scripts/verify_rdbc_standardization_denominator.py`
- Create: `scripts/test_verify_rdbc_standardization_denominator.py`

**Interfaces:**
- Produces: 52 + 17 semantic rows；every current `spi/resource` public item classified
  `MERGE_STANDARD/RDBC_EXTENSION/DRUID_INTERNAL/CONCRETE_IMPL`；every wrapper and Druid impl
  bound to a target standard entry。

- [ ] **Step 1: Write fail-closed verifier self-tests**

Semantic TSV columns：

```text
item_key<TAB>java_fqcn<TAB>java_member<TAB>rust_shape<TAB>semantic_disposition<TAB>canonical_rust_entry<TAB>target_state<TAB>normalized_signature<TAB>call_kind<TAB>index_null_adapter<TAB>error_adapter<TAB>lifecycle_owner<TAB>adapter_implementation<TAB>adapter_state<TAB>logical_implementation<TAB>logical_state<TAB>evidence_level<TAB>capability_state<TAB>notes
```

Standardization TSV columns：

```text
old_path<TAB>classification<TAB>target_path<TAB>compatibility<TAB>source_file<TAB>normalized_signature<TAB>target_task
```

Production-consumer TSV columns：

```text
source_path<TAB>occurrence_kind<TAB>symbol<TAB>old_path<TAB>target_path<TAB>target_task<TAB>state
```

It inventories every production import、type、trait object、impl and call occurrence across
`druid-rdbc`、`druid` and `druid-wrapper`；fixed file counts are observations, not denominators。

Self-tests reject missing/duplicate/unknown、wrong 52/17 count、empty canonical entries、fake
implementations、unresolved old paths and oracle hash drift。

```bash
python3 scripts/test_verify_jdbc42_semantic_inventory.py
python3 scripts/test_verify_rdbc_standardization_denominator.py
python3 scripts/verify_jdbc42_semantic_inventory.py
```

Expected: self-tests PASS；live inventory RED before generation/disposition。

- [ ] **Step 2: Generate and classify exact live surface**

```bash
python3 scripts/generate_jdbc42_semantic_inventory.py \
  --archive-sha256 5f9c0657c534319a272751431a1f5919cfdcd4e0ac4e0fb9b7e181bc27fa489b \
  --semantic docs/verification/rdbc-first/jdbc42-semantic-inventory.tsv \
  --oracle docs/verification/rdbc-first/jdbc42-oracle.json
python3 scripts/verify_rdbc_standardization_denominator.py --emit-live \
  docs/verification/rdbc-first/rdbc-physical-standardization.tsv \
  docs/verification/rdbc-first/rdbc-adapter-implementations.tsv \
  docs/verification/rdbc-first/rdbc-production-consumers.tsv
```

Populate every row。`rust_shape` is `TRAIT/DTO/ENUM_KIND/MODULE/CALLBACK/EXCLUDED_RUNTIME`；
no requirement creates one Rust type per Java object。Task 2 requires unique
`target_state=PLANNED_TARGET` and allows adapter/logical `PLANNED` with `-` implementation paths。

- [ ] **Step 3: Run GREEN denominator**

```bash
python3 scripts/verify_jdbc42_semantic_inventory.py
python3 scripts/verify_rdbc_standardization_denominator.py
```

Expected: `JAVA_SQL=52`、`JAVAX_SQL=17`、`UNKNOWN=0`、all current rows have one planned target；
live implementation resolution is not claimed before Tasks 4/5。

- [ ] **Step 4: Commit**

```bash
git add docs/verification/rdbc-first/jdbc42-* \
  docs/verification/rdbc-first/rdbc-physical-standardization.tsv \
  docs/verification/rdbc-first/rdbc-adapter-implementations.tsv \
  docs/verification/rdbc-first/rdbc-production-consumers.tsv scripts
git diff --cached --check
git commit -m "test(rdbc): freeze single-standard migration denominator"
```

### Task 3: 定义独立、纯净的 standard traits

**Files:**
- Modify: `crates/druid-rdbc/src/lib.rs`
- Create: `crates/druid-rdbc/src/connection/mod.rs`
- Create: `crates/druid-rdbc/src/connection/connection.rs`
- Create: `crates/druid-rdbc/src/statement/mod.rs`
- Create: `crates/druid-rdbc/src/statement/statement.rs`
- Create: `crates/druid-rdbc/src/statement/prepared_statement.rs`
- Create: `crates/druid-rdbc/src/statement/callable_statement.rs`
- Create: `crates/druid-rdbc/src/result_set/mod.rs`
- Create: `crates/druid-rdbc/src/result_set/result_set.rs`
- Create: `crates/druid-rdbc/src/metadata/mod.rs`
- Create: `crates/druid-rdbc/src/metadata/database_meta_data.rs`
- Create: `crates/druid-rdbc/src/metadata/result_set_meta_data.rs`
- Create: `crates/druid-rdbc/src/metadata/parameter_meta_data.rs`
- Create: `crates/druid-rdbc/src/driver/mod.rs`
- Create: `crates/druid-rdbc/src/driver/driver.rs`
- Create: `crates/druid-rdbc/src/driver/connection_factory.rs`
- Move: `crates/druid/src/spi/driver_extension_descriptor.rs` → `crates/druid-rdbc/src/driver/driver_extension_descriptor.rs`
- Move/Split: `crates/druid/src/spi/driver_extension_registry.rs` → `crates/druid-rdbc/src/driver/driver_registry.rs`
- Create: `crates/druid-rdbc/src/data_source/mod.rs`
- Create: `crates/druid-rdbc/src/data_source/common_data_source.rs`
- Create: `crates/druid-rdbc/src/data_source/data_source.rs`
- Create: `crates/druid-rdbc/src/data_source/pooled_connection.rs`
- Create: `crates/druid-rdbc/src/data_source/connection_pool_data_source.rs`
- Create: `crates/druid-rdbc/src/data_source/xa_connection.rs`
- Create: `crates/druid-rdbc/src/data_source/xa_data_source.rs`
- Create: `crates/druid-rdbc/src/wrapper/mod.rs`
- Create: `crates/druid-rdbc/src/wrapper/wrapper.rs`
- Create: `crates/druid-rdbc/src/wrapper/standard_unwrapped.rs`
- Create: `crates/druid-rdbc/src/extensions/mod.rs`
- Create: `crates/druid-rdbc/src/extensions/connection_capabilities.rs`
- Split: standard vs concrete/resource-runtime rows named by Task 2 manifests。
- Create: `scripts/verify_rdbc_single_standard_purity.py`
- Create: `scripts/test_verify_rdbc_single_standard_purity.py`
- Create: `crates/druid-rdbc/tests/standard_external_implementor_test.rs`
- Create: `crates/druid-rdbc/tests/standard_surface_test.rs`
- Create: `crates/druid-rdbc/tests/standard_handle_ownership_compile_test.rs`
- Create: `crates/druid-rdbc/tests/standard_metadata_ownership_compile_test.rs`

**Interfaces:**
- Consumes: Task 2 exact targets。
- Produces: unique standard traits independent of Druid；old `Physical*` remains explicitly
  transitional and non-canonical until Task 6；Druid-only rows remain outside standard modules。

- [ ] **Step 1: Write purity and external implementor RED tests**

Purity rejects standard modules containing Druid/pool/filter/holder/lease/cache/resource-owner
types。External temp crate implements standard traits without `druid`。Ownership compile test uses
a mock actor handle and proves Statement/ResultSet outlive the create/query call without borrowing
the caller connection。

```bash
python3 scripts/test_verify_rdbc_single_standard_purity.py
python3 scripts/verify_rdbc_single_standard_purity.py --phase scaffold
cargo test -p druid-rdbc --test standard_external_implementor_test
cargo test -p druid-rdbc --test standard_handle_ownership_compile_test
cargo test -p druid-rdbc --test standard_metadata_ownership_compile_test
```

Expected: self-test PASS；live RED until standard modules exist。

- [ ] **Step 2: Define Rust JDBC-like signatures**

Use JDBC names and semantics, Rust `Result/Option/async` shapes and typed descriptors。Core object
traits are object-safe；generic helpers live in `*Ext`。Example canonical creation：

```rust
async fn create_statement(
    &mut self,
    options: StatementOptions,
) -> Result<Box<dyn Statement>, RdbcError>;

async fn next(&mut self) -> Result<bool, RdbcError>;
```

`ConnectionFactory` returns `Box<dyn Connection>`。`DriverRegistry` stores standard drivers/
factories and performs no Druid pool policy。The async `ResultSet::next` signature is final here；
Task 7 adds Stream adaptation/runtime hardening without changing the canonical signature。
Statement methods use `&mut self` and return `Box<dyn ResultSet>`；each Statement owns a cloneable
executor/native handle。Freeze the exact Send/Sync/'static matrix from the spec in external compile
fixtures。

Metadata compile fixture proves `database_meta_data()` returns owned `Box<dyn DatabaseMetaData>`；
live metadata queries return `Box<dyn ResultSet>`；ResultSetMetaData/ParameterMetaData are
`Send + Sync + 'static` snapshots returned through owned `Arc` handles。

- [ ] **Step 3: Freeze the temporary dual-contract migration boundary**

Do not alias the new standard trait to old `Physical*` while their method sets differ。Record every
standard↔Physical signature/default mapping in the Task 2 TSV。Old Physical remains deprecated
migration input；new code and documentation use only standard names。This temporary coexistence ends
at Task 6 and is never described as two canonical standards。

- [ ] **Step 4: Run standard/purity/API gates**

```bash
python3 scripts/verify_jdbc42_semantic_inventory.py --require-standard-entries
python3 scripts/verify_rdbc_single_standard_purity.py --phase transitional
cargo test -p druid-rdbc --test standard_surface_test
cargo test -p druid-rdbc --test standard_external_implementor_test
cargo test -p druid-rdbc --test standard_handle_ownership_compile_test
cargo test -p druid-rdbc --test standard_metadata_ownership_compile_test
cargo doc -p druid-rdbc --no-deps
```

Expected: standard surface PASS；standard traits are independent/object-safe；Druid-only rows are
absent from canonical modules；all temporary Physical rows have one target standard mapping。

- [ ] **Step 5: Commit**

```bash
git add crates/druid-rdbc scripts/verify_rdbc_single_standard_purity.py \
  scripts/test_verify_rdbc_single_standard_purity.py \
  docs/verification/rdbc-first/jdbc42-semantic-inventory.tsv \
  docs/verification/rdbc-first/rdbc-physical-standardization.tsv
git diff --cached --check
git commit -m "feat(rdbc): define unified JDBC-like standard traits"
```

### Task 4: 让所有 wrapper integrations 直接实现 standard

**Files:**
- Modify: every wrapper production row in `rdbc-adapter-implementations.tsv`。
- Modify: every `target_task=4` wrapper row in `rdbc-production-consumers.tsv`。
- Modify: `crates/druid-wrapper/src/prepared_parameter_materializer.rs`
- Modify: `crates/druid-wrapper/src/prepared_parameter_state.rs`
- Create: `crates/druid-wrapper/src/sqlx/sqlx_connection_actor.rs`
- Modify: `crates/druid-wrapper/src/sqlx/sqlx_connection_adapter.rs`
- Modify: `crates/druid-wrapper/src/sqlx/sqlx_prepared_statement.rs`
- Modify: `crates/druid-wrapper/src/sqlx/sqlx_streaming_result_set.rs`
- Modify: `crates/druid-wrapper/src/sqlx/sqlx_database_meta_data.rs`
- Modify: `crates/druid-wrapper/tests/common/physical_connection_contract.rs`
- Modify: `crates/druid-wrapper/tests/adapter_boundary_test.rs`
- Modify: `crates/druid-wrapper/tests/sqlx_connection_adapter_test.rs`
- Modify: `crates/druid-wrapper/tests/rbdc_connection_adapter_test.rs`
- Modify: `crates/druid-wrapper/tests/toasty_connection_adapter_test.rs`
- Modify: `crates/druid-wrapper/tests/duckdb_connection_adapter_test.rs`
- Create: `scripts/verify_wrapper_standard_impls.py`
- Create: `scripts/test_verify_wrapper_standard_impls.py`
- Create: `crates/druid-wrapper/tests/standard_adapter_conformance_test.rs`
- Create: `crates/druid-wrapper/tests/statement_executor_ownership_test.rs`
- Create: `crates/druid-wrapper/tests/metadata_actor_ownership_test.rs`
- Modify: `docs/verification/rdbc-first/jdbc42-semantic-inventory.tsv`
- Modify: `docs/verification/rdbc-first/rdbc-production-consumers.tsv`

**Interfaces:**
- Consumes: Task 3 standard traits plus transitional Physical traits。
- Produces: SQLx/Toasty/RBDC/DuckDB/libSQL/HTTP/JDBC Agent factories、connections、statements、
  result sets、metadata、resources directly implementing canonical standard traits。

- [ ] **Step 1: Write direct-impl RED gate**

Verifier parses impl blocks and rejects canonical adapter families that implement only
`Physical*`。It requires both temporary impls to delegate to the same inherent/private adapter core
or generated helper；duplicated SQL execution/default/error logic fails。

The ownership test creates a Statement, drops the create-call borrow, executes through the
statement-owned actor, receives `Box<dyn ResultSet>`, and advances it mutably。It must also prove the
connection/lease remains owned until Statement/ResultSet close。

Metadata ownership test drops the `database_meta_data` call borrow, executes a live metadata query,
advances its owned ResultSet, and separately proves ResultSetMetaData/ParameterMetaData snapshots
remain immutable and connection-borrow-free。

```bash
python3 scripts/test_verify_wrapper_standard_impls.py
python3 scripts/verify_wrapper_standard_impls.py
python3 scripts/verify_jdbc42_semantic_inventory.py --phase adapter-live
cargo test -p druid-wrapper --test standard_adapter_conformance_test
cargo test -p druid-wrapper --test statement_executor_ownership_test
cargo test -p druid-wrapper --test metadata_actor_ownership_test
```

Expected: RED while canonical standard impls are absent。

- [ ] **Step 2: Add standard impls over one implementation core**

Add canonical standard impls while retaining old Physical impls only for Druid Task 5 migration。
Both delegate to one inherent/private adapter core；do not copy execution/default/error bodies。
Standard `ConnectionFactory` returns `Box<dyn Connection>`；the transitional factory continues only
until holder cutover。SQLx Statement/PreparedStatement owns `Arc<SqlxConnectionActor>` or an
equivalent owned executor；no returned standard object borrows `&mut SqlxConnectionAdapter`。
SQLx live DatabaseMetaData uses the same actor ownership；metadata ResultSet owns its query lease，
while ResultSetMetaData/ParameterMetaData are immutable snapshots。

- [ ] **Step 3: Run configured adapter matrix**

```bash
python3 scripts/verify_wrapper_standard_impls.py
cargo test -p druid-wrapper --test standard_adapter_conformance_test
cargo test -p druid-wrapper --test statement_executor_ownership_test
cargo test -p druid-wrapper --test metadata_actor_ownership_test
cargo test -p druid-wrapper --test adapter_boundary_test
cargo test -p druid-wrapper --test sqlx_connection_adapter_test
cargo test -p druid-wrapper --test rbdc_connection_adapter_test
cargo test -p druid-wrapper --test toasty_connection_adapter_test
cargo test -p druid-wrapper --tests
cargo check -p druid-wrapper --all-features
cargo test -p druid-wrapper --all-features --tests
```

Expected: all configured tests PASS；unconfigured drivers explicit `NOT_RUN`。

- [ ] **Step 4: Commit**

```bash
git add crates/druid-wrapper scripts/verify_wrapper_standard_impls.py \
  scripts/test_verify_wrapper_standard_impls.py \
  docs/verification/rdbc-first/rdbc-adapter-implementations.tsv \
  docs/verification/rdbc-first/rdbc-production-consumers.tsv \
  docs/verification/rdbc-first/jdbc42-semantic-inventory.tsv
git diff --cached --check
git commit -m "refactor(wrapper): implement unified rdbc standard"
```

### Task 5: 让 Druid pooled proxies 与 holder 使用同一 standard

**Files:**
- Modify: `crates/druid/src/core/druid_connection_holder.rs`
- Modify: `crates/druid/src/core/druid_pooled_connection.rs`
- Modify: `crates/druid/src/core/druid_pooled_statement.rs`
- Modify: `crates/druid/src/core/druid_pooled_prepared_statement.rs`
- Modify: `crates/druid/src/core/druid_pooled_callable_statement.rs`
- Modify: `crates/druid/src/core/druid_pooled_result_set.rs`
- Modify: `crates/druid/src/core/database_meta_data_proxy_impl.rs`
- Modify: `crates/druid/src/core/mod.rs`
- Modify: `crates/druid/src/pool/pool_inner.rs`
- Modify: `crates/druid/src/pool/druid_pool.rs`
- Modify: `crates/druid/src/core/physical_connection_lease.rs`
- Move: `PhysicalConnectionInfo` to Druid create/holder internal。
- Move: `PreparedStatementKey/PreparedStatementMethodType` to Druid cache internal。
- Move: resource context/factory/owner/id/kind/state/runtime capability to Druid internal。
- Move: Druid Wrapper proxy hooks and MySQL metadata flag to Druid internal。
- Modify: every `target_task=5` Druid row in `rdbc-production-consumers.tsv`。
- Create: `crates/druid/tests/single_standard_logical_contract_test.rs`
- Create: `crates/druid/tests/raw_logical_ownership_test.rs`
- Create: `crates/druid/tests/standard_metadata_ownership_test.rs`
- Create: `scripts/verify_druid_single_standard.py`
- Create: `scripts/test_verify_druid_single_standard.py`
- Modify: `docs/verification/rdbc-first/jdbc42-semantic-inventory.tsv`
- Modify: `docs/verification/rdbc-first/rdbc-production-consumers.tsv`

**Interfaces:**
- Consumes: standard wrapper implementations。
- Produces: `DruidConnectionHolder.connection: Box<dyn Connection>`；Druid pooled proxies implement
  same standard；raw/logical lifecycle encoded only in owner policy；Druid-only runtime removed from
  rdbc public surface。

- [ ] **Step 1: Write ownership/lifecycle RED tests**

Tests prove：

```text
factory output enters holder as standard Connection
logical close recycles exactly once
eviction/shutdown invokes raw Connection::close exactly once
external lease never enters native pool
reset uses standard rollback/auto-commit/read-only/catalog/schema/warnings
prepared cache stores standard PreparedStatement
```

```bash
python3 scripts/test_verify_druid_single_standard.py
python3 scripts/verify_druid_single_standard.py
python3 scripts/verify_jdbc42_semantic_inventory.py --phase logical-live
cargo test -p druid --test single_standard_logical_contract_test
cargo test -p druid --test raw_logical_ownership_test
cargo test -p druid --test standard_metadata_ownership_test
```

Expected: RED on old Physical holder/pooled impl and rdbc-owned runtime。

- [ ] **Step 2: Switch holder/proxies and internalize Druid policy**

Use private owner strategy enum `NativePool | ExternalLease`；do not create public Raw/Physical
marker。Pooled proxies implement standard methods and preserve filter/stat/recycle semantics。
DatabaseMetaDataProxy owns a logical lease/actor；metadata snapshots never borrow it。

- [ ] **Step 3: Run GREEN Druid gates**

```bash
python3 scripts/verify_druid_single_standard.py
cargo test -p druid --test single_standard_logical_contract_test
cargo test -p druid --test raw_logical_ownership_test
cargo test -p druid --test standard_metadata_ownership_test
cargo test -p druid --test recycle_semantics_test
cargo test -p druid --test prepared_statement_cache_test
cargo test -p druid --test rdbc_external_consumer_test
cargo check --workspace --all-targets --all-features
```

Expected: all PASS；no Druid production import requires `Physical*` except temporary compatibility
module scheduled for Task 6 deletion。

- [ ] **Step 4: Commit**

```bash
git add crates/druid crates/druid-rdbc \
  scripts/verify_druid_single_standard.py scripts/test_verify_druid_single_standard.py \
  docs/verification/rdbc-first/rdbc-physical-standardization.tsv \
  docs/verification/rdbc-first/rdbc-production-consumers.tsv \
  docs/verification/rdbc-first/jdbc42-semantic-inventory.tsv
git diff --cached --check
git commit -m "refactor(druid): use unified rdbc standard in pool and proxies"
```

### Task 6: 删除 `Physical*` names 并完成 standard purity

**Files:**
- Delete: `crates/druid-rdbc/src/spi/mod.rs` and remaining implementation files after row disposition。
- Modify: `crates/druid-rdbc/src/lib.rs`
- Modify: `crates/druid-rdbc/src/resource/mod.rs`
- Modify: `crates/druid-rdbc/src/compat/mod.rs`
- Modify: `crates/druid-rdbc/Cargo.toml`
- Modify: `crates/druid/src/core/mod.rs`
- Modify: `crates/druid/src/spi/mod.rs`
- Modify: `crates/druid-wrapper/Cargo.toml`
- Modify: `Cargo.lock`
- Create: `api/druid-rdbc.single-standard.after.txt`
- Create: `docs/rdbc/physical-to-standard-migration.md`
- Create: `scripts/verify_no_physical_standard.py`
- Create: `scripts/test_verify_no_physical_standard.py`
- Create: `crates/druid-rdbc/tests/no_physical_public_surface_test.rs`
- Modify: every remaining `target_task=6` row in `rdbc-production-consumers.tsv`。
- Modify: `docs/verification/rdbc-first/rdbc-production-consumers.tsv`

**Interfaces:**
- Consumes: all wrapper/Druid standard implementations and Task 2 dispositions。
- Produces: zero public/production `Physical*` names；no `spi` module；final standard purity；exact
  approved breaking migration table。

- [ ] **Step 1: Write final deletion RED gate**

Verifier rejects `Physical[A-Z]` in public API、production imports/impls and rustdoc，and requires
every old path in migration TSV to be `MERGED_STANDARD/INTERNALIZED/CONCRETE_MOVED/DELETED`。

```bash
python3 scripts/test_verify_no_physical_standard.py
python3 scripts/verify_no_physical_standard.py
cargo test -p druid-rdbc --test no_physical_public_surface_test
```

Expected: RED while aliases/spi/compat paths exist。

- [ ] **Step 2: Delete aliases and clean dependencies**

Remove all `Physical*` names after confirming standard impls resolve。Remove Tokio/parking_lot or other
dependencies used only by Druid runtime。Do not leave a private duplicate trait tree。

- [ ] **Step 3: Run purity/API/DAG gates**

```bash
python3 scripts/verify_no_physical_standard.py
python3 scripts/verify_rdbc_single_standard_purity.py --phase final
python3 scripts/verify_wrapper_standard_impls.py
python3 scripts/verify_druid_single_standard.py
python3 scripts/verify_rdbc_standardization_denominator.py --require-consumers-final
python3 scripts/verify_rdbc_dependency_boundary.py
cargo tree -p druid-rdbc
cargo tree --invert -p druid-rdbc
cargo check --workspace --all-targets --all-features
cargo test -p druid-wrapper --tests
cargo public-api -p druid-rdbc -sss --all-features \
  > api/druid-rdbc.single-standard.after.txt
```

Expected: all PASS；API snapshot contains standard names and zero `Physical*`。

- [ ] **Step 4: Commit**

```bash
git add Cargo.lock crates api docs/rdbc/physical-to-standard-migration.md scripts \
  docs/verification/rdbc-first/rdbc-production-consumers.tsv
git diff --cached --check
git commit -m "refactor(rdbc): remove physical compatibility surface"
```

### Task 7: 统一 resource、capability 与 async ResultSet

**Files:**
- Modify: standard resource trait files named by semantic inventory。
- Modify: `crates/druid-rdbc/src/result_set/result_set.rs`
- Completed early in Task 6 for required behavior preservation:
  `crates/druid-rdbc/src/result_set/result_set_ext.rs`
- Completed early in Task 6 for required behavior preservation:
  `crates/druid-rdbc/src/extensions/buffered_row_set.rs`
- Completed early in Task 6 for value-level ResultSet/RowSet descriptors only:
  `crates/druid-rdbc/src/types/resource_handle.rs`
- Modify: `crates/druid-rdbc/src/extensions/connection_capabilities.rs`
- Modify: Druid internal resource owner/release files from Task 5。
- Modify: wrapper resource/result-set implementations from Task 4。
- Rename: `crates/druid-wrapper/src/sqlx/sqlx_streaming_result_set.rs` → `crates/druid-wrapper/src/sqlx/sqlx_buffered_result_set.rs`
- Create: `crates/druid-wrapper/src/sqlx/sqlx_async_result_set.rs`
- Modify: `crates/druid-wrapper/src/sqlx/mod.rs`
- Create: `crates/druid-rdbc/tests/resource_standard_contract_test.rs`
- Create: `crates/druid/tests/resource_owner_cancellation_test.rs`
- Create: `crates/druid-wrapper/tests/async_result_set_test.rs`

**Interfaces:**
- Produces: one standard resource contract；one standard ResultSet；optional Stream adapter only；
  truthful capability；cancel-safe exactly-once Druid owner cleanup。

### Task 7 bounded pull-forward denominator

The owner-approved Task 6 fix round pulls forward only the three production components needed to
preserve the retired ResultSet/RowSet behavior. This is a sequencing change; it does not reduce Task 7 requirements.
The value-level handle aliases do not close `ResourceOperations`, `ResourceHandleFactory`, free/
lifecycle semantics or owner cleanup.

| component | state | exact identity | source or remaining obligation |
| --- | --- | --- | --- |
| `BUFFERED_ROW_SET` | `COMPLETED_EARLY_TASK6` | `druid_rdbc::extensions::BufferedRowSet` | `crates/druid-rdbc/src/extensions/buffered_row_set.rs` |
| `RESULT_SET_EXT` | `COMPLETED_EARLY_TASK6` | `druid_rdbc::result_set::ResultSetExt` | `crates/druid-rdbc/src/result_set/result_set_ext.rs` |
| `VALUE_LEVEL_RESOURCE_HANDLES` | `COMPLETED_EARLY_TASK6` | `druid_rdbc::types::resource_handle aliases` | `crates/druid-rdbc/src/types/resource_handle.rs` |
| `RESOURCE_OPERATIONS_AND_FACTORY` | `IMPLEMENTED_TESTED` | `ResourceOperations` + `ResourceHandleFactory` | `resource_standard_contract_test` 31/31；标准 operations/factory 源码已核对 |
| `RESOURCE_TRAIT_LIFECYCLE` | `IMPLEMENTED_TESTED` | standard resource traits | 同一 31/31 契约覆盖 1-based、free-after-use、SQLXML 消耗/释放状态 |
| `RDBC_RESULT_SET_FORWARDING` | `IMPLEMENTED_TESTED` | `SharedResultSet` | `task7_shared_result_set_contract_test` 6/6；共享转发身份独立于 `BufferedRowSet` |
| `CAPABILITY_TRUTHFULNESS` | `TASK7_PENDING` | `ConnectionCapabilities` | JDBC Agent callable 误报已修复，H2 实库覆盖已声明的 catalog/schema/savepoint 行为；仍需完整跨后端 capability=true 矩阵与发布门禁证据 |
| `DRUID_OWNER_RELEASE_EXACTLY_ONCE` | `IMPLEMENTED_TESTED` | Druid resource owner/release | `resource_owner_cancellation_test` 112/112；driver release、重复通知与父租约处置合同 |
| `DRUID_CANCELLATION_HARDENING` | `IMPLEMENTED_TESTED` | Druid supervised release | 同一 112/112 合同覆盖取消、重试与父连接清理 |
| `SQLX_BUFFERED_RENAME` | `IMPLEMENTED_TESTED` | `SqlxBufferedResultSet` | 编译模块与 Task 4 live 清单均指向真实缓冲实现；旧 Streaming 文件已退休 |
| `ASYNC_RESULT_SET_STREAMING` | `IMPLEMENTED_TESTED` | `SqlxAsyncResultSet` | `async_result_set_test` 15/15；容量 1 请求队列、按 next 拉取与提前释放同一 actor |

Task 7 remaining implementation denominator: **1**. The three completed-early Task 6 rows and
the seven `IMPLEMENTED_TESTED` Task 7 rows do not count as remaining implementation work; Task 7
itself stays open until all eight obligations and the full GREEN matrix are verified. The
completed-early rows remain part of Task 6 behavior-preservation evidence.

- [ ] **Step 1: Write RED shared tests**

Cover 1-based resource position、free-after-use、SQLXML state、capability=true behavior、caller
cancellation、driver release vs owner notification、bounded SQLx first-row consumption without
false streaming name。

```bash
cargo test -p druid-rdbc --test resource_standard_contract_test
cargo test -p druid --test resource_owner_cancellation_test
cargo test -p druid-wrapper --test async_result_set_test
```

Expected: RED on access/context split、cancellation risk and eager `fetch_all` naming。

- [ ] **Step 2: Implement unified behavior**

Adapter resources and Druid proxies both implement standard resource traits。Druid release task is
supervised and exactly-once。`ResultSet` is the only canonical cursor；`ResultSetExt` adapts it to
Stream without a second driver port。`BufferedRowSet` is explicit eager extension。

- [ ] **Step 3: Run GREEN matrices**

```bash
cargo test -p druid-rdbc --test resource_standard_contract_test
cargo test -p druid --test resource_owner_cancellation_test
cargo test -p druid-wrapper --test async_result_set_test
cargo test -p druid-wrapper --test standard_adapter_conformance_test
cargo test -p druid-wrapper --tests
```

Expected: all configured tests PASS。

- [ ] **Step 4: Commit**

```bash
git add crates/druid-rdbc crates/druid crates/druid-wrapper
git diff --cached --check
git commit -m "feat(rdbc): unify resource capability and async result contracts"
```

### Task 8: 完成 differential、SemVer、真实数据库和文档门禁

**Files:**
- Create: `docs/rdbc/standard-contract-v1.md`
- Create: `docs/rdbc/integration-author-guide.md`
- Create: `docs/verification/rdbc-first/rdbc-single-standard-unification.md`
- Modify: `docs/verification/rdbc-first/status.md`
- Modify: `docs/druid-rust-Architecture.zh_CN.md`
- Modify: `README.md`
- Modify: `README.zh-CN.md`
- Create: `api/rdbc-single-standard-breaking.allowlist.tsv`
- Create: `api/druid-rdbc.semver-checks.txt`
- Create: `api/druid.semver-checks.txt`
- Create: `scripts/verify_rdbc_single_standard_breaking_api.py`
- Modify: `crates/druid-bench/benches/adapter.rs`
- Create: `crates/druid-bench/tests/java_rust_jdbc42_differential.rs`

**Interfaces:**
- Produces: requirement-by-requirement ADR-RDBC-006 completion audit and honest residual matrix。

- [ ] **Step 1: Run final standard/DAG/quality gates**

```bash
python3 scripts/verify_jdbc42_semantic_inventory.py --require-final-evidence
python3 scripts/verify_no_physical_standard.py
python3 scripts/verify_rdbc_single_standard_purity.py --phase final
python3 scripts/verify_wrapper_standard_impls.py
python3 scripts/verify_druid_single_standard.py
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo doc --workspace --all-features --no-deps
```

Expected: all PASS。

- [ ] **Step 2: Run API/SemVer breaking allowlist**

```bash
cargo public-api -p druid-rdbc -sss --all-features > api/druid-rdbc.single-standard.final.txt
cargo public-api -p druid -sss --all-features > api/druid.single-standard.final.txt
cargo semver-checks --version
set +e
cargo semver-checks --package druid-rdbc \
  --baseline-rev 2c421efa6caf8bfc983633529fc3775c251f9492 \
  --all-features --verbose > api/druid-rdbc.semver-checks.txt 2>&1
rdbc_semver_status=$?
cargo semver-checks --package druid \
  --baseline-rev 2c421efa6caf8bfc983633529fc3775c251f9492 \
  --all-features --verbose > api/druid.semver-checks.txt 2>&1
druid_semver_status=$?
set -e
python3 scripts/verify_rdbc_single_standard_breaking_api.py \
  --allowlist api/rdbc-single-standard-breaking.allowlist.tsv \
  --rdbc-report api/druid-rdbc.semver-checks.txt --rdbc-exit-code "$rdbc_semver_status" \
  --druid-report api/druid.semver-checks.txt --druid-exit-code "$druid_semver_status"
```

If the tool is absent, STOP this step and request installation authorization。Approved pre-release
breaks must exactly equal allowlist rows；extra/missing findings fail。

- [ ] **Step 3: Run differential/real DB/benchmark**

```bash
cargo test -p druid-bench --test java_rust_jdbc42_differential -- --nocapture
cargo test -p druid-wrapper --test database_profile_live_contract_test -- --nocapture
cargo bench -p druid-bench --bench adapter -- --noplot
```

Configured evidence is recorded；missing credentials remain `NOT_RUN` and cannot produce true
capability certification。

- [ ] **Step 4: Write completion audit and Commit**

Every spec gate is `PROVED/CONTRADICTED/INCOMPLETE/NOT_VERIFIED` with artifact path。

```bash
git add README.md README.zh-CN.md crates/druid-bench docs api scripts
git diff --cached --check
git commit -m "docs(rdbc): record single-standard unification evidence"
```
