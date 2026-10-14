# JDBC Agent Rust XA 客户端证据

本记录按 6.5.1–6.5.5 分阶段追加。6.5.1 关闭 RPC 合同测试与普通连接兼容性基线；6.5.2 关闭显式 XA 数据源入口、标准消费者、协议往返及未协商能力错误合同；6.5.3 已由 owner/租约故障注入、外部标准池真实 Agent+H2 消费和 Druid 原生池边界核验关闭。6.5.4 Rust/PostgreSQL 跨 session 恢复和 6.5.5 消费者迁移/API 文档仍未关闭。

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

## 6.5.3 已完成：XA owner 事件与 sticky 故障隔离

### 已完成的适配器层证据

- `JdbcAgentXaConnection` 实现标准连接事件监听器注册/移除，并为 `ConnectionEvent` 保留稳定的 `PooledConnectionSource`。该来源是与 XA owner 共用状态的标准池化代理，持有方可通过它继续获取逻辑租约或关闭 owner；不会形成自引用强环。
- 逻辑连接关闭成功后仅通知一次；XA owner 和资源保持可用。再次借出时旧代次失效；被动关闭旧代次也产生一次关闭通知。回调执行前释放 session 与租约锁。
- SQL/XA 请求导致底层 transport 不可用时，session 进入 sticky 隔离状态，之后请求快速失败；SQL 错误事件保留原始 SQL 异常。非 SQL transport 错误则映射为安全的标准连接失效事件（SQLState `08006`、`SQLNonTransientConnectionException`），不复制可能含敏感信息的原始错误文本；调用方仍收到原始 `RdbcError`。fatal owner 错误事件最多通知一次。
- 真实 Agent EOF 注入测试确认外部标准池收到稳定 owner 的错误事件、标记 discard，后续 borrow 被拒绝，owner 也不再提供 XA resource。回调尝试获取 event source、重入注册新监听器并在末尾 panic；事件使用固定快照，新监听器不追溯收到当前事件，panic 被捕获，主错误返回与 sticky 隔离仍完成。
- 正向真实 Agent+H2 测试确认关闭逻辑连接只归还 lease，XA owner/resource 继续可用，同一 owner 能再次发放新代次并执行请求。
- 为避免错误宣称语句事件支持，JDBC Agent XA `PooledConnection` 的 statement listener 操作继续使用标准默认 `0A000`，直到 PreparedStatement 事件路径也被实现和验证。

### TDD 与已运行验证

- TDD 红测：真实 Agent+H2 外部消费者首次运行失败于 owner 错误事件计数，实际 `0`、预期 `1`；修复后事件、SQLState、discard、禁止重借、资源失效和原始非 SQL 错误断言均通过。
- 环境：Windows x86_64，Java `17.0.12`；JDBC Agent shaded jar `0.0.0-design`；H2 `2.3.232`。运行时 jar 为本地 ignored 构建产物，不加入 Git。
- `cargo test -p druid-wrapper --features jdbc-agent --lib --locked --offline`：37 passed。
- `cargo test -p druid-wrapper --features jdbc-agent --test jdbc_agent_xa_external_consumer --locked --offline`：2 passed、2 个环境门控实库测试忽略。
- 设置 `DRUID_JAVA_EXECUTABLE=D:\Java\jdk-17.0.12\bin\java.exe`、`DRUID_JDBC_AGENT_JAR=agents/jdbc-agent/target/druid-jdbc-agent.jar`、`DRUID_JDBC_DRIVER_JAR=agents/jdbc-agent/target/driver-libs/h2-2.3.232.jar` 后运行 `cargo test -p druid-wrapper --features jdbc-agent --test jdbc_agent_xa_external_consumer --locked --offline -- --ignored --nocapture`：2 passed；包含真实 H2 XA session、逻辑租约归还和 `System.exit(23)` Agent EOF 故障注入。listener panic 是测试中的故障注入，panic hook 输出符合预期，测试仍通过。
- `cargo fmt -p druid-wrapper -- --check`、`git diff --check`、`openspec validate close-rdbc-jdbc42-contract-gaps --strict --no-interactive`：均通过。

### 外部池与 Druid 原生池边界

- 外部池只通过 `druid-rdbc` 的 `XaDataSource`、`XaConnection`、`PooledConnection` 和 `ConnectionEventListener` 完成实际借出、归还、重借与 discard；没有依赖 Druid 私有 holder。
- CodeGraph 对源码边界的核验：`crates/druid-rdbc/src/driver/connection_factory.rs:20-21` 的 `ConnectionFactory::create` 返回 `Box<dyn Connection>`；`crates/druid/src/pool/config.rs:206,429` 的 native builder 只接收 `Arc<dyn ConnectionFactory>`；`crates/druid/src/pool/druid_pool.rs:375-376` 由 native pool 返回 `DruidPooledConnection`。因此现有 Druid native pool 没有 XA owner 池化入口。本任务按规格记录此边界，不伪称 XA owner 已由 `DruidPooledConnection` 管理；若未来要求该能力，应作为单独 API/生命周期设计。
- 6.5.3 关闭只证明 owner/租约/池事件边界，不证明真实 PostgreSQL 跨 session `recover`；6.5.4 仍须从 Rust RDBC 消费入口验证恢复及独立数据库读回。

### 本阶段源码与运行时 SHA-256

```text
agents/jdbc-agent/target/druid-jdbc-agent.jar 0C91D24212BE21DA2E12F20639FC254835EAD77026643F3F9BE915E2873BE645
agents/jdbc-agent/target/driver-libs/h2-2.3.232.jar 8DAE62D22DB8982C3DCB3826EDB9C727C5D302063A67EEF7D63D82DE401F07D3
crates/druid-wrapper/src/jdbc_agent/jdbc_agent_xa_connection.rs 3AEE549B1617DE7FB650258A1160972DF4E7945B1153BF0654A86158A027871E
crates/druid-wrapper/src/jdbc_agent/jdbc_agent_xa_session.rs 40D677BED2879CBF5B5CDFB0545532401706817DCEC9E9C727F52A7BA48A16FB
crates/druid-wrapper/src/jdbc_agent/jdbc_agent_xa_session_transport.rs 2E163C78560341C04A87872840CC6BC5917858310AE9AFD830E3BC0F7789502B
crates/druid-wrapper/tests/jdbc_agent_xa_external_consumer.rs 4727F3C826225FE3600139FF8B8407650BB2427786BC069F77BBB5831D1347D7
```

## 6.5.4 Rust RDBC/PostgreSQL 跨 session XA 恢复

### 真实后端闭环

- Rust 外部消费者显式配置 `org.postgresql.xa.PGXADataSource`，通过 JDBC Agent 建立 XA session；RDBC `Connection::exec` 在同一 session 中执行建表和两笔分支写入。两条分支分别执行 `start → SQL INSERT → end(TMSUCCESS) → prepare`，并确认 PostgreSQL 返回 `XA_OK`。
- 测试关闭逻辑 RDBC 连接及原 XA owner，销毁第一条 Agent session；随后通过同一显式 XA 数据源新建 owner/session，以 `TMSTARTRSCAN | TMENDRSCAN` 恢复并逐字段比较两个完整 Xid，再分别执行 phase-two commit 与 rollback。
- Java Testcontainers 外层在 Rust 子进程结束后，用独立 `DriverManager` 连接查询表：只读到 `id=1, payload=commit-after-recovery`，未读到 rollback 分支的 `id=2`。因此数据库最终状态由独立 JDBC 会话核验，而非 Rust 返回值或 Agent 日志推断。
- 同批运行的 Rust 普通/负向测试验证既有普通 `open_session` 载荷不变、普通 Agent session 在发送 XA RPC 前 fail closed，以及 Agent 未协商 XA 时保留 SQLState `0A000` 并清理 session。

### 环境与精确命令

- Windows 11 x86_64；Oracle Java `17.0.12`；Maven `3.9.9`；rustc `1.97.1 (8bab26f4f 2026-07-14)`；Cargo `1.97.1 (c980f4866 2026-06-30)`。
- Docker Desktop Engine `29.8.1`；Testcontainers Java `2.0.5`；`postgres:18.6` image digest `sha256:5a5a84b19854a9ffaa54082c166ff4ec27473a361e496e5ea167f298f2da9722`；PostgreSQL JDBC `42.7.14`。容器启动参数为 `postgres -c max_prepared_transactions=10`。
- Maven Agent 构建：`mvn -o -f agents/jdbc-agent/pom.xml -DskipTests package`，退出码 `0`。
- 真实容器端到端：`mvn -o -f agents/jdbc-agent/pom.xml -Ddruid.rustXaIntegration=true -Dtest=RustXaClientRecoveryIntegrationTest test`，退出码 `0`；JUnit `1 passed, 0 failed, 0 skipped`。Java harness 中实际启动的 Rust 命令为：

```powershell
cargo test -p druid-wrapper --features jdbc-agent --test jdbc_agent_xa_external_consumer --locked --offline rust_client_recovers_prepared_branches_across_sessions -- --ignored --exact --nocapture
```

该子进程退出码 `0`，实际运行 `1 passed; 0 failed; 0 ignored; 4 filtered out`；输出保存在被忽略的 `agents/jdbc-agent/target/rust-xa-client-recovery.log`。

- Rust 客户端普通/负向与 XA 回归：`cargo test -p druid-wrapper --features jdbc-agent --lib --locked --offline`，退出码 `0`，`37 passed`。其中实际通过 `ordinary_open_session_payload_keeps_the_existing_contract`、`ordinary_agent_session_fails_closed_before_sending_xa_rpc`、`explicit_xa_configuration_fails_closed_when_agent_does_not_negotiate_xa` 和 `missing_xa_capability_preserves_0a000_when_session_cleanup_fails`。
- 外部消费者常规模式：`cargo test -p druid-wrapper --features jdbc-agent --test jdbc_agent_xa_external_consumer --locked --offline`，退出码 `0`，`2 passed, 3 ignored`；三项忽略测试均需真实外部 Agent 环境，其中 PostgreSQL 项由上面的 Java harness 实际启用并通过。
- Agent 端协议回归：`mvn -o -f agents/jdbc-agent/pom.xml -Dtest=XaAgentProtocolTest test`，退出码 `0`，`2 passed`；作为 Rust 端到端闭环的额外 Agent 协议回归，不替代 Rust 客户端测试。
- `cargo fmt -p druid-wrapper -- --check`、`git diff --check`、`openspec validate close-rdbc-jdbc42-contract-gaps --strict --no-interactive`：均退出码 `0`。

### 源码身份

以下 SHA-256 是本阶段测试后核对的源码/运行制品身份；Agent shaded jar 是本地构建文件，不加入 Git：

| 文件 | SHA-256 |
|---|---|
| `crates/druid-wrapper/tests/jdbc_agent_xa_external_consumer.rs` | `6894F8C11A81B40898266248EBD9D589FE810EF9126D8BF3C76F8E606EB64494` |
| `agents/jdbc-agent/src/test/java/io/github/easy4rust/druid/agent/RustXaClientRecoveryIntegrationTest.java` | `267444E1FE0D9D967E61167DFE62801148B24EA977F820904FF0020CD0E7BC2B` |
| `agents/jdbc-agent/pom.xml` | `25B1FAEC81D887EAB23CB90E7AA05828D4CD2CFBD6B673EB000742089C009830` |
| `agents/jdbc-agent/target/druid-jdbc-agent.jar` | `21454C2C872C85CCC9990203F1E11FD466E472EC13B19900F462F49C1CECBC04` |

本项关闭 XA-03、XA-05 的 Rust 客户端真实恢复场景。它不代表 XA 配置文档/API/SemVer 审查（6.5.5）、CommonDataSource（第 7 批）、全 JDBC 覆盖门禁（第 8–9 批）或 P04–P07 已完成。

## 6.5.5 XA 对外交付与 API/SemVer 审查

### 文档和可视化

- 新增 [JDBC Agent XA 集成指南](../../../../docs/jdbc-agent-xa.md)，并从中文 README 的 JDBC Agent 章节链接。指南说明 `jdbc-agent` feature、Agent 与 JDBC driver classpath、显式 `PGXADataSource` 属性、标准 RDBC 获取路径、秘密配置保护、普通连接迁移、恢复前提和故障隔离。
- 指南 Mermaid 时序图明确事务管理器、`XaDataSource`、物理 `XaConnection` owner、逻辑 `Connection` 租约、`XaResource` 和数据库之间的关系。逻辑连接关闭只归还租约；关闭 owner 才关闭 XA session 并使资源失效。
- 恢复步骤与 6.5.4 已运行的 PostgreSQL 闭环一致：`start → SQL → end → prepare → close logical connection → close original owner → new XA session → recover(full Xid) → phase-two commit/rollback`。明确 PostgreSQL 18.6/pgjdbc 42.7.14 是已验证组合；H2 不作为跨 session 恢复证据。
- 普通 Agent 连接仍由普通 factory 创建，不能通过 URL 或驱动名称提升为 XA；未协商 XA 时保留 `0A000`。文档限制凭据示例使用环境变量，并说明 `JdbcAgentXaDataSource::Debug` 仅输出属性数量，不泄露属性值。

### 外部消费者和真实后端验证

- 在 `jdbc_agent_xa_external_consumer.rs` 增加只依赖 `druid-rdbc` 的完整公开合同编译样例：取得 `XaConnection`、执行 `XaResource::start`、通过逻辑连接执行 SQL 并关闭租约、调用 `end/prepare`，按投票 commit 或完成只读分支，最后关闭 owner。测试通过函数项引用强制编译该消费者样例；该 helper 本身为 compile-only，真实行为由同文件现有消费者测试和 6.5.4 PostgreSQL harness 验证。
- `cargo test -p druid-wrapper --features jdbc-agent --test jdbc_agent_xa_external_consumer --locked --offline`：退出码 `0`，2 passed、3 ignored。忽略项分别需要 H2 Agent、故障注入或由外部 Maven harness 驱动的 PostgreSQL 环境；普通消费者签名和显式数据源配置测试实际运行。
- `mvn -o -f agents/jdbc-agent/pom.xml '-Ddruid.rustXaIntegration=true' '-Dtest=RustXaClientRecoveryIntegrationTest' test`：退出码 `0`，JUnit 1 passed、0 failed、0 skipped。Testcontainers `2.0.5` 实际运行 Docker Engine `29.8.1` 与 PostgreSQL `18.6`；该 Java harness 启动 Rust 外部消费者并从独立 JDBC 会话核对 commit/rollback 数据库终态。pgjdbc 为 `42.7.14`，详见 6.5.4 环境记录。
- `cargo fmt -p druid-wrapper -- --check`、`git diff --check`、`openspec validate close-rdbc-jdbc42-contract-gaps --strict --no-interactive`：均退出码 `0`。广域 `cargo fmt --check` 在 Windows 遇到仓库中超长路径（OS error 206），故使用受影响包范围的 rustfmt 检查；未将广域命令报告为通过。

### API 与 SemVer 评审

- 当前源码公开的中立 API 为 `druid_rdbc::data_source::{XaDataSource, XaConnection, XaResource, Xid, XaFlags, XaPrepareResult, XaError}`；`XaConnection::xa_resource()` 是带默认不支持实现的新增方法，以保留既有外部实现的源兼容。`JdbcAgentXaDataSource` 通过 `druid-wrapper` 的 `jdbc-agent` feature 暴露，位于 `druid_wrapper::jdbc_agent`，具体连接/资源适配器类型不是应用必须依赖的标准合同。
- 当前 workspace package version 为 `0.0.0-design`，本地 Git 没有版本 tag，核心 crate 配置为 `publish = false`；没有已发布稳定 SemVer 基线。因此本次是预发布、加法式接口记录，不能声称已经发布或承诺某一稳定版本。发布前应在确定版本策略时重新评审；移除/重命名公开接口需按最终兼容承诺处理，trait 默认方法维持当前的实现端源兼容性。
- API 文本快照不是当前源码的有效差异证明：`api/druid-rdbc.single-standard.after.txt` 有 `xa_resource_name`，但未列出当前源码中的 `XaConnection::xa_resource()`；`api/druid-wrapper.single-standard.after.txt` 未列出当前 `JdbcAgentXaDataSource`。本机没有 `cargo-public-api` 命令，本项不安装工具、不伪造生成快照；本次用 CodeGraph 返回的当前源码、模块 re-export 和外部消费者编译结果完成定向审查。第 8 批的快照/失效门禁仍需独立完成。

### 本阶段源码身份

| 文件 | SHA-256 |
|---|---|
| `docs/jdbc-agent-xa.md` | `62449C0A4475091ECCFBB6879897E363C8527B397460D7BD50B10CCE2A298EF5` |
| `README.zh-CN.md` | `EC42D53A803E3A5CF6351B1A34185178617050536DD49C22426A2CC5C2E96FCA` |
| `crates/druid-wrapper/tests/jdbc_agent_xa_external_consumer.rs` | `C410C1EB9BD9AA1F1D993F95B1FCDC2FADB91BD55BD505357EC797FDD15B2216` |
| `crates/druid-rdbc/src/data_source/xa_connection.rs` | `070AA62647D20A3D7A89E088AE39C1D782771E8FBCCC908250BDF03114242C79` |
| `crates/druid-wrapper/src/jdbc_agent/mod.rs` | `BDF3AB1303E031368C7FB6D5313AAD848D7E11C585B836CEB8B82B985F64B406` |

本项关闭 XA-06 和 6.5.5 的文档、公开消费者及手工 API/SemVer 审查。运行中的 PostgreSQL 测试提供实库行为证据；旧 API 快照保持标记过期，自动提取/失效门禁仍留给第 8 批。

