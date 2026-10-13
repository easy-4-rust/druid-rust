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

