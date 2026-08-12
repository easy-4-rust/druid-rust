# druid-wrapper 模块迁移实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 将 Java druid-wrapper 模块的 13 个生产 Java 对象以及 Rust DB 生态（SQLx/RBDC/bb8/deadpool）完整迁移到 Rust `crates/druid-wrapper`，实现可选数据库 Adapter 和外部池适配的完整迁移。

**Architecture:** `crates/druid-wrapper` 是独立产品 crate，提供可选数据库 Adapter 和外部池适配。内部按 adapter 类型分为 `src/sqlx`（SQLx direct adapter）、`src/sqlx/bb8`（bb8 external bridge）、`src/sqlx/deadpool`（deadpool external bridge）、`src/rbdc`（RBDC direct adapter）。direct adapter 与 external pool bridge 必须分层，禁止池中池。Toasty 标准实现归 `druid` 内置，不归 wrapper。

**Tech Stack:**
- SQLx 0.8（direct adapter）
- RBDC 4.9（direct adapter）
- bb8（external pool bridge）
- deadpool（external pool bridge）
- rbdc-sqlite 4.9.5（隔离真实 SQLite 测试）

---

## 硬边界

- `SqlxConnectionAdapter`/`RbdcConnectionAdapter` 只包装一个物理连接，不持有 pool
- `ToastyConnectionAdapter` 属于 `druid::toasty`，wrapper 不复制、不重命名
- `SqlxBb8Pool`/`SqlxDeadpoolPool` 直接实现 `Pool`，不实现 `PhysicalConnectionFactory`
- 一个 lease 只归还原所有者一次
- 公共 Druid API 不泄漏 SQLx/RBDC/bb8/deadpool 具体类型
- Java wrapper 的配置、factory、MBean 结果不能因生态不同而静默删除

---

## 对象状态总览

| 维度 | Java | Rust | 当前 |
| :--- | :--- | :--- | :--- |
| 生产对象 | 13 | Adapter 已物理归入 wrapper 内部目录 | PARTIAL |
| 内置标准 driver | JDBC driver connection | Toasty 0.9（归 druid） | SQLite 已证，多数据库 PARTIAL |
| direct driver 扩展 | JDBC DataSource wrapper | SQLx/RBDC | PARTIAL |
| external pool | c3p0/DBCP/Proxool facade | bb8/deadpool bridge | PARTIAL |
| 配置兼容 | 完整 properties | 构造器参数子集 | MISSING/PARTIAL |
| 管理统计 | MBean/properties | PoolState 子集 | PARTIAL |
| 真实 SQLite | Java wrapper 未专门限定 | SQLx direct/bb8/deadpool | 已有主链证据 |

---

## 代码结构

```
crates/druid-wrapper/
  src/
    sqlx/           -- SQLx direct adapter
      bb8/          -- bb8 external pool bridge
      deadpool/     -- deadpool external pool bridge
    rbdc/           -- RBDC direct adapter
    driver/         -- 驱动注册
    c3p0/           -- c3p0 兼容配置
    dbcp/           -- DBCP 兼容配置
    dbcp2/          -- DBCP2 兼容配置
    proxool/        -- Proxool 兼容配置
    duckdb/         -- DuckDB adapter
    http_sql/       -- HTTP SQL adapter
    jdbc_agent/     -- JDBC agent adapter
    libsql/         -- LibSQL adapter
    rdbc/           -- RDBC 公共类型
```

共 108 个 .rs 文件。

---

## 阶段总览

| Stage | 内容 | 验收 | 状态 |
|-------|------|------|------|
| W0 | 物理 crate 归并为单一 druid-wrapper | 仅保留 wrapper 模块和内部目录 | DONE |
| W0-B | Toasty 内置、wrapper 扩展边界 | facade/类型归属/禁止 pool-in-pool | 已建立 |
| W1 | DriverAdapter/capability/error contract | direct adapter 统一测试 | PARTIAL |
| W2 | SQLx SQLite 完整类型/事务/prepared/metadata | 真实 SQLite + Java differential | PARTIAL |
| W3 | RBDC SQLite 真实 driver | 非 mock RBDC SQLite | PARTIAL |
| W4 | bb8/deadpool lease/timeout/broken/shutdown | 真实 SQLite | PARTIAL |
| W5 | Java c3p0/DBCP/DBCP2 factory/config 语义 | property fixture 差分 | TODO |
| W6 | ProxoolDataSource/Constants 配置面 | 逐字段默认值/单位/错误 | TODO |
| W7 | MBean/PoolState 协议映射 | 字段快照 | TODO |
| W8 | 多数据库 driver capability | 见数据库驱动集成矩阵 D0-D5 | PARTIAL |
| W9 | 覆盖率、文档、兼容迁移指南 | llvm-cov 100% | TODO |

---

## Stage W0 — 物理 crate 归并

**目标：** 将原 druid-sqlx、druid-rbdc、druid-sqlx-bb8、druid-sqlx-deadpool 物理归并为单一 `druid-wrapper`。

- [x] **Step 1:** 归并为单一 `druid-wrapper` crate — **DONE**
- [x] **Step 2:** 建立 Toasty 内置/wrapper 扩展边界 — **DONE**

**出口门禁：** 仅保留 wrapper 模块和内部目录。

---

## Stage W1 — DriverAdapter/capability/error contract

**目标：** 建立 direct adapter 统一测试。

- [x] **Step 1:** SQLx/RBDC SQLWarning Adapter 合同（W1-R1）— **DONE**：SQLx 5/5 + RBDC 6/6
- [ ] **Step 2:** 完整 capability 探测
- [ ] **Step 3:** 错误分类统一

**出口门禁：** direct adapter 统一测试通过。

### W1-R1：SQLx/RBDC SQLWarning Adapter 合同

两个 direct Adapter 已实现 `PhysicalConnection::warnings/clear_warnings`。SQLx 与 RBDC 的公开 Connection SPI 都不暴露 JDBC warning 链，因此返回 `None` 是驱动能力的精确表示。`clear_warnings` capability 同步标记为 true。SQLx 使用真实内存 SQLite 验证，RBDC 使用公开 trait fixture 验证。

---

## Stage W2 — SQLx SQLite 完整类型/事务/prepared/metadata

**目标：** 迁移 SQLx SQLite 完整能力。

- [x] **Step 1:** SQLx Prepared 资源 setter 与真实 SQLite batch（W2-R1）— **DONE**
- [x] **Step 2:** SQLx SQLite DatabaseMetaData 真实查询（W2-R2）— **IMPLEMENTED_UNVERIFIED**
- [ ] **Step 3:** SQLx Any/PostgreSQL/MySQL 适配
- [ ] **Step 4:** Blob/Clob/SQLXML 对象错误矩阵
- [ ] **Step 5:** Java/SQLite 统一差分

**出口门禁：** 真实 SQLite + Java differential 通过。

### W2-R1：SQLx Prepared 资源 setter 与真实 SQLite batch

`SqlxPreparedStatement` 已从 SQL token 升级为物理 Prepared 对象，保存 1-based 参数槽和 batch 值快照。SQLx Adapter 覆盖 parameter-aware update/query/generic/batch。真实 SQLite 验证 binary stream、character reader、URL、RowId、Blob/Clob/NClob、query/update/generic SELECT、ResultSet 生命周期、两批顺序和 Filter descriptor sets。

### W2-R2：SQLx SQLite DatabaseMetaData 真实查询与能力矩阵

`SqlxDatabaseMetaData` 借用当前 `SqliteConnection`，不创建第二连接。在 `PhysicalDatabaseMetaData` 的 173 个方法中显式覆盖 160 个。表/列/主键/索引/外键均从当前物理 SQLite 连接读取。13 项保持显式 `UnsupportedOperation`。

---

## Stage W3 — RBDC SQLite 真实 driver

**目标：** 迁移 RBDC SQLite 真实驱动能力。

- [x] **Step 1:** RBDC Prepared 资源参数与隔离真实 SQLite（W3-R1）— **PARTIAL**
- [x] **Step 2:** RBDC LOB/SQLXML/Object setter（C2-R38）— **DONE**：RBDC 合同 8/8
- [ ] **Step 3:** RBDC Adapter + 真实 SQLite 单进程端到端
- [ ] **Step 4:** RBDC generic execute 结果判型

**出口门禁：** 非 mock RBDC SQLite 通过。

### W3-R1：RBDC Prepared 资源参数与隔离真实 SQLite

`RbdcPreparedStatement` 已升级为物理 Prepared 对象。严格 RBDC SPI fixture 覆盖资源 setter 时机、错误、query/update、批次值与顺序。隔离工程使用 `rbdc-sqlite 4.9.5` 内存库完成参数化 BLOB/TEXT/URL/RowId 插入和查询回读。由于 `links=sqlite3` 冲突，两段证据不能合并冒充单条端到端测试。

---

## Stage W4 — bb8/deadpool lease/timeout/broken/shutdown

**目标：** 迁移外部池 lease/timeout/broken/shutdown 语义。

- [ ] **Step 1:** bb8 lease 完整生命周期
- [ ] **Step 2:** deadpool lease 完整生命周期
- [ ] **Step 3:** broken lease 状态映射
- [ ] **Step 4:** shutdown 语义

**出口门禁：** 真实 SQLite 通过。

---

## Stage W5 — Java c3p0/DBCP/DBCP2 factory/config 语义

**目标：** 迁移 Java wrapper 的配置和 factory 语义。

- [ ] **Step 1:** c3p0 配置属性映射
- [ ] **Step 2:** DBCP/DBCP2 配置属性映射
- [ ] **Step 3:** property fixture 差分

**出口门禁：** property fixture 差分通过。

---

## Stage W6 — ProxoolDataSource/Constants 配置面

**目标：** 迁移 Proxool 配置面。

- [ ] **Step 1:** ProxoolDataSource 逐字段默认值
- [ ] **Step 2:** Constants 配置常量
- [ ] **Step 3:** 单位/错误语义

**出口门禁：** 逐字段默认值/单位/错误通过。

---

## Stage W7 — MBean/PoolState 协议映射

**目标：** 迁移 MBean/PoolState 管理协议。

- [ ] **Step 1:** MBean 字段快照映射
- [ ] **Step 2:** PoolState 管理字段

**出口门禁：** 字段快照通过。

---

## Stage W8 — 多数据库 driver capability

**目标：** 建立多数据库驱动能力矩阵。

- [x] **Step 1:** 驱动类型纠正与依赖闭包（W8-R1）— **DONE**：D1 编译证据
- [x] **Step 2:** Toasty 全 feature D1 — **DONE**
- [x] **Step 3:** SQLite D3 — **DONE**
- [ ] **Step 4:** PostgreSQL/MySQL/Turso D3 真实服务证据
- [ ] **Step 5:** Tiberius/Oracle/ClickHouse/DuckDB/Firebird/ODBC 候选评估

**出口门禁：** 见数据库驱动集成矩阵 D0-D5。

### W8-R1：驱动类型纠正与依赖闭包

Toasty PostgreSQL 真实连接层是 `tokio-postgres`；MySQL 由 `mysql_async` 承担，Turso 由 `turso` 承担。DynamoDB capability 为非 SQL，不启用 Toasty DynamoDB feature。`cargo check -p druid --all-features --all-targets` 编译通过，证明 D1。除 SQLite 外尚无真实服务证据。

---

## Stage W9 — 覆盖率、文档、兼容迁移指南

**目标：** 实现 llvm-cov 100% 和完整文档。

- [ ] **Step 1:** workspace llvm-cov 100%
- [ ] **Step 2:** 兼容迁移指南
- [ ] **Step 3:** 数据库驱动集成矩阵更新

**出口门禁：** llvm-cov 100%。

---

## SQLite 严格合同

当前 `sqlite_wrapper_semantics_test` 从 facade 验证：

- SQLx direct 创建真实 SQLite connection
- DDL、参数绑定、查询和类型
- SQLWarning 初始为空、clear 成功，关闭后拒绝 warning 操作
- SQLite CallableStatement capability 明确 unsupported
- bb8/deadpool 真实 SQLite lease 执行并归还原外部池

底层已有 SQLx、bb8、deadpool 真实 SQLite 测试；SQLx direct 当前 6/6。

SQLite 主 workspace 不直接覆盖 RBDC 真实驱动；当前是严格 trait fixture 加隔离真实 `rbdc-sqlite` 值合同。

---

## 验收命令

```bash
cargo test -p druid-wrapper --test sqlite_wrapper_semantics_test
cargo test -p druid-wrapper
cargo test --workspace
cargo llvm-cov --workspace --summary-only
```

---

文档版本：v3.0 (superpowers plan format)

基线日期：2026-08-12

状态：PARTIAL（W0-W4 部分完成，W5-W9 未开始）
