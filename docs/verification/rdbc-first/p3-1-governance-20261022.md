# P3.1 证据：`druid-rdbc::governance` 纯策略下沉（第一批）

- 日期：2026-10-22
- ADR：ADR-CRATE-003（ACCEPTED）
- 范围：只迁移**与驱动/运行时无关**的判定函数；`druid` 的运行时装配保持不变（P3.2 再切换调用）

## 1. 本批内容

| 文件 | 内容 |
|---|---|
| `crates/druid-rdbc/src/governance/mod.rs` | 模块声明；明确禁止依赖 Tokio/Axum/Tonic/驱动 |
| `crates/druid-rdbc/src/governance/shrink_policy.rs` | `ShrinkDecision`、`ShrinkPolicy`、`classify(...)`，逐条对应 Java `DruidDataSource#shrink(boolean checkTime, boolean keepAlive)` 的 `checkTime == true` 分支 |
| `crates/druid-rdbc/src/lib.rs` | 新增 `pub mod governance;` |

## 2. 语义对照（Java → Rust）

| Java 分支 | Rust |
|---|---|
| `phyTimeoutMillis > 0 && phyConnectTimeMillis > phyTimeoutMillis` | `phy_timeout_millis > 0 && connect_age_millis > phy_timeout_millis` → `Evict` |
| `idleMillis < minEvictableIdleTimeMillis && idleMillis < keepAliveBetweenTimeMillis` → `break` | → `Retain`（调用方据此停止扫描） |
| `idleMillis >= minEvictableIdleTimeMillis && i < checkCount` | → `Evict` |
| `idleMillis > maxEvictableIdleTimeMillis` | → `Evict` |
| `keepAlive && idleMillis >= keepAliveBetweenTimeMillis && currentTimeMillis - lastKeepTimeMillis >= keepAliveBetweenTimeMillis` | → `KeepAlive` |
| 其余 | → `Retain` |

`checkCount = poolingCount - minIdle` 与 Java 一致（`saturating_sub` 防止下溢）。

## 3. 验证

```
cargo test -p druid-rdbc --lib
  governance::shrink_policy::tests::* ... ok（5 个）
  test result: ok. 10 passed

cargo clippy -p druid-rdbc --lib -- -D warnings -A clippy::pedantic
  exit 0
```

覆盖的边界：
1. 双阈值以下 → 保留（对应 Java 的 `break` 语义）；
2. `index < checkCount` 且达到 `minEvictable` → 淘汰；
3. 达到 `minEvictable` 但 `index >= checkCount` → 保留；
4. 超过 `maxEvictable` → 即使 `index >= checkCount` 也淘汰；
5. `phyTimeout` 优先于空闲判定；
6. 保活到期 → `KeepAlive`，距上次保活不足 → 保留。

## 4. 未做（下一批）

- P3.2：`druid::pool::pool_inner::recycle` 的 shrink 调用改为委托 `druid_rdbc::governance::shrink_policy`，并跑 `task6_pool_maintenance_semantics_test_parity` 差分；
- 等待队列公平性、统计聚合、熔断/限流仍留在原处，后续按同一方式迁入 `governance/`。
