# Driver Registry and Feature Boundaries Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 DriverManager、inventory extension 和 80 产品 registry 收敛为一条 canonical 建池路径，并让 Cargo feature 真正隔离 SQLx、Toasty、RBDC、外部池和管理工具依赖。

**Architecture:** `druid_rdbc::driver::DriverRegistry` 成为唯一 standard provider lookup；
`DruidDriverRegistry + DruidDatabasePoolBuilder` 在 Wrapper 中负责产品 profile 到 provider
的解析；Java-compatible DriverManager 使用同一 provider registry 的显式注册 overlay，
不让 `druid` 反向依赖 Wrapper。具体 driver dependencies 全部 optional，feature 名表示
实际编译能力而不是目录声明。

**Tech Stack:** Cargo features、inventory removal、async factory、cargo-hack、cargo tree

**Spec:** `docs/superpowers/specs/2026-08-24-rdbc-single-standard-trait-tree.md`

## Global Constraints

- 前置：P04 已完成 single-standard unification；wrapper/Druid 均实现 standard，
  `Physical*` 已删除。
- 80 产品 catalog 数量不是支持数量；只有 verified/certified 证据计入支持。
- Registry 不得在 Tokio runtime 内调用 `Handle::current().block_on`。
- 禁用 feature 时不得编译或链接对应依赖。
- DriverManager Java compatibility 行为保留，但不能形成第二条产品建池规则。

2026-10-13 当前验证：`verify_feature_dependency_boundaries.py` 通过；`druid-wrapper` 的 `--no-default-features`、单独 `sqlx`、`sqlx-bb8`、`sqlx-deadpool`、`toasty-sqlite`、`rbdc` 六组 `cargo check --locked` 均退出 0。`verify_package_ownership.py` 验证 6 个 workspace package、2 个 binary 及 CI 调用归属。单独 `--all-features` 本机检查在 DuckDB 原生 C++ 编译时主动停止，不能计作通过；P04 的标准/兼容性前置条件及 P05 完整关闭仍开放。

---

### Task 1: 冻结三条当前解析路径并选定 canonical path

**Files:**
- Create: `crates/druid-wrapper/tests/driver_resolution_path_test.rs`
- Create: `docs/verification/rdbc-first/driver-resolution-before.md`
- Modify: `docs/rdbc/contract-v1.md`

**Interfaces:**
- Produces: canonical `DriverRegistry` lookup、`DruidDriverRegistry::resolve` product
  mapping and compatibility delegation map

- [ ] **Step 1: 写路径 RED test**

For the same SQLite URL, resolve through DriverManager, inventory lookup and
`DruidDatabasePoolBuilder`; record driver name, URL, checker, sorter, factory type and errors. Assert
product profile mapping uses `DruidDatabasePoolBuilder`, while both product and compatibility paths
obtain factories from the same `DriverRegistry`.

- [ ] **Step 2: Run RED**

```bash
cargo test -p druid-wrapper --test driver_resolution_path_test -- --nocapture
```

Expected: FAIL because current paths use independent registries/factories.

- [ ] **Step 3: 记录当前差异**

Write exact observed differences and public callers to `driver-resolution-before.md`; do not change
code in this step.

- [ ] **Step 4: Commit RED evidence**

```bash
git add crates/druid-wrapper/tests/driver_resolution_path_test.rs \
  docs/verification/rdbc-first/driver-resolution-before.md docs/rdbc/contract-v1.md
git commit -m "test(driver): freeze registry resolution differences"
```

### Task 2: 移除同步 inventory factory 陷阱

**Files:**
- Modify/Delete: `crates/druid-rdbc/src/driver/driver_extension_descriptor.rs`
- Modify: `crates/druid-rdbc/src/driver/driver_registry.rs`
- Modify/Delete: `crates/druid-wrapper/src/driver/extensions.rs`
- Modify: `crates/druid-wrapper/src/driver/druid_driver_registry.rs`
- Modify: `crates/druid-wrapper/src/lib.rs`
- Test: `crates/druid-wrapper/tests/driver_extension_registry_test.rs`

**Interfaces:**
- Produces: explicit async-safe registry/provider installation

- [ ] **Step 1: 写 nested-runtime RED test**

Inside `#[tokio::test]`, execute the current registered Toasty factory and assert it returns a
Result without panic. Use `std::panic::AssertUnwindSafe` with
`futures::FutureExt::catch_unwind` only to prove the current failure.

- [ ] **Step 2: Run RED**

```bash
cargo test -p druid-wrapper --test driver_extension_registry_test \
  factory_is_async_runtime_safe -- --exact
```

Expected: FAIL/panic at `Handle::current().block_on`.

- [ ] **Step 3: 改为显式 provider registration**

Prefer deleting inventory and registering provider constructors in
`druid_rdbc::driver::DriverRegistry`; `DruidDriverRegistry::builtin` installs the Wrapper-owned
providers and profile mappings. A constructor only captures configuration synchronously; actual I/O
occurs in standard `ConnectionFactory::create().await`.

Delete only the old synchronous inventory registration path/descriptor when no compatibility row
requires it；the canonical `DriverRegistry` definition and file must remain。

If inventory remains for third-party linkage, use an async factory signature returning a boxed Send
future; no `block_on` is permitted.

- [ ] **Step 4: Run GREEN tests**

```bash
cargo test -p druid-wrapper --test driver_extension_registry_test
cargo test -p druid-wrapper --test driver_registry_test
cargo test -p druid-wrapper --test driver_resolution_path_test
```

Expected: all PASS and one canonical resolution path.

- [ ] **Step 5: Commit**

```bash
git add crates/druid-rdbc/src/driver crates/druid-wrapper
git commit -m "refactor(driver): unify async-safe provider registry"
```

### Task 3: 让 DriverManager 委托 canonical registry

**Files:**
- Modify: `crates/druid/src/sql/driver_manager.rs`
- Modify: `crates/druid-rdbc/src/compat/driver.rs`
- Modify: `crates/druid-rdbc/src/compat/driver_manager.rs`
- Test: `crates/druid/tests/rdbc_standard_test.rs`
- Test: `crates/druid-wrapper/tests/driver_resolution_path_test.rs`

**Interfaces:**
- Consumes: `druid_rdbc::driver::DriverRegistry`
- Produces: Java-compatible register/order/deregister overlay without duplicate provider metadata

- [ ] **Step 1: 写 reentrancy 和 default acceptance RED test**

Register a driver whose `accepts_url` re-enters registry read/write and assert resolution does not
deadlock. Register a minimal driver that does not override `accepts_url` and assert it does not claim
an arbitrary URL.

- [ ] **Step 2: Run RED with timeout**

```bash
cargo test -p druid --test rdbc_standard_test \
  driver_manager_reentrant_accepts_url -- --exact
```

Expected: current implementation times out/deadlocks or default-accepts the URL.

- [ ] **Step 3: 实现 snapshot-before-callback**

Clone the ordered driver Arc list while holding the registry lock, release the lock, then call
external `accepts_url`. Change the trait default to `false`. Product profile resolution and explicit
runtime registrations share `DriverRegistry`; the latter remain an ordered Java compatibility
overlay. `druid` must not add a dependency on `druid-wrapper`.

- [ ] **Step 4: Run GREEN**

```bash
cargo test -p druid --test rdbc_standard_test
cargo test -p druid-wrapper --test driver_resolution_path_test
```

Expected: PASS; no external callback executes under registry lock.

- [ ] **Step 5: Commit**

```bash
git add crates/druid crates/druid-rdbc crates/druid-wrapper/tests
git commit -m "fix(driver): make compatibility registry reentrant safe"
```

### Task 4: 真实隔离 Wrapper feature dependencies

**Files:**
- Modify: `crates/druid-wrapper/Cargo.toml`
- Modify: `crates/druid-wrapper/src/lib.rs`
- Create: `scripts/verify_feature_dependency_boundaries.py`
- Create: `crates/druid-wrapper/tests/feature_surface_test.rs`

**Interfaces:**
- Produces feature map:

```toml
default = ["driver-catalog", "toasty-sqlite"]
driver-catalog = []
sqlx = ["dep:sqlx"]
sqlx-bb8 = ["sqlx", "dep:bb8"]
sqlx-deadpool = ["sqlx", "dep:deadpool"]
toasty = ["dep:toasty", "dep:toasty-core"]
toasty-sqlite = ["toasty", "toasty/sqlite"]
rbdc = ["dep:rbdc", "dep:rbs"]
jdbc-agent = ["driver-catalog"]
```

- [ ] **Step 1: 写 no-default RED dependency test**

`verify_feature_dependency_boundaries.py` runs `cargo tree -p druid-wrapper --no-default-features
-e normal` and rejects SQLx, Toasty, RBDC, bb8 and deadpool.

- [ ] **Step 2: Run RED**

```bash
python3 scripts/verify_feature_dependency_boundaries.py
```

Expected: FAIL listing current unconditional dependencies.

- [ ] **Step 3: Make dependencies optional and cfg modules**

Set `optional = true` for each feature-owned dependency and guard only the matching modules/re-exports.
Delete empty compatibility features in `druid` unless they forward a real optional dependency.

- [ ] **Step 4: Run feature powerset**

```bash
cargo check -p druid-wrapper --no-default-features
cargo check -p druid-wrapper --no-default-features --features sqlx
cargo check -p druid-wrapper --no-default-features --features sqlx-bb8
cargo check -p druid-wrapper --no-default-features --features sqlx-deadpool
cargo check -p druid-wrapper --no-default-features --features toasty-sqlite
cargo check -p druid-wrapper --no-default-features --features rbdc
cargo check -p druid-wrapper --all-features
python3 scripts/verify_feature_dependency_boundaries.py
```

Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/druid-wrapper crates/druid/Cargo.toml scripts
git commit -m "build(wrapper): enforce real driver feature boundaries"
```

### Task 5: 修复 publish 和 CI ownership 门禁

**Files:**
- Modify: `crates/druid-rdbc/Cargo.toml`
- Modify: `crates/druid/Cargo.toml`
- Modify: `crates/druid-wrapper/Cargo.toml`
- Modify: `crates/druid-metrics/Cargo.toml`
- Modify: `.github/workflows/driver-matrix.yml`
- Modify: `.github/workflows/database-profile-certification.yml`
- Create: `scripts/verify_package_ownership.py`

**Interfaces:**
- Produces: all packages non-publishable and `druid-driver` owned by `druid-wrapper`

- [ ] **Step 1: 写 package ownership RED test**

Parse Cargo metadata and assert:

```text
druid-driver owner == druid-wrapper
druid-admin owner == druid-admin
publish == [] for every workspace package
```

- [ ] **Step 2: Run RED**

```bash
python3 scripts/verify_package_ownership.py
```

Expected: FAIL because several packages are publishable and CI invokes `druid-driver` through
`druid-admin`.

- [ ] **Step 3: 修复 manifests 和 workflows**

Set `publish = false` on every package. Replace aggregation commands with:

```bash
cargo run -p druid-wrapper --features managed-driver-install \
  --bin druid-driver -- aggregate-evidence sqlite sqlx \
  target/contract-evidence/sqlite-runs \
  target/contract-evidence/sqlite-evidence.json
```

Use these exact commands for the other two workflow sites:

```bash
cargo run -p druid-wrapper --features managed-driver-install \
  --bin druid-driver -- aggregate-evidence duckdb native \
  target/contract-evidence/duckdb-runs \
  target/contract-evidence/duckdb-evidence.json

cargo run -p druid-wrapper --features managed-driver-install \
  --bin druid-driver -- aggregate-evidence \
  "${{ inputs.profile }}" "${{ inputs.runtime_mode }}" \
  target/contract-evidence/runs \
  "target/contract-evidence/${{ inputs.profile }}-evidence.json"
```

- [ ] **Step 4: Run GREEN**

```bash
python3 scripts/verify_package_ownership.py
cargo check --workspace --all-targets --all-features
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates .github scripts/verify_package_ownership.py
git commit -m "build(workspace): align features package ownership and publish policy"
```
