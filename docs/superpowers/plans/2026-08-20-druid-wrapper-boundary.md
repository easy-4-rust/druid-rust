# Druid Wrapper Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将全部具体数据库 Adapter、vendor checker/sorter 和 driver 管理工具收敛到 `druid-wrapper`，使 `druid-core` 只保留稳定 SPI。

**Architecture:** Wrapper 依赖 `druid-core` 并通过 inventory 注册具体能力；Core 不反向依赖 Wrapper。Direct Adapter 与外部池 bridge 继续保持互斥，禁止 pool-in-pool。

**Tech Stack:** Toasty、SQLx、RBDC、bb8、deadpool、inventory、Cargo features

**Spec:** `docs/superpowers/plans/2026-08-20-five-crate-architecture-spec.md`

## Global Constraints

- Core 不得出现具体数据库依赖和具体连接类型。
- `DruidPooledConnection -> DruidConnectionHolder -> PhysicalConnection` 所有权不变。
- `PhysicalConnectionLease` 只用于 bb8/deadpool。
- 不保留 `druid::toasty` 兼容路径。
- 保留当前未提交 Wrapper/SQLx 修改。

---

### Task 1: 建立依赖边界 RED 门禁

**Files:**
- Create: `scripts/verify_core_dependency_boundary.py`
- Create: `crates/druid-wrapper/tests/adapter_boundary_test.rs`

**Interfaces:**
- Consumes: package `druid-core`
- Produces: forbidden dependency and adapter ownership contracts

- [ ] **Step 1: 写 Core 禁止依赖测试**

脚本执行 `cargo metadata`，递归检查 `druid-core` dependency graph 不含：

```text
toasty, toasty-core, sqlx, rbdc, duckdb, libsql, bb8, deadpool,
prometheus, reqwest, tonic, axum, topcoat
```

脚本读取完整 `cargo metadata` 的 `resolve.nodes`，从 `druid-core` package ID 深度遍历 dependency IDs；任何 package name 命中以下常量即退出 1：

```python
FORBIDDEN = {
    "toasty", "toasty-core", "sqlx", "rbdc", "duckdb", "libsql",
    "bb8", "deadpool", "prometheus", "reqwest", "tonic", "axum", "topcoat",
}
```

脚本必须同时打印遍历到的 package 名，便于失败时定位真实依赖路径。

- [ ] **Step 2: 运行 RED**

```bash
python3 scripts/verify_core_dependency_boundary.py
```

Expected: FAIL while Toasty remains in Core.

- [ ] **Step 3: 写 Adapter ownership test**

静态断言：direct Adapter 实现 `PhysicalConnection`/factory；bb8/deadpool 实现 `Pool` 并返回 `DruidPooledConnection`；一个类型不得同时作为 direct factory 和 external pool。

### Task 2: 迁移 Toasty

**Files:**
- Move: `crates/druid-core/src/toasty` → `crates/druid-wrapper/src/toasty`
- Move: related tests from `crates/druid-core/tests` → `crates/druid-wrapper/tests`
- Modify: `crates/druid-core/Cargo.toml`
- Modify: `crates/druid-wrapper/Cargo.toml`
- Modify: `crates/druid-wrapper/src/lib.rs`
- Modify: root `Cargo.toml`

**Interfaces:**
- Produces: `druid_wrapper::toasty::{ToastyConnectionFactory, ToastyConnectionAdapter}`
- Consumes: `druid_core::core::PhysicalConnection*`

- [ ] **Step 1: 写 Wrapper Toasty compile RED 测试**

```rust
use druid_wrapper::toasty::{ToastyConnectionAdapter, ToastyConnectionFactory};

#[test]
fn toasty_is_exposed_only_by_wrapper() {}
```

- [ ] **Step 2: 运行 RED**

```bash
cargo test -p druid-wrapper --test toasty_boundary_test
```

Expected: FAIL because Wrapper has no Toasty module.

- [ ] **Step 3: 移动实现和 feature**

```toml
toasty-sqlite = ["dep:toasty", "toasty/sqlite"]
toasty-postgresql = ["dep:toasty", "toasty/postgresql"]
toasty-mysql = ["dep:toasty", "toasty/mysql"]
toasty-turso = ["dep:toasty", "toasty/turso"]
```

- [ ] **Step 4: 运行 GREEN 和 Core boundary**

```bash
cargo test -p druid-wrapper --test toasty_boundary_test
python3 scripts/verify_core_dependency_boundary.py
```

Expected: PASS.

### Task 3: 建立 Driver Extension Registry

**Files:**
- Create: `crates/druid-core/src/spi/driver_extension_descriptor.rs`
- Create: `crates/druid-core/src/spi/driver_extension_registry.rs`
- Create: `crates/druid-wrapper/src/driver/extensions.rs`
- Modify: Core pool/factory resolution entrypoints
- Test: `crates/druid-wrapper/tests/driver_extension_registry_test.rs`

**Interfaces:**
- Produces:

```rust
pub struct DriverExtensionDescriptor {
    pub db_type: DbType,
    pub factory: fn(&DriverConfig) -> Result<Arc<dyn PhysicalConnectionFactory>, DruidError>,
    pub checker: Option<fn() -> Arc<dyn ValidConnectionChecker>>,
    pub sorter: Option<fn() -> Arc<dyn ExceptionSorter>>,
}
```

- [ ] **Step 1: 写 registry RED 测试**

测试要求启用 MySQL extension 后按 `DbType::mysql` 同时解析 factory/checker/sorter；未链接 Wrapper 时返回明确 `NoDriverExtension`。

- [ ] **Step 2: 运行 RED**

```bash
cargo test -p druid-wrapper --test driver_extension_registry_test
```

- [ ] **Step 3: 实现 inventory registry**

Core 定义 descriptor 与 lookup；Wrapper 提交 inventory 项。Core 不 import 任何 Wrapper module。

- [ ] **Step 4: 迁移 vendor checker/sorter**

把 MySQL/PostgreSQL/Oracle/DB2/OceanBase 等具体实现和对应测试移到 Wrapper，Core 保留 trait、generic null/ping 实现和错误类型。

- [ ] **Step 5: 验证 GREEN**

```bash
cargo test -p druid-wrapper --test driver_extension_registry_test
cargo test -p druid-wrapper --all-features
```

### Task 4: 迁移 driver 管理工具与 binary

**Files:**
- Move: `crates/druid-admin/src/driver` → `crates/druid-wrapper/src/driver_admin`
- Move: `crates/druid-admin/src/bin/druid-driver.rs` → `crates/druid-wrapper/src/bin/druid-driver.rs`
- Move: related Admin tests → Wrapper tests
- Modify: both package manifests and public modules

**Interfaces:**
- Produces: Wrapper-owned `druid-driver` binary and driver management API
- Removes: Admin direct Wrapper dependency and driver features

- [ ] **Step 1: 写 binary ownership RED 测试**

测试读取 Cargo metadata，断言 `druid-driver` target 的 package 为 `druid-wrapper`。

- [ ] **Step 2: 运行 RED**

```bash
python3 scripts/verify_five_crate_topology.py --require-binary druid-driver=druid-wrapper
```

- [ ] **Step 3: 移动并修复导入**

保留 checksum、bundle、安装路径、JDBC Agent diagnostics 行为；Admin 不再声明 driver feature。

- [ ] **Step 4: 验证 installer 合同**

```bash
cargo test -p druid-wrapper driver_installer
cargo check -p druid-wrapper --all-targets --all-features
cargo tree -p druid-core -e normal
```

Expected: Core tree 无禁止依赖，Wrapper tests/check PASS.

**Suggested commit:** `refactor(wrapper): centralize database and driver integrations`
