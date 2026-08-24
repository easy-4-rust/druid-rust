# JDBC 4.2 Standard Contract and Druid Bridge Realignment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 `druid-rdbc` 收敛为 Java 8 JDBC 4.2 标准合同，把完整 Physical hierarchy 与 resource runtime 原子迁回 public `druid::bridge::physical`，让 Druid logical/raw facades 实现标准合同、wrapper 实现完整 Physical hierarchy。

**Architecture:** 先强制退出 transitional `DruidError` waiver，再冻结 52 + 17 标准分母；随后建立纯标准 modules、增加 `druid::bridge::physical` 完整 hierarchy seam、切换 wrapper imports，并在一个 SCC owner swap 中移除 `druid_rdbc::spi/Physical*`。Druid Adapter pattern 一对一映射 standard/Physical，最后强化 capability/lifecycle/cursor，并以 differential/real-DB/public-API 门禁收口。

**Tech Stack:** Rust 2021、async-trait、Tokio、thiserror、bitflags、cargo-public-api、cargo-semver-checks、OpenJDK 8 source inventory、SQLx/Toasty/RBDC/bb8/deadpool

**Spec:** `docs/superpowers/specs/2026-08-24-jdbc42-standard-bridge-layering.md`

## Global Constraints

- Java source oracle 固定 OpenJDK `jdk8u502-b07` / commit `7373384725418fa645fb4508ec05a869bc7fc3e7` / archive SHA-256 `5f9c0657c534319a272751431a1f5919cfdcd4e0ac4e0fb9b7e181bc27fa489b`。
- Java Druid 固定 `1.2.28` / `33824c3dec1612711f9bb4e409319bcab2e4cd0e`。
- Rust MSRV `1.95`，default `1.97.1`，Edition `2021`，`unsafe_code=forbid`。
- `standard purity`：`druid-rdbc` 最终只拥有 JDBC standard contract；禁止 `spi`、
  `Physical*`、Druid/pool/filter/lease/owner runtime。
- `druid::bridge::physical` 是唯一 adapter-author canonical path；必须保留完整
  `PhysicalConnection`、`PhysicalStatement`、`PhysicalPreparedStatement`、
  `PhysicalCallableStatement`、`PhysicalResultSet`、`PhysicalDatabaseMetaData`、
  `PhysicalResultSetMetaData`、`PhysicalParameterMetaData`、`PhysicalResource` hierarchy；
  `druid-wrapper` 实现该 hierarchy。
- `DruidPooledConnection`/`RawConnectionFacade` 等实现 standard traits，禁止实现 Physical traits。
- `druid-rdbc <- druid <- druid-wrapper` normal DAG 不得产生反边。
- 每个 Java 对象一个 `.rs`；`mod.rs` 仅声明和定向重导出；生产代码禁止 wildcard import。
- P03 transitional public path 不承诺零 breaking：所有删除/identity change 必须有 API diff、external implementor 和 migration disposition。
- 禁止 Git worktree、自动切换分支、reset/amend/force/push/publish。
- `cargo-semver-checks` 缺失时只阻塞需要它的 closeout step；不得静默安装或伪报 PASS。

---

### Task 1: 强制退出 `compat::DruidError` waiver

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
- Modify: every production file emitted by `scripts/verify_rdbc_error_waiver_exit.py --emit-live`.

**Interfaces:**
- Consumes: P03 24-variant transitional identity and
  `TRANSITIONAL_BRIDGE_OWNER_MISPLACED_UNTIL_P04` handoff。
- Produces: `RdbcError` for every standard/bridge/adapter fallible boundary；single product
  `druid::core::DruidError`；exact `WAIVER_EXIT_GATE_1..6` report。

- [ ] **Step 1: Freeze live RED denominator**

`verify_rdbc_error_waiver_exit.py --emit-live` scans `crates/druid-rdbc/src`、
`crates/druid/src` and `crates/druid-wrapper/src` with a Rust-aware tokenizer and writes one row per
fallible signature/import/fully-qualified construction：

```text
source_path<TAB>symbol<TAB>current_error<TAB>target_error<TAB>owner<TAB>disposition
```

Run：

```bash
python3 scripts/verify_rdbc_error_waiver_exit.py --emit-live \
  docs/verification/rdbc-first/rdbc-error-disposition.tsv
python3 scripts/verify_rdbc_error_waiver_exit.py \
  --disposition docs/verification/rdbc-first/rdbc-error-disposition.tsv
```

Expected: second command FAILS on all six final gates while compat mixed enum and DruidError
signatures remain。

- [ ] **Step 2: Write RED error fidelity tests**

Tests must construct：

```rust
let counts = BatchUpdateCounts::from_large(vec![1, i64::from(i32::MAX) + 7, -2, -3]);
assert_eq!(counts.large(), &[1, i64::from(i32::MAX) + 7, -2, -3]);
assert_eq!(counts.jdbc_i32()[1], (i64::from(i32::MAX) + 7) as i32);
assert!(counts.try_i32().is_err());
```

Also assert SQLState/vendor/cause/next/warning、`SQLClientInfoException.failed_properties`、
DataTruncation payload、`SQL_FEATURE_UNSUPPORTED_0A000` and
`JAVA_DEFAULT_UNSUPPORTED_TYPED` are distinct。

```bash
cargo test -p druid-rdbc --test error_contract_test
cargo test -p druid --test druid_error_transition_test
```

Expected: RED because `RdbcError` is still scaffold and counts are `i32`。

- [ ] **Step 3: Implement final error ownership**

`BatchUpdateCounts` owns `Vec<i64>` and exposes `large()`、Java-compatible `jdbc_i32()` and
checked `try_i32()`。`RdbcError` is non-exhaustive and includes typed SQL、resource、argument、
driver、discard、batch、validation、feature-unsupported、default-unsupported and other cases。
`DruidError` is defined only in `druid` and wraps `RdbcError` once at product boundary。

- [ ] **Step 4: Prove the six canonical gates**

The verifier reports exactly：

```text
WAIVER_EXIT_GATE_1_COMPAT_REMOVED=PASS
WAIVER_EXIT_GATE_2_STANDARD_CLEAN=PASS
WAIVER_EXIT_GATE_3_BRIDGE_ADAPTERS_CLEAN=PASS
WAIVER_EXIT_GATE_4_PRODUCT_OWNER_UNIQUE=PASS
WAIVER_EXIT_GATE_5_BASELINE_DISPOSED=PASS
WAIVER_EXIT_GATE_6_COMPAT_VERIFIED=PASS
```

Run：

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

Expected: all PASS；Task 2 remains blocked unless all six gate IDs are present and PASS。

- [ ] **Step 5: Commit**

```bash
git add crates/druid-rdbc crates/druid crates/druid-wrapper scripts \
  docs/verification/rdbc-first/rdbc-error-disposition.tsv \
  docs/verification/rdbc-first/rdbc-error-waiver-exit.md
git diff --cached --check
git commit -m "refactor(rdbc): separate standard and product errors"
```

### Task 2: 冻结 69 对象 JDBC standard inventory

**Files:**
- Create: `docs/verification/rdbc-first/jdbc42-standard-inventory.tsv`
- Create: `docs/verification/rdbc-first/jdbc42-oracle.json`
- Create: `scripts/generate_jdbc42_inventory.py`
- Create: `scripts/verify_jdbc42_inventory.py`
- Create: `scripts/test_verify_jdbc42_inventory.py`
- Create: `crates/druid-rdbc/tests/jdbc_type_code_test.rs`
- Create: `crates/druid-rdbc/tests/jdbc_default_behavior_test.rs`

**Interfaces:**
- Consumes: OpenJDK oracle and final `RdbcError`。
- Produces: unique 52 `java.sql` + 17 `javax.sql` object denominator、all member/default/
  overload rows、standard/implementation ownership and target entry points。

- [ ] **Step 1: Write fail-closed inventory verifier RED tests**

TSV columns are exact：

```text
item_key<TAB>java_fqcn<TAB>java_member<TAB>standard_owner<TAB>implementation_owner<TAB>bridge_requirement<TAB>semantic_disposition<TAB>evidence_level<TAB>capability_state<TAB>rust_entry_point<TAB>physical_entry_point<TAB>standard_signature<TAB>physical_signature<TAB>call_kind<TAB>index_adapter<TAB>error_adapter<TAB>return_adapter<TAB>lifecycle_owner<TAB>logical_adapter<TAB>wrapper_implementation<TAB>notes
```

Self-tests reject missing/duplicate key、unknown enum、wrong object counts、empty entry point、
Rust extension using fake Java key and mismatched oracle hash。

```bash
python3 scripts/test_verify_jdbc42_inventory.py
python3 scripts/verify_jdbc42_inventory.py
```

Expected: self-test PASS；live verifier RED because inventory is absent/empty。

- [ ] **Step 2: Generate oracle rows**

```bash
python3 scripts/generate_jdbc42_inventory.py \
  --archive-sha256 5f9c0657c534319a272751431a1f5919cfdcd4e0ac4e0fb9b7e181bc27fa489b \
  --output docs/verification/rdbc-first/jdbc42-standard-inventory.tsv \
  --oracle docs/verification/rdbc-first/jdbc42-oracle.json
```

Every Java item starts `semantic_disposition=UNKNOWN` and the verifier must remain RED until each
row has an approved standard owner/implementation owner/entry point or Java-runtime exclusion。

- [ ] **Step 3: Fill exact disposition and type/default fixtures**

`standard_owner` is `DRUID_RDBC_STANDARD` or `JAVA_RUNTIME_ONLY`；implementation owner is
`DRUID_LOGICAL`、`DRUID_BRIDGE`、`WRAPPER_ADAPTER` or `NOT_APPLICABLE`。Populate all rows and add
fixtures for 39 type codes、Statement/ResultSet/metadata constants and Java 8 default behavior。
`bridge_requirement` is `REQUIRED_PHYSICAL`、`LOGICAL_ONLY`、`JAVA_RUNTIME_ONLY` or
`RUST_EXTENSION`；every required row ultimately names one standard method、physical method、
normalized signatures、sync/async call kind、index/error/return adapters、lifecycle owner、
logical adapter and wrapper implementation。Task 2 may use `-` only for fields whose owning task is
explicitly later；phase-specific verifiers reject `-` when that task completes。

- [ ] **Step 4: Run GREEN**

```bash
python3 scripts/verify_jdbc42_inventory.py
cargo test -p druid-rdbc --test jdbc_type_code_test
cargo test -p druid-rdbc --test jdbc_default_behavior_test
```

Expected: `JAVA_SQL_OBJECTS=52`、`JAVAX_SQL_OBJECTS=17`、`UNKNOWN=0` and tests PASS。

- [ ] **Step 5: Commit**

```bash
git add docs/verification/rdbc-first/jdbc42-* scripts/generate_jdbc42_inventory.py \
  scripts/verify_jdbc42_inventory.py scripts/test_verify_jdbc42_inventory.py \
  crates/druid-rdbc/tests/jdbc_type_code_test.rs \
  crates/druid-rdbc/tests/jdbc_default_behavior_test.rs
git diff --cached --check
git commit -m "test(rdbc): freeze JDBC 4.2 standard inventory"
```

### Task 3: 建立纯 JDBC standard modules 并拆分污染类型

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
- Create: `crates/druid-rdbc/src/data_source/mod.rs`
- Create: `crates/druid-rdbc/src/data_source/common_data_source.rs`
- Create: `crates/druid-rdbc/src/data_source/data_source.rs`
- Create: `crates/druid-rdbc/src/data_source/pooled_connection.rs`
- Create: `crates/druid-rdbc/src/data_source/connection_pool_data_source.rs`
- Create: `crates/druid-rdbc/src/data_source/xa_connection.rs`
- Create: `crates/druid-rdbc/src/data_source/xa_data_source.rs`
- Create: `crates/druid-rdbc/src/driver/mod.rs`
- Create: `crates/druid-rdbc/src/driver/driver.rs`
- Create: `crates/druid-rdbc/src/driver/driver_action.rs`
- Create: `crates/druid-rdbc/src/driver/driver_property_info.rs`
- Create: `crates/druid-rdbc/src/wrapper/mod.rs`
- Create: `crates/druid-rdbc/src/wrapper/wrapper.rs`
- Create: `crates/druid-rdbc/src/wrapper/standard_unwrapped.rs`
- Move: every inventory row whose `standard_owner=DRUID_RDBC_STANDARD` and current defining crate is
  not `druid-rdbc` into the exact one-object file named by its `rust_entry_point`。
- Split: `crates/druid-rdbc/src/spi/result_set_meta_data.rs`
- Split: `crates/druid-rdbc/src/spi/wrapper.rs`
- Split: `crates/druid-rdbc/src/resource/array.rs`
- Split: `crates/druid-rdbc/src/resource/blob.rs`
- Split: `crates/druid-rdbc/src/resource/clob.rs`
- Split: `crates/druid-rdbc/src/resource/n_clob.rs`
- Split: `crates/druid-rdbc/src/resource/ref_value.rs`
- Split: `crates/druid-rdbc/src/resource/sql_xml.rs`
- Create: `scripts/verify_rdbc_standard_purity.py`
- Create: `scripts/test_verify_rdbc_standard_purity.py`
- Create: `crates/druid-rdbc/tests/standard_surface_test.rs`
- Create: `crates/druid-rdbc/tests/standard_external_implementor_test.rs`

**Interfaces:**
- Consumes: Task 2 inventory。
- Produces: standard `Connection/Statement/PreparedStatement/CallableStatement/ResultSet/
  Metadata/Resource/DataSource/Driver/Wrapper` traits using only `druid-rdbc` types。
- Transitional `spi` may still exist until Task 5, but standard modules cannot import it。

- [ ] **Step 1: Write purity and external-implementor RED tests**

Purity rejects imports/paths matching：

```text
druid, Physical, Druid, Pool, Filter, Lease, Owner, RdbcResourceContext,
RdbcResourceFactory, async QueryCursor runtime
```

under standard module roots。External fixture implements minimal standard traits from a temp crate
without depending on `druid`。

```bash
python3 scripts/test_verify_rdbc_standard_purity.py
python3 scripts/verify_rdbc_standard_purity.py
cargo test -p druid-rdbc --test standard_external_implementor_test
```

Expected: self-test PASS；purity/implementor RED until modules exist and polluted types split。

- [ ] **Step 2: Define standard trait roots**

Use open, object-safe traits for runtime objects；generic helpers live in `*Ext`。All methods return
`RdbcError` and use Task 2 DTO/newtypes。Standard `Wrapper` signatures are：

```rust
fn is_wrapper_for(&self, iface: Option<TypeId>) -> Result<bool, RdbcError>;
fn unwrap(&self, iface: Option<TypeId>) -> Result<StandardUnwrapped<'_>, RdbcError>;
```

For all 52 + 17 standard objects, create or move exactly one object into the inventory-declared
file。Compatibility aliases may re-export the same type but cannot define a second object。

- [ ] **Step 3: Split metadata/resource/wrapper**

Standard snapshot/traits remain in standard modules。Physical backend、Druid MySQL8 flag、
Access/Context handle、physical unwrap stay transitional under old modules for Task 5 move；there is
no cross-reference from standard to transitional。

- [ ] **Step 4: Prove inventory coverage and purity**

```bash
python3 scripts/verify_jdbc42_inventory.py --require-entry-points
python3 scripts/verify_rdbc_standard_purity.py
cargo test -p druid-rdbc --test standard_surface_test
cargo test -p druid-rdbc --test standard_external_implementor_test
cargo doc -p druid-rdbc --no-deps
```

Expected: all PASS；rustdoc standard facade contains no `Physical*` or Druid owner type。

- [ ] **Step 5: Commit**

```bash
git add crates/druid-rdbc scripts/verify_rdbc_standard_purity.py \
  scripts/test_verify_rdbc_standard_purity.py \
  docs/verification/rdbc-first/jdbc42-standard-inventory.tsv
git diff --cached --check
git commit -m "feat(rdbc): define pure JDBC standard contract"
```

### Task 4: 建立 `druid::bridge::physical` 加法 seam 并切换 wrapper imports

**Files:**
- Create: `crates/druid/src/bridge/mod.rs`
- Create: `crates/druid/src/bridge/physical/mod.rs`
- Create: `crates/druid/src/bridge/physical/physical_connection.rs`
- Create: `crates/druid/src/bridge/physical/physical_connection_factory.rs`
- Create: `crates/druid/src/bridge/physical/physical_statement.rs`
- Create: `crates/druid/src/bridge/physical/physical_prepared_statement.rs`
- Create: `crates/druid/src/bridge/physical/physical_callable_statement.rs`
- Create: `crates/druid/src/bridge/physical/physical_result_set.rs`
- Create: `crates/druid/src/bridge/physical/physical_database_meta_data.rs`
- Create: `crates/druid/src/bridge/physical/physical_result_set_meta_data.rs`
- Create: `crates/druid/src/bridge/capability.rs`
- Create: `crates/druid/src/bridge/external_lease.rs`
- Create: `crates/druid/src/bridge/unwrap.rs`
- Modify: `crates/druid/src/lib.rs`
- Modify: `crates/druid/src/core/mod.rs`
- Modify: `crates/druid/src/spi/mod.rs`
- Create: `docs/verification/rdbc-first/druid-bridge-imports.before.tsv`
- Create: `docs/verification/rdbc-first/druid-bridge-imports.after.tsv`
- Create: `docs/verification/rdbc-first/jdbc42-standard-physical-declarations.tsv`
- Create: `scripts/verify_druid_bridge_import_boundary.py`
- Create: `scripts/test_verify_druid_bridge_import_boundary.py`
- Create: `scripts/verify_jdbc42_physical_declarations.py`
- Create: `scripts/test_verify_jdbc42_physical_declarations.py`
- Modify: every wrapper production file in the tracked before/after import manifests。
- Create: `crates/druid/tests/bridge_type_identity_test.rs`

**Interfaces:**
- Consumes: transitional Physical/resource-runtime definitions still owned by rdbc。
- Produces: canonical `druid::bridge::physical::*` seam for the seven existing Physical traits as
  exact directional re-exports；wrapper imports only physical bridge paths；existing type identity
  remains unchanged。Missing `PhysicalParameterMetaData`/`PhysicalResource` and new Prepared/
  Callable supertrait relations are recorded `PENDING_TASK5`, not falsely declared complete。

- [ ] **Step 1: Generate live wrapper import RED manifests**

Manifest columns：

```text
source_path<TAB>family<TAB>old_import<TAB>new_import<TAB>symbol
```

```bash
python3 scripts/verify_druid_bridge_import_boundary.py --emit-before \
  docs/verification/rdbc-first/druid-bridge-imports.before.tsv
python3 scripts/verify_druid_bridge_import_boundary.py --self-test
python3 scripts/test_verify_druid_bridge_import_boundary.py
python3 scripts/verify_druid_bridge_import_boundary.py
python3 scripts/test_verify_jdbc42_physical_declarations.py
python3 scripts/verify_jdbc42_physical_declarations.py --phase seam
```

`jdbc42-standard-physical-declarations.tsv` columns are：

```text
item_key<TAB>standard_trait<TAB>standard_method<TAB>standard_signature<TAB>physical_trait<TAB>physical_method<TAB>physical_signature<TAB>call_kind<TAB>index_adapter<TAB>error_adapter<TAB>return_adapter<TAB>lifecycle_owner<TAB>phase_disposition
```

Expected: self-tests PASS；live gates RED while wrapper imports `druid_rdbc::spi` or any
existing Physical declaration lacks an exact row。In `--phase seam`, only the two new traits and
supertrait changes may be `PENDING_TASK5`; every other `REQUIRED_PHYSICAL` row must resolve live。

- [ ] **Step 2: Add exact bridge re-export seam**

Each bridge file contains declarations/re-exports only；no copied implementation。
`druid::bridge::physical` is public and exposes the seven existing Physical traits。
`druid::core::Physical*` and `druid::spi::*` target the same bridge identity。Physical traits do
not inherit standard traits。This task does not invent missing traits or change supertraits。

- [ ] **Step 3: Cut wrapper imports only**

Replace every manifest row old import with `druid::bridge::physical`。Do not move trait definitions or change
method bodies in this task。

- [ ] **Step 4: Prove additive identity**

```bash
python3 scripts/verify_druid_bridge_import_boundary.py --emit-after \
  docs/verification/rdbc-first/druid-bridge-imports.after.tsv
python3 scripts/verify_druid_bridge_import_boundary.py
python3 scripts/verify_jdbc42_physical_declarations.py --phase seam
cargo test -p druid --test bridge_type_identity_test
cargo check --workspace --all-targets --all-features
cargo test -p druid-wrapper --test adapter_boundary_test
```

Expected: exact live-manifest equality、old/new TypeId/trait object identity and all tests PASS。

- [ ] **Step 5: Commit**

```bash
git add crates/druid/src/bridge crates/druid/src/lib.rs crates/druid/src/core/mod.rs \
  crates/druid/src/spi/mod.rs crates/druid-wrapper scripts/verify_druid_bridge_import_boundary.py \
  scripts/test_verify_druid_bridge_import_boundary.py \
  scripts/verify_jdbc42_physical_declarations.py \
  scripts/test_verify_jdbc42_physical_declarations.py \
  docs/verification/rdbc-first/druid-bridge-imports.*.tsv \
  docs/verification/rdbc-first/jdbc42-standard-physical-declarations.tsv \
  crates/druid/tests/bridge_type_identity_test.rs
git diff --cached --check
git commit -m "refactor(druid): establish adapter bridge seam"
```

### Task 5: 原子迁移 Physical/Resource runtime SCC owner

**Files:**
- Move: every `DRUID_BRIDGE` or `SPLIT_REQUIRED` row from
  `docs/verification/rdbc-first/rdbc-bridge-breaking-inventory.tsv` into `crates/druid/src/bridge`。
- Modify: `crates/druid-rdbc/src/lib.rs`
- Delete: `crates/druid-rdbc/src/spi/mod.rs` and all remaining bridge-only files under `spi/`。
- Modify: `crates/druid-rdbc/src/resource/mod.rs`
- Modify: `crates/druid-rdbc/Cargo.toml`
- Modify: `crates/druid/Cargo.toml`
- Modify: `crates/druid-wrapper/Cargo.toml`
- Modify: `Cargo.lock`
- Modify: every Druid/wrapper source row in bridge/import manifests。
- Modify: `crates/druid/src/bridge/physical/mod.rs`
- Create: `crates/druid/src/bridge/physical/physical_parameter_meta_data.rs`
- Create: `crates/druid/src/bridge/physical/physical_resource.rs`
- Create: `api/druid-rdbc.standard.after.txt`
- Create: `api/druid.bridge.after.txt`
- Create: `docs/rdbc/bridge-migration.md`
- Create: `scripts/verify_bridge_owner.py`
- Create: `scripts/test_verify_bridge_owner.py`
- Create: `crates/druid/tests/logical_not_bridge_compile_test.rs`
- Create: `crates/druid/tests/physical_external_implementor_test.rs`
- Modify: `docs/verification/rdbc-first/jdbc42-standard-physical-declarations.tsv`

**Interfaces:**
- Consumes: Task 3 pure standard modules、Task 4 import seam、P03 breaking inventory。
- Produces: trait definitions physically owned by `druid::bridge::physical`；no rdbc SPI/runtime owner；
  wrapper impl targets unchanged complete Physical path；new PhysicalParameterMetaData/
  PhysicalResource declarations；Prepared/Callable physical supertrait relations；
  logical-not-Physical compile failure。

- [ ] **Step 1: Write owner RED gate**

`verify_bridge_owner.py` rejects：

```text
pub mod spi
Physical[A-Z]
RdbcResource(Context|Factory|Owner|State|Id|Kind|Capabilities)
Rdbc(Array|Blob|Clob|NClob|Ref|SqlXml)Access
```

under `druid-rdbc` and requires the tracked target row under `druid::bridge::physical`。It also rejects any
wrapper `impl` whose trait path does not resolve through bridge, and fails if any required member of
the complete Physical hierarchy is absent。

```bash
python3 scripts/test_verify_bridge_owner.py
python3 scripts/verify_bridge_owner.py
cargo test -p druid --test physical_external_implementor_test
```

Expected: self-test PASS；owner gate RED before swap；external implementor RED until complete
hierarchy、object safety、Prepared/Callable supertraits and factory return types exist。

- [ ] **Step 2: Move the complete SCC atomically**

Move definitions、Druid holder/lease/filter consumers、wrapper impls、resource runtime and
manifests in one task。Do not leave a temporary `druid-rdbc -> druid` dependency or copy a second
definition。Standard resource traits stay in rdbc；Druid concrete handles implement them。
Create `PhysicalParameterMetaData` and `PhysicalResource` as new public bridge traits；change
`PhysicalPreparedStatement: PhysicalStatement` and
`PhysicalCallableStatement: PhysicalPreparedStatement` in the same breaking task。Mark every
declaration row `OWNED_TASK5` and remove all `PENDING_TASK5`。

- [ ] **Step 3: Remove pooled-to-bridge impl and transitional paths**

Delete `impl PhysicalConnection for DruidPooledConnection`。Keep logical
forwarding as inherent/private helper until Task 6 standard trait impl。`druid_rdbc::spi::*` is
removed and documented breaking；`druid::core/spi` compatibility paths re-export bridge only。

- [ ] **Step 4: Run owner/DAG/API gates**

```bash
python3 scripts/verify_bridge_owner.py
python3 scripts/verify_rdbc_standard_purity.py
python3 scripts/verify_druid_bridge_import_boundary.py
python3 scripts/verify_jdbc42_physical_declarations.py --phase owner
python3 scripts/verify_rdbc_dependency_boundary.py
cargo tree -p druid-rdbc
cargo tree --invert -p druid-rdbc
cargo test -p druid --test logical_not_bridge_compile_test
cargo test -p druid --test physical_external_implementor_test
cargo check --workspace --all-targets --all-features
cargo test -p druid-wrapper --tests
cargo public-api -p druid-rdbc -sss --all-features > api/druid-rdbc.standard.after.txt
cargo public-api -p druid -sss --all-features > api/druid.bridge.after.txt
```

Expected: purity/owner/import/DAG PASS；compile test proves logical handle lacks bridge bound；
API snapshots and migration table enumerate every removed `druid_rdbc::spi` path。

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml Cargo.lock crates api docs/rdbc/bridge-migration.md \
  docs/verification/rdbc-first/jdbc42-standard-physical-declarations.tsv scripts
git diff --cached --check
git commit -m "refactor(rdbc): move driver bridge ownership to druid"
```

### Task 6: 让 Druid logical handles 实现 JDBC standard traits

**Files:**
- Modify: `crates/druid/src/core/druid_pooled_connection.rs`
- Modify: `crates/druid/src/core/druid_pooled_statement.rs`
- Modify: `crates/druid/src/core/druid_pooled_prepared_statement.rs`
- Modify: `crates/druid/src/core/druid_pooled_callable_statement.rs`
- Modify: `crates/druid/src/core/druid_pooled_result_set.rs`
- Modify: `crates/druid/src/core/database_meta_data_proxy_impl.rs`
- Modify: `crates/druid/src/core/mod.rs`
- Create: `crates/druid/src/core/raw_connection_facade.rs`
- Create: `crates/druid/src/core/raw_statement_facade.rs`
- Create: `crates/druid/src/core/raw_prepared_statement_facade.rs`
- Create: `crates/druid/src/core/raw_callable_statement_facade.rs`
- Create: `crates/druid/src/core/raw_result_set_facade.rs`
- Create: `crates/druid/src/core/raw_database_meta_data_facade.rs`
- Create: `crates/druid/src/core/raw_result_set_meta_data_facade.rs`
- Create: `crates/druid/src/core/raw_parameter_meta_data_facade.rs`
- Create: `crates/druid/src/core/raw_array_facade.rs`
- Create: `crates/druid/src/core/raw_blob_facade.rs`
- Create: `crates/druid/src/core/raw_clob_facade.rs`
- Create: `crates/druid/src/core/raw_n_clob_facade.rs`
- Create: `crates/druid/src/core/raw_ref_facade.rs`
- Create: `crates/druid/src/core/raw_sql_xml_facade.rs`
- Create: `crates/druid/src/core/raw_struct_facade.rs`
- Modify: `crates/druid/src/sql/connection.rs`
- Modify: `crates/druid/src/sql/statement.rs`
- Modify: `crates/druid/src/sql/prepared_statement.rs`
- Modify: `crates/druid/src/sql/callable_statement.rs`
- Modify: `crates/druid/src/sql/result_set.rs`
- Modify: `crates/druid/src/sql/database_meta_data.rs`
- Modify: `crates/druid/src/sql/data_source.rs`
- Create: `crates/druid/tests/jdbc_standard_logical_contract_test.rs`
- Create: `crates/druid/tests/jdbc_standard_external_consumer_test.rs`
- Create: `docs/verification/rdbc-first/jdbc42-physical-adapter-bindings.tsv`
- Create: `docs/verification/rdbc-first/jdbc42-physical-method-map.tsv`
- Create: `scripts/verify_jdbc42_physical_adapter_bindings.py`
- Create: `scripts/test_verify_jdbc42_physical_adapter_bindings.py`
- Create: `scripts/verify_jdbc42_physical_method_map.py`
- Create: `scripts/test_verify_jdbc42_physical_method_map.py`
- Modify: `docs/verification/rdbc-first/jdbc42-standard-inventory.tsv`

**Interfaces:**
- Consumes: Task 3 standard traits and Task 5 bridge raw handles。
- Produces: pooled logical and non-pooled `Raw*Facade` types implementing standard traits；each
  standard method delegates through the mapped full Physical hierarchy；single conversion
  `RdbcError -> DruidError` at product boundary；live adapter-binding matrix joined with Task 5
  declarations into the final physical method map。

- [ ] **Step 1: Write logical RED compile/behavior fixtures**

External temp crate compiles generic bounds：

```rust
fn connection<T: druid_rdbc::connection::Connection>(_: &mut T) {}
fn statement<T: druid_rdbc::statement::Statement>(_: &mut T) {}
fn result_set<T: druid_rdbc::result_set::ResultSet>(_: &mut T) {}
fn result_set_meta_data<T: druid_rdbc::metadata::ResultSetMetaData>(_: &T) {}
fn parameter_meta_data<T: druid_rdbc::metadata::ParameterMetaData>(_: &T) {}
fn blob<T: druid_rdbc::resource::Blob>(_: &mut T) {}
```

The test generator reads every `REQUIRED_PHYSICAL` row and emits pooled/raw facade compile
obligations；the hand-written anchors above are only smoke checks, not the denominator。Behavior
tests cover auto-commit、1-based index、`Value::Null`/`was_null`、statement current result、
close/recycle、metadata and resource lifecycle。

```bash
cargo test -p druid --test jdbc_standard_logical_contract_test
cargo test -p druid --test jdbc_standard_external_consumer_test
python3 scripts/test_verify_jdbc42_physical_adapter_bindings.py
python3 scripts/verify_jdbc42_physical_adapter_bindings.py
```

Expected: RED until standard trait impls exist。

- [ ] **Step 2: Implement standard traits by composition**

Trait methods delegate to existing inherent/filter-aware logical operations and then the mapped
Physical method。`Raw*Facade` provides the same standard contract without pooling。Do not implement
Physical traits on logical/raw facades；do not duplicate JDBC semantics between inherent and trait
methods。

`jdbc42-physical-adapter-bindings.tsv` columns are：

```text
item_key<TAB>logical_adapter<TAB>raw_facade<TAB>wrapper_implementation<TAB>binding_kind<TAB>source_evidence
```

The verifier parses live impl blocks and call expressions；nonempty strings without resolvable
symbols/calls fail。The final method-map verifier joins this table with Task 5 declarations and
validates name、normalized signature、sync/async、index/error/return adapters and lifecycle owner。

- [ ] **Step 3: Run standard and compatibility gates**

```bash
python3 scripts/verify_jdbc42_inventory.py --require-entry-points
python3 scripts/verify_jdbc42_physical_adapter_bindings.py
python3 scripts/verify_jdbc42_physical_method_map.py
cargo test -p druid --test jdbc_standard_logical_contract_test
cargo test -p druid --test jdbc_standard_external_consumer_test
cargo test -p druid --test rdbc_external_consumer_test
cargo test -p druid --test druid_pooled_result_set_semantics_test
```

Expected: all PASS；inventory marks logical standard entries at least `TESTED`。

- [ ] **Step 4: Commit**

```bash
git add crates/druid scripts/verify_jdbc42_physical_adapter_bindings.py \
  scripts/test_verify_jdbc42_physical_adapter_bindings.py \
  scripts/verify_jdbc42_physical_method_map.py \
  scripts/test_verify_jdbc42_physical_method_map.py \
  docs/verification/rdbc-first/jdbc42-standard-inventory.tsv \
  docs/verification/rdbc-first/jdbc42-physical-adapter-bindings.tsv \
  docs/verification/rdbc-first/jdbc42-physical-method-map.tsv
git diff --cached --check
git commit -m "feat(druid): implement JDBC standard contracts on logical handles"
```

### Task 7: 在 bridge owner 上强化 capability、lifecycle 和 cursor

**Files:**
- Modify: `crates/druid/src/bridge/capability.rs`
- Modify: `crates/druid/src/bridge/physical/physical_resource.rs`
- Modify: `crates/druid/src/bridge/physical/physical_result_set.rs`
- Create: `crates/druid/src/bridge/query_cursor.rs`
- Create: `crates/druid/src/bridge/buffered_row_set.rs`
- Modify: `crates/druid-wrapper/src/sqlx/sqlx_connection_adapter.rs`
- Rename: `crates/druid-wrapper/src/sqlx/sqlx_streaming_result_set.rs` → `crates/druid-wrapper/src/sqlx/sqlx_buffered_row_set.rs`
- Create: `crates/druid/tests/bridge_resource_cancellation_test.rs`
- Create: `crates/druid/tests/bridge_resource_owner_failure_test.rs`
- Create: `crates/druid-wrapper/tests/sqlx_cursor_test.rs`
- Modify: `crates/druid-wrapper/tests/common/physical_connection_contract.rs`
- Modify: `crates/druid-wrapper/tests/adapter_boundary_test.rs`
- Modify: `crates/druid-wrapper/tests/sqlx_connection_adapter_test.rs`
- Modify: `crates/druid-wrapper/tests/rbdc_connection_adapter_test.rs`
- Modify: `crates/druid-wrapper/tests/toasty_connection_adapter_test.rs`
- Modify: `crates/druid-wrapper/tests/duckdb_connection_adapter_test.rs`
- Modify: `crates/druid-wrapper/tests/jdbc_agent_h2_contract_test.rs`
- Modify: `crates/druid-wrapper/tests/jdbc_agent_hsqldb_contract_test.rs`

**Interfaces:**
- Consumes: final bridge owner and standard resource/metadata capability vocabulary。
- Produces: executable truthful capability、cancel-safe exactly-once release、explicit
  `BufferedRowSet`/`QueryCursor` bridge extension。

- [ ] **Step 1: Write RED capability/lifecycle/cursor tests**

Tests assert：capability=true implies behavior；caller abort during release cannot cancel driver
cleanup；driver released/owner failed does not double-free；owner notification uses 3 attempts with
10ms/50ms backoff and 1s deadline；SQLx cursor consumes 65/10,000 rows without `fetch_all`。

```bash
cargo test -p druid --test bridge_resource_cancellation_test
cargo test -p druid --test bridge_resource_owner_failure_test
cargo test -p druid-wrapper --test sqlx_cursor_test
```

Expected: RED on current runtime/eager SQLx implementation。

- [ ] **Step 2: Implement bridge runtime**

Release operation is owned by a supervised task, not caller future。Driver and owner phases have
separate states；unknown driver outcome requires reconciliation。`QueryCursor` owns its lease/actor
and never borrows `&mut PhysicalConnection` beyond return。`BufferedRowSet` is explicit eager type。

- [ ] **Step 3: Run all configured adapter matrices**

```bash
cargo test -p druid --test bridge_resource_cancellation_test
cargo test -p druid --test bridge_resource_owner_failure_test
cargo test -p druid-wrapper --test adapter_boundary_test
cargo test -p druid-wrapper --test sqlx_cursor_test
cargo test -p druid-wrapper --tests
```

Expected: all configured tests PASS；unconfigured real DB remains explicit `NOT_RUN`。

- [ ] **Step 4: Commit**

```bash
git add crates/druid crates/druid-wrapper
git diff --cached --check
git commit -m "feat(druid): harden adapter bridge runtime contracts"
```

### Task 8: 收口 differential、SemVer、真实数据库和文档

**Files:**
- Create: `docs/rdbc/standard-contract-v1.md`
- Create: `docs/rdbc/bridge-author-guide.md`
- Create: `docs/verification/rdbc-first/jdbc42-bridge-realignment.md`
- Modify: `docs/verification/rdbc-first/status.md`
- Modify: `docs/druid-rust-Architecture.zh_CN.md`
- Modify: `README.md`
- Modify: `README.zh-CN.md`
- Modify: `crates/druid-bench/benches/adapter.rs`
- Create: `crates/druid-bench/tests/java_rust_jdbc42_differential.rs`
- Create: `api/rdbc-bridge-breaking.allowlist.tsv`
- Create: `api/druid-rdbc.semver-checks.txt`
- Create: `api/druid.semver-checks.txt`
- Create: `scripts/verify_rdbc_bridge_breaking_api.py`

**Interfaces:**
- Consumes: Tasks 1–7 artifacts。
- Produces: exact completion audit for ADR-RDBC-005 and honest residual capability matrix。

- [ ] **Step 1: Run full quality/API gates**

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo doc --workspace --all-features --no-deps
python3 scripts/verify_jdbc42_inventory.py --require-entry-points
python3 scripts/verify_rdbc_standard_purity.py
python3 scripts/verify_bridge_owner.py
python3 scripts/verify_druid_bridge_import_boundary.py
python3 scripts/verify_jdbc42_physical_declarations.py --phase final
python3 scripts/verify_jdbc42_physical_adapter_bindings.py
python3 scripts/verify_jdbc42_physical_method_map.py
```

Expected: all PASS。

- [ ] **Step 2: Run public API and semver gates**

```bash
cargo public-api -p druid-rdbc -sss --all-features > api/druid-rdbc.standard.final.txt
cargo public-api -p druid -sss --all-features > api/druid.bridge.final.txt
cargo semver-checks --version
set +e
cargo semver-checks --package druid-rdbc \
  --baseline-rev 2912808f572180b72bae09617b80c3a04a9d75a9 \
  --all-features --verbose > api/druid-rdbc.semver-checks.txt 2>&1
rdbc_semver_status=$?
cargo semver-checks --package druid \
  --baseline-rev 2912808f572180b72bae09617b80c3a04a9d75a9 \
  --all-features --verbose > api/druid.semver-checks.txt 2>&1
druid_semver_status=$?
set -e
python3 scripts/verify_rdbc_bridge_breaking_api.py \
  --allowlist api/rdbc-bridge-breaking.allowlist.tsv \
  --rdbc-report api/druid-rdbc.semver-checks.txt \
  --rdbc-exit-code "$rdbc_semver_status" \
  --druid-report api/druid.semver-checks.txt \
  --druid-exit-code "$druid_semver_status"
```

If `cargo semver-checks` is absent, STOP this step and request installation authorization；do not
mark Task 8 complete。Approved breaking changes are expected to make raw semver-checks nonzero；
the verifier requires every finding to match one exact allowlist row and rejects missing、extra or
changed findings。A zero exit is accepted only when the corresponding allowlist is empty。

- [ ] **Step 3: Run differential and real-DB evidence**

```bash
cargo test -p druid-bench --test java_rust_jdbc42_differential -- --nocapture
cargo test -p druid-wrapper --test database_profile_live_contract_test -- --nocapture
cargo bench -p druid-bench --bench adapter -- --noplot
```

Every configured DB/version produces a report；missing credentials/config remain `NOT_RUN` and
force capability below `REAL_DB_VERIFIED`。

- [ ] **Step 4: Write completion report**

Report each spec completion condition as `PROVED`、`CONTRADICTED`、`INCOMPLETE` or
`NOT_VERIFIED` with exact command/output artifact。Do not infer TCK compliance or full RowSet。

- [ ] **Step 5: Commit**

```bash
git add README.md README.zh-CN.md crates/druid-bench docs api scripts
git diff --cached --check
git commit -m "docs(rdbc): record JDBC standard and bridge realignment evidence"
```
