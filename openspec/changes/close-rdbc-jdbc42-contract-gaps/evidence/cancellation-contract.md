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

新增正向执行测试分别运行递归 CTE 缓冲查询、普通 `execute`、普通/预编译更新、PreparedStatement 查询、生成键查询和请求生成键的长时间更新；均在预算内以 SQLState `HY008` 结束，且随后同一连接可继续查询。生成键更新额外验证取消后不提交行，也不发布部分或上一轮生成键。Future drop 的资源处置现按“不确定执行不得复用”处理，详见下方 4.3 进展；不会把 Future drop 描述成后端取消。既有合同测试还覆盖流式游标取消、重复请求、迟到请求不污染下一代、预取消只消费一次及新语句隔离。

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
- 新增 `pooled_statement_cancellation_handle_expires_after_real_pool_lease_return`，使用真实 DruidPool + SQLx SQLite 借出、归还、再次借出连接，验证旧普通 Statement 句柄返回 `ConnectionDiscarded`，新租约仍可查询。
- 新增 `pooled_prepared_cancellation_handle_cannot_cross_cached_checkout`，使用真实 DruidPool + SQLx SQLite 关闭并归还 PreparedStatement，再从实际 per-connection LRU checkout 同一语句；断言缓存命中计数为 1、旧取消句柄被拒绝、新句柄可中断 SQLite VM，且同一新租约后续查询成功。
- 扩展 `callable_lease_return_recycles_raw_but_rejects_previous_logical_handle`：在真实 DruidPool 和可控合同 Callable 驱动中验证归还后同一 raw Callable 语句被下一租约复用、旧控制句柄被拒绝且不触达底层、新控制句柄只调用新 owner 的底层取消对象。SQLx `Connection#prepare_call` 默认不支持，因此本测试不声称 SQLx SQLite 提供 Callable 能力。
- `cargo test -p druid-wrapper --features sqlx --test sqlx_statement_cancel_contract_test --locked --offline`：16 passed、2 ignored（实时 PostgreSQL/MySQL 负向用例）。
- `cargo test -p druid --test callable_cache_lifecycle_test --locked --offline`：6 passed。
- `cargo fmt -p druid -p druid-wrapper -- --check`：通过；`git diff --check`：通过。
- 同一 SQLx 合同套件中的 `independent_cancel_control_is_generation_bound`、`late_cancel_control_does_not_become_pending_for_next_generation` 和 `cancel_releases_connection_while_old_cursor_is_retained` 分别验证同代重复请求、已完成代次迟到请求、同连接其他语句隔离及旧游标/新代次隔离；与三类 DruidPool 代理生命周期测试共同满足 4.2。
- 因上述池归还与缓存 checkout、三类代理、迟到/重复/共享连接测试均有实际证据，任务 4.2 已关闭。仍缺取消未确认/清理失败时 owner 丢弃连接的故障注入和精确回收计数断言，4.3/4.6 不关闭。

对应源码 SHA-256（本次验证后）：

| 文件 | SHA-256 |
|---|---|
| `crates/druid-wrapper/src/sqlx/sqlx_prepared_statement.rs` | `385E4FA7D3194AD245C9361A6F79E0672EB7C4B5ECA0AE392544CC43FE83D2CD` |
| `crates/druid-wrapper/src/sqlx/sqlx_connection_actor.rs` | `E7698DADB9DF2F0858925519241787331F39D85DAA5FFF9CD7665E99D6387610` |
| `crates/druid-wrapper/tests/sqlx_statement_cancel_contract_test.rs` | `4313F14DA3BF564C4DB298CBAF22F59A8EB49E4E6858A33F1F528A9B199A9636` |
| `crates/druid/src/core/druid_pooled_statement/cancellation_control_tests.rs` | `8835DCF396F31638C2EA41C757038C6C52AB36D61302864D35FAADD940A24B8B` |
| `crates/druid/src/core/druid_pooled_prepared_statement/cancellation_control_tests.rs` | `4BE7380EE089134193061F85E29DAF017F7B8584050F778A829922961C9FDF2C` |
| `crates/druid/src/core/druid_pooled_callable_statement/cancellation_control_tests.rs` | `8FC4162F89C2D858C9F8ED7F07CE7606B86D096E70D8897893C1415102909B2C` |
| `crates/druid/tests/callable_cache_lifecycle_test.rs` | `E4DC411EBB942229EF8F5CA39122B61D27CE30AB35265EC30798AD66CE137B59` |
| `crates/druid-wrapper/tests/sqlx_statement_cancel_contract_test.rs` | `22A365018F18C8858F975A94CD968CE7D44CB265C27B1E25994FCF8E5D385326` |

**边界与未完成项：**SQLite 用例证明真实 SQLite VM 执行路径中断及连接清理；PostgreSQL 18.6 和 MySQL 8.0.36 实库只验证当前明确不支持/`0A000`。普通/Prepared SQLx 代理及 Callable 合同夹具现已覆盖真实 DruidPool 借还路径，迟到/重复请求与共享连接隔离也有测试，故 4.2 已完成。4.3 的清理失败、超时、Future drop、自然完成竞态和 owner 关闭处理现均有实现与测试并已关闭。

## 4.4：SQLx 后端 × 执行入口取消能力矩阵

矩阵见 [sqlx-cancellation-capability-matrix.md](sqlx-cancellation-capability-matrix.md)。能力按具体后端和执行入口声明，不从共享 trait、adapter 类型或 SQLx crate 名称推断。SQLite 原生路径覆盖缓冲/流式查询、异步结果集、通用 execute、普通与 prepared 更新/批处理及 ReturnGeneratedKeys 查询/更新；列名/列索引生成键选择仍精确返回 `0A000`。PostgreSQL 18.6、MySQL 8.0.36 真实连接和 Any 包装 SQLite 的 fallback 分别验证明确不支持，不会误报 SQLite 原生能力。

新增普通 batch 与 prepared batch 的活动取消合同：取消原因保留为 `HY008`，失败 entry 不报告完成计数且没有部分写入。当前定向验证结果：

- `cargo test -p druid-wrapper --features sqlx --test sqlx_statement_cancel_contract_test --locked --offline`：21 passed、2 ignored（需真实 PostgreSQL/MySQL 环境的负向用例；既有真实版本结果如上记录）。
- `cargo test -p druid-wrapper --features sqlx --lib cancellation_cleanup_tests --locked --offline`：5 passed。
- `cargo test -p druid-wrapper --features sqlx --lib any_backend_cancellation_is_exactly_not_supported --locked --offline`：1 passed。
- `cargo fmt -p druid-wrapper -- --check`、`git diff --check`：通过。

4.6 仍需完成矩阵中逐路径正/负向回归与代理回归；4.7 仍需同步用户用法文档和最终证据。该矩阵不宣称 PostgreSQL/MySQL 支持执行中取消，也不代表取消批、JDBC 全面对齐或项目生产就绪。

4.4 源码 SHA-256：

| 文件 | SHA-256 |
|---|---|
| `crates/druid-wrapper/src/sqlx/sqlx_connection_adapter.rs` | `8D3F7984C55484499AD61E8C8F6811D15B88617429A3F5E3C23C3EC21798E62D` |
| `crates/druid-wrapper/tests/sqlx_statement_cancel_contract_test.rs` | `7B8AE8DA0D41A639A6D46461F715AFA48C49935B250BDA230E7E9B20C2985DBD` |

## 4.5：SQLite VM 终态与连接处置

通过正在执行递归 CTE 的真实 SQLx SQLite VM 取消用例采集运行时版本、精确 SQL、执行开始时间、取消请求结果、数据库错误终态和后续查询结果。命令：

```powershell
cargo test -p druid-wrapper --features sqlx --test sqlx_statement_cancel_contract_test independent_cancel_interrupts_running_buffered_sqlite_query --locked --offline -- --exact --nocapture
```

实测输出：

```text
backend=SQLx SQLite runtime_version=3.46.0 sql="WITH RECURSIVE counter(value) AS (VALUES(0) UNION ALL SELECT value + 1 FROM counter WHERE value < 5000000) SELECT sum(value) FROM counter" execution_start_unix_ms=1791919923366
cancellation_request_result=accepted
execution_terminal=SQLite interrupt mapped to SQLState HY008
post_cancel_query=SELECT 9 result=9 same_physical_connection=reused
test result: ok; 1 passed
```

该测试在同一个物理 SQLite 连接上于长查询运行期间接受取消，数据库执行以 `HY008` 终止；取消清理屏障后执行 `SELECT 9` 得到 9，证明该已确认清理的连接可复用。运行时版本来自执行中的 `SELECT sqlite_version()`，不是 Cargo feature 或 crate 版本推测。这个用例是嵌入式 SQLite VM 实测，不等价于独立数据库服务端；PostgreSQL/MySQL 的取消能力仍为精确 `0A000`。

4.5 专项测试通过。取消/自然完成竞争与迟到取消隔离仍由矩阵引用的 4.2/4.3 合同测试覆盖；完整复跑将在 4.6 同批完成。当前测试源码 SHA-256：

| 文件 | SHA-256 |
|---|---|
| `crates/druid-wrapper/tests/sqlx_statement_cancel_contract_test.rs` | `0B538B85FBF161E58A940F7584A1FB46F038F7A95ACD8BCF9B51518680810C06` |

## 4.6：执行入口与池化代理回归

4.4 的矩阵现有每条声明支持的 SQLite 执行入口都对应执行中取消测试；本次重跑完整矩阵套件：

```powershell
cargo test -p druid-wrapper --features sqlx --test sqlx_statement_cancel_contract_test --locked --offline
```

结果：**22 passed、2 ignored**。忽略项是需要外部服务 URL 的 PostgreSQL/MySQL 测试；两项真实服务上的 `0A000` 结果已在 4.2 前序记录中分别核验。矩阵还通过 wrapper 单测 `any_backend_cancellation_is_exactly_not_supported`（1 passed）验证 Any fallback，并通过 `cancellation_cleanup_tests`（5 passed）覆盖清理屏障失败/超时、owner close 竞态和错误保真。

新增 `unsupported_sqlite_generated_key_selectors_return_0a000` 先验证了此前缺陷：列索引/列名选择返回 `UnsupportedOperation`，与矩阵要求不符；修复后用同一测试确认两种模式在 PreparedStatement 创建前都返回 SQLState `0A000`。SQLite `ReturnGeneratedKeys` 查询和更新的执行中取消正向入口继续由矩阵对应测试覆盖。

Druid 池化代理回归：`cargo test -p druid --lib cancellation_control --locked --offline`：4 passed，覆盖普通/Prepared/Callable 代理转发、缓存 checkout 和旧租约句柄失效；`cargo test -p druid --test callable_cache_lifecycle_test --locked --offline`：6 passed，覆盖 Callable 关闭、复用、参数和错误状态。直接 SQLx adapter 的批处理、缓冲/流式执行、生成键路径、同连接后续使用及不确定清理时物理连接丢弃由上述 22 个通过用例覆盖。

格式检查与 `git diff --check` 通过；本变更严格 OpenSpec 校验通过。4.6 已关闭。4.7 只剩用户文档、矩阵与最终证据同步。

4.6 源码 SHA-256：

| 文件 | SHA-256 |
|---|---|
| `crates/druid-wrapper/src/sqlx/sqlx_connection_adapter.rs` | `4C20A7164136853848502FBB25EBB7E92C7F2407010EF6148C24B1EE7B4A2389` |
| `crates/druid-wrapper/tests/sqlx_statement_cancel_contract_test.rs` | `94CB49839C93C0ECFE575DCBDCB3CBCF9E3236A81EF4A76EC3A7E4C57604FBEC` |

## 4.7：取消文档与证据一致性

中文主架构文档新增 Statement 执行取消与池化租约说明，包含执行前获取独立句柄的时序图、取消请求结果与数据库执行终态的区别、旧句柄不得跨租约影响缓存 checkout、清理未确认时丢弃物理 owner，以及按后端和版本限定的能力表。能力矩阵已列出所有正向入口及不支持路径的精确错误；列索引/列名生成键选择现有显式 `0A000` 测试，不与取消能力混为一谈。

验证结果：

- `cargo test -p druid-wrapper --features sqlx --test sqlx_statement_cancel_contract_test --locked --offline`：22 passed、2 ignored。
- `cargo test -p druid-wrapper --features sqlx --lib cancellation_cleanup_tests --locked --offline`：5 passed。
- `cargo test -p druid-wrapper --features sqlx --lib any_backend_cancellation_is_exactly_not_supported --locked --offline`：1 passed。
- `cargo test -p druid --lib cancellation_control --locked --offline`：4 passed。
- `cargo test -p druid --test callable_cache_lifecycle_test --locked --offline`：6 passed。
- `cargo fmt -p druid-wrapper -- --check`、`git diff --check`、`openspec validate close-rdbc-jdbc42-contract-gaps --strict --no-interactive`：通过。

4.7 已关闭；取消批 4.1–4.7 完成。关闭只表示该 OpenSpec 变更中的取消合同已验收，不表示其他六项能力、P04–P07 或整个项目达到生产就绪。

4.7 文档和矩阵 SHA-256：

| 文件 | SHA-256 |
|---|---|
| `docs/druid-rust-Architecture.zh_CN.md` | `B57D2E655F364DA073A118A34B100D1EBF33E3F8E90E52057056D5917975AE1B` |
| `openspec/changes/close-rdbc-jdbc42-contract-gaps/evidence/sqlx-cancellation-capability-matrix.md` | `04E12B2087EFAD53BE148B4CA94D3C6E16BDF329A37D8D122F0EFDF8CC8E125D` |

## 4.3 进展：未确认执行清理时丢弃 owner

- `SqliteExecutionCleanupGuard` 覆盖 SQLx SQLite 的缓冲查询、生成键查询、更新、生成键更新、结果类型预检和流式游标。只有成功清除 progress handler 后 guard 才标记确认；清理返回错误、超过 3 秒，或执行 Future 在清理确认前被 drop，均将适配器和 actor 的共享不可用状态置位。
- actor 的 `Connection::is_closed()` 现在观察共享不可用状态。Druid 的 holder 回收逻辑据此选择 discard；不会因底层适配器私有 `discarded` 标记不可见而误入空闲池。
- 更新旧 Future-drop 测试：查询 Future drop 不返回“已取消”结果；若 SQLite 执行/清理状态未确认，连接返回 `ConnectionDiscarded`，然后物理连接关闭。
- 新增真实 `DruidPool` 故障场景：在递归 SQLite VM 查询期间 abort 执行任务，断言池 `active_count=0`、`idle_count=0`、`discard_count` 精确增加一次、`destroy_count` 精确增加一次；下一次获取会新建替代物理连接。
- actor 单测以注入的清理错误与超时结果验证错误载荷保留或映射为 `ConnectionDiscarded`，两种情况下 owner 均被 poison；另验证 drop guard poison、显式 close 幂等，以及成功清理后原始 SQL 异常消息与 SQLState 保留。
- 自然完成与取消并发测试并行执行 `SELECT 42` 和取消请求，只接受一个成功或 `HY008` 终态，并确认下一执行代次正常；owner 关闭测试在清理 guard 持有连接期间启动关闭，证明 owner 等待清理屏障后再关闭物理连接。
- 真实查询期限测试设置 1 秒 statement timeout，执行 SQLite VM 长查询，验证精确 `HYT00`/超时消息、handler 清理后连接可继续查询。
- 最新验证：`cargo test -p druid-wrapper --features sqlx --test sqlx_statement_cancel_contract_test --locked --offline`：19 passed、2 ignored（实库 PostgreSQL/MySQL 负向用例）；`cargo test -p druid-wrapper --features sqlx --lib cancellation_cleanup_tests --locked --offline`：5 passed；`cargo test -p druid --test callable_cache_lifecycle_test --locked --offline`：6 passed；`cargo fmt -p druid-wrapper -- --check` 与 `git diff --check` 通过。
- 4.3 的验收现已完整满足，OpenSpec 任务 4.3 已勾选；4.4–4.7 仍未关闭。

4.3 本轮源码 SHA-256：

| 文件 | SHA-256 |
|---|---|
| `crates/druid-wrapper/src/sqlx/sqlx_connection_actor.rs` | `32CDE95195C2177EB3191EE06B0DD207151F7B40F62F6FB88D774C0C727A2F4A` |
| `crates/druid-wrapper/src/sqlx/sqlx_connection_adapter.rs` | `8A9A3DBA222238AAD856A3D3212D8A2B694FE49AEC55447A5CAF7795A1E9DF04` |
| `crates/druid-wrapper/src/sqlx/sqlx_cursor_request_test.rs` | `1FF4EF24C889C1C0DBD7FE958A3E035068C9AD703DA9ED4445B00868A53EB7B2` |
| `crates/druid-wrapper/tests/sqlx_statement_cancel_contract_test.rs` | `F67C2706F574ADA4430CE5475D09696BFD552E36C2FA344123FBD9542FC56A69` |
