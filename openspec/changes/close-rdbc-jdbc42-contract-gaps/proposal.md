# 补齐 RDBC 与 JDBC 4.2 的可观察合同

## Why

2026-10-14 对 `main@af9a694` 的 CodeGraph 审查确认：69 个 JDBC 对象已登记声明，但驱动协商、时间转换、执行中取消及 `javax.sql` 接入仍有行为差异或标准入口缺失。需要将这些确定缺口转成有限、可验收的增量，而不是继续以对象数量或重复全仓回归推断完成度。

## What Changes

- 修复注册驱动连接协商：继续尝试后续驱动，全部失败时保留首次 SQL 异常；区分 `get_driver` 与 `get_connection` 的选择语义。
- **BREAKING（行为）**：修正 JDBC `Timestamp` 本地时区/毫秒转换、整秒格式化与 `Time` 纳秒舍弃语义，补 Java 8 差分证据。
- 为唯一 `Statement` 标准添加独立取消句柄，支持执行中的取消，防止迟到取消影响下一次执行；保留独占执行与非 `Sync` 语句实现。
- 补齐 `PooledConnection` 的连接/语句监听器注册、移除及可观察事件语义。
- **BREAKING（接口候选）**：为 `XaConnection` 提供可操作的标准 XA 资源，覆盖分支操作、恢复扫描、错误与 owner 生命周期，资源名称仅作为诊断信息。
- **BREAKING（接口候选）**：补齐 `CommonDataSource` 的日志、父日志命名空间和可失败超时查询；具体兼容迁移以 API 差异审查确定，不默许删除旧入口。
- 建立 Java 方法/重载/默认行为的独立分母，关联现有 Rust 迁移表、标准测试、代理和适配器证据；明确可选能力的不支持结果。
- 为 XA 客户端补齐面向应用与事务管理器的集成合同：显式数据源配置、owner 与逻辑租约边界、跨 session 恢复、故障处置、普通模式迁移和 API/SemVer 评审；文档示例必须由外部消费者编译或运行验证。
- 将资源能力、RowSet、流式/缓冲结果集的生命周期和错误传播纳入方法级证据；只验证并修复已选场景中的合同不一致，不要求所有后端实现所有 JDBC 可选能力。
- 将应用/ORM → 池化代理 → 标准合同 → 数据库适配器的职责边界补入中文主架构图，区分模块依赖、标准接口关系和后端实现能力。
- 每批完成受影响验证后停止；本次规范编写不授权代码实现、持续自动执行、发布或恢复原 18 项长期任务。

## Capabilities

### New Capabilities

以下是首次进入 OpenSpec 的能力边界，并不表示对应代码全部需要新建。

- `rdbc-driver-negotiation`：驱动查找、连接尝试顺序和异常保真。
- `rdbc-jdbc-temporal-values`：JDBC 时间值转换、精度和格式的可观察行为。
- `rdbc-statement-cancellation`：可跨任务持有、与执行身份绑定的语句取消。
- `rdbc-pooled-connection-events`：池化 owner 的监听器及逻辑连接/预编译语句事件。
- `rdbc-xa-resource-contract`：标准 XA 资源获取、事务分支和恢复协议。
- `rdbc-common-data-source`：数据源公共配置、日志与超时错误合同。
- `rdbc-jdbc-method-conformance`：JDBC 方法分母、能力证据和有界验收。

### Modified Capabilities

无。仓库此前没有 `openspec/specs/` 主规格；现有 ADR 作为架构基线引用，不复制成另一套完整架构规格。

## Impact

- 核心：`crates/druid-rdbc/src/{driver,types,statement,data_source,error}`；XA 中立类型的位置由设计文档约束。
- 必要接入：`crates/druid/src/sql/driver_manager.rs`、池化 statement/holder/event/XA 代码，以及 `crates/druid-wrapper` 的受影响适配器。
- 验证：对应 Rust 合同、Java 8 oracle、能力矩阵、API 快照与精确的兼容变更记录；不新增 ZIP、PDB 或编译产物到版本库。
- 不增加数据库驱动、HTTP 框架或连接池对 `druid-rdbc` 的生产依赖；不初始化或升级 CLI/插件。
- 保留本轮之前已修改的中英文 README 架构图；不把图示更新算作功能完成证据。

### 规格归属和范围

- 架构约束继续引用 [ADR-RDBC-006](../../../docs/superpowers/specs/2026-08-24-rdbc-single-standard-trait-tree.md)：单一 trait 树、owned handles、错误/资源归属与依赖方向。
- 本变更的 `specs/`、`design.md`、`tasks.md` 是上述七项增量的唯一规格和任务来源。原 P04 Task6–8、P05–P07 计划仅引用这里的成果，不再复制这些新增任务；历史完成状态不自动晋升。
- 本变更不迁移或覆盖既有 Superpowers 规格，不修改其核心需求、非目标或全项目验收标准。
- 不承诺所有 JDBC 方法都由所有后端实现，不宣称 JDBC TCK 合规、完整生产认证或原 18 项任务完成。
- 新发现的范围外缺口必须登记位置、影响与证据，不自动加入本变更或开启修复循环。

### 交付和关闭条件

交付 proposal、design、七份能力规范及带验收证据要求的 tasks。后续实施只有在七份规范的场景均有对应实现/处置及实际验证、架构图与能力矩阵一致、公开 API 迁移已核验、任务全部完成时才能关闭本变更；CLI 的文档完整状态不代表功能完成。
