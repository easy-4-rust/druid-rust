# Statement 独立取消入口与 SQLx SQLite 执行中断

## 任务 4.1：标准独立取消合同

新增 `StatementCancellation` 标准异步接口和 `Statement::cancellation_control`。调用方可在执行前取得 `Arc` 取消句柄；执行 Future 独占借用非 `Sync` Statement 时，另一任务通过句柄发起取消。旧 `Statement::cancel(&mut self)` 保留兼容；默认新接口明确返回 SQLState `0A000`。

旧入口的借用限制由 `Statement::cancellation_control` 的 compile-fail doctest 记录；示例在 Future 持有 `&mut Statement` 时再次调用 `cancel(&mut self)`，必须编译失败。新合同覆盖一项实现了 `Cell` 字段（因此非 `Sync`）的外部集成测试，执行挂起后从另一个任务调用取消句柄并验证 `HY008` 结果；另有最小 Statement 实现验证默认 `0A000`。

验证：

- `cargo test -p druid-rdbc --test statement_cancellation_control_contract_test --locked --offline`：2 passed。
- `cargo test -p druid-rdbc --doc --locked --offline`：2 passed，其中 1 项是预期编译失败的旧借用示例。
- 此部分仅证明公共控制面合同，不单独证明适配器已支持真实取消。

## 当前执行证据：SQLx SQLite 路径接入（部分 4.2–4.6）

SQLx SQLite 的取消句柄绑定执行代次，并将同一原子中断信号传入缓冲查询、流式游标、通用 `Statement::execute`、PreparedStatement 查询/更新和生成键查询路径。SQLite progress handler 在数据库 VM 中检查中断/截止时间；执行收尾等待 worker 释放句柄并清理 handler，无法确认时将物理连接标记为不可复用。`execute` 的返回类型预检也在取消和超时控制下，避免描述阶段阻塞后才进入可取消执行。

新增正向执行测试分别运行递归 CTE 缓冲查询、普通 `execute`、普通/预编译更新、PreparedStatement 查询、生成键查询和请求生成键的长时间更新；均在预算内以 SQLState `HY008` 结束，且随后同一连接可继续查询。生成键更新额外验证取消后不提交行，也不发布部分或上一轮生成键。另测了调用方丢弃执行 Future 后同一连接仍可恢复；此项只证明资源恢复，不将 Future drop 当成后端取消证据。既有合同测试还覆盖流式游标取消、重复请求、迟到请求不污染下一代、预取消只消费一次及新语句隔离。

本次验证：

- `cargo test -p druid-wrapper --features sqlx --test sqlx_statement_cancel_contract_test --locked --offline`：14 passed，真实 PostgreSQL/MySQL 负向能力用例默认忽略。
- 使用运行中的 PostgreSQL 18.6 容器并注入临时进程级 `DRUID_DATABASE_URL`，执行 `cargo test -p druid-wrapper --features sqlx --test sqlx_statement_cancel_contract_test live_postgres_cancellation_control_reports_feature_not_supported --locked --offline -- --ignored --nocapture`：1 passed；验证真实 PostgreSQL 连接上的取消控制明确返回 `0A000`。
- 使用临时 MySQL 8.0.36 容器并注入临时进程级 `DRUID_MYSQL_URL`，执行 `cargo test -p druid-wrapper --features sqlx --test sqlx_statement_cancel_contract_test live_mysql_cancellation_control_reports_feature_not_supported --locked --offline -- --ignored --exact --nocapture`：1 passed；验证真实 MySQL 连接上的取消控制明确返回 `0A000`。测试后容器已停止并移除，未保留数据库卷。
- `cargo test -p druid --lib --locked --offline`：155 passed。
- `cargo fmt -p druid-wrapper`：通过。
- `git diff --check`：通过。

**Druid pooled owner 隔离覆盖（部分 4.2）**

- `cargo test -p druid --lib cancellation_control --locked --offline`：4 passed。包括普通 Statement 在原租约失效后拒绝旧控制句柄、Prepared/Callable 状态代理委托，以及 Prepared/Callable 底层语句模拟缓存 checkout 后旧 owner gate 拒绝取消、新 owner 控制句柄仍可取消。
- `cargo test -p druid --lib --locked --offline`：157 passed。
- Prepared/Callable 测试将同一 raw statement 从旧逻辑 owner 转交给新 owner，验证 owner 身份隔离；当前仍缺真实 DruidPool + SQLx 驱动的端到端缓存命中、连接归还和竞争调度证据，因此任务 4.2/4.6 不关闭。

对应源码 SHA-256（本次验证后）：

| 文件 | SHA-256 |
|---|---|
| `crates/druid-wrapper/src/sqlx/sqlx_prepared_statement.rs` | `385E4FA7D3194AD245C9361A6F79E0672EB7C4B5ECA0AE392544CC43FE83D2CD` |
| `crates/druid-wrapper/src/sqlx/sqlx_connection_actor.rs` | `E7698DADB9DF2F0858925519241787331F39D85DAA5FFF9CD7665E99D6387610` |
| `crates/druid-wrapper/tests/sqlx_statement_cancel_contract_test.rs` | `4313F14DA3BF564C4DB298CBAF22F59A8EB49E4E6858A33F1F528A9B199A9636` |
| `crates/druid/src/core/druid_pooled_statement/cancellation_control_tests.rs` | `8835DCF396F31638C2EA41C757038C6C52AB36D61302864D35FAADD940A24B8B` |
| `crates/druid/src/core/druid_pooled_prepared_statement/cancellation_control_tests.rs` | `4BE7380EE089134193061F85E29DAF017F7B8584050F778A829922961C9FDF2C` |
| `crates/druid/src/core/druid_pooled_callable_statement/cancellation_control_tests.rs` | `8FC4162F89C2D858C9F8ED7F07CE7606B86D096E70D8897893C1415102909B2C` |

**边界与未完成项：**SQLite 用例证明真实 SQLite VM 执行路径中断及连接清理；PostgreSQL 18.6 和 MySQL 8.0.36 实库只验证当前明确不支持/`0A000`。Druid pooled Prepared/Callable 目前有底层句柄重交接隔离测试，但真实池缓存命中/租约归还端到端证据、所有后端/执行路径能力矩阵、取消未确认时 owner 丢弃连接的故障注入，以及外部数据库正向取消及服务端终态证据仍未完成。因此 tasks 4.2–4.7 保持未勾选；本记录不关闭取消批，也不代表 JDBC 全面对齐或生产就绪。
