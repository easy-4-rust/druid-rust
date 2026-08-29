# 什么是 druid-rust

**druid-rust** 是阿里巴巴 Druid `1.2.28` 到 Rust 的规划式完整语义迁移：连接池、JDBC 语义层、SQL 解析与防火墙、监控统计、动态多数据源——Druid 全家桶能力在 Rust 中的系统性重建。

## 定位

它不是逐行翻译，也不是"借鉴架构后删功能"的精简版，而是一条**契约式迁移**路线：

- Java 侧的每个对象与方法先进入**对象总账**（对象级台账）；
- 每个迁移点同时给出 **Java oracle**（原始语义）与 **Rust contract**（Rust 侧语义契约）；
- 最终以**真实数据库证据**（SQLite / PostgreSQL / MySQL / SQL Server / DuckDB 等真实连接的契约测试）三方验证。

截至 2026-08，`druid` core 已登记约 **1,644 个核心对象语义**，迁移仍在进行中。

## 功能矩阵

| 能力 | 说明 | 状态 |
| --- | --- | --- |
| DruidPool 连接池 | 初始化、收缩、保活、连接泄漏检测 | 迁移中 |
| druid-rdbc trait 树 | Connection / Statement / ResultSet / Driver / 元数据的 JDBC 语义层 | 契约 v1 已定稿 |
| SQL 解析 + Wall 防火墙 | SQL AST 解析、WallFilter 防注入 | 迁移中 |
| 监控统计 | Stat 指标、非阻塞 metrics 运行时、Prometheus / gRPC 上报 | 迁移中 |
| 动态多数据源 | HA / ArcSwap 热切换 | 迁移中 |
| druid-admin 管理面 | Axum 实现 `/druid/*` 监控端点、认证、TLS/mTLS | 迁移中 |
| XA / 2PC | XaResource 两阶段提交 | 示例已可运行 |

## 与 Java Druid 的关系

- **基线**：Druid `1.2.28`（上游 commit `33824c3`）。
- **非官方发行**：本项目与 Alibaba Druid 官方无从属关系，发布时保留来源说明。
- **Rust 生态适配**：内置 Toasty 数据源，并提供 SQLx / RBDC / bb8 / deadpool / DuckDB / libSQL / JDBC Agent 等适配器——Java 世界的 `DruidDataSource` 在这里对应一组 Rust 原生类型与 trait 实现。

## 当前状态

- workspace 版本 `0.0.0-design`，所有 crate `publish = false`——**尚未发布 crates.io**，请以 Git 依赖接入；
- 公共 API 尚未冻结，CI 通过 public-api 快照 + semver-checks 门禁守护演进；
- MSRV **1.95**，工具链锁定 1.97.1，`unsafe_code = "forbid"`。

## 相关资源

- [GitHub 仓库](https://github.com/easy-4-rust/druid-rust)
- [架构文档（仓库内）](https://github.com/easy-4-rust/druid-rust/blob/main/docs/druid-rust-Architecture.zh_CN.md)
- [Easy-4-Rust 生态门户](https://easy-4-rust.github.io/)
