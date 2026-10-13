# JDBC Agent XA 协议与真实恢复证据

## 覆盖范围

本记录证明 Agent 的显式 XADataSource 接入、XAResource RPC 载荷和 PostgreSQL 跨 session 恢复行为。它不证明所有 JDBC Agent 客户端/事务管理器集成已经完成，也不代表 P04–P07 或整个仓库生产就绪。

## 实际验证

- 环境：OpenJDK 17.0.12、Maven 3.9.9、Docker Desktop 29.8.1。
- 固定测试依赖：PostgreSQL 镜像 `postgres:18.6`、pgjdbc `42.7.14`、Testcontainers `2.0.5`；PostgreSQL 使用 `max_prepared_transactions=10`。
- 定向命令：`mvn -f agents/jdbc-agent/pom.xml -Dtest=XaAgentProtocolTest test`，退出码 `0`，2 项测试通过。
- 覆盖正向协议：显式配置 `org.postgresql.xa.PGXADataSource`；保留 `formatId`、global transaction id、branch qualifier 和 flags；同一 owner 上 start → SQL insert → end → 关闭逻辑连接 → prepare → 重开逻辑连接 → 关闭 XA owner；新 session 以 `TMSTARTRSCAN`、`TMNOFLAGS`、`TMENDRSCAN` 恢复并核对完整 Xid，再 commit；最后由独立普通 JDBC 连接读取已提交行。
- 覆盖其他 XA RPC：`isSameRM`、设置/读取 timeout、prepare vote、recover 分页、commit。
- 覆盖负向路径：普通 `DriverManager` session 的 `capabilities.xa` 为 false；对其调用 `xa_start` 返回 SQLState `0A000`；`XAException.errorCode` 保存在结构化错误 `vendorCode` 中。
- 覆盖已有回归：最终源码下 `mvn -f agents/jdbc-agent/pom.xml test` 退出码 `0`，8 项测试通过（AgentServerTest 5 项、XaAgentProtocolTest 2 项、XaRecoveryIntegrationTest 1 项）。

## 实现与身份

实现由 `AgentServer` 显式创建和持有 `XAConnection`，`AgentSession` 将 XA owner 与可关闭/重开的逻辑 `Connection` 分开；普通 JDBC session 不伪装 XA。`AgentXid` 对字节数组做防御性复制并按 XA 字段值实现相等比较。`XaDataSourceFactory` 仅接受明确给出的 XADataSource 类名及其 JavaBean 属性，不从 JDBC URL 推断 XA。

下列 SHA-256 对应本次测试源码输入；发布提交后如源码变化，证据必须重算：

| 文件 | SHA-256 |
|---|---|
| `agents/jdbc-agent/src/main/java/io/github/easy4rust/druid/agent/AgentServer.java` | `EF13AFA4E130740133974BA92E033D88990561111EE9F86DE14E9F1EFCC212C1` |
| `agents/jdbc-agent/src/main/java/io/github/easy4rust/druid/agent/AgentSession.java` | `3E03C0F944CADC2018658D7A0BAAA6A3082E89DAE683012DFBED52E29E9139EB` |
| `agents/jdbc-agent/src/main/java/io/github/easy4rust/druid/agent/AgentXid.java` | `4B53E3A5633F5FA37A73B3E5A60D8D36F575E1267FBCBE61FE444386F3AE61EF` |
| `agents/jdbc-agent/src/main/java/io/github/easy4rust/druid/agent/AgentError.java` | `C5B2B307EC88DAA3A47A2EB478AB87228805E67D7F960E460B08CCBB3CE2007D` |
| `agents/jdbc-agent/src/main/java/io/github/easy4rust/druid/agent/XaDataSourceFactory.java` | `C782684BE4B97D41320FCDF063F6852CC74407E6BBD1FDF1731A7A5822D8A9E8` |
| `agents/jdbc-agent/src/test/java/io/github/easy4rust/druid/agent/XaAgentProtocolTest.java` | `8763F586B6EFB04E9B1A5BF594EC576540C4262A42815008B8A80C2B6DD33DF3` |
| `agents/jdbc-agent/pom.xml` | `25B1FAEC81D887EAB23CB90E7AA05828D4CD2CFBD6B673EB000742089C009830` |
