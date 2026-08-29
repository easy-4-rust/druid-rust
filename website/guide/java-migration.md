# 从 Java 迁移

## 方法论：不是翻译，是契约

druid-rust 的迁移流程对每个 Java 对象/方法执行三步：

1. **登记**：进入对象总账（`docs/superpowers/specs/` 中的对象/语义映射规格）；
2. **立契**：同时写下 Java oracle（原语义，含边界行为）与 Rust contract（Rust 侧契约，含错误模型）；
3. **验证**：以真实数据库的契约测试出证据，归档到 `docs/verification/`。

这保证了"迁移完成"是可审计的，而不是"大概齐了"。

## 命名与概念映射

| Java (Druid 1.2.28) | Rust (druid-rust) |
| --- | --- |
| `DruidDataSource` | `DruidPool` + Factory 组合 |
| `Connection` / `PreparedStatement` / `ResultSet` | `druid_rdbc` 标准 trait 树同名概念 |
| `Filter` 链（FilterChain） | `filter` 链（Rust trait 实现） |
| `WallFilter` / `WallProvider` | wall 模块 |
| `StatFilter` / `DruidDataSourceStat` | stat 模块 + druid-metrics 运行时 |
| `DynamicDataSource` | dynamic 模块（ArcSwap 热切换） |
| `/druid/*` StatViewServlet | `druid-admin`（Axum） |
| JDBC DriverManager | `druid-wrapper` 驱动目录 + `druid-driver` CLI |

## 行为差异（有意为之）

- **异步优先**：JDBC 的阻塞语义在 Rust 侧映射为 async trait，保持语义等价而非性能等价；
- **错误模型**：受检异常体系映射为 `DruidError` / `druid_rdbc` 错误类型；
- **非 SQL 中间件**：Redis / MongoDB / Kafka 等不进入驱动目录（Java Druid 生态里它们经由 Wall/SQL 无关路径，本项目明确裁剪）。

## 从 Java Druid 迁移你的应用

1. 把 `DruidDataSource` 配置换成 `DruidPool` / 适配器构造参数；
2. JDBC 调用改为 `druid-rdbc` trait 方法（方法语义一一对应，参数命名 snake_case 化）；
3. 监控端点从 StatViewServlet 切到 `druid-admin`；
4. 需要 JDBC 长尾驱动时，走 `jdbc-agent` feature（JVM agent 桥接）。

## 对齐进度

- 契约 v1（RDBC 标准 trait 树）：已定稿，`docs/rdbc/standard-contract-v1.md`
- Phase 1 四驱动 conformance（tiberius / mysql_async / toasty postgres / toasty turso）：9/9 全绿
- 详细对象级台账与验证报告见仓库 `docs/verification/` 与 `docs/superpowers/`
