# External Pool RDBC Conformance — Task 3

日期：2026-08-21

## 范围与结论

本报告只证明本机实际配置并执行的 `SQLx + SQLite` 路径：原生 `DruidPool`
（其物理连接为 direct SQLx）、`SqlxBb8Pool` 和 `SqlxDeadpoolPool`。三个入口共用
`tests/common/physical_connection_contract.rs`，因此 PreparedStatement、Metadata、
幂等 close/reacquire、结构化错误、fatal discard、显式 close rollback 与 dirty Drop
均由同一真实 SQLite 合同覆盖。

外部池仍直接实现 `Pool` 并持有 `PhysicalConnectionLease`；没有把 bb8/deadpool
嵌套为 `DruidPool` 的物理连接工厂。

## 适配器矩阵

| 入口 | Prepared / Metadata / close-reacquire | Invalid SQL | dirty transaction explicit close | dirty transaction Drop | release / no nested pool | 结果 |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| direct SQLx via `DruidPool` | `item` prepared insert、SQLite metadata、重复 close 与重新查询通过 | `select * from definitely_missing_table` 返回 `DruidError::SqlException`；SQLite sorter 触发即时 discard，重取连接可 `select 1` | 未提交 insert 在 close 后重取为 0 行、auto-commit | 重取新的 logical lease、auto-commit，且 `sqlite_master` 不再有脏表 | direct factory 路径；外部池 boundary tests 共同守住 factory 边界 | VERIFIED（configured SQLite） |
| bb8 | 同上 | 同上 | 同上 | 同上；`sqlite_master` 无脏表；既有专测断言 `recycle_count=0`、`discard_count=1`、重建后 `create_count=2` | `bb8_adapter_is_not_direct_factory` 通过 | VERIFIED（configured SQLite） |
| deadpool | 同上 | 同上 | 同上 | 同上；`sqlite_master` 无脏表；既有专测断言 `recycle_count=0`、`discard_count=1`、重建后 `create_count=2` | `deadpool_adapter_is_not_direct_factory` 通过 | VERIFIED（configured SQLite） |

`sqlite_wrapper_semantics_test` 还直接观察了 SQLx SQLite fatal 路径：回收回调只收到
一次 `Discard { recycle_error: None }`，fatal 后两次 close 均不产生第二次回收，且可
新建直连 adapter 执行 `SELECT 1`。

## 新增合同

1. invalid SQL 必须保留 `sql_exception()`，不会被扁平化为普通字符串错误。
2. SQLite database error 被显式 fatal sorter 命中后，当前 holder 必须立刻消失；
   两次 close 均幂等，随后获取的 lease 仍能执行 SQL。
3. 显式 close 在 non-auto-commit 事务中先 rollback；重新获取时没有未提交行。
4. dirty Drop 必须 discard/replacement，而非把租约复用：下一租约是 auto-commit，且
   `sqlite_master` 不再含脏表。

## 新鲜验证证据

| 命令 | exit | 结果 |
| :--- | :---: | :--- |
| `cargo fmt --check` | 0 | 格式通过 |
| `git diff --check` | 0 | 无 whitespace error |
| `cargo test -p druid-wrapper --test sqlx_connection_adapter_test rdbc_connection_contract` | 0 | 1 passed（direct SQLx） |
| `cargo test -p druid-wrapper --test sqlx_bb8_pool_test rdbc_connection_contract` | 0 | 1 passed（bb8） |
| `cargo test -p druid-wrapper --test sqlx_deadpool_pool_test rdbc_connection_contract` | 0 | 1 passed（deadpool） |
| `cargo test -p druid-wrapper --test sqlite_wrapper_semantics_test real_sqlite_database_error_flows_through_sorter_and_discards_physical_connection` | 0 | 1 passed（direct error/release exactly once） |
| `cargo test -p druid-wrapper --tests` | 0 | wrapper unit/integration suite 通过；上述 3 个共享合同入口均通过，bb8/deadpool dirty-drop 专测均通过 |
| `cargo test -p druid --test recycle_semantics_test` | 0 | 12 passed（含 `dirty_drop_discards_instead_of_leaking_transaction_to_idle_queue`） |
| `cargo test -p druid --test prepared_statement_cache_test` | 0 | 6 passed |

## 明确未验证边界

本报告不把以下对象标记为已验证，即使部分模块的非 Task-3 测试随 wrapper suite 运行：

| 边界 | 状态 | 原因 |
| :--- | :--- | :--- |
| PostgreSQL | UNVERIFIED | 未配置或执行真实 PostgreSQL 服务与本合同 |
| MySQL / MariaDB | UNVERIFIED | 未配置或执行真实 MySQL 服务与本合同 |
| Toasty | UNVERIFIED | 未执行本 Task 3 的 configured real-SQLite shared-pool conformance matrix |
| RBDC adapters | UNVERIFIED | 未执行本 Task 3 的 configured real-SQLite shared-pool conformance matrix |
| JDBC Agent | UNVERIFIED | 没有配置 Java agent/database；wrapper suite 中 JDBC agent target 显示 0 tests |

因此，本次结论仅适用于这里记录的 direct SQLx、bb8、deadpool 三种 configured SQLite
获取模式；它不是跨数据库、跨驱动或发布就绪声明。
