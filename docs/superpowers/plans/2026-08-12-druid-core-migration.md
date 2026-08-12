# druid 核心模块迁移实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 Java Druid core 模块的 1,644 个生产对象的功能语义完整迁移到 Rust `crates/druid`，涵盖 core/pool/sql/stats/dynamic/toasty 六个内部目录，实现语义完成率 100%、对象可追溯率 100%。

**Architecture:** `crates/druid` 是 Java `/core` 模块唯一的源码、发布与验收边界。内部按职责分为 `src/core`（公共对象/SPI/Filter）、`src/pool`（数据源与 native pool）、`src/sql`（parser/AST/Wall）、`src/stats`（SQL/datasource 统计）、`src/dynamic`（HA/selector）、`src/toasty`（默认内置实现）。`PhysicalConnection` 是内部最小 SPI，Toasty 是内置默认实现，SQLx/RBDC 是 wrapper 扩展。

**Tech Stack:**
- Rust 1.97.1 (MSRV 1.95)，Toasty 0.9，sqlparser-rs
- tokio、tracing、ArcSwap、Prometheus
- bigdecimal、chrono（强类型）

---

## 对象状态总览

| 维度 | Java core | Rust 当前 | 判断 |
| :--- | ---: | ---: | :--- |
| 生产对象/文件 | 1,644 | 368 .rs 文件，已物理归并到 `crates/druid/src/*` | PARTIAL |
| Pool 主链 | 完整 | native acquire/recycle、部分维护、PS cache | PARTIAL |
| SQL/AST/方言 | 1,268 个对象 | sqlparser-rs facade + Lexer/Wall 规则 + Token/Keywords | PARTIAL |
| Filter/Proxy | ~94 个对象族 | 185+ ResultSet Filter 调用 + FilterAdapter/EventAdapter/Manager | PARTIAL |
| Stat | 完整 SQL/datasource/web/spring 统计 | merge/stat filter + 分层统计 + 管理快照 | PARTIAL |
| 真实数据库 | JDBC 多驱动 | Toasty SQLite 内置主链 + SQLx 扩展主链 | PARTIAL |

---

## 阶段总览

| Stage | 范围 | 退出门禁 | 状态 |
|-------|------|----------|------|
| C0 | 三模块边界、对象总账、CodeGraph 基线 | 1,644 分母冻结；core 内部实现归并进 `crates/druid` | DONE |
| C1 | PhysicalConnection、Toasty 内置标准、错误、typed values | 内置与每个扩展 Adapter 执行统一 contract | PARTIAL |
| C2 | DruidDataSource、holder、pooled connection、维护任务 | Java acquire/recycle/shrink/fill 差分 | PARTIAL |
| C3 | Statement/Prepared/Callable/ResultSet/LOB/metadata | 全方法矩阵 + 真实数据库 | PARTIAL |
| C4 | Filter/Proxy 调用链 | 每个 Java hook 有事件和顺序测试 | PARTIAL |
| C5 | SQL parser、AST、Visitor、输出、参数化 | 逐方言 corpus 差分 | PARTIAL |
| C6 | WallProvider/WallConfig/规则 | 全字段开关与 violation 差分 | PARTIAL |
| C7 | Stat/JMX 结果迁移 | 指标字段、聚合、reset、并发一致 | PARTIAL |
| C8 | HA/XA/vendor/support/util | 故障注入与真实驱动矩阵 | PARTIAL |
| C9 | 文档、注释、兼容面、覆盖率 | workspace + llvm-cov 100% | TODO |

---

## Stage C0 — 三模块边界与对象总账

**目标：** 冻结 1,644 个 Java 对象分母，将 core 内部实现物理归并进 `crates/druid`。

- [x] **Step 1:** 收敛 workspace 产品边界为三个 crate — **DONE**
- [x] **Step 2:** 冻结 1,644 对象分母 — **DONE**
- [x] **Step 3:** core 内部实现按职责归并到 src/core、src/pool、src/sql、src/stats、src/dynamic、src/toasty — **DONE**

**出口门禁：** 1,644 分母冻结；core 内部实现归并进 `crates/druid`。

---

## Stage C1 — PhysicalConnection、Toasty 内置标准、错误、typed values

**目标：** 建立对象安全、异步、最小化的 `PhysicalConnection` SPI，以及内置 Toasty 标准实现。

- [x] **Step 1:** Toasty 内置数据源标准化（C1-R1）— **DONE**：SQLite 3 个 direct contract + 1 个 core 纵向 contract 通过
- [x] **Step 2:** WrapperAdapter/PoolableWrapper 无损迁移（C1-R2）— **DONE**：Java 34/34 + Rust 6/6 + SQLite 3/3
- [ ] **Step 3:** 补齐错误码、source 和 context（DruidError）
- [ ] **Step 4:** 参数类型、null、LOB、OUT 参数（Value/JdbcParameter）

**出口门禁：** 内置与每个扩展 Adapter 执行统一 contract。

---

## Stage C2 — DruidDataSource、holder、pooled connection、维护任务

**目标：** 迁移 DruidDataSource/DruidConnectionHolder/DruidPooledConnection 完整状态机。

- [x] **Step 1:** 连接创建与回收语义（P2-R1）— **DONE**
- [x] **Step 2:** 连接池重试、等待与取消安全（C2-R4）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 3:** ExceptionSorter 平台异常与 vendor 对象（C2-R8/R9）— **DONE**：Java vendor 23/23 + Rust 10/10
- [x] **Step 4:** 全 Connection 操作异常入口（C2-R10）— **DONE**：23 条物理连接操作路径
- [x] **Step 5:** PS 关闭清理异常（C2-R11）— **DONE**
- [x] **Step 6:** 普通 Statement 对象边界（C2-R12）— **DONE**
- [x] **Step 7:** ResultSet 只读游标与 trace（C2-R13）— **DONE**
- [x] **Step 8:** ResultSet 强类型值（C2-R14）— **DONE**
- [x] **Step 9:** ResultSet 资源 getter/update（C2-R15/R16/R17）— **DONE**
- [x] **Step 10:** typed getObject 与 JDBC Object（C2-R18）— **DONE**
- [x] **Step 11:** ResultSetMetaData 标准列契约（C2-R19/R20）— **DONE**
- [x] **Step 12:** JdbcResultSetStat 独立统计（C2-R21）— **DONE**
- [x] **Step 13:** ResultSet FilterChain 与 StatFilter（C2-R22）— **DONE**
- [x] **Step 14:** StatFilterContext/Listener（C2-R23/R24）— **DONE**
- [x] **Step 15:** Statement/PS batch Filter/统计（C2-R25/R26）— **DONE**
- [x] **Step 16:** Statement/PS generic execute（C2-R27/R28）— **DONE**
- [x] **Step 17:** SQLWarning 四类对象（C2-R29/R30）— **DONE**
- [x] **Step 18:** ResultSet#getStatement 动态身份（C2-R31/R32）— **DONE**
- [x] **Step 19:** PS setter 持久绑定参数（C2-R33）— **DONE**
- [x] **Step 20:** PS 继承属性与缓存回收（C2-R34）— **DONE**
- [x] **Step 21:** Prepared 资源 setter 与 batch（C2-R35）— **DONE**
- [x] **Step 22:** ResultSet 默认能力与 typed object（C2-R39）— **DONE**
- [x] **Step 23:** ResultSet 标量 getter 物理重载（C2-R40）— **DONE**
- [x] **Step 24:** 物理 ResultSet 贯通（C2-R41）— **DONE**
- [x] **Step 25:** ResultSet FilterChain 全方法族（C2-R42~R56）— **DONE**：185 个精确 Filter 调用
- [x] **Step 26:** FilterAdapter/EventAdapter/Manager（C2-R57~R59）— **DONE**
- [x] **Step 27:** MergeStat/编码/日期 Filter（C2-R60）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 28:** 日志语义边界（C2-R61）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 29:** ConfigFilter/ConfigTools（C2-R62）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 30:** AutoLoad ServiceLoader（C2-R63）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 31:** Filter 配置生命周期（C2-R64）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 32:** Pool/Checker/Wall/Stat 台账复核（C2-R65）— **PARTIAL**
- [ ] **Step 33:** 公平锁切换、fatalError 统一差分、活跃租约强制 shutdown
- [ ] **Step 34:** removeAbandoned、密码版本动态失效、后台 creator/destroy task
- [ ] **Step 35:** keepAlive 失败后 minIdle 回填

**出口门禁：** 并发压力下容量不越界、无丢连接、无双重归还；Java pool fixture 差分通过。

---

## Stage C3 — Statement/Prepared/Callable/ResultSet/LOB/metadata

**目标：** 全方法矩阵 + 真实数据库验证。

- [x] **Step 1:** Callable 输入重载无损化（C3-R2）— **DONE**：115 个 Java 方法对应入口
- [x] **Step 2:** Callable Decimal/日期时间强类型（C3-R3）— **DONE**
- [x] **Step 3:** Callable Blob 资源语义（C3-R4）— **DONE**
- [x] **Step 4:** Callable Clob/NClob/字符流（C3-R5）— **DONE**
- [x] **Step 5:** Prepared/Callable Wrapper 统一（C3-R7）— **DONE**
- [ ] **Step 6:** CallableStatement 完整 JDBC 方法族和真实驱动能力
- [ ] **Step 7:** JDBC setXxx 参数状态与 batch
- [ ] **Step 8:** Oracle implicit cache 驱动调用
- [ ] **Step 9:** statement trace/listener/result-set 清理
- [ ] **Step 10:** 真实 PostgreSQL/MySQL 存储过程 Adapter

**出口门禁：** 全方法矩阵 + 真实数据库验证通过。

---

## Stage C4 — Filter/Proxy 调用链

**目标：** 迁移 Druid FilterChain 职责链语义。

- [x] **Step 1:** ResultSet FilterChain 185 个精确调用（C2-R42~R56）— **DONE**
- [x] **Step 2:** FilterAdapter 默认适配（C2-R57）— **DONE**
- [x] **Step 3:** FilterEventAdapter 事件模板（C2-R58）— **DONE**
- [x] **Step 4:** FilterManager alias/工厂（C2-R59）— **DONE**
- [ ] **Step 5:** 完整 FilterChainImpl（~498 个 public/protected 方法）
- [ ] **Step 6:** 嵌套 ResultSet/Clob 自动代理
- [ ] **Step 7:** Connection/Statement/Callable 精确 hook
- [ ] **Step 8:** 真实可更新游标 Filter

**出口门禁：** 每个 Java hook 有事件和顺序测试。

---

## Stage C5 — SQL parser、AST、Visitor、输出、参数化

**目标：** 完整迁移 Druid SQL 对外语义。

- [x] **Step 1:** SQLParserFeature 位契约（C2-R65.60）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 2:** SQLType 完整 128 项分类（C2-R65.61）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 3:** LayoutCharacters/CharTypes UTF-16 词法基础（C2-R65.62）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 4:** DialectFeature 双 mask（C2-R65.63）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 5:** Token 335 项 + Keywords hash 查找（C2-R65.64）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 6:** Parser 四异常对象（C2-R65.65）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 7:** SymbolTable 单槽 intern（C2-R65.66）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 8:** SQLInsertValueHandler 流式协议（C2-R65.67）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 9:** Lexer UTF-16 通用扫描状态机（C2-R65.68）— **PARTIAL**
- [ ] **Step 10:** 方言专用 nextToken 快速入口
- [ ] **Step 11:** SQLStatementParser/AST 类型族/Parent/Attribute
- [ ] **Step 12:** Visitor/output visitor/format/parameterize/restore/fingerprint
- [ ] **Step 13:** schema repository/表列解析/SQL transform/builder
- [ ] **Step 14:** PagerUtils/schema repository

**出口门禁：** Druid SQL 测试语料按方言差分；parse→AST→output 达 100%。

---

## Stage C6 — WallProvider/WallConfig/规则

**目标：** 在 P4 兼容 AST 上迁移 Wall 全部安全语义。

- [x] **Step 1:** WallConfig 默认值恢复（C2-R65.57）— **PARTIAL**
- [x] **Step 2:** WallContext/WallUpdateCheckItem（C2-R65.16/R17）— **PARTIAL**
- [x] **Step 3:** WallProviderStatValue 管理快照（C2-R65.1）— **PARTIAL**
- [x] **Step 4:** 七方言 Provider/Visitor（C2-R65.57）— **PARTIAL**
- [x] **Step 5:** Wall tenant ResultSet 隐藏列（C2-R65.28）— **PARTIAL**
- [x] **Step 6:** Wall tenant AST 改写（C2-R65.29）— **PARTIAL**
- [x] **Step 7:** Wall doPrivileged 同步/异步作用域（C2-R65.35）— **IMPLEMENTED_UNVERIFIED**
- [ ] **Step 8:** WallConfig 全字段行为接线
- [ ] **Step 9:** 各方言 visitor 完整规则矩阵
- [ ] **Step 10:** Java/Rust 放行/拒绝/错误码/统计一致

**出口门禁：** 每个 WallConfig 字段至少有开/关两组行为测试。

---

## Stage C7 — Stat/JMX 结果迁移

**目标：** 迁移 Jdbc*Stat/StatFilter/DruidStatManager 统计口径。

- [x] **Step 1:** DruidDataSourceStatValue 真实区间统计（C2-R65.2）— **PARTIAL**
- [x] **Step 2:** JdbcConnectionStat/JdbcStatementStat 分层统计（C2-R65.3）— **PARTIAL**
- [x] **Step 3:** JdbcSqlStat 行数/batch/运行态/事务（C2-R65.4/R5）— **PARTIAL**
- [x] **Step 4:** SQL reset 保留身份语义（C2-R65.6）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 5:** 慢 SQL 参数管理快照（C2-R65.7）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 6:** SQL 最近错误管理字段（C2-R65.8）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 7:** ResultSet 读取量/流资源统计（C2-R65.9）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 8:** Blob/Clob 双层打开计数（C2-R65.10）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 9:** JdbcSqlStat 管理协议字段（C2-R65.11）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 10:** SQL 管理 ID 与 FNV-64 HASH（C2-R65.12）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 11:** basic.json 运行时边界（C2-R65.13）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 12:** SQL facade 列表过滤（C2-R65.14）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 13:** JdbcStatContext/Trace singleton（C2-R65.56）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 14:** 周期统计发布 DataSourceStatSink（C2-R65.37）— **PARTIAL**
- [x] **Step 15:** JdbcStatManager 管理 reset/log（C2-R65.51）— **PARTIAL**
- [ ] **Step 16:** JdbcConnectionStat/JdbcStatementStat 完整 histogram
- [ ] **Step 17:** 并发线性一致性差分

**出口门禁：** 固定时钟与并发场景下 Java/Rust 快照逐字段一致。

---

## Stage C8 — HA/XA/vendor/support/util

**目标：** 故障注入与真实驱动矩阵。

- [x] **Step 1:** HighAvailableDataSource 与 selector（C2-R65.52）— **PARTIAL**
- [x] **Step 2:** Random selector 验证/摘除/恢复（C2-R65.53）— **PARTIAL**
- [x] **Step 3:** File Node 发现/PoolUpdater（C2-R65.54）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 4:** ZooKeeper 节点注册与监听（C2-R65.55）— **PARTIAL**
- [x] **Step 5:** DbType 严格语义（C2-R65.30）— **PARTIAL**
- [x] **Step 6:** JdbcUtils JDBC 兼容识别（C2-R65.31）— **PARTIAL**
- [x] **Step 7:** DatabaseMetaData SPI/代理/Adapter（C2-R65.32/R33/R34）— **PARTIAL**
- [x] **Step 8:** DruidDataSource 可重启生命周期（C2-R65.36）— **PARTIAL**
- [x] **Step 9:** discardConnection/testConnectionInternal（C2-R65.38）— **PARTIAL**
- [x] **Step 10:** onFatalError 门禁与即时 discard（C2-R65.39）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 11:** createError/failContinuous/failFast（C2-R65.40）— **PARTIAL**
- [x] **Step 12:** GetConnectionTimeoutException 诊断（C2-R65.41）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 13:** 逻辑获取计数与等待门限（C2-R65.42）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 14:** asyncInit/initExceptionThrow（C2-R65.43）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 15:** Filter connection_connect 物理建连（C2-R65.44/R45/R46/R47/R48）— **PARTIAL**
- [x] **Step 16:** resetStatEnable/resetCount（C2-R65.49）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 17:** DataSourceClosed/Disable 错误（C2-R65.50）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 18:** DataSourceProxy 门面（C2-R65.25）— **PARTIAL**
- [x] **Step 19:** ResultSetMetaDataProxy（C2-R65.26）— **PARTIAL**
- [x] **Step 20:** Proxy identity/attributes/Filter（C2-R65.18）— **PARTIAL**
- [x] **Step 21:** ConnectionProxy 时间/验证/TransactionInfo（C2-R65.19）— **PARTIAL**
- [x] **Step 22:** ResultSetProxy 列映射（C2-R65.20）— **PARTIAL**
- [x] **Step 23:** StatementExecuteType（C2-R65.21）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 24:** Filter 创建/连接事件 Proxy 身份（C2-R65.22）— **PARTIAL**
- [x] **Step 25:** ConnectionProxy properties（C2-R65.23）— **PARTIAL**
- [x] **Step 26:** JdbcParameter 对象族（C2-R65.24）— **IMPLEMENTED_UNVERIFIED**
- [x] **Step 27:** Statement close 同步事件（C2-R65.27）— **PARTIAL**
- [x] **Step 28:** Clob/NClob Druid Proxy（C2-R65.58）— **PARTIAL**
- [x] **Step 29:** Connection LOB 创建（C2-R65.59）— **PARTIAL**
- [ ] **Step 30:** XA/两阶段提交状态机
- [ ] **Step 31:** restart(Properties) 运行期配置
- [ ] **Step 32:** 真实 PostgreSQL/MySQL/Turso 驱动矩阵

**出口门禁：** 故障注入下无跨事务切换、无连接泄漏。

---

## Stage C9 — 文档、注释、兼容面、覆盖率

**目标：** workspace + llvm-cov 100%。

- [ ] **Step 1:** 所有对象头部中文 doc 注释（Java FQCN、职责、映射形态）
- [ ] **Step 2:** 所有公开方法保留 Java Javadoc 语义
- [ ] **Step 3:** lib.rs/mod.rs 只做模块文档、声明和重导出
- [ ] **Step 4:** README/API/配置/错误/指标/兼容矩阵同步更新
- [ ] **Step 5:** 四份迁移文档 CI 校验
- [ ] **Step 6:** workspace llvm-cov 100%

**出口门禁：** workspace + llvm-cov 100%。

---

## 关键 SEM-* 验收标准引用

| SEM ID | 描述 | 当前 |
|--------|------|------|
| SEM-CONN-005 | PreparedStatement setter/参数绑定 | PARTIAL |
| SEM-CONN-007 | ExceptionSorter 全矩阵 | DONE |
| SEM-CONN-009 | Connection 错误/fatal 统计 | PARTIAL |
| SEM-CONN-015 | PS 继承属性与关闭恢复 | DONE |
| SEM-CONN-016 | Prepared 资源参数物理 setter | PARTIAL（Toasty SQLite PASS） |
| SEM-FLT-024~038 | ResultSet Filter 方法族 | DONE |
| SEM-FLT-039 | FilterAdapter | PARTIAL |
| SEM-FLT-040 | FilterEventAdapter | PARTIAL |
| SEM-FLT-041 | FilterManager | PARTIAL |
| SEM-NFR-* | 全局非功能需求 | 153 个唯一 ID |

---

## SQLite 严格真实测试矩阵

| 能力 | SQLite 验收 | 当前证据 |
| :--- | :--- | :--- |
| connect/ping/close | 必测 | Toasty 内置 contract + SQLx 扩展 contract |
| DDL/DML/查询/参数绑定 | 必测真实结果 | `toasty_connection_adapter_test` + `sqlite_core_semantics_test` |
| 类型 | NULL/INTEGER/REAL/TEXT/BLOB/BOOLEAN | Toasty raw + SQLx |
| transaction/savepoint | commit/rollback/savepoint | Toasty 3 个 contract + SQLx |
| PreparedStatement | prepare、真实执行、缓存复用 | 已测 |
| acquire/recycle | native、bb8、deadpool | 已测 |
| CallableStatement | 必须返回 unsupported | 已测 |
| Wrapper/unwrap | 自身、PhysicalConnection、SQLite Adapter | 34/34 + 3/3 |

---

## 验收命令

```bash
./mvnw -pl core -DskipTests=false test
cargo test -p druid --test sqlite_core_semantics_test
cargo test -p druid --test toasty_connection_adapter_test
cargo test -p druid --test wrapper_semantics_test
cargo test -p druid-wrapper --test sqlite_wrapper_semantics_test
cargo test --workspace
cargo llvm-cov --workspace --summary-only
python3 ~/.agents/skills/rust-java-migration/scripts/audit_migration_layout.py \
  --rust-root /Users/wandl/workspaces/workspace-github-easy-4-rust/druid-rust
```

---

文档版本：v3.0 (superpowers plan format)

基线日期：2026-08-12

状态：PARTIAL（P0-P8 部分完成，P9-P10 未开始）
