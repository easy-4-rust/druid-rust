# 功能详解

## 连接池（DruidPool）

对齐 Java Druid 连接池的核心语义：池初始化、空闲收缩、保活检测、连接泄漏 trace、`DruidPool` 与 filter 链协同工作。池语义以真实数据库契约测试验证。

## druid-rdbc：JDBC 语义层

`druid-rdbc` 定义唯一的 JDBC-like 标准 trait 树：

- `Driver` / `ConnectionFactory`：建立标准连接
- `Connection` / `Statement` / `PreparedStatement`：执行与参数绑定
- `ResultSet`：行游标
- 元数据与错误模型

所有适配器（Toasty、SQLx、mysql_async、tiberius……）都实现这套契约，上层代码与具体驱动解耦。

## SQL 解析与 Wall 防火墙

- 基于 sqlparser 的 SQL AST 解析；
- WallFilter 防注入语义迁移，规则与 Java Druid WallFilter 对齐。

## 监控统计

- **Stat**：执行统计、慢 SQL 记录；
- **druid-metrics**：非阻塞指标运行时（sampler / aggregator / timeline），支持 Prometheus 拉取与 gRPC（tonic）上报两种出口；
- **druid-admin**：Axum 实现的 `/druid/*` HTTP 管理面，带认证与 TLS/mTLS。

## 动态多数据源

HA / ArcSwap 方案实现数据源热切换，对应 Java Druid 的 `DynamicDataSource` 生态位。

## 驱动支持矩阵

`druid-wrapper` 维护 **80 项 SQL-only 驱动目录**（`database-drivers.manifest.json`），按 15 / 25 / 40 三阶段交付，每项驱动有四级证据分级：

| 证据级别 | 含义 |
| --- | --- |
| declared | 目录登记 |
| experimental | 已有实验实现 |
| verified | 契约测试验证 |
| certified | 认证（全矩阵 CI 绿） |

CI 中跑真实契约测试的组合：

- SQLite（Toasty + SQLx）、DuckDB（duckdb-native）× 5 平台 × Rust 1.95/1.97.1
- JDBC Agent：H2 2.3.232、HSQLDB 2.7.4 × 3 OS × JDK 17/21
- conformance：SQLite / PostgreSQL / MySQL / SQL Server / Toasty PostgreSQL / Toasty Turso / mysql_async

Redis / MongoDB / Kafka 等**非 SQL** 中间件明确不在目录内。

## XA / 两阶段提交

`druid::core::xa` 提供 `XaResource` / `Xid` / XA 状态机，自定义资源（如内存账本）实现 trait 即可参与 2PC，见 `crates/druid/examples/xa_demo.rs`。

## Cargo Features 速查

```toml
# druid（内置 Toasty 数据源）
features = ["sqlite", "postgresql", "mysql", "turso"]

# druid-wrapper（默认 driver-catalog + sqlite）
features = [
  "mysql-native",        # mysql_async 0.37
  "sqlserver-native",    # tiberius 0.12
  "duckdb-native",
  "libsql-native",
  "http-sql",
  "jdbc-agent",          # 长尾 JDBC 驱动经 JVM agent 接入
  "all-databases",
]
```
