# Trait 例外复审与 P3.3/P3.4 可行性（2026-10-23）

> 复审对象：`crates/druid-rdbc/src/extensions/row_set.rs`、`crates/druid-rdbc/src/result_set/result_set_ext.rs`。
> 决定依据：P6 例外于 2026-10-23 批准（复审截止 2026-12-31），本文是**首次复审**，不改变代码。

## 1. 事实（可复现）

| 项 | 值 | 命令 |
|---|---:|---|
| `row_set.rs` 行数 | 1,128 | `(Get-Content ...).Count` |
| `row_set` 方法数 | 117（含 async/多行签名） | `Select-String '^    (async )?fn '` |
| `result_set_ext.rs` 行数 | 1,216 | 同上 |
| `result_set_ext` 方法数 | 84 | 同上 |
| `ResultSetExt` 实现方式 | **blanket**：`impl<T: ResultSet + ?Sized> ResultSetExt for T {}` | `result_set_ext.rs:735` |
| `RowSet` 实现方式 | 手工 impl（`SharedRowSet` 等），无 blanket | `extensions/row_set` 调用点 |

## 2. 结论：`ResultSetExt` 可以安全拆为 supertrait（建议 GO）

理由：
1. 已存在 blanket impl，**调用方**只依赖 `T: ResultSetExt`；把方法定义移到 supertrait 后，通过 supertrait bound 仍可调用，源码兼容；
2. 不需要下游实现新 trait（blanket 已覆盖所有 `ResultSet`）；
3. 唯一风险是方法体内部互相调用导致的 trait bound 环，需要按依赖分层（base → types → labels → lob/time）。

拆分草案：

| supertrait | 方法域 | 预计行数 |
|---|---|---:|
| `ResultSetValueExt`（base） | 标量读取、`was_null`、cursor 位置 | ~300 |
| `ResultSetNumericExt` | `get_i8..get_u64`、`get_f32/f64`、`get_decimal` | ~250 |
| `ResultSetTemporalExt` | `get_date/time/timestamp` | ~200 |
| `ResultSetLobExt` | `get_bytes/blob/clob/nclob/ascii_stream` | ~250 |
| 根 `ResultSetExt` | `pub trait ResultSetExt: ... + ResultSet {}` | ~30 |

验收：拆分后 `cargo test -p druid-rdbc --lib` + `cargo test -p druid --lib` + public-api 快照 diff 必须为空（方法路径仍可通过根 trait 访问）。

## 3. `RowSet`：需要配套调整，建议暂缓（HOLD）

- 无 blanket impl；`SharedRowSet`、`RowSetInternal` 等实现会从 1 个 impl 变为多个 supertrait impl，改动面大于 `ResultSetExt`；
- 拆为 `RowSetParameters` / `RowSetConfiguration` / `RowSetListeners` 后，根 `RowSet` 仍可保持方法可用，但所有实现方需要拆 impl；
- 建议在 `ResultSetExt` 拆分验证通过后，按同一模式推进，并保留旧 `RowSet` 作为 supertrait 组合，降低下游破坏。

## 4. P3.3（Filter/Wall 接口下沉）可行性

现状事实：

| 接口 | 行数 | 依赖 |
|---|---:|---|
| `core/filter.rs` | 635 | `DruidPooledConnection`、`FilterChain`、`DruidError` |
| `sql/wall.rs` | 711 | `DruidError`、`WallProvider`、AST |
| `sql/wall_provider.rs` | 770 | `WallConfig`、`WallCheckResult` |
| `sql/wall_filter.rs` | 553 | `Filter` 运行时类型 |

阻断点：接口签名直接引用 `druid` 的运行时类型（`DruidPooledConnection`、`FilterChain`），而 ADR-CRATE-003 要求 `druid-rdbc` 不反向依赖 `druid`。

建议分两步（不改变本季度范围）：
1. **P3.3a**：把与运行时无关的 `WallConfig`、`WallCheckResult`、`WallViolation` 等值对象下沉 `druid-rdbc::governance`（可平移，无泛型化）；
2. **P3.3b**：Filter/Wall 的 trait 在 rdbc 中以**泛型上下文**定义（`FilterContext` 关联类型），`druid` 侧提供 `impl FilterContext for DruidPooledConnection`；此步骤需要 public-api 评审，暂不启动。

## 5. P3.4（druid 门面收敛）

前置依赖 P3.3b；当前 `druid` 同时承载池运行时与 SQL/Wall 实现，直接收敛会造成 API 断裂。建议保持 `druid` 为门面 + 运行时，直到 P3.3b 完成。

## 6. 本轮决定

- `ResultSetExt`：**GO**（下一轮实施 supertrait 拆分，含 public-api 快照对比）；
- `RowSet`：**HOLD**（等 `ResultSetExt` 模式验证）；
- P3.3：**P3.3a GO**（值对象下沉），P3.3b HOLD；
- P3.4：HOLD。
