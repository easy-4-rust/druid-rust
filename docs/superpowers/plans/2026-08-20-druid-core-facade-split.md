# Druid Core and Facade Split Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将当前 `druid` package 机械拆成 `druid-core` 实现 crate 与 `druid` 稳定门面，同时保持现有最终用户 API 路径和连接池行为。

**Architecture:** 第一阶段只改变编译边界；第二阶段建立 typed observability SPI 并移除 Core 的进程级管理全局变量。具体数据库 Adapter 暂不在本计划迁移，由 Wrapper 计划负责。

**Tech Stack:** Cargo workspace、Rust 2021、cargo-public-api、CodeGraph

**Spec:** `docs/superpowers/plans/2026-08-20-five-crate-architecture-spec.md`

## Global Constraints

- 保留当前 tracked/untracked 修改；移动目录必须使用 `git mv`。
- 机械拆分阶段禁止行为重构。
- `druid::sql::*`、Pool、Filter、Wall 和错误类型保持。
- exactly-once recycle 与 native/external pool 所有权不变。
- 不 commit/push，不使用 worktree。

---

### Task 1: 锁定 topology 与 public API RED 基线

**Files:**
- Create: `scripts/verify_five_crate_topology.py`
- Create: `api/druid-public-api.before.txt`
- Modify: `Cargo.toml`

**Interfaces:**
- Produces: 五 package topology contract、Facade API baseline

- [ ] **Step 1: 写 workspace topology 验证脚本**

脚本执行并解析 `cargo metadata --no-deps`，断言 package 名严格等于：

```rust
[
    "druid",
    "druid-admin",
    "druid-core",
    "druid-metrics",
    "druid-wrapper",
]
```

脚本实现固定为：

```python
import argparse
import json
import subprocess

EXPECTED = ["druid", "druid-admin", "druid-core", "druid-metrics", "druid-wrapper"]
parser = argparse.ArgumentParser()
parser.add_argument("--require-binary", action="append", default=[])
args = parser.parse_args()
metadata = json.loads(subprocess.check_output(
    ["cargo", "metadata", "--format-version", "1", "--no-deps"], text=True
))
packages = sorted(package["name"] for package in metadata["packages"])
if packages != EXPECTED:
    raise SystemExit(f"workspace packages {packages!r} != {EXPECTED!r}")
for requirement in args.require_binary:
    binary, expected_package = requirement.split("=", 1)
    owners = [
        package["name"]
        for package in metadata["packages"]
        if any(binary == target["name"] and "bin" in target["kind"] for target in package["targets"])
    ]
    if owners != [expected_package]:
        raise SystemExit(f"binary {binary!r} owners {owners!r} != {[expected_package]!r}")
```

- [ ] **Step 2: 运行 RED**

```bash
python3 scripts/verify_five_crate_topology.py
```

Expected: FAIL because `druid-core` and `druid-metrics` do not exist.

- [ ] **Step 3: 保存 public API 和依赖基线**

```bash
cargo public-api -p druid -sss --all-features > api/druid-public-api.before.txt
cargo metadata --format-version 1 --no-deps > target/five-crate-metadata.before.json
cargo tree -p druid > target/druid-tree.before.txt
git rev-parse HEAD > target/five-crate-baseline-rev.txt
```

Expected: all commands exit 0.

### Task 2: 机械创建 `druid-core`

**Files:**
- Move: `crates/druid` → `crates/druid-core`
- Modify: `crates/druid-core/Cargo.toml`
- Modify: `Cargo.toml`
- Modify: `crates/druid-core/tests/*.rs`
- Modify: `crates/druid-core/examples/*.rs`

**Interfaces:**
- Produces: package `druid-core`, Rust crate `druid_core`
- Preserves: current internal modules and behavior

- [ ] **Step 1: 移动 package**

```bash
git mv crates/druid crates/druid-core
```

- [ ] **Step 2: 修改 package/lib 名**

```toml
[package]
name = "druid-core"

[lib]
name = "druid_core"
```

- [ ] **Step 3: 机械修复 integration tests**

每个原集成测试和 example 在 imports 前加入：

```rust
extern crate druid_core as druid;
```

不得改动断言和测试语义。

- [ ] **Step 4: 验证 Core 行为**

```bash
cargo test -p druid-core
```

Expected: original tests pass with identical test names and assertions.

### Task 3: 创建稳定门面 `druid`

**Files:**
- Create: `crates/druid/Cargo.toml`
- Create: `crates/druid/src/lib.rs`
- Create: `crates/druid/tests/facade_compile_test.rs`
- Modify: `Cargo.toml`

**Interfaces:**
- Consumes: `druid_core`
- Produces: stable `druid::*` facade

- [ ] **Step 1: 写 facade compile test**

```rust
use druid::core::{DruidError, DruidPooledConnection, PhysicalConnection};
use druid::pool::{DruidDataSource, DruidPool};
use druid::sql::{Connection, ResultSet, SQLException};

#[test]
fn facade_preserves_core_rdbc_and_pool_paths() {
    fn assert_send<T: Send>() {}
    assert_send::<DruidError>();
}
```

- [ ] **Step 2: 运行 RED**

```bash
cargo test -p druid --test facade_compile_test
```

Expected: FAIL because the new facade package has not been created.

- [ ] **Step 3: 实现最小 facade**

```rust
pub use druid_core::{core, dynamic, pool, spi, sql};

pub mod stats {
    pub use druid_core::stats::*;
    #[cfg(feature = "metrics")]
    pub use druid_metrics::*;
}
```

Cargo 默认 feature 为空；Metrics/Wrapper features 在后续计划补充。

- [ ] **Step 4: 验证 GREEN 和 API diff**

```bash
cargo test -p druid --test facade_compile_test
cargo public-api -p druid -sss > api/druid-public-api.after.txt
diff -u api/druid-public-api.before.txt api/druid-public-api.after.txt
```

Expected: compile test PASS；diff 只包含经规格批准的 facade 表示差异。

### Task 4: 建立 typed observability SPI

**Files:**
- Create: `crates/druid-core/src/stats/data_source_identity.rs`
- Create: `crates/druid-core/src/stats/druid_telemetry_snapshot.rs`
- Create: `crates/druid-core/src/stats/snapshot_unavailable.rs`
- Modify: `crates/druid-core/src/stats/data_source_monitorable.rs`
- Modify: `crates/druid-core/src/pool/druid_data_source.rs`
- Test: `crates/druid-core/tests/data_source_monitorable_snapshot_test.rs`

**Interfaces:**
- Produces:

```rust
pub trait DataSourceMonitorable: Send + Sync {
    fn identity(&self) -> DataSourceIdentity;
    fn try_snapshot(&self) -> Result<DruidTelemetrySnapshot, SnapshotUnavailable>;
    fn reset_stat(&self);
}
```

- [ ] **Step 1: 写 typed snapshot RED 测试**

测试要求 snapshot 包含 datasource identity、pool snapshot、SQL list、Wall snapshot 和采样时间；Busy 不得阻塞等待。

- [ ] **Step 2: 运行 RED**

```bash
cargo test -p druid-core --test data_source_monitorable_snapshot_test
```

Expected: FAIL because the new typed API is absent.

- [ ] **Step 3: 实现 typed DTO 和 try_snapshot**

`SnapshotUnavailable` 精确包含 `Busy` 与 `Closed`；不得把 `serde_json::Value` 作为权威字段类型。

- [ ] **Step 4: 删除 Core 全局注册依赖**

移除 `DruidDataSource::register_monitoring` 对 `DruidDataSourceStatManager::global()` 的直接调用，增加显式转换：

```rust
pub fn monitorable(self: &Arc<Self>) -> Arc<dyn DataSourceMonitorable>;
```

- [ ] **Step 5: 验证 GREEN**

```bash
cargo test -p druid-core --test data_source_monitorable_snapshot_test
cargo check -p druid-core --all-targets
cargo check -p druid --all-targets
```

Expected: all commands exit 0.

**Suggested commit:** `refactor(core): split implementation crate from public facade`
