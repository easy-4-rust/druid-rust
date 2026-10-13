# Statement 独立取消入口与 SQLx SQLite 执行中断

## 任务 4.1：标准独立取消合同

新增 `StatementCancellation` 标准异步接口和 `Statement::cancellation_control`。调用方可在执行前取得 `Arc` 取消句柄；执行 Future 独占借用非 `Sync` Statement 时，另一任务通过句柄发起取消。旧 `Statement::cancel(&mut self)` 保留兼容；默认新接口明确返回 SQLState `0A000`。

旧入口的借用限制由 `Statement::cancellation_control` 的 compile-fail doctest 记录；示例在 Future 持有 `&mut Statement` 时再次调用 `cancel(&mut self)`，必须编译失败。新合同覆盖一项实现了 `Cell` 字段（因此非 `Sync`）的外部集成测试，执行挂起后从另一个任务调用取消句柄并验证 `HY008` 结果；另有最小 Statement 实现验证默认 `0A000`。

验证：

- `cargo test -p druid-rdbc --test statement_cancellation_control_contract_test --locked --offline`：2 passed。
- `cargo test -p druid-rdbc --doc --locked --offline`：2 passed，其中 1 项是预期编译失败的旧借用示例。
- 此部分仅证明公共控制面合同，不单独证明适配器已支持真实取消。

## 当前执行证据：SQLx SQLite 路径接入（部分 4.2–4.6）

SQLx SQLite 的取消句柄绑定执行代次，并将同一原子中断信号传入缓冲查询、流式游标、通用 `Statement::execute`、更新及生成键执行路径。SQLite progress handler 在数据库 VM 中检查中断/截止时间；执行收尾等待 worker 释放句柄并清理 handler，无法确认时将物理连接标记为不可复用。`execute` 的返回类型预检也在取消和超时控制下，避免描述阶段阻塞后才进入可取消执行。

新增正向执行测试分别运行递归 CTE 缓冲查询、普通 `execute` 与长更新；三者都在预算内以 SQLState `HY008` 结束，且随后同一连接可继续查询。既有合同测试还覆盖流式游标取消、重复请求、迟到请求不污染下一代、预取消只消费一次及新语句隔离。

本次验证：

- `cargo test -p druid-wrapper --features sqlx --test sqlx_statement_cancel_contract_test --locked --offline`：9 passed。
- `cargo test -p druid --lib --locked --offline`：155 passed。
- `cargo fmt -p druid-wrapper`：通过。
- `git diff --check`：通过。

对应源码 SHA-256（本次验证后）：

| 文件 | SHA-256 |
|---|---|
| `crates/druid-wrapper/src/sqlx/sqlx_prepared_statement.rs` | `385E4FA7D3194AD245C9361A6F79E0672EB7C4B5ECA0AE392544CC43FE83D2CD` |
| `crates/druid-wrapper/src/sqlx/sqlx_connection_actor.rs` | `E7698DADB9DF2F0858925519241787331F39D85DAA5FFF9CD7665E99D6387610` |
| `crates/druid-wrapper/tests/sqlx_statement_cancel_contract_test.rs` | `60CD0ACD629BEE47DD3C015F209C82D0A7689D58E1097C4177F5B94C19629A86` |

**边界与未完成项：**这些测试运行在 SQLite 数据库引擎上，证明 SQLite VM 执行路径中断及本地连接清理，不是外部数据库服务器日志/会话终态证据。PostgreSQL、MySQL 等后端的逐项 `0A000` 证据、PreparedStatement/CallableStatement 的实时取消端到端覆盖、旧池租约缓存复用覆盖、生成键长执行覆盖，以及取消未确认时 owner 丢弃连接的故障注入仍未完成。因此 tasks 4.2–4.7 保持未勾选；本记录不关闭取消批，也不代表 JDBC 全面对齐或生产就绪。
