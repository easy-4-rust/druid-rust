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

`crates/druid/src/sql/wall_violation.rs` 与 `wall_deny_stat.rs` 变为 `pub use` 转发模块，类型身份不变，调用方无需改动。

## 2. 验证

```
cargo check -p druid-rdbc -p druid --lib        # exit 0
cargo test -p druid-rdbc --lib                  # 10 passed
cargo test -p druid --lib                       # 161 passed
cargo clippy -p druid-rdbc -p druid --lib -- -D warnings -A clippy::pedantic
```

## 2.1 新增依赖

`druid-rdbc` 增加 `serde`、`serde_json`（纯数据序列化，不涉及运行时/驱动），用于 `WallTableStatValue` 与 `WallSqlStatValue`。

## 3. 未做

- 其余 Wall 值对象（`WallCheckResult`、`WallSqlStat`、`WallTableStat` 等）仍依赖 `sqlparser::ast::Statement` 与更多统计类型，按同一模式分批迁移；
- Filter 接口（P3.3b）因引用 `DruidPooledConnection` 仍 HOLD，需要泛型上下文设计。
