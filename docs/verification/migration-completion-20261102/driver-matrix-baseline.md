# 驱动 profile 证据基线（2026-11-02）

- 来源：`crates/druid-wrapper/assets/database-drivers.manifest.json`（schema v3）
- 明细：`driver-profiles-baseline.tsv`（由 `cargo run -p druid-test --bin driver_profile_baseline` 重新生成；含 profile_id、display_name、artifact、platform）
- 目录 profile 总数：**80**

## 当前支持状态

| supportStatus | 数量 | 是否计入公开支持 |
|---|---:|---|
| declared | 46 | 否 |
| experimental | 34 | 否 |
| verified | 0 | 是 |
| certified | 0 | 是 |
| **合计** | **80** | **0** |

## 运行时与 provider 分布

| 维度 | 值 |
|---|---|
| runtimeMode | sqlx 34、jdbc-agent 42、native 2、http-sql 2 |
| providerId | sqlx 34、jdbc-agent 42、duckdb-rs 1、libsql 1、rqlite-http 1、cloudflare-d1-http 1 |

## 约束

- `supported_count()` 当前必须为 **0**，因为没有任何 verified/certified 证据。
- 目录存在、工厂实现、SQLite 路由或单一平台测试都不能晋升 supportStatus。
- 每个 verified/certified 行必须有数据库产品版本、驱动版本、feature、平台、正向/负向合同和真实运行产物。
- 未配置环境记录 `NOT_RUN`，ignored 不计通过。
- 逐产品矩阵任务（9.2-9.8）完成前，该文件保持基线而非完成声明。

## 9.2 统一矩阵 runner 结果（2026-11-02）

命令：`cargo run -p druid-test --bin driver_matrix`，输出 `driver-matrix-results.tsv`。

| 产品 | feature | 平台 | 结果 |
|---|---|---|---|
| SQLite | sqlx | windows/x86_64 | **13/13 PASS**：connect、query、update、prepared、metadata、types、null、batch、transaction、savepoint、cancel、close、error |
| 其余 79 个 profile | 各 runtime/provider | windows/x86_64 | **1027 NOT_RUN**：未配置对应数据库/凭据；每个 profile × 13 操作均保留原因 |

每个结果行包含 `profile_id`、产品版本（未验证时为 `unverified`）、feature/provider、平台、capability、status、detail。SQLite 的行是真实 SQLx 内存库执行结果，不是 mock；其余产品未因目录存在或 SQLite 通过而晋升。9.3 仍需 MySQL/PostgreSQL 真实实例，9.4-9.8 仍需各自宿主。
