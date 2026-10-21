# ADR-CRATE-003：以 `druid-rdbc` 为内核、`druid` 为门面的六 crate 拓扑

- 状态：**ACCEPTED（2026-10-22 用户批准）**；P3 启动，先执行 P3.1（纯策略下沉），不一次性搬迁运行时
- 日期：2026-10-22
- 取代：与 ADR-CRATE-002 的“核心逻辑留在 `druid`”部分冲突，需显式裁决
- 关联：[迁移路线图](../迁移路线图.md) P3、[迁移验收规范](../迁移验收规范.md)

## 1. 背景

ADR-CRATE-002（ACCEPTED）规定：

- `druid-rdbc` = JDBC 4.2 标准合同；
- `druid` = Pool / Filter / SQL / Wall / Stat / Dynamic；
- 禁止 `druid-rdbc → druid` 反向依赖。

最新产品指令要求：

- `druid-rdbc` 承载 “JDBC 范式的 RDBC API + **Druid 核心逻辑** + 全部接口”；
- `druid` 退化为对外门面；
- `druid-wrapper` 承载全部真实驱动整合；
- `druid-metrics` 采集与上报；`druid-admin` 独立后端 + 静态前端；`druid-bench` 只做验证。

两者在“核心逻辑归属”上直接冲突。若不裁决就搬代码，会出现双实现漂移、公共 API 破裂和验证口径失效。

## 2. 决策（已批准）

采用**分层内核**而非整体搬迁：

1. `druid-rdbc` 新增 `governance/`：只放**与驱动无关**的算法与策略（池容量/等待/淘汰策略、统计聚合、熔断/限流、2PC 事务日志与状态机、能力协商）。
2. `druid` 保留**装配与运行时**：`DruidPool`、`DruidPooledConnection`、Filter/Wall/Stat 的运行时接线；内部改为调用 `druid-rdbc::governance`。
3. `druid` 最终只保留门面：`DruidDataSource`、`druid::sql::*` 兼容路径、builder 与重导出。
4. 依赖方向保持 `druid → druid-rdbc`，**不引入反向边**；`druid-rdbc` 仍不依赖 Tokio runtime/Axum/Tonic/具体驱动。

理由：把“策略算法”与“运行时装配”分离，既满足“核心逻辑进 rdbc”，又避免 rdbc 被异步运行时与驱动生态污染（ADR-CRATE-002 的初衷）。

## 3. 分期与回滚

| 期 | 动作 | 回滚 |
|---|---|---|
| P3.1 | 在 rdbc 建 `governance/`，先迁移纯函数策略（等待队列公平性、淘汰判定、统计聚合） | 删除模块即可，无外部影响 |
| P3.2 | `druid` 内部改调用 governance，保持公共 API 与行为 | feature flag 切回旧实现 |
| P3.3 | Filter/Wall 的**接口**下沉 rdbc，实现留 druid | 保留旧 trait 别名 |
| P3.4 | `druid` 收敛为门面，删除重复实现 | 按 public-api 快照回滚 |

每期必须：public-api 快照不变（或 SemVer 允许）、差分用例 MATCH、`cargo tree` 无禁止依赖。

## 4. 与 dbx 借鉴的关系

dbx 的“枚举 dispatch + 宏分发”与“2PC 持久化日志”属于 `druid-rdbc::governance` 与 `druid-rdbc::dispatch`，不放入 `druid` 运行时层。这样 `druid` 只负责选择与装配，分发逻辑可被所有 wrapper 复用。

## 5. 历史备选：未批准时的默认路径（已不适用）

维持 ADR-CRATE-002：核心逻辑继续留在 `druid`，仅在文档中登记“与产品指令的偏差”，并在路线图 P3 保持阻塞。任何情况下都不允许两套实现并行上线。
