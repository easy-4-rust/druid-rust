# P3.3a 证据：Wall 值对象下沉（第一批）

- 日期：2026-10-23
- ADR：ADR-CRATE-003（ACCEPTED）
- 范围：只迁移与运行时无关的值对象；`druid` 侧保留同名模块 re-export，公共路径不变。

## 1. 迁移内容

| 类型 | 新位置 | 旧路径（兼容） |
|---|---|---|
| `WallViolation` | `druid-rdbc::governance::wall::violation` | `druid::sql::wall_violation::WallViolation` |
| `WallDenyStat` | `druid-rdbc::governance::wall::deny_stat` | `druid::sql::wall_deny_stat::WallDenyStat` |
| `WallSqlFunctionStat` | `druid-rdbc::governance::wall::sql_function_stat` | `druid::sql::wall_sql_function_stat::WallSqlFunctionStat` |
| `WallSqlTableStat` | `druid-rdbc::governance::wall::sql_table_stat` | `druid::sql::wall_sql_table_stat::WallSqlTableStat` |
| `WallTableStatValue` | `druid-rdbc::governance::wall::table_stat_value` | `druid::sql::wall_table_stat_value::WallTableStatValue` |
| `WallSqlStatValue` | `druid-rdbc::governance::wall::sql_stat_value` | `druid::sql::wall_sql_stat_value::WallSqlStatValue` |
| `WallTableStat` | `druid-rdbc::governance::wall::table_stat` | `druid::sql::wall_table_stat::WallTableStat` |
| `WallSqlStat` | `druid-rdbc::governance::wall::sql_stat` | `druid::sql::wall_sql_stat::WallSqlStat` |
| `WallUpdateCheckItem` | `druid-rdbc::governance::wall::update_check_item` | `druid::sql::wall_update_check_item::WallUpdateCheckItem` |
| `WallCheckResult` | `druid-rdbc::governance::wall::check_result` | `druid::sql::wall_check_result::WallCheckResult` |
| `fingerprint`（SQL 指纹） | `druid-rdbc::governance::hashing` | `druid::stats::fingerprint`（委托） |

`crates/druid/src/sql/wall_violation.rs` 与 `wall_deny_stat.rs` 变为 `pub use` 转发模块，类型身份不变，调用方无需改动。

## 2. 验证

```
cargo check -p druid-rdbc -p druid --lib        # exit 0
cargo test -p druid-rdbc --lib                  # 10 passed
cargo test -p druid --lib                       # 161 passed
cargo clippy -p druid-rdbc -p druid --lib -- -D warnings -A clippy::pedantic
```

## 2.1 新增依赖

`druid-rdbc` 增加 `serde`、`serde_json`、`xxhash-rust`、`sqlparser`（纯数据/AST，不涉及运行时/驱动），用于剩余 Wall 值对象与 `WallCheckResult`。

## 3. 未做

- 2026-11-01 更正：上文列出的 `WallCheckResult`、`WallSqlStat`、`WallTableStat` 等十个模块现已位于 `druid-rdbc/src/governance/wall`；原先“这些对象未迁移”的表述已过时。它们使用 AST/统计类型不等于迁移缺失。当前局部回归与剩余边界见 [核对报告](../current-code-audit-20261101/README.md)；原第 2 节命令仍为历史记录。
- Filter 接口（P3.3b）因引用 `DruidPooledConnection` 仍 HOLD，需要泛型上下文设计。
