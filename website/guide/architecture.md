# 架构与 Crate 组成

druid-rust 是一个六 crate workspace，职责分层清晰，依赖单向流动。

## Workspace 拓扑

| crate | 职责 |
| --- | --- |
| `druid-rdbc` | **唯一 JDBC-like 标准 trait 树**：Connection / Statement / ResultSet / Driver / 元数据，契约见 `docs/rdbc/standard-contract-v1.md` |
| `druid` | Druid 核心语义主体：pool / sql / wall / stat / dynamic 多数据源 + 内置 Toasty（约 1,644 个 core 对象语义） |
| `druid-wrapper` | 生态适配层：SQLx / RBDC / bb8 / deadpool / DuckDB / libSQL / HTTP SQL / JDBC Agent + 80 项驱动目录，附 `druid-driver` CLI |
| `druid-metrics` | 非阻塞指标运行时：sampler / aggregator / timeline / Prometheus / gRPC |
| `druid-admin` | Axum HTTP 管理面（datasource / sql / wall / metrics 端点）、认证、TLS/mTLS、独立 binary |
| `druid-bench` | Criterion 基准：pool / filter_chain / wall / adapter / runtime（仅开发验证） |

依赖方向：

```
druid-rdbc ← druid ← druid-wrapper
              ↑
        druid-metrics ← druid-admin
```

## 设计原则

1. **单一标准 trait 树**：所有适配器面向 `druid-rdbc` 编程，不各自为政——这是从"physical 接口"迁移到"standard 契约"的关键决策（见 `docs/rdbc/` 系列文档）。
2. **契约式迁移**：Java oracle + Rust contract + 真实数据库证据三重验证，每个语义点可追溯。
3. **零 unsafe**：workspace `unsafe_code = "forbid"`，clippy `all + pedantic`。
4. **证据留档**：`docs/verification/` 保存各阶段验收报告，`docs/superpowers/` 保存规格与执行计划。

## 适配器接入

为新数据库/新运行时写适配器时，从 [适配器接入者指南](https://github.com/easy-4-rust/druid-rust/blob/main/docs/rdbc/integration-author-guide.md) 开始：实现 `druid-rdbc` 的 Driver / Connection 契约，再挂入 `druid-wrapper` 的驱动目录。

## 质量门禁（CI）

- rust-contracts：fmt / check / clippy / test / doc
- public-api 快照（nightly + cargo-public-api）+ PR semver-checks
- 驱动矩阵：SQLite / DuckDB × 5 平台；JDBC Agent H2 / HSQLDB × 3 OS × JDK 17/21
- MSRV 1.95 专项 job + 供应链（cargo audit / deny）
