# 以当前代码为准的剩余迁移工作

> 2026-11-02 状态更新：修订 04 已获批准并落实到生产 Vue / Rust。本文件保留设计阶段的范围与基线记录；当前实现、验证及尚未接入指标以 [生产落地记录](../../verification/admin-live-console-20261102.md) 和 OpenSpec `admin-live-console` 为准。
状态：增量核对与设计建议，2026-11-01；不是全量迁移完成证书。

## 1. 结论与判定范围

当前仓库已具备相当多真实能力。新版 Admin 应在既有独立服务、Vue 前端、管理接口、SQL 解析和 metrics 协议上增量建设，不应重新实现一套管理台。旧计划未更新、DTO 字段存在、测试通过和端到端能力齐备，是四件不同的事。

本轮用 CodeGraph 追踪 Java `MonitorViewServlet` / `MonitorStatService`，Rust 的 v1 API、管理数据源、快照仓库、metrics 协议、控制台与路由；结合前一轮 [当前代码核对](../../verification/current-code-audit-20261101/README.md)、[逐方法核验](../../verification/method-semantics-20261101/README.md) 与现行 OpenSpec。两个 Druid 仓库均已有索引，无需重新初始化。

此前普查的 1,443 条未定位名称映射仍是 REVIEW_REQUIRED，**不是 1,443 个缺失功能**。本轮没有逐篇通读全部历史文档、逐方法重验全部 Java core，也没有跑全驱动真实数据库矩阵，因此不提供虚假的全项目完成百分比。

## 2. 已实现但文档滞后：复用，不重建

| ID | 当前事实 | 代码证据（仓库相对路径） | 处理 |
|---|---|---|---|
| D01 | workspace 有七成员：五产品 crate＋bench＋test | `Cargo.toml` | 保留 `druid-test`；不是还缺五个 crate |
| D02 | rdbc 有 governance，部分纯策略、Wall 值对象与 hashing 已下沉 | `crates/druid-rdbc/src/governance/`、ADR-003 当前进度 | P3.1/P3.3a 不整体重做；剩余运行时边界单列 |
| D03 | 独立 Admin、嵌入 Vue 静态资产、版本化 API 已存在 | `crates/druid-admin/{build.rs,src/druid_admin_application.rs,web/}` | 保留框架与发布方式 |
| D04 | 数据源详情有池配置、连接/事务统计；SQL 有解析信息、错误及直方图；Wall 有表/函数和名单视图 | `web/src/views/{DatasourceDetail,SqlDetail,WallView}.vue` | 整理呈现和能力标识，不重复建设基本页面 |
| D05 | sqlx/rbdc/bb8/deadpool 整合归属 wrapper | `crates/druid-wrapper/Cargo.toml` 与对应模块 | 按 feature 验证现有整合，不恢复旧独立 crate |
| D06 | 快照、ACK、生命周期、背压与有界仓库已有实现 | `druid-metrics/src/`、`druid-admin/src/repository/metrics_repository.rs` | 补可观测性和故障验证，不重建传输协议 |
| D07 | 慢 SQL 页面、SQL 服务端解析是后来实现，早期审计快照未完全包含 | `web/src/views/SlowSql.vue`、`src/servlet/monitor_view_servlet/sql_parse.rs` | 本轮选定 Admin 测试复跑；前端语义缺口仍单列 |

## 3. 已实现但缺验证：优先延伸已有测试

| ID | 已有实现 / 证据 | 尚缺什么 | 完成证据要求 |
|---|---|---|---|
| V01 | 本轮 24 项 Admin 针对性测试通过 | 不覆盖全部 Vue 交互、浏览器权限态、多实例 ID 冲突、完整 Java 管理语义 | 原有 integration tests 增补合同；浏览器实际断言；Java 服务请求/响应差分 |
| V02 | SQL/Wall/池/Filter/排序器已有大量单测和差分 | 逐对象、逐方法、原始用例与资源账本未全闭合 | SOURCE_PARITY、RUST_OBLIGATION、VALUE_ADD 分账；源码/资源哈希、原断言和差异审批关联 |
| V03 | metrics 任务监督、有界队列和 Admin TTL/容量预算已有测试 | 释放门禁的真实进程故障、重启、乱序/重复、停机以及跨宿主内存/时延证据不足 | 复用 `admin-runtime` / `metrics-runtime` 数值预算；p50/p95/p99、RSS、未刷出与待 ACK 数，失败不能跳过计完成 |
| V04 | wrapper 注册、SQLite 路由与 XA API 已有测试 | 先前 3 项真实 XA 宿主测试 ignored；不能代表数据库恢复通过 | 真实 JDBC Agent/驱动、数据库与恢复日志；Windows/Linux/macOS 支持范围具名 |
| V05 | 支持多数据库的驱动映射与能力声明 | 产品名称、协议兼容和某版本可执行能力未构成全量互通矩阵 | 每个数据库×版本×驱动×平台登记连接、类型、元数据、批处理、事务、取消、恢复及负能力 |
| V06 | 本轮 scoped llvm-cov TOTAL 行 26.91% | 不是默认＋全部 feature 的整仓覆盖，也没有分支覆盖证据 | 按技能/正式验收门槛跑全范围，分别报告 crate、关键对象和整仓分母；不得通过减少范围过门禁 |

## 4. 已确认缺口与语义偏差

“缺口”按行为记录，并不意味着整个模块未实现。以下没有在本轮偷偷改为已完成。

| ID | 事实与定位 | 最小修复方向 | 验收用例 |
|---|---|---|---|
| G01 | Vue 数据源路由仅 `:identity`，SQL 仅 `:id`；v1 数据源查找只匹配数字 identity；SQL 可收 serviceId 但客户端不传 | 在现有 API 上增加明确复合定位；旧模糊请求按兼容策略处理 | 两服务、两池拥有相同 ID，交叉跳转/刷新/深链接均返回正确对象；模糊请求不静默选首项 |
| G02 | `SlowSql.vue` 以 SQL 累计最大耗时筛选，却汇总这些 SQL 的全部 executeCount 作为慢 SQL 执行占比 | 先改为“曾慢 SQL 涉及的执行占比（累计）”；真正慢请求比例待窗口计数/样本合同 | 一个 SQL 执行 100 次仅一次慢，不得显示慢请求占比 100% |
| G03 | v1 basic/list 的部分错误路径使用 `unwrap_or_default`；ManagementSource 在远端空/失败时可回退本地 | 返回来源、部分失败与新鲜度；保留最后成功快照并明确标记 | 远端失败且本地空不显示“全部健康/零错误”；混合源不伪装为单一 poll/push |
| G04 | protobuf `SqlStatMsg` 仅 fingerprint、执行数/总时长/取回行/更新行；缺 max/error/histogram 等丰富字段 | 能力协商与字段可用性先行；按批准协议增量补采集 | push 不支持的字段显示“不支持/未上报”，不得被 DTO 默认零渲染为真实值 |
| G05 | `timeline.rs` 已有三档环形缓冲，但此次定位仅见自身/测试使用；快照仓库是最新态，没有已接通的历史查询链路 | 复用缓冲原语，补采样/降采样、重置/断点语义和历史 API | 不补造过去数据；跨 boot/epoch 断线；超过保留期明示；内存预算通过 |
| G06 | `App.vue` 用 `route.fullPath + tick` 作为 router-view key，刷新重建视图 | 请求驱动的增量更新；取消旧请求，保留筛选/分页/滚动/抽屉 | 慢请求晚返回不能覆盖新上下文；自动刷新不清搜索、不关闭详情 |
| G07 | 慢 SQL 拉取 100000 行后浏览器筛选；v1 perPageCount 只有下限，无上限 | 服务端过滤/排序/有界分页；对旧客户端定义明确上限策略 | 大数据源不能触发无界响应；跨页排序稳定，查询过滤与 total 一致 |
| G08 | v1 OpenAPI 是手写 JSON，现行规格要求与 handler 定义同源生成 | 后续让 schema/路由生成共享定义，并加漂移检查 | JSON 类型、错误响应、参数可选性与真实端点一致 |
| G09 | 连接列表与活跃堆栈存在 legacy poll 路径，push 明确不支持；v1 未暴露这些能力 | 首先复用 poll 能力并在 UI 按能力显示；push 扩展另立验收 | 不把“不支持”变成空连接列表；鉴权/脱敏与 legacy 一致或更严 |
| G11 | 排序器非法配置/Phoenix 日志副作用尚未对齐，构造环境等仍缺验证 | 延续上一批逐方法清单，不重写已经 MATCH 的解析 | 原 Java 可观察副作用与 Rust 对照；区分实现缺口和验证缺口 |

G01–G09 证据主要来自 `crates/druid-admin/src/servlet/monitor_view_servlet/{v1_api.rs,console_routes.rs,monitor_view_a.rs}`、`service/management_source.rs`、`web/src/` 和 `crates/druid-metrics/proto/druid_metrics_v1.proto`。G11 见上一批逐方法报告。G10 在评审中移至 N01，不再计作已批准迁移缺口，保留编号空位避免引用漂移。

新增设计建议 N01：当前 `config/admin_role.rs` 仅有 Operator / Ingest。专门的只读 Viewer 是 A4 可选增强，现行规格未要求，不计 Java 迁移未完成；获批后再补服务端读取范围、命令拒绝和脱敏测试，不能只在前端藏按钮。

## 5. 待核实，不能当作已证实缺失

- **持久化 2PC 协调器**：已有 XA 状态和资源 recover 接口，尚未定位并验证持久化协调器全链路；进程内状态不等于崩溃恢复。沿用原核对报告的“待核实”分类，不与上表 G03 混淆。后续明确 `2pc:tx:{id}` 日志、逐参与者决策、Mixed / unknown 状态与恢复矩阵；Mixed 绝不通过再次 commit 来探测。Admin 仅在有真实状态与权限后展示，不能先做一个“重试提交”按钮。
- **70+ / 78 种数据库**：属于覆盖目标，不能由别的项目的驱动清单推导本项目已实现数量。[DBX 文档](https://dbxio.com/cn/docs/what-is-dbx)与[驱动列表](https://dbxio.com/cn/drivers)本次查阅宣传范围已为 100+；可借鉴能力矩阵与分平台交付，不借用其数字作 Druid 验收。枚举 dispatch / 宏分发和持久化 2PC 的内部实现本轮未读取 DBX 源码，不把用户提出的借鉴方向当成已核实源码事实。
- **架构 P3 收尾**：ADR-003 已批准，不能再次要求架构二选一；当前 `druid` 仍有运行时。最后搬迁需要同时满足 rdbc 无具体驱动/运行时污染与 facade 兼容，不为目录好看直接搬文件。

## 6. 剩余执行顺序（建议，未执行）

| 批次 / 任务 | 依赖与范围 | 退出条件 / 回滚 |
|---|---|---|
| R1 语义证据收敛 | 延续对象/方法台账；先 RDBC 公开合同、异常、池/Filter、SQL/Wall、管理接口 | 每条关联现有实现/Java 用例/资源/实际结果；不能定位即待核实；批准差异独立登记 |
| A1 Admin 数据可信度 | G01/G02/G03/G04/G06/G07；依照正式 admin-web-console 建增量 change | 同 ID、多源失败、能力缺失、计数重置、刷新竞态测试先红后绿；兼容旧入口，可回退旧控制台 |
| A2 浅色交互落地 | A1 稳定后替换现有 Vue 视图，保留 API 与静态嵌入 | 三尺寸、键盘、空/错/权限/部分数据验收；现有 SQL 解析/配置/Wall 功能不丢失 |
| A3 趋势与采集健康 | G05；把现有 self_metrics 接通独立观测，不阻塞 SQL 热路径 | 协议兼容、采样正确、预算内存、断线/重启、饱和停机全部有数值证据 |
| A4 诊断与操作 | 真实连接/堆栈能力；命令 ACK、审计和 Viewer 增量规格 | 命令超时是 unknown 而非失败重试；重复点击不重复执行；敏感字段服务端脱敏 |
| R2 拓扑收敛 | ADR-003 P3.3b / P3.4，复用已下沉策略 | public-api 快照、调用点等价、依赖图约束和原差分；分阶段可回退，不留下双实现 |
| R3 驱动与事务 | 基于真实驱动 feature / 环境矩阵；2PC 先明确边界与定位 | 负能力、真实 DB 事务恢复、Mixed、进程崩溃结果；不把 ignored 算通过 |
| R4 发布门禁 | R1–R3 与 Admin 数据链路收敛 | 技能要求的全测试/覆盖、默认和全 feature、MSRV、API、lint、宿主与故障预算；原始日志和源码快照归档 |

A1/A2 可与独立的 R1 台账工作分轨，但同一文件应明确归属并保留并行修改。此处没有安排多 Agent 自动改代码，也没有擅自指定时间/工期。

## 7. 文档纠偏方式

已有四份迁移文档及 2026-11-01 审计报告继续有效；通过当前报告的入口链接补充新结论，不覆盖历史基线。实现完成后才把对应任务从“方案/待验证”升级；设计原型、24 个 Admin 测试、26.91% scoped 覆盖率都不能升级“Java Druid 全量迁移完成”。


## 原型补齐记录（revision 04）

主页及八个主页面的紧凑监控交互已补齐，59 项浏览器检查通过，详见 [验证记录](verification.md)。此进度仅属于设计评审交付：A1–A4、G01–G08、全仓迁移与覆盖率门禁的生产状态保持原分类，不因演示指标或原型下钻已可点击而标记完成。实施时复用现有 Vue 页面和真实 API，逐项保留原版字段与操作。
