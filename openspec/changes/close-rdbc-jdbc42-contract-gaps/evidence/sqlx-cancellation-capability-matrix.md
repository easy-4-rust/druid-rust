# SQLx 取消能力矩阵（4.4）

本矩阵只描述 SQLx adapter 已验证的标准 Statement 取消入口，不代表所有 SQLx 驱动、JDBC 方法或 JDBC TCK 能力。

## 后端能力

| SQLx 后端 | 取消控制结果 | 状态与证据 |
|---|---|---|
| SQLite 原生连接 | 返回控制句柄；通过 SQLite progress handler 将原子取消/期限送入 VM；执行和清理完成后可复用连接，无法确认时丢弃 owner | `SUPPORTED`。正向执行证据见下表；连接清理与池丢弃见 [取消合同证据](cancellation-contract.md)。 |
| PostgreSQL 原生连接 | `SQLState 0A000` | `UNSUPPORTED`。PostgreSQL 18.6 实库测试 `live_postgres_cancellation_control_reports_feature_not_supported` 已运行通过；不发送无效果的取消请求。 |
| MySQL 原生连接 | `SQLState 0A000` | `UNSUPPORTED`。MySQL 8.0.36 实库测试 `live_mysql_cancellation_control_reports_feature_not_supported` 已运行通过；不发送无效果的取消请求。 |
| SQLx `Any` fallback | `SQLState 0A000` | `UNSUPPORTED`。单测 `any_backend_cancellation_is_exactly_not_supported` 以 Any 包装的 SQLite 连接验证：不能因为底层测试驱动是 SQLite 而把 `Any` 误报为原生 SQLite 能力。 |

生产代码的能力门位于 `SqlxPreparedStatement::cancellation_control`：只有 actor 的 `backend_key == "sqlite"` 才提供取消对象；其他 backend 精确返回 feature-not-supported。未经实测的驱动不继承 SQLite 能力。

## SQLite 原生后端 × 执行入口

| 执行入口 | 状态 | 精确验证入口 |
|---|---|---|
| 缓冲查询 | `SUPPORTED` | `independent_cancel_interrupts_running_buffered_sqlite_query`、`independent_cancel_interrupts_running_prepared_sqlite_query` |
| 流式查询 | `SUPPORTED` | `cancel_active_cursor_releases_actor_and_allows_a_fresh_execution`、`cancel_releases_connection_while_old_cursor_is_retained` |
| 异步结果集 `next` | `SUPPORTED`（流式执行的结果集接口） | 同上两个活动游标测试；另验证旧游标退出后新代次仍可读取，旧游标不能取消新代次。 |
| 通用 `Statement::execute` | `SUPPORTED` | `independent_cancel_interrupts_running_sqlite_execute`；路径在结果类型预检和实际执行中使用同一取消代次。 |
| 普通更新 | `SUPPORTED` | `independent_cancel_interrupts_running_sqlite_update` |
| PreparedStatement 查询/更新 | `SUPPORTED` | `independent_cancel_interrupts_running_prepared_sqlite_query`、`independent_cancel_interrupts_running_prepared_sqlite_update` |
| 普通 batch | `SUPPORTED` | `cancellation_interrupts_running_sqlite_statement_batch_entry`；断言失败 entry 不发布完成计数、无部分写入，原 SQLState 为 `HY008`。 |
| prepared batch | `SUPPORTED` | `cancellation_interrupts_running_sqlite_prepared_batch_entry`；断言参数集失败计数为空、无部分写入，原 SQLState 为 `HY008`。 |
| 生成键查询 `ReturnGeneratedKeys` | `SUPPORTED` | `independent_cancel_interrupts_running_sqlite_generated_key_query` |
| 生成键更新 `ReturnGeneratedKeys` | `SUPPORTED` | `independent_cancel_interrupts_running_sqlite_generated_key_update`；取消不得提交行或发布部分/旧键。 |
| 生成键列索引/列名选择 | `UNSUPPORTED` | SQLx adapter 在执行前精确返回 `0A000`（`sqlx_generated_keys_column_selection`）；不随 SQLite `ReturnGeneratedKeys` 的支持而推断。 |

SQLite 的以上能力共享 Statement 执行代次和原子中断信号；每个 SQL 执行路径安装 progress handler，只有清除 handler 的屏障确认后才允许复用物理连接。SQLState `HY008` 表示数据库执行被中断；取消请求的接受本身不作为执行终态。Future 被 drop 或清理结果不确定时不宣称数据库已取消，而是把 actor 标记为不可用，让 DruidPool 丢弃物理 owner。

## 未覆盖边界

- 正向测试使用实际 SQLx SQLite VM，当前本机 runtime 版本与执行终态记录见 [取消合同证据](cancellation-contract.md) 的 4.5 小节；SQLite `:memory:` 是真实嵌入式数据库执行，不代表独立数据库服务端。
- PostgreSQL/MySQL 只验证真实连接上的 `0A000` 负向能力，不声明其执行入口具备取消能力。
- `Any` 以 SQLite Any 驱动验证 capability gate；其他 Any backend 的连接建立与取消均为 `NOT_RUN`，仍按 `0A000` 分支处理。
- 除表内列出的入口外，不从共享 trait、SQLx crate 名称或 adapter 类型推断取消支持。
