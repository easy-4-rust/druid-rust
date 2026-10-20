# rdbc-jdbc-method-conformance Specification

## Purpose
将对象声明、方法覆盖、真实行为和适配器能力分开计量，建立源自冻结 Java 8 的方法分母及可追溯验证，让有限增量能够客观关闭，也让未支持或未验证的能力被准确呈现。

## Requirements

### Requirement: MC-01 Java 来源定义方法分母

方法清单 SHALL 从冻结的 Java 8 java.sql/javax.sql 来源提取，对公开声明方法、构造器、常量、重载和默认行为建立独立身份，并显式描述继承来源；不得从现有 Rust 方法反推完整 JDBC 分母。

#### Scenario: Java 方法在 Rust 中不存在
- **WHEN** Java 来源包含某个监听器方法而 Rust 没有对应入口
- **THEN** 清单将其标记为缺口，不因所属对象已 DECLARED 或 Rust 迁移表通过而忽略

#### Scenario: 多个 Java 重载合并到一个 Rust 描述符
- **WHEN** 多个 Java 签名映射到同一个类型化 Rust 方法
- **THEN** 每个 Java 签名保留独立身份及参数、NULL、错误、默认行为和对应场景，不按一个方法消除分母

### Requirement: MC-02 证据等级和能力声明分离

清单 SHALL 分别记录标准入口、合同测试、具体适配器、逻辑代理和实库证据；能力声明为支持时 SHALL 有对应正向行为证据，不支持时 SHALL 有精确错误证据，缺失证据必须保持未验证。

#### Scenario: 对象可导入但无方法运行证据
- **WHEN** 一个 Rust 类型通过公开入口编译检查
- **THEN** 仅将其声明维度标为通过，不自动晋升方法、适配器或实库证据

#### Scenario: 可选后端未启用
- **WHEN** 某后端没有启用或缺少其运行环境
- **THEN** 记录 NOT_RUN 及原因，不能作为支持证明，也不能将已要求的验收项改成已完成

### Requirement: MC-03 兼容变更精确登记

公开 API 或可观察行为变化 SHALL 对应具体 Java 方法、Rust 入口、消费者迁移和实际测试；错误类型、SQLState、对象身份及资源语义 SHALL 被纳入验证，不以批量更新快照或放宽 allowlist 代替审查。

#### Scenario: 语句属性过滤器拒绝或改写参数
- **WHEN** 标准语句的属性设置或取消经过注册的过滤器，过滤器拒绝、包围调用或改写同一操作的参数
- **THEN** 拒绝保留原始错误并仅计错一次，不提前修改代理属性；成功后代理缓存采用最终生效参数，预编译和调用语句继承同一路径
- **AND** 参数改写仍须通过原操作的边界校验，过滤器错误不得借用驱动属性不支持回退而静默成功；取消不支持始终保留为错误

#### Scenario: 游标名称的 NULL 与空字符串保持可区分
- **WHEN** 消费者通过标准可空游标名称入口传入 None、Some("") 或非空名称
- **THEN** 逻辑代理、过滤器与支持该操作的驱动收到相同的可空值；不将 None 转成空字符串
- **AND** 默认实现对 Some 委托既有非空入口，对 None 明确报告不支持；逻辑代理不得将 NULL 不支持结果回退成成功

#### Scenario: 结果状态过滤器拒绝先于驱动与游标处置
- **WHEN** 属性读取过滤器拒绝普通、预编译或可调用语句的 `get_update_count` 或有效处置参数的 `get_more_results`
- **THEN** 原始 SQL 异常保留并计错一次，拒绝发生在底层驱动访问和当前结果集处置之前
- **AND** 已关闭语句直接拒绝访问，不再次调用过滤器；正常关闭后连接仍可归还并复用

#### Scenario: 生成键读取拒绝保留结果资源及错误计数
- **WHEN** 属性读取过滤器拒绝普通、预编译或可调用语句的 `generated_keys`
- **THEN** 在创建辅助结果资源或读取驱动生成键之前返回原始 SQL 异常且计错一次；既有当前游标仍保持开放
- **AND** 普通语句的驱动生成键错误也走标准异常分类；已关闭语句不调用过滤器

#### Scenario: 结果集三个独立属性通过真实 getter 访问
- **WHEN** 读取语句的结果集类型、并发模式或保留方式
- **THEN** 标准合同分别提供可覆盖的 typed getter，旧实现默认委托 options；池化代理分别分派同名过滤器并调用底层 getter
- **AND** 仅对该 getter 的精确不支持标签使用创建参数回退；其他 SQL 错误保留并计错，不以聚合 options 冒充三个独立驱动调用

#### Scenario: 当前结果集读取与整数推进的过滤器优先级
- **WHEN** 调用 get_result_set 或 get_more_results_current（包括参数 0）时过滤器拒绝对应读取/推进操作
- **THEN** 原始 SQL 异常计错一次，拒绝先于结果读取、游标处置及整数校验
- **AND** 没有拒绝过滤器时非法整数仍返回参数错误且不处置资源；逻辑关闭仍先于过滤器检查

#### Scenario: 固定37例实时语句差分
- **WHEN** 对冻结的 Java Druid 1.2.28 原始语句测试与 Rust 执行同一份37例输入
- **THEN** 每例独立观察异常类别、错误码、SQLState、消息、错误计数、活动/空闲连接和关闭后复用；全部输入和两端输出保留
- **AND** 两项已批准 closeOnCompletion 差异保留分母并独立标记，其他差异不得以泛化白名单放行

#### Scenario: Mock 分类器与物理关闭清理实时差分
- **WHEN** 执行冻结 ExceptionSorterTest 的原始 test_exceptionSorter，并在相同 MockDriver、mysql 类型及预编译缓存配置下关闭物理连接后关闭语句
- **THEN** 原断言必须运行通过；两端分别记录分类器、异常类、错误码、可空 SQLState/消息、丢弃计数及逻辑连接关闭后的活动/空闲计数
- **AND** 完整原始单例分母和来源哈希保持冻结，任何未批准的差异使实时比较失败

#### Scenario: 固定七例 Callable 实时差分
- **WHEN** 对冻结 Java Druid 1.2.28 的 DruidPooledCallableStatementTest 执行全部七例
- **THEN** 保留原 SQL、执行标志、过滤器启停、wasNull 拒绝标志、按索引/名称 String 目标及 Callable/Prepared 解包角色；运行原断言并保留错误字段、计数、关闭与连接复用观测
- **AND** 类型化取值的错误计数及原生/逻辑对象身份差异必须独立报告，未批准时比较失败，不以其他字段一致删除这些用例

#### Scenario: Callable wasNull 经过可包围的标准过滤器
- **WHEN** 池化 CallableStatement 调用 was_null，已注册过滤器拒绝、短路、包围驱动读取或改写布尔结果
- **THEN** 按注册顺序分派，拒绝发生在底层 was_null 前；原始 SQL 错误仅分类和计错一次，并保留既有失败语句退出缓存策略
- **AND** 已关闭语句或失效租约先于过滤器和驱动拒绝访问；无过滤器或默认过滤器保持原驱动结果，成功关闭后连接仍可复用

#### Scenario: 固定六例 Pool 获取与回收实时差分
- **WHEN** 对冻结 Java Druid 1.2.28 的 getConnection、recycle、recycle2 三个原始测试类执行全部六例
- **THEN** 保留 NULL 凭据、最大活动连接数、SQL、事务状态及回滚过滤器输入；运行原断言后逐例比较异常字段、借出/归还计数、逻辑关闭和替换连接复用
- **AND** 原始类中的非 test 方法不加入 JUnit 分母，新增补充观测在两端对称执行；未经批准的错误分类或资源关闭差异使实时比较失败

#### Scenario: 固定两例 Wall 多语句拒绝实时差分
- **WHEN** 对冻结 Java Druid 1.2.28 的 WallMultiStatementTest 两个原始方法执行相同 SQL 及 Oracle/MySQL 方言入口
- **THEN** 保留原始断言、SQL 和两例分母，分别记录并逐项比较两端布尔校验结果，任何未一致结果使实时差分失败
- **AND** 默认运行中忽略的 Java 用例不能充当实测证据；这两例不替代整个 Wall、SQL 安全或 P07 差分验收

#### Scenario: 预编译与可调用语句属性错误取消缓存资格
- **WHEN** 已启用语句缓存的 PreparedStatement 或 CallableStatement 在最大字段长度、最大行数、查询超时、抓取方向、抓取大小或三个结果集属性的读取中收到过滤器或驱动 SQL 错误
- **THEN** 原始 SQL 错误只分类和计错一次，逻辑关闭时底层失败语句只关闭一次且不再进入缓存；同一 SQL 必须重新创建语句
- **AND** 覆盖首次创建和已缓存句柄检出两条路径；保留既有缓存计数规则，健康替代语句仍可复用，物理连接无需因普通非致命属性错误而被替换

#### Scenario: 固定五例 ResultSet 跨语言观测
- **WHEN** 对照冻结 Java Druid 1.2.28 的 DruidPooledResultSetTest 四例及 DruidPooledResultSetTest2 一例
- **THEN** 执行原 SQL、绑定值、解包目标、类型化读取重载和完整导航顺序，保留逐例结果、异常及连接生命周期观测
- **AND** 解包的原生/逻辑身份、类型化读取和抓取峰值按实测报告；未经批准的差异保持失败，不删减用例或静默扩充允许差异

#### Scenario: 结果集对象与资源读取错误完整计错一次
- **WHEN** 对象的普通/类型映射/目标类型读取、标量与资源描述符读取、字节流或字符流读取收到过滤器或驱动错误
- **THEN** 统一记录数据源、结果集及连接错误，并只向创建语句计错一次；不重复执行致命错误分类，不增加 SQL 执行次数
- **AND** 保留原始异常类别、错误码、SQLState 和消息；关闭及归还连接仍遵循既有所有权合同

#### Scenario: 默认实现新增了方法
- **WHEN** 第三方以最小实现接入新增可选合同
- **THEN** 外部编译合同证明其兼容路径，默认不支持结果有测试，不静默声称功能可用

#### Scenario: 发生有意破坏性变更
- **WHEN** 时间转换或标准接口签名产生兼容差异
- **THEN** 差异逐项关联迁移说明和验证记录；未审阅的 SemVer 失败仍保留失败状态

#### Scenario: 保留已批准的 JDBC closeOnCompletion 标准实现
- **WHEN** 对照 Java Druid 1.2.28 的 `Statement#closeOnCompletion` 和 `#isCloseOnCompletion` 不支持行为
- **THEN** 按 2026-10-20 用户确认，Rust 保留 JDBC 标准实现，登记为明确允许的行为差异，不把 Java 的不支持结果冒充等价结果
- **AND** 结果集依赖释放、语句自动关闭及关闭状态仍必须以 Rust 生命周期合同验证；此批准不豁免其他 Java/Rust 差异

#### Scenario: 致命 SQL 错误后的连接拒绝保留原始原因
- **WHEN** Oracle 或 OceanBase Oracle 分类器将错误码 28、SQLState `xxx`、消息 `xx` 的 SQL 异常判为致命，随后调用同一逻辑连接的 `commit`
- **THEN** 连接操作被拒绝，原始 typed SQL 错误通过 source 链保留，不将该拒绝误报为连接泄漏
- **AND** Rust 以既有产品错误边界保存原始异常值，不仅凭字段相等宣称 Java 引用身份已经保留；正常关闭或租约超时仍遵循各自既有合同

### Requirement: MC-04 验收范围和终止条件固定

本变更 SHALL 按固定批次验收；同一未修改输入、环境和命令的失败不得无限重跑。通过目标测试及受影响回归后 SHALL 结束该批，新发现的范围外问题只登记，不自动扩展实现范围。

#### Scenario: 重复出现同一失败
- **WHEN** 已有失败在同一输入下再次出现且没有新的可验证原因
- **THEN** 停止重跑，报告失败签名、影响、已尝试动作和所需条件，不自动增加超时或缩减断言

#### Scenario: 本变更所有场景完成
- **WHEN** 所有任务完成且各规范场景均有新鲜证据、兼容变更已处置
- **THEN** 只关闭本变更，不自动关闭原 P04–P07、整个生产就绪状态或 TCK 合规状态

### Requirement: MC-05 架构文档表达标准合同边界

中文主架构文档 SHALL 用可读的 Mermaid 图展示应用/ORM、Druid 池化代理、`druid-rdbc` 标准合同与 `druid-wrapper` 数据库适配器之间的关系，并展示连接执行链及元数据、类型、异常、资源和 DataSource/Driver/RowSet 等合同分组。图旁 SHALL 说明依赖箭头、接口特化关系，以及 owner/租约和数据库 I/O 的责任归属；图示不得暗示具体后端能力均已实现。

#### Scenario: 读者沿图定位职责
- **WHEN** 读者从应用调用路径或某项标准能力开始查看架构文档
- **THEN** 能从图中识别统一合同、池化层和数据库适配器的边界，并由邻近说明区分模块依赖、标准对象关系与尚待验证的实现状态

### Requirement: MC-06 资源与异步结果集能力按实际合同验收

LOB、Array、Ref、SQLXML、SQLData、RowSet 及异步/缓冲结果集 SHALL 以方法和生命周期为单位记录标准合同、具体适配器能力与验证状态。资源能力集合 SHALL 与可调用操作一致；释放、关闭、错误传播、游标/owner 归属及不支持行为 SHALL 有可执行合同证据。不得因为标准类型或方法已声明，就推断后端已经实现；也不得要求所有后端支持所有可选能力。

#### Scenario: 后端只实现部分资源操作
- **WHEN** 某后端只支持资源类型的一部分标准操作
- **THEN** 能力查询只返回实际支持的操作，未支持操作返回明确的 feature-not-supported/`0A000` 或该方法规定的精确不支持错误；方法证据逐项关联该后端和测试

#### Scenario: 资源释放失败或重复释放
- **WHEN** 资源释放、结果集关闭或 owner 级清理发生错误，或调用方重复释放
- **THEN** 合同保留原始错误和资源身份，已释放/关闭状态及重试语义确定；不得吞掉错误、重复释放底层资源或把不确定 owner 重新标为可复用

#### Scenario: 异步结果集与连接 owner 生命周期
- **WHEN** 流式或缓冲结果集耗尽、显式关闭、执行取消或逻辑连接关闭
- **THEN** 游标与 actor/连接租约按已声明的所有权规则释放，关闭错误可观察，`known_row_count` 不额外消费游标；Future drop 只按已验证的清理语义记录，不推断数据库操作已取消

#### Scenario: RowSet 或其他扩展能力没有适配器实现
- **WHEN** 标准扩展入口存在，但当前后端没有对应能力或尚无运行证据
- **THEN** 分别标记精确不支持或 `NOT_RUN`，不把声明覆盖率计作适配器行为通过，也不自动扩展成所有后端的实现任务

### Requirement: Task 6 migration evidence is current and exact

The repository SHALL bind every retired standard identity to one authoritative disposition and exact target, and SHALL keep generated migration documentation synchronized with that authority. Preserved behavior SHALL have source-bound method and test parity evidence. Task 6 SHALL remain open while any required API, rustdoc, SemVer, workspace, feature, or MSRV gate lacks an actual result.

#### Scenario: Migration authority and generated views agree

- **WHEN** the Task 6 migration authority verifier runs against the current checkout
- **THEN** authority rows, the physical-standardization manifest, and the generated migration table agree field-for-field
- **AND** every non-deleted target is bound to the exact canonical standard surface

#### Scenario: Preserved method and test behavior is certified

- **WHEN** the Task 6 method and deleted-test parity gates run
- **THEN** every required baseline method and test identity has a unique current target or an explicitly approved identity-only disposition
- **AND** behavioral targets execute successfully with evidence bound to current source hashes

#### Scenario: Public and build compatibility is certified

- **WHEN** Task 6 acceptance is evaluated
- **THEN** the current public API matches its reviewed snapshot, rustdoc and SemVer checks have actual results, and required workspace, feature, and MSRV checks have actual results
- **AND** any missing tool, unrun command, stale hash, or failed required gate keeps Task 6 open

### Requirement: SQLx SQLite reports resource support truthfully
The SQLx SQLite adapter SHALL report its connection-level capabilities and return the standard feature-not-supported error for resource factories it does not implement. An adapter-level test against a real in-memory SQLite connection SHALL verify the capability descriptor and the exact SQLState `0A000` for Array, Blob, Clob, NClob, SQLXML, and Struct creation. This evidence applies only to SQLx SQLite and MUST NOT imply that another backend supports or rejects the same operation.

#### Scenario: Unsupported resource creation on SQLx SQLite
- **WHEN** a caller invokes a resource factory on a live SQLx SQLite connection for Array, Blob, Clob, NClob, SQLXML, or Struct
- **THEN** the adapter returns a feature-not-supported error with SQLState `0A000`
- **AND** the test does not interpret the resource type's presence in the standard API as backend support

### Requirement: SQLx SQLite asynchronous result sets are verified against a live driver
The SQLx SQLite adapter SHALL expose query rows through the standard asynchronous ResultSet contract. A live in-memory SQLite test SHALL verify ordered row values, end-of-stream behavior, and explicit result-set close.

#### Scenario: Iterate a live SQLx SQLite result set
- **WHEN** a caller executes a multi-row query through the standard Statement and ResultSet interfaces
- **THEN** each row is observable in order through asynchronous `next`
- **AND** `next` returns false after the last row
- **AND** explicit close completes successfully

### Requirement: SQLx asynchronous ResultSet preserves an in-flight row when next is cancelled
The SQLx asynchronous ResultSet SHALL retain a response that was accepted for an in-flight `next` call when the caller cancels that future. A subsequent `next` call on the same open ResultSet SHALL observe that response exactly once.

#### Scenario: Retry next after cancellation
- **WHEN** `next` is pending on an accepted row response and the caller cancels that `next` future
- **AND** the response later supplies one row
- **THEN** a subsequent `next` returns `true` for that row
- **AND** the row number advances once

### Requirement: SQLx SQLite resource and asynchronous ResultSet evidence stays backend-scoped
Evidence for capability reporting, unsupported resource factories, and live asynchronous ResultSet iteration SHALL identify SQLx SQLite as its tested backend and SHALL NOT imply equivalent behavior for other adapters.

#### Scenario: Report unsupported SQLx SQLite resources truthfully
- **WHEN** the live SQLx SQLite test invokes Array, Blob, Clob, NClob, SQLXML, or Struct creation
- **THEN** each unsupported factory returns SQLState `0A000`
- **AND** the test records that this result applies only to SQLx SQLite

#### Scenario: Iterate and close a live SQLx SQLite result set
- **WHEN** a standard Statement executes a multi-row query against in-memory SQLx SQLite
- **THEN** rows are returned in order, EOF is observed, and explicit ResultSet close succeeds

### Requirement: HTTP SQL and libSQL do not advertise unsupported standard connection capabilities

The HTTP SQL and libSQL adapters SHALL advertise no transaction, savepoint, catalog, schema, or callable capability until those operations have backend evidence. Calls to transaction, savepoint, and callable operations SHALL return the standard unsupported-operation error. SQL Array creation SHALL return the standard feature-not-supported SQLState `0A000`.

#### Scenario: Deterministic transport keeps unsupported capabilities explicit

- **GIVEN** a standard connection created by the HTTP SQL or libSQL factory with its deterministic transport
- **WHEN** the caller reads capabilities and invokes unsupported transaction, savepoint, callable, and SQL Array operations
- **THEN** all five flags are false and each operation returns its documented standard error
- **AND** this test certifies adapter behavior only, not live remote service behavior

### Requirement: JDBC Agent close cascades preserve every JDBC failure

The JDBC Agent SHALL attempt to close every owned ResultSet, cursor, and PreparedStatement in a close cascade. If multiple JDBC closes fail, the first `SQLException` SHALL remain primary and every later failure SHALL be attached as a suppressed exception. The corresponding cursor and prepared-statement registrations SHALL be removed even when closing fails.

#### Scenario: Cursor and its temporary statement both fail to close

- **WHEN** closing a cursor produces a ResultSet close failure followed by a temporary PreparedStatement close failure
- **THEN** the ResultSet failure is primary and the statement failure is suppressed
- **AND** both close operations were attempted

#### Scenario: Multiple cursors and their prepared statement fail to close

- **WHEN** closing a registered PreparedStatement produces failures from two related cursors and the PreparedStatement
- **THEN** a cursor failure is primary and the other two failures remain observable as suppressed exceptions
- **AND** all three closes were attempted and all session registrations were removed

### Requirement: MC-07 Rust 公开形态与冻结 Java 身份分别校验

JDBC 对象清单 SHALL 保留冻结 Java 8 对象身份、签名和来源摘要，同时将 Rust 形态和 canonical path 绑定当前公开 API 的真实声明。已审阅的形态或路径迁移 SHALL 以逐项固定映射纳入 oracle 规范化，不能批量放宽源码摘要或把静态声明提升为适配器、逻辑代理及能力运行证据。带 `#[non_exhaustive]` 的公开枚举 SHALL 被识别为枚举；关联常量 SHALL NOT 冒充枚举成员。

#### Scenario: 六个类型形态和授权分类路径迁移
- **WHEN** 当前 RDBC public API 对五个异常载荷及 `DriverRegistry` 暴露 struct，对授权分类暴露真实枚举成员
- **THEN** 清单使用相应 DTO 形态和真实成员路径；冻结 Java 来源校验仍通过，其他字段和 PLANNED 证据状态不变

#### Scenario: 带属性枚举与关联常量
- **WHEN** public API 含 `#[non_exhaustive] pub enum` 及其真实成员，或只含同名关联常量
- **THEN** 前者通过形态校验，后者不能满足 `ENUM_KIND` 成员要求

### Requirement: JDBC Agent Blob operations preserve driver behavior

The JDBC Agent SHALL expose a session-owned `java.sql.Blob` through the canonical RDBC Blob contract. Length, one-based byte ranges and search positions, zero-based source offsets for ranged writes, truncation, and release SHALL preserve the driver's values and SQL errors. A capability bit SHALL be advertised only after its operations have positive adapter evidence; an unsupported operation SHALL return the standard `0A000` error without a false success.

#### Scenario: H2 byte operations and original unsupported errors

- **WHEN** a caller creates an H2 Blob, writes bytes with both setter overloads, reads a one-based range, then requests search and truncation
- **THEN** byte values and the zero-based source offset are exact, while H2's unsupported search and truncation retain the original `HYC00` SQLState and `50100` vendor code

#### Scenario: HSQLDB successful search and truncation

- **WHEN** a caller writes an HSQLDB Blob, searches a two-byte pattern from a one-based start, and truncates it
- **THEN** search returns the one-based match position or no match, and truncation changes the length and remaining bytes exactly

#### Scenario: Driver rejection and unsupported operation

- **WHEN** the driver rejects a position or a method is not implemented by the adapter
- **THEN** the original driver SQLState/vendor code is retained, or the unsupported method returns `0A000`, respectively

### Requirement: JDBC Agent Blob streams and release have bounded ownership

Blob input and output streams SHALL retain their own cursor and close state under the parent Blob and session. Transfers SHALL be bounded per protocol request, not eagerly materialize an entire large Blob. Explicit release SHALL close child streams before freeing the driver Blob; a failed driver `free` SHALL leave the Blob handle retryable. Logical connection, pooled owner, and session close SHALL invalidate all child handles and attempt remaining cleanup while preserving failures.

#### Scenario: Large streamed Blob

- **WHEN** a caller reads and writes a Blob larger than one protocol page through the standard streams
- **THEN** bytes remain ordered and exact, each transfer respects the configured page bound, and EOF and explicit close are observable

#### Scenario: Release with outstanding streams

- **WHEN** a Blob is freed or its owner closes while a child stream remains open
- **THEN** later stream and Blob operations fail as closed, underlying resources are released at most once, and a failed Blob release remains retryable

### Requirement: JDBC Agent Array resources have session ownership

The JDBC Agent SHALL register each created `java.sql.Array` under an opaque session-local handle. Explicit free SHALL invoke the driver once and invalidate the handle only after successful release. A failed release SHALL remain retryable. Closing a logical connection or session SHALL attempt to free all remaining arrays, aggregate failures, and continue closing the connection.

#### Scenario: Explicit release and logical close

- **WHEN** a client frees a registered Array and later closes the logical connection
- **THEN** the Array is released exactly once, cannot be read through its former handle, and unrelated session resources remain isolated

#### Scenario: Failed release

- **WHEN** a driver's `Array#free` fails
- **THEN** the handle remains registered for retry, and closing the session still attempts resource cleanup

### Requirement: JDBC Agent Array methods reflect real driver behavior

The JDBC Agent adapter SHALL expose only Array operations implemented by its protocol. Values, SQL NULL, one-based ranges, metadata, result-set cursors, and release SHALL preserve the driver contract or propagate the driver's error. Unsupported overloads SHALL return the standard SQLState `0A000` before accessing the driver.

#### Scenario: H2 Array roundtrip

- **WHEN** a caller creates an H2 Array with mixed values and SQL NULL, reads its metadata, range, and cursor, then frees it
- **THEN** the returned values and one-based indices match H2 and further reads fail with the standard closed-resource behavior

### Requirement: Static JDBC declarations have explicit capability disposition

The Task 8 semantic verifier SHALL allow `NOT_APPLICABLE` runtime capability state only for static DTO, enum-kind, or module declarations with no adapter or logical implementation, excluding the active DriverManager registry. It SHALL reject the same disposition on behavioral contracts or implemented rows. This evidence-only classification SHALL leave the frozen Java source mapping unchanged and SHALL NOT satisfy pending adapter or logical behavior requirements.

#### Scenario: Static value declaration

- **WHEN** a DTO, enum-kind, or module has neither adapter nor logical implementation
- **THEN** the inventory may mark its runtime capability as `NOT_APPLICABLE`, retaining `V0_STATIC` evidence and the original source digest

#### Scenario: Behavioral declaration

- **WHEN** a trait, callback, active registry, or implemented row claims static `NOT_APPLICABLE` capability
- **THEN** verification fails without modifying the Java source oracle

### Requirement: JDBC Agent Clob resources have retryable session ownership

The JDBC Agent SHALL register each created `java.sql.Clob` under a session-local opaque handle. Explicit free SHALL close child streams before invoking the driver and SHALL invalidate the handle only after a successful driver free. A failed driver free SHALL retain the handle for retry. Logical connection and session close SHALL attempt all remaining cleanup, aggregate failures, and continue closing the JDBC connection.

#### Scenario: Failed release and later close

- **WHEN** the driver rejects the first `Clob.free()` and later accepts a retry
- **THEN** the original handle remains usable for retry, successful free invalidates it, and the driver is not freed twice

### Requirement: JDBC Agent Clob operations preserve character semantics and driver errors

The standard adapter SHALL preserve Java UTF-16 unit length, one-based Clob positions, zero-based source offsets, SQL NULL boundaries, and driver SQLState/vendor codes for supported operations. It SHALL report only positively verified capabilities; unsupported operations SHALL return SQLState `0A000` without a false success.

#### Scenario: Character operations against real JDBC drivers

- **WHEN** an external RDBC consumer writes and reads a Clob containing a supplementary character and tests range, search, and truncation against H2 and HSQLDB
- **THEN** UTF-16 units and successful driver results are exact, while unsupported driver operations retain their original SQL errors

### Requirement: JDBC Agent Clob streams use bounded transfers and parent ownership

Character and ASCII streams SHALL transfer bounded pages, preserve order and EOF, and become invalid after explicit free, logical close, pooled-owner close, or session close. Cancellation SHALL not leak a child handle or replay accepted data.

#### Scenario: Large streamed Clob

- **WHEN** a consumer reads and writes more than one protocol page and then closes the parent with a child still open
- **THEN** data remains exact within the page bound and later child operations fail as closed

### Requirement: JDBC Agent NClob preserves national-character resource identity

The JDBC Agent SHALL create an actual driver `java.sql.NClob` and expose it as the canonical RDBC `NClob`, whose inherited Clob methods retain Java UTF-16 unit and one-based position semantics. A successful ordinary Clob creation SHALL NOT be treated as proof that NClob creation succeeded. Driver SQLState and vendor code SHALL be preserved on NClob failures.

#### Scenario: Driver creates an NClob

- **WHEN** an external RDBC consumer invokes `create_n_clob` against a driver that supports it
- **THEN** the returned resource is an NClob, supports its positively verified inherited operations, and preserves supplementary and isolated UTF-16 units

#### Scenario: Driver rejects NClob creation or an inherited operation

- **WHEN** the real driver rejects NClob creation or a requested operation
- **THEN** the original SQLState and vendor code are visible and no ordinary Clob is substituted

### Requirement: JDBC Agent NClob has session and child-stream ownership

Each NClob SHALL have a session-local opaque handle. Explicit free SHALL close child streams before freeing the driver resource, retain the handle after failed free for retry, and invalidate it after successful free. Logical connection, pooled-owner, and session close SHALL invalidate outstanding NClob and child handles while attempting remaining cleanup and preserving close errors.

#### Scenario: Free with outstanding child stream

- **WHEN** a consumer frees an NClob while a reader or writer remains open
- **THEN** subsequent parent and child operations fail as closed, the driver resource is freed at most once, and any failed free remains retryable

#### Scenario: Pooled owner closes a logical NClob

- **WHEN** a pooled logical connection or its owner closes with an NClob and child stream still open
- **THEN** those handles cannot operate after close and the underlying resource cleanup follows the owner lifecycle

### Requirement: JDBC Agent NClob capability claims follow real-driver evidence

The adapter SHALL advertise only NClob operations proven by positive real-driver tests. Inherited methods not implemented by the adapter SHALL report SQLState `0A000`; driver-specific unsupported results SHALL preserve the driver's original errors. Bounded character and ASCII stream transfers SHALL retain ordering, EOF, cancellation, and parent invalidation without materializing an entire large NClob in one protocol response.

#### Scenario: H2 preserves unsupported search and truncation errors

- **WHEN** an external consumer exercises NClob search and truncation against H2 2.3.232
- **THEN** the adapter preserves the driver's SQLState `HYC00` and vendor code `50100` for each operation, and does not claim either operation as successful H2 behavior

#### Scenario: H2 rejects NClob ASCII writer creation

- **WHEN** an external consumer requests an ASCII output stream from an H2 2.3.232 NClob
- **THEN** the adapter preserves the driver's `HYC00`/`50100` error while the NClob remains releasable

#### Scenario: HSQLDB verifies successful search and truncation

- **WHEN** an external consumer exercises NClob search and truncation against HSQLDB 2.7.4
- **THEN** search returns the verified one-based position and truncation leaves the verified UTF-16 length and contents; the capability mask includes only positively verified adapter operations

#### Scenario: More than one protocol page

- **WHEN** a consumer writes and reads an NClob larger than one protocol page
- **THEN** the returned UTF-16 units are exact, each transfer respects the page bound, and closing the parent invalidates the stream; HSQLDB also verifies successful bounded ASCII transfer

### Requirement: JDBC Agent callable capability reflects executable operations

The JDBC Agent adapter SHALL return an owned standard `CallableStatement` from `Connection::prepare_call` only when the current session can prepare and execute the call. Its reported callable capability SHALL reflect the implemented protocol and connected driver, not the presence of a Rust trait declaration alone. Other adapters SHALL retain their verified unsupported behavior.

#### Scenario: Supported driver

- **WHEN** a connected driver supports a callable operation and an application prepares it through the standard connection
- **THEN** the returned handle can register OUT parameters, bind IN parameters, execute, read OUT values, and close within the same session

#### Scenario: Unsupported driver or method

- **WHEN** the driver or protocol cannot perform a requested callable operation
- **THEN** the capability and operation report a specific standard unsupported SQL error, without a false successful value or leaked server handle

### Requirement: Callable parameter and result semantics survive the protocol

The Agent protocol SHALL preserve indexed versus named parameter selection, SQL type and optional scale/type name, SQL NULL and `was_null`, typed OUT conversion, the current result sequence, and SQLState/vendor-code/cause information. It SHALL reject invalid 1-based indexes and unknown names before claiming execution success. Re-execution SHALL invalidate stale OUT values and result handles.

#### Scenario: IN and OUT round trip

- **WHEN** a caller registers an OUT parameter, binds an IN value, executes and reads the result against a real JDBC database
- **THEN** the value and null status match the driver, with the specified selector and SQL type retained

#### Scenario: Standard max-rows property reaches each callable result set

- **WHEN** a caller sets a positive `Statement::set_max_rows` limit before executing a call that returns multiple result sets
- **THEN** the JDBC statement applies that limit to every result set, including results reached through `getMoreResults`, without discarding their order

#### Scenario: Failure and retry

- **WHEN** a call fails or is cancelled, then the statement is executed again
- **THEN** the old OUT values and cursors are inaccessible, the original JDBC error remains observable, and the same open CallableStatement can execute again

#### Scenario: Cancel the current execution

- **WHEN** a caller cancels an active callable execution or its result-page fetch
- **THEN** only that execution is interrupted, its dependent cursors are released, and cancellation cannot affect a later execution on the same statement

### Requirement: Callable ownership is bounded by session and statement lifetimes

The Agent SHALL close callable statements and dependent cursors on explicit close, session close, and connection failure. Cancellation SHALL release the current execution's dependent cursors while keeping the callable statement open for retry. A handle from another session or a closed statement SHALL fail without reaching its former JDBC object. Callback and resource cleanup SHALL happen once.

#### Scenario: Close and cross-session access

- **WHEN** a caller closes a callable statement or sends its handle through another session
- **THEN** the Agent rejects subsequent operations and releases the original driver resources exactly once
