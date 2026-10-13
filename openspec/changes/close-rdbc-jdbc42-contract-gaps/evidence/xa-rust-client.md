# JDBC Agent Rust XA 客户端证据

本记录按 6.5.1–6.5.5 分阶段追加。当前只关闭 6.5.1 的 RPC 合同测试与普通连接兼容性基线；不代表标准 XA 数据源入口、owner 生命周期、Rust/PostgreSQL 端到端恢复或事务管理器消费者已完成。

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

