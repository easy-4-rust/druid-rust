# XA 后端恢复能力探测

本记录只选择真实 JDBC Agent 6.4 的数据库测试后端，不表示 JDBC Agent 协议已经支持 XA，也不关闭任务 6.4。

## 环境

- Java：Maven 使用 Oracle JDK 17.0.12；Agent 测试以 `--release 17` 编译。
- Maven：3.9.9，位于 `D:\Java\maven\apache-maven-3.9.9`。
- PostgreSQL：Docker/Testcontainers `postgres:18.6`，隔离临时容器；启动参数 `-c max_prepared_transactions=10`。
- PostgreSQL JDBC：42.7.14；Testcontainers Java：2.0.5。
- PostgreSQL XA 数据源：`org.postgresql.xa.PGXADataSource`，通过显式 server、port、database、user、password 属性配置。
- 容器由测试停止，不连接或修改其他运行中的数据库容器。

## H2 2.3.232 结果

测试创建真实 `XAConnection`，执行 `start`、SQL insert、`end(TMSUCCESS)`，并确认 `prepare` 返回 `XA_OK`。关闭原 XA owner 后，使用新 XA owner 和 `TMSTARTRSCAN | TMENDRSCAN` 扫描；H2 返回的 Xid 列表不包含该 prepared Xid。首次以“必须找回 Xid”为断言的探测按预期失败（`AgentServerTest.java:93`，`expected true but was false`），证明 H2 不可作为本次断连恢复的正向后端。最终保留的诊断用例确认这个不支持行为，避免以后误选 H2 作为恢复证据。

命令：

```powershell
& 'D:\Java\maven\apache-maven-3.9.9\bin\mvn.cmd' -f agents/jdbc-agent/pom.xml '-Dtest=AgentServerTest#h2RecoveryCapabilityCheckDoesNotMistakeMissingBranchForRecovery' test
```

结果：`Tests run: 1, Failures: 0, Errors: 0`；退出码 `0`。

## PostgreSQL 18.6 结果

独立真实 JDBC 测试经 PostgreSQL XADataSource 执行 prepare；关闭初始 XAConnection 后，通过新 XAConnection 恢复完整 Xid、执行二阶段 commit，并由独立普通 JDBC Connection 查询到预期行。容器参数显式启用 prepared transactions。此结果足以选定 PostgreSQL 为后续 Agent 协议集成后端；目前调用路径仍是测试直接操作 `PGXADataSource`，所以不构成 Agent 端到端证据。

命令：

```powershell
& 'D:\Java\maven\apache-maven-3.9.9\bin\mvn.cmd' -f agents/jdbc-agent/pom.xml -Dtest=XaRecoveryIntegrationTest test
```

结果：`Tests run: 1, Failures: 0, Errors: 0`；退出码 `0`。运行日志确认 Testcontainers 2.0.5 使用本地 Docker，启动 `postgres:18.6` 并成功运行测试。

## 源码身份

以下 SHA-256 对应上述运行后的源文件：

| 文件 | SHA-256 |
|---|---|
| `agents/jdbc-agent/src/test/java/io/github/easy4rust/druid/agent/AgentServerTest.java` | `54224804641561c9bd8695c81d0dd27805e773f3b6490847d759fbef5d9a8026` |
| `agents/jdbc-agent/src/test/java/io/github/easy4rust/druid/agent/XaRecoveryIntegrationTest.java` | `9c12de35839ee7c347e0d6d21c0d1caa69f76d127ff7ff8bb1bd7248569d09f7` |
| `agents/jdbc-agent/pom.xml` | `25b1faec81d887eab23cb90e7aa05828d4cd2cfbd6b673eb000742089c009830` |

## 尚缺证据

- JDBC Agent 使用真实 XADataSource 建立 session，及 SQL/XA 共用同一 session 的协议测试。
- 完整 Xid、recover 分页、flags、vote 和 XA 错误码的 JSON-RPC 往返测试。
- 关闭旧 Agent session、从新 Agent session recover 后 commit/rollback，并以独立 JDBC 连接核验最终状态。
- 逻辑 Connection close 与物理 XAConnection owner close 的失效顺序和回归测试。

以上任一缺口未完成前，任务 6.4 保持未勾选。
