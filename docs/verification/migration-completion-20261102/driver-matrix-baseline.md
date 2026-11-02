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
