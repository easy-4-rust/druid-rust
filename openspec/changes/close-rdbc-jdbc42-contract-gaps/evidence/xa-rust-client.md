# JDBC Agent Rust XA 客户端证据

本记录按 6.5.1–6.5.5 分阶段追加。6.5.1 关闭 RPC 合同测试与普通连接兼容性基线；6.5.2 关闭显式 XA 数据源入口、标准消费者、协议往返及未协商能力错误合同。6.5.3 owner/租约故障注入、6.5.4 Rust/PostgreSQL 端到端恢复和 6.5.5 消费者迁移/API 文档仍未关闭。

## 6.5.1 XA RPC 合同与普通连接兼容

### 环境和命令

- 环境：Windows x86_64，Cargo `1.97.1 (c980f4866 2026-06-30)`，rustc `1.97.1 (8bab26f4f 2026-07-14)`。
- `cargo fmt -p druid-wrapper -- --check`：退出码 `0`。
- `cargo test -p druid-wrapper --features jdbc-agent jdbc_agent_xa_resource --lib --locked --offline`：退出码 `0`，5 项测试通过。
- `cargo test -p druid-wrapper --features jdbc-agent ordinary_open_session_payload_keeps_the_existing_contract --lib --locked --offline`：退出码 `0`，1 项测试通过。
- `openspec validate close-rdbc-jdbc42-contract-gaps --strict --no-interactive`：退出码 `0`。
- `git diff --check`：退出码 `0`。

### 行为覆盖与边界

- 假传输层逐项断言 `xa_start/end/prepare/commit/rollback/forget/recover` 操作、Xid 字节、flags、一阶段标志、只读投票和 recover Xid 字节往返。
- 验证普通 session XA 请求在发送 RPC 前失败，错误保留 SQLState `0A000`；另验证 XA 资源管理器身份必须来自共享身份凭证。
- 验证超时请求及驱动拒绝时的结构化 vendor code/SQLState 保留，非法 prepare 投票和无法表示的超时不发送 RPC。
- 捕获普通连接 factory 发出的 `open_session` 载荷，并与既有 URL、连接属性、验证 SQL 形状逐字段比较；生产运行时握手代码未改动。
- 本组测试使用确定性假传输，只证明 Rust RPC 映射与负向合同。它们不证明 Agent 实际协商、session 共享、XA owner 关闭语义或真实数据库恢复；这些仍由 6.5.2–6.5.5 验收。

### 源码身份

SHA-256（对应本阶段验证的当前输入）：

| 文件 | SHA-256 |
|---|---|
| `crates/druid-wrapper/src/jdbc_agent/jdbc_agent_xa_resource.rs` | `9C45BB04ABA0DBB5436CD6E584B7265F3D26B6BF58859F0F94DEAE74E482EBD3` |
| `crates/druid-wrapper/src/jdbc_agent/jdbc_agent_connection.rs` | `2BACA0D888372DAE67953FEAFAE8359B085C165E914FFDD263A4B8C0A8D1F414` |
| `crates/druid-wrapper/src/jdbc_agent/jdbc_agent_connection_factory.rs` | `DBA404D2729F22B2E2497AFD9F36FCA7801BFE0C2200456C22B702B5824D216A` |
| `crates/druid-wrapper/src/jdbc_agent/mod.rs` | `7FD783D1E20461A77CD0A4D3AB61E81B5EF1E061BF6E584AF5087D541AA61134` |

## 6.5.2 显式数据源与标准资源入口

### 行为与范围

- `JdbcAgentXaDataSource` 通过调用方提供的 XADataSource 类名和属性建立独立 session；不发送普通 `url`/`properties` 字段，不从 URL 推断 XA。普通 factory 的既有 `open_session` 载荷测试继续通过。
- 标准入口实现 `CommonDataSource + XaDataSource`，返回标准 `XaConnection`/`XaResource`。外部 integration consumer 只用 `druid-rdbc` 的 trait 类型取得资源并调用 `start`，不引用 Druid 产品的 XA 资源类型。
- XA RPC 合同测试验证完整 Xid、flags、prepare vote、one-phase 参数、recover 字节和结构化错误；未协商 XA 在发出 XA RPC 前精确返回 SQLState `0A000`。session 清理同时失败时仍保留 `SQLFeatureNotSupportedException`/`0A000`，消息附带清理错误，且确实尝试关闭 session。
- 同一 session 的逻辑 SQL 和 XA RPC 共用传输串行化路径；定向单测已验证 SQL 操作、逻辑句柄关闭、XA start 的操作序列。该证据是 Rust 适配器层确定性传输测试，不冒充真实 Java Agent 或 PostgreSQL XA 恢复结果。

### TDD 与验证

- 新增清理失败时保持 `0A000` 的测试后，首轮执行因断言看到 `RdbcError::Other("XA capability was not negotiated and session cleanup failed…")` 失败；修复后测试通过。这确认测试实际捕获了被规格禁止的错误降级。
- `cargo test -p druid-wrapper --features jdbc-agent --lib --locked --offline`：33 passed。
- `cargo test -p druid-wrapper --features jdbc-agent --test jdbc_agent_xa_external_consumer --locked --offline`：2 passed；从外部 crate 编译标准 XA 数据源约束与事务管理器式标准资源调用。
- `cargo test -p druid-wrapper --features jdbc-agent jdbc_agent_xa_data_source --lib --locked --offline`：5 passed。
- `cargo test -p druid-wrapper --features jdbc-agent ordinary_open_session_payload_keeps_the_existing_contract --lib --locked --offline`：1 passed。
- `cargo test -p druid-wrapper --features jdbc-agent jdbc_agent_xa_resource --lib --locked --offline`：5 passed；覆盖 start/end/prepare/commit/rollback/forget/recover、Xid、flags、投票、超时和结构化错误。
- `mvn -f agents/jdbc-agent/pom.xml -Dtest=XaAgentProtocolTest test`：BUILD SUCCESS，2 passed、0 failed、0 skipped；使用 Testcontainers PostgreSQL 18.6。该测试验证 Java Agent XA 协议端；不作为 Rust 客户端跨 session recovery 的替代证据。
- `cargo fmt -p druid-wrapper -- --check`、`git diff --check` 与 `openspec validate close-rdbc-jdbc42-contract-gaps --strict --no-interactive`：均通过。Rust 工具链为 rustc/cargo 1.97.1。

### 边界

本节不关闭 6.5.3 的连接池归还、owner 故障隔离、重复关闭和传输不确定性注入；也不关闭 6.5.4 的 Java Agent + PostgreSQL Testcontainers 跨 session recovery。没有真实 Agent 启动和数据库读回结果前，不得声称 XA 恢复链已完成。

### 本阶段源码 SHA-256

```text
crates/druid-wrapper/Cargo.toml EC13218217DED5D1A61E7BC7701FF81117B819CD54FA028DD24D70540719060E
crates/druid-wrapper/src/jdbc_agent/jdbc_agent_connection.rs FCA50EBCCBD0A9ECFAD179A577CDD4E0FECB4E0EB985C3F1E76F6176F589A080
crates/druid-wrapper/src/jdbc_agent/jdbc_agent_xa_connection.rs DAA02B337BBFEA6D512A0748AB0CAD983E15C573D0853DD72F1AADF0315A8A78
crates/druid-wrapper/src/jdbc_agent/jdbc_agent_xa_data_source.rs 72949D05FA361CAE09BEDCD2A3FA3089406E2F048A2D2EA6A8EE7A79BBC3C13D
crates/druid-wrapper/src/jdbc_agent/jdbc_agent_xa_resource.rs 9C45BB04ABA0DBB5436CD6E584B7265F3D26B6BF58859F0F94DEAE74E482EBD3
crates/druid-wrapper/src/jdbc_agent/jdbc_agent_xa_session.rs 09C11905F436E9AF97FFB52C93177E8495F024466D6522D30DF112FA153FEC3D
crates/druid-wrapper/src/jdbc_agent/jdbc_agent_xa_session_transport.rs 4C10FC20A161A949DB59F61BF7C03A6571A1286B77E13CEBCF82BD71B5F15AFF
crates/druid-wrapper/src/jdbc_agent/mod.rs BDF3AB1303E031368C7FB6D5313AAD848D7E11C585B836CEB8B82B985F64B406
crates/druid-wrapper/src/standard_adapter_executor.rs 5CC69A7088A170BFC8F07E40EB6C8CA1FD49C97EAE7465E29E2A1A4FBBB7CBAC
crates/druid-wrapper/tests/jdbc_agent_xa_external_consumer.rs DB1BAF5B4674BB867750708CC6A051B4B69CA12B429DC428AD467966EA9A7C7D
```

## 6.5.3 进行中：XA owner 事件与 sticky 故障隔离

### 已完成的适配器层证据

- `JdbcAgentXaConnection` 实现标准连接事件监听器注册/移除，并为 `ConnectionEvent` 保留稳定的 `PooledConnectionSource`。该来源是与 XA owner 共用状态的标准池化代理，持有方可通过它继续获取逻辑租约或关闭 owner；不会形成自引用强环。
- 逻辑连接关闭成功后仅通知一次；XA owner 和资源保持可用。再次借出时旧代次失效；被动关闭旧代次也产生一次关闭通知。回调执行前释放 session 与租约锁。
- SQL/XA 请求导致底层 transport 不可用时，session 进入 sticky 隔离状态，之后请求快速失败；保留原始 SQL 异常的 SQLState， owner 错误事件最多通知一次。owner 关闭失败原样返回并发送连接错误事件；重复关闭仍幂等。
- 为避免错误宣称语句事件支持，JDBC Agent XA `PooledConnection` 的 statement listener 操作继续使用标准默认 `0A000`，直到 PreparedStatement 事件路径也被实现和验证。

### 已运行验证

- `cargo test -p druid-wrapper --features jdbc-agent --lib --locked --offline`：37 passed。
- `cargo test -p druid-wrapper --features jdbc-agent --test jdbc_agent_xa_external_consumer --locked --offline`：2 passed。
- `cargo test -p druid-wrapper --features jdbc-agent jdbc_agent_xa_connection::tests --lib --locked --offline`：3 passed，覆盖逻辑关闭事件、致命 SQL 错误通知及 owner 关闭失败。
- `cargo fmt -p druid-wrapper -- --check`、`git diff --check`、`openspec validate close-rdbc-jdbc42-contract-gaps --strict --no-interactive`：均通过。

### 尚未关闭的验收

- 外部标准 XA 池消费者的借出/归还行为尚未实现；当前 `jdbc_agent_xa_external_consumer` 只证明公共接口兼容。
- CodeGraph 核验到 Druid 原生池由普通 `ConnectionFactory` 创建 `Connection`，暴露 `DruidPooledConnection`；当前没有 XA owner 的池化入口。因此本项要求先验证并记录此产品边界，不将普通 Druid 代理测试冒充 XA 池正向证据。若实际验收要求 XA owner 由 Druid 原生池管理，需要单独设计 XA 池代理 API 与生命周期，不在此处假设已存在。
- 适配器层 fake transport 测试不替代 Java Agent/PostgreSQL 实库恢复；6.5.4 仍需从 Rust RDBC 消费入口验证跨 session `recover` 与数据库读回。

### 本阶段源码 SHA-256

```text
crates/druid-wrapper/src/jdbc_agent/jdbc_agent_xa_connection.rs BA2CBDA4075D4AE532859291725FC8642746679521DA388BA46CD991A20C259D
crates/druid-wrapper/src/jdbc_agent/jdbc_agent_xa_session.rs 40D677BED2879CBF5B5CDFB0545532401706817DCEC9E9C727F52A7BA48A16FB
crates/druid-wrapper/src/jdbc_agent/jdbc_agent_xa_session_transport.rs 2E163C78560341C04A87872840CC6BC5917858310AE9AFD830E3BC0F7789502B
```

