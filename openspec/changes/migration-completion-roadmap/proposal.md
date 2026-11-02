# Proposal

## Why

当前仓库已有大量真实实现，但最新主线、逐方法证据、源测试账本和真实宿主矩阵均未闭合，不能以历史绿灯或同名类型数量宣称迁移完成。需要一条以当前代码和新鲜证据为准的总收口路线，先恢复可信基线，再完成语义验收、架构收敛、真实数据库验证及发布门禁。

## What Changes

- 恢复当前主线可信基线：修复严格 Clippy/依赖声明问题，保留 Java 大小写敏感 Filter 语义，并让现有 OpenSpec 语义 change 完成真实验收后归档。
- 建立完整 SOURCE_PARITY、TEST_ASSET 与 JDBC 逐方法证据账本，将 `druid-test` 扩展为全项目验收包；使用 `druid-bench` 的 Java golden/live differential，禁止以测试数量、编译或覆盖率替代逐用例 `MATCH`。
- 按 Pool/Filter/Stat、SQL/Wall、RDBC、HA/Dynamic、DBCP 兼容层和 Admin 完成差分验收；`admin-live-console` 保持独立 change，本路线只消费其结果，不重复实现控制台。
- 按 ADR-CRATE-003 完成 P3.3b/P3.4：`druid` 收敛为兼容门面，`druid-rdbc` 保持标准/governance 纯净和无反向依赖。
- 在 `druid-rdbc::dispatch` 增加枚举 + 宏的静态分发原型，先覆盖 sqlx/toasty/jdbc-agent 三族，再以基准决定是否推广。
- 新增 durable 2PC 协调器及 `2pc:tx:{id}` 持久化日志、幂等重放、逐参与者决策、unknown/Mixed 和崩溃恢复证据；Mixed 绝不通过重新 commit 探测。
- 将 80 个驱动 profile 从 declared/experimental 按真实数据库、版本、feature 和平台证据逐项升级；未验证项不得标为支持。
- 以全量 `cargo llvm-cov`、全测试、MSRV、public-api/SemVer、benchmark、供应链、真实宿主和回滚演练形成最终发布判定；未闭合前保持 `NOT_READY`，不发布 crate。

## Capabilities

### New Capabilities

- `migration-verification`: 全源测试、测试资源、JDBC 方法、差分、覆盖率和整仓验收的账本与证据门禁。
- `driver-evidence-matrix`: 驱动 profile 在真实数据库、版本、feature 和平台上的能力与负能力验证矩阵。
- `static-driver-dispatch`: 在保持外部扩展点的前提下，以枚举和宏统一异构驱动的执行与元数据入口，并用基准约束性能。
- `durable-two-phase-commit`: 持久化事务日志、逐参与者决策、unknown/Mixed 状态与崩溃恢复，不与现有进程内 XA 状态机或资源 recover 混淆。

### Modified Capabilities

无。现有 `rdbc-jdbc-method-conformance`、`rdbc-xa-resource-contract`、`admin-runtime`、`metrics-runtime`、`admin-web-console` 和 `supply-chain-release-gate` 作为依赖继续适用，本 change 不改写其要求。

## Impact

- 影响 `crates/{druid,druid-rdbc,druid-wrapper,druid-metrics,druid-admin,druid-bench,druid-test}` 的实现、测试与发布验证，不改变既有公共 API 语义。
- 影响 `docs/migration/`、`docs/verification/rdbc-first/`、`openspec/changes/` 的证据与规格同步；保留历史证据，禁止覆盖或批量伪造完成状态。
- `admin-live-console`、`filter-class-identity-parity`、`exception-sorter-property-parity` 为独立前置 change；本路线不复制其任务。
- `memory-stats` 若用于生产代码必须是正式依赖；不得依靠 dev-dependency 通过局部测试。
- 两个已批准的超 800 行单 trait 文件例外继续有效且不得增长，复审日期为 2026-12-31。
- `webapp/weburi/websession/spring` 等 JVM 平台监控面如需 `PLATFORM_NA`，必须提供平台证据；产品级 Rust HTTP 埋点另立需求，不纳入本迁移完成率的偷减分母。
