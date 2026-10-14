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

#### Scenario: 默认实现新增了方法
- **WHEN** 第三方以最小实现接入新增可选合同
- **THEN** 外部编译合同证明其兼容路径，默认不支持结果有测试，不静默声称功能可用

#### Scenario: 发生有意破坏性变更
- **WHEN** 时间转换或标准接口签名产生兼容差异
- **THEN** 差异逐项关联迁移说明和验证记录；未审阅的 SemVer 失败仍保留失败状态

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
