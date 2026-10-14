# Tasks

本文件是本增量的唯一实施清单。需求 ID 对应本目录 `specs/`，技术方案见 [design.md](design.md)。

## 当前状态与执行边界

- 进度：49/56 项完成；6.5.1–6.5.5 已分别由 Rust XA RPC 合同/兼容性、显式数据源与标准入口、owner/外部池真实生命周期、Rust 客户端经 Agent 在 PostgreSQL 的跨 session 恢复，以及 XA 对外交付证据关闭；7.1–7.4 公共数据源合同、诊断和迁移文档已关闭；5.5 原生池标准 owner 路径、8.1 冻结 Java 方法清单提取器、8.2 方法级证据关联/负向门禁、8.3 源码哈希/输入版本失效门禁、8.4 范围/场景追溯报告和 8.5 架构图边界审查亦完成。证据见 [XA 客户端证据](evidence/xa-rust-client.md)、[公共数据源证据](evidence/common-data-source-timeout.md)、[原生池标准 owner 证据](evidence/pooled-connection-owner.md)、[Java API 清单证据](evidence/java8-api-method-inventory.md)、[方法证据台账](evidence/method-evidence-ledger.md)、[证据新鲜度门禁](evidence/method-evidence-freshness.md)、[范围/场景报告](evidence/scope-and-scenario-report.md) 与 [架构图边界审查](evidence/architecture-boundary-review.md)。其余 7 项仍未完成。完成项必须同时有实现和本地证据，不能仅凭旧测试记录勾选。
- 取消批证据见 [取消合同证据](evidence/cancellation-contract.md) 和 [SQLx 取消能力矩阵](evidence/sqlx-cancellation-capability-matrix.md)：SQLite runtime 3.46.0 VM 执行中断及同物理连接复用、批处理与结果集入口、清理屏障、Future drop/owner 关闭竞态及连接池精确丢弃计数、PostgreSQL/MySQL/Any/生成键选择精确 `0A000`、三类池化代理的真实租约/缓存隔离及取消代次测试均有记录；取消批 4.1–4.7 已关闭。
- SQLx 后端×执行入口矩阵、真实 SQLite 执行终态与连接处置、逐路径支持/不支持测试、三类池化代理生命周期回归和用户文档已完成；XA 中立标准 API、JDK 8 常量/分支状态机及 6.3 扫描、身份、超时和示例 owner 失效均有独立证据。6.4 探测已证实 H2 2.3.232 不满足断开后恢复，并以 PostgreSQL 18.6 Testcontainers + pgjdbc 42.7.14 通过独立 JDBC 恢复试验；现已补齐真实 Agent 协议和 Rust RDBC 跨 session 恢复，细节见 [XA 后端恢复探测](evidence/xa-backend-recovery-probe.md) 与 [Rust XA 客户端证据](evidence/xa-rust-client.md)。取消范围保持关闭。
- 每批只运行该批目标测试和直接受影响的回归；达到表格中的停止点即停并报告。最终综合门禁只在第 9 批运行一次，不循环重跑已通过的全量命令。
- 真实后端、Java 8 或 XA 环境不可用时，相关任务记为 `NOT_RUN` 并说明阻断，不用 mock、编译成功或其他版本的 oracle 替代实际证据，也不自动安装依赖或扩大时间预算。
- 执行顺序固定为：XA Rust 客户端实库恢复 6.5.4 与 XA 对外交付 6.5.5 已完成；公共数据源 7.1–7.4 已完成；因 PE-02 补充要求，当前先完成 5.5 原生池标准 owner 路径，再进入 8.1 方法级证据。随后按顺序处理 8.x 和综合验收 9.x；每项独立验收、提交和推送，不循环重跑已通过的全量门禁。
- 范围仅限本变更列出的七项能力、MC-06 的资源/结果集方法级验收及证据门禁。MC-06 只要求把现有后端能力和缺口准确分级，不要求所有后端实现所有 JDBC 扩展。额外发现登记到后续清单，不自动加任务；关闭本变更不代表 P04–P07、全 JDBC/TCK 合规或整个项目生产就绪。

### 架构视角的补充路线

沿中文主架构图的调用边界验收，不按类型或文件数量判断完整度。图中的 `druid-rdbc` 是统一合同，`druid` 持有池化 owner、租约与回收策略，`druid-wrapper` 承担具体后端 I/O；后续验证须分别给出合同、代理、适配器和真实后端证据。该路线只归并已有规范与任务，不额外扩大本 change 的能力范围：

| 架构合同面 | 要闭合的行为 | 对应规范与任务 | 关闭证据 |
|---|---|---|---|
| 连接执行链：`Connection → Statement → ResultSet`，及 `PreparedStatement → CallableStatement` | 语句关闭/失效通知、身份保持、回调重入与失败隔离；普通与预编译入口在逻辑关闭、缓存、取消和租约归还时行为一致 | PE-03/04；5.3–5.4、6.5.3、8.6–8.8 | 外部标准消费者、Druid 代理生命周期回归、指定适配器行为；事件顺序和源码身份记录 |
| 驱动与数据源入口：`Driver`、`DataSource`、公共配置 | 查找与连接协商边界清晰；超时失败可观察；日志按实例隔离且不泄露凭据；XA 显式创建和普通模式拒绝提升 | DN-01–03、CD-01–04、XA-05；2.1–2.4、6.5.1–6.5.5、7.1–7.4 | 外部驱动/数据源消费者、错误与脱敏注入、XA 真实后端恢复；迁移及 API/SemVer 审查 |
| 元数据、SQL 类型与异常 | 按冻结 Java 方法/重载逐项映射；空值、错误类别/SQLState、默认行为及兼容性差异可追溯 | MC-01–04；1.3、8.1–8.4、9.1–9.4 | 方法级差异清单、合同/适配器证据分层、外部消费者结果；未验证能力明确标为 `NOT_RUN` |
| 资源与扩展：LOB、Array、SQLXML、SQLData、RowSet | 按方法核对实际操作、能力查询、释放错误与 owner 归属；扩展缺失时返回精确不支持结果，不从类型声明推断支持 | MC-06；8.6–8.8 | 有限方法级矩阵、重复/失败释放与结果集生命周期测试、逐后端能力状态 |

按依赖顺序执行时，先闭合池事件，再闭合 XA 与公共数据源，最后补齐方法级矩阵及总体验收。每个阶段仅运行该阶段的目标测试和直接受影响回归；真实环境不可用时保留 `NOT_RUN`，不以 mock 或编译结果替代。架构图验收沿用 8.5，不重复改写已经符合该合同的图。

### 接下来交付队列与停止条件

本队列将当前未关闭项压成有限的四个交付，不新增能力范围，也不因发现其他缺口自动开启实现。依次完成并分别提交/推送；每个交付到停止点后向用户报告结果，继续下一批须有明确继续指令。

| 顺序 | 当前缺口 | 最小交付 | 通过证据 | 停止点 |
|---|---|---|---|---|
| A. XA owner 生命周期（已完成） | 非 SQL Agent 传输故障事件未通知外部池 | 已补安全连接错误事件投影、真实 Agent EOF discard 测试、逻辑租约复用测试及 Druid 普通池边界结论 | 红测 `0 != 1`，修复后真实 Agent+H2 两项集成测试通过；源码哈希与 CodeGraph 类型边界见 [XA 客户端证据](evidence/xa-rust-client.md#653-已完成xa-owner-事件与-sticky-故障隔离) | 已关闭 6.5.3；原生 Druid XA 池支持若未来需要须单独设计 |
| B. XA 实库恢复（已完成） | Rust 客户端尚无 Agent + PostgreSQL 跨 session 真实读回 | 已从 Rust RDBC 外部消费入口执行两条 XA 分支 prepare，关闭原 session，新建 session recover 后分别 commit/rollback，并由独立 JDBC 连接读回 | Testcontainers/Agent/PostgreSQL 版本、命令、退出码、完整 Xid 与最终读回、源码哈希，见 [Rust XA 客户端证据](evidence/xa-rust-client.md#654-rust-rdbc-postgresql-跨-session-xa-恢复) | 已关闭 6.5.4；完成后停止于本功能点 |
| C. XA 对外交付（已完成） | XA 配置和生命周期文档、API/SemVer 审查尚未验收 | 已增加 XA 集成指南、事务生命周期 Mermaid 图、标准消费者编译/行为验证及 API/SemVer 评审 | 文档、测试命令、真实 PostgreSQL 结果、源码哈希与 API 快照差异见 [Rust XA 客户端证据](evidence/xa-rust-client.md#655-xa-对外交付与-apisemver-审查) | 已于 6.5.5 通过并停止 XA 批 |
| D. 公共合同与关闭证据 | 第 7–9 批仍未完成 | 按既有任务依赖逐批完成 CommonDataSource、方法分母与资源/结果集有限矩阵，最后只运行一次综合门禁 | 7.x、8.x 对应证据及 9.x 最终门禁 | 每批独立停止；只有第 9 批通过才能关闭此 OpenSpec change，不能据此宣称 P04–P07 或全项目生产就绪 |

本轮已完成 C（6.5.5 XA 对外交付），并以 PostgreSQL 实库和外部消费者验证；本批停止。D（7.x–9.x）仍按既有依赖顺序执行；对任何重复失败，只有实现、输入或环境发生可验证变化后才重跑，否则记录失败签名与所需条件。

| 批次 | 依赖 | 交付与停止点 |
|---|---|---|
| 1 准备 | 无 | 固定基线、映射、后端和命令预算；环境缺失显式列出 |
| 2 驱动协商 | 1 | 注册表和 DriverManager 合同验证通过即结束 |
| 3 时间语义 | 1 | Java 8 差分及 Rust 原生值回归通过即结束 |
| 4 取消控制 | 1、2 | 执行/租约竞态及一个真实后端验收通过即结束 |
| 5 池化事件 | 1 | 标准入口、通知和清理测试通过即结束 |
| 6 XA 资源 | 1、5 | 标准协议与真实恢复闭环通过即结束 |
| 7 公共数据源 | 1、2、6 | 三类数据源配置、错误和日志隔离通过即结束 |
| 8 方法证据 | 2–7 | 七项能力、资源及结果集场景全部可追溯，其他缺口独立列出 |
| 9 综合验收 | 8 | 一次约定的集成门禁及兼容审查，结束本增量 |

批次顺序表示依赖。每个功能批验证后分别提交、推送；出现具体阻断时只暂停受影响批次并记录证据。每批落地自身测试、迁移说明和证据，最后一批仅做跨模块集成核验。停止点是硬边界：不得因“还有未完成项”自动开始下一批；只有后续明确的继续指令才推进下一批。

## 最近关闭批次：4.1–4.7 取消控制

本批已按 SC-01–05 完成独立取消控制合同、SQLx 能力矩阵、真实 SQLite VM 终态及连接处置、正负向适配器与池化代理验证。证据见 [取消合同证据](evidence/cancellation-contract.md) 和 [SQLx 取消能力矩阵](evidence/sqlx-cancellation-capability-matrix.md)。本批停止；未收到后续继续指令时不自动开始第 5 批。

验收时逐层区分四类证据，不能相互替代：

1. **标准合同**：不支持能力精确返回 0A000；支持能力可在 Statement 执行 Future 挂起时由独立任务调用。
2. **Druid 代理**：普通、PreparedStatement、CallableStatement 均转发到原语句的取消控制；关闭逻辑句柄、归还租约或复用缓存语句后，旧句柄不能触达新租约。
3. **适配器执行路径**：分别检查流式游标、缓冲查询和 `execute` 等入口。能力只接入部分路径时，未接入路径必须拒绝声明支持，并在能力矩阵中精确列出。
4. **真实后端**：至少运行一个由准备阶段选定的真实后端长查询；记录请求开始、取消结果、服务端执行终态、后续查询结果及连接复用/丢弃决定。小型即时 `SELECT`、mock、取消 Future 本身和本地 cursor drop 均不能替代该证据。环境不可用则记录 `NOT_RUN`，本批不通过。

本批定向回归已覆盖 `druid-rdbc` 取消合同、`druid` 普通/预编译/调用语句代理生命周期，以及 SQLx SQLite 的执行终态和资源清理。最终命令、feature、后端版本、退出码和源码哈希均记录在取消合同证据中。

### 下一步执行顺序与明确停止点

本执行批的目标是关闭“SQLx 取消控制与 Druid 租约的端到端合同”，不扩展成所有后端都支持取消，也不把 `NOT_RUN` 记为通过。每个阶段只运行列出的目标测试和直接受影响回归；出现未解决的验收失败时停止在该阶段，报告具体阻断。

1. **核对文档和最终证据（4.7）**：检查中文架构文档是否准确说明预先获取取消句柄、请求结果与执行终态的区别、租约隔离、清理失败时丢弃，以及矩阵中的实际后端/执行入口能力。核对证据哈希和严格 OpenSpec 校验；通过后报告并停止，不自动开始 5.1。

**本批完成判定：**4.1–4.7 全部有实际证据，且后端正向和负向路径、连接处置、旧租约隔离均可复核，才将取消批标记为完成。仅有模拟测试、类型/符号存在、编译成功、Future 结束或实库 `0A000` 均不满足该判定。

## 1. 固定验收输入与范围

- [x] 1.1 核对实施时 HEAD、工作区、ADR-RDBC-006 与相关模块，用 CodeGraph 重新确认六类缺口；交付基线记录，明确相对 `af9a694` 已修复和仍缺失项，验证未覆盖既有 README 等修改。（MC-04）
- [x] 1.2 核验冻结 OpenJDK 8u 与 Druid 1.2.28 来源身份，记录 Java 8 运行时、时区数据及输入；实际运行最小时间 oracle，缺 Java 8 则记录阻断原因，不用其他版本替代。（TV-04、PE-01）
- [x] 1.3 将六类缺口的 Java 签名映射到 Rust 入口，补 XA 的 `javax.transaction.xa` 来源；交付可审阅的映射，逐项核验重载、默认行为和继承来源没有被对象级计数隐藏。（MC-01）
- [x] 1.4 检查已有可用数据库及适配器，固定取消正向验收后端和 JDBC Agent XA 恢复数据源；交付版本、feature、启动条件、资源清理和命令期限清单，实际连通检查结果与未运行项分别记录。（SC-01、XA-03、MC-02）
- [x] 1.5 为各批选择目标测试与直接受影响回归，记录精确命令、超时和预计证据位置；检查不包含无条件全仓循环、不新增安装/发布步骤，所有规范场景均有计划验证入口。（MC-04）

## 2. 修复驱动连接协商

- [x] 2.1 在注册表合同测试中加入“首个 SQL 异常、后续成功”“空结果继续”“不调用 accepts_url/name”案例；运行并记录当前缺陷导致的失败，确认不是测试设施错误。（DN-01、DN-02）
- [x] 2.2 修改 `driver_registry.rs` 的连接遍历及首次异常保留，保持查找入口独立；验证成功短路、全部失败的完整异常载荷、空注册表结果和 DriverManager 的 08001 映射。（DN-01、DN-02）
- [x] 2.3 覆盖注册回调重入、顺序快照、单驱动 SQL 超时与整体期限的区别；验证整体取消/到期不启动后续驱动、无注册锁死锁且无凭据输出。（DN-03）
- [x] 2.4 更新驱动实现指南与方法证据，运行注册表、DriverManager 及标准外部驱动的受影响测试；核对文档示例和实际调用顺序一致后结束本批。（DN-01–03、MC-03）

## 3. 修复 JDBC 时间值

- [x] 3.1 建立 Java 8 时间差分输入与隔离进程运行器，覆盖 UTC/上海/纽约、负 epoch、DST gap/overlap、时区变更和范围边界；保存原始 oracle 输出并确认 Rust 当前差异可复现。（TV-01、TV-04）
- [x] 3.2 修正 `Timestamp` 为绝对时刻身份及本地字段转换，验证 epoch 0 在上海为 08:00、本地零点为 -28800000 毫秒，支持运行时默认时区变更，并实现 Java 8 DST gap/overlap 选择策略。（TV-01）
- [x] 3.3 修正纳秒校验及 `.0`、`.000000001`、`.12` 格式；验证非法修改保持旧值、超范围返回错误，无 panic 或溢出。（TV-02、TV-04）
- [x] 3.4 修正 JDBC `Time` 的 LocalTime 转换舍弃纳秒，验证来回转换为整秒，同时直接使用 Rust SQL TIME 值仍保留原精度。（TV-03）
- [x] 3.5 完成时间行为破坏的迁移说明，核对所有 From/TryFrom、序列化及适配器消费入口；运行差分与受影响值合同测试，记录 Windows OS 时区切换未实测的限制后结束本批。（TV-01–04、MC-03）

## 4. 增加独立取消控制

- [x] 4.1 增加外部实现合同案例：非 Sync Statement 执行 Future 活跃时，另一任务持有预先取得的取消对象；用 compile-fail doctest 记录旧接口借用冲突，交付唯一标准树中的独立异步取消接口，验证最小实现和 0A000 默认行为。（SC-01、MC-03）
- [x] 4.2 实现 owner、逻辑租约、语句和执行代次绑定；在普通 Statement、PreparedStatement、CallableStatement 三类 Druid 代理上验证控制句柄均到达原语句，旧租约/已关闭句柄不能借缓存复用影响新租约；以可控调度覆盖旧请求迟到、重复请求和共享连接隔离。（SC-02、SC-04）
- [x] 4.3 接入取消域隔离、请求期限和清理屏障；注入未确认、自然完成竞争和 owner 关闭，验证只有一个终态、完整 SQL 错误、无重复回收，不确定连接不能重新借出。分别验证执行 Future 被 drop 时不伪称后端取消。（SC-03、SC-04；证据见 [取消合同证据](evidence/cancellation-contract.md)）
- [x] 4.4 建立 [SQLx 后端 × 执行入口能力矩阵](evidence/sqlx-cancellation-capability-matrix.md)，覆盖流式查询、缓冲查询、通用 `execute`、更新、普通/预编译批处理、生成键查询及异步结果集；逐项核对取消标记到数据库 VM 的正向证据、代次隔离、清理屏障和未支持后端的精确 `0A000`。能力声明不依据共享 trait 或 adapter 类型推断。（SC-01、SC-05）
- [x] 4.5 在固定的 SQLx SQLite VM 上执行长递归查询取消，采集运行时版本、精确 SQL、执行开始、取消请求结果、SQLite 中断终态、后续查询及同一物理连接复用结果；取消/自然完成竞争与迟到请求隔离由既有定向合同测试覆盖。专项测试使用 `--nocapture` 记录直接证据，源码哈希与命令已写入取消合同证据。此项仅证明 SQLite，不为 PostgreSQL/MySQL 声称取消支持。（SC-02–05、MC-02）
- [x] 4.6 对矩阵中声明支持的 SQLite 路径运行正向适配器测试，对 SQLx PostgreSQL/MySQL/Any 和生成键列选择运行精确 `0A000` 测试；覆盖取消目标代次不穿透到下一次执行、同连接其他语句和新租约。运行普通/PreparedStatement/CallableStatement 委托、关闭、缓存复用及租约归还回归；结果、命令和源码哈希见 [取消合同证据](evidence/cancellation-contract.md)。（SC-01–05）
- [x] 4.7 更新中文架构文档中的标准及池化代理取消用法、后端/路径能力矩阵和关闭说明；将取消请求结果与执行终态分开记录，不把 Future drop 或本地结果集结束描述为数据库已取消。目标测试、代理回归、格式和 OpenSpec 严格验证均通过；本批到此停止。（SC-01–05、MC-02、MC-04）

## 5. 补齐池化连接事件合同

- [x] 5.1 从冻结 Druid 来源提取重复注册/移除案例，增加标准接口四个监听器方法及身份匹配合同；用仅依赖 `druid-rdbc` 的外部池实现编译、运行验证，无私有 holder 下转型。（PE-01；证据见 [池化事件证据](evidence/pooled-connection-events.md)）
- [x] 5.2 将逻辑关闭、致命连接错误和物理 owner 清理接入统一通知；验证原 owner 身份、完整异常、重复关闭只归还一次和失败连接不会再借出。产品层事件已沿用 holder 快照，且现已隔离监听器 panic，避免跳过 fatal 处置或逻辑关闭回收；定向证据见 [池化事件证据](evidence/pooled-connection-events.md)。（PE-02）
- [x] 5.3 贯通 PreparedStatement/CallableStatement 的关闭与失效事件；验证携带原语句身份、移除监听器生效、回调重入无锁死及可展开 panic 不跳过必要清理。Prepared 与 Callable 的执行 SQL 异常均发送错误事件；逻辑关闭只发送一次，并在锁外调用监听器快照。实现、命令、退出码与源码哈希见 [池化事件证据](evidence/pooled-connection-events.md#pe-03-preparedstatementcallablestatement-事件)。（PE-03、PE-04）
- [x] 5.4 更新外部池事件示例与生命周期说明，运行标准事件及产品 holder/逻辑代理回归；逐项核对事件顺序和冻结重复注册行为，证据落盘后结束本批。文档见 [池化连接事件与生命周期](../../../docs/rdbc/pooled-connection-events.md)，命令、退出码、事件顺序与源码哈希见 [池化事件证据](evidence/pooled-connection-events.md#pe-05-外部池示例生命周期和顺序验收)。（PE-01–04、MC-03）
- [x] 5.5 为 Druid 原生池增加标准 `PooledConnection` owner 入口，保持现有 `DataSource`/`DruidPooledConnection` API 不变。新增 owner 持有 holder、稳定事件 source、单活动租约和代际校验逻辑代理；逻辑关闭后 holder 保留供再借，owner 关闭先拒绝新租约、使旧句柄失效并复位活动连接，再一次性归还或丢弃。新增消费者测试验证稳定 source、两次逻辑关闭事件、活动 owner 关闭、physical create/close 和 active/idle 计数；生命周期与架构图已更新。命令、退出码和源码哈希见 [原生池标准 owner 证据](evidence/pooled-connection-owner.md)。（PE-02、MC-02、MC-03）

## 6. 提供可操作 XA 资源

- [x] 6.1 将 XA 中立接口、Xid、flags、prepare 结果和结构化错误纳入唯一标准来源，保留必要迁移重导出；验证一对象一文件、无产品反向依赖，外部事务管理器能获取并调用标准资源。（XA-01、XA-02；证据见 [XA 标准合同证据](evidence/xa-standard-contract.md)）
- [x] 6.2 实现 XA 协议示例桥接 start/end/prepare/commit/rollback/forget，按冻结 JDK 8 `javax.transaction.xa` 逐项验证 `TMNOFLAGS/TMJOIN/TMRESUME/TMSTARTRSCAN/TMENDRSCAN/TMSUCCESS/TMFAIL/TMSUSPEND/TMONEPHASE/XA_OK/XA_RDONLY` 数值；用状态机和外部消费者测试覆盖 read-only 投票、一阶段提交、未知分支、重复/非法转换及完整 XA 错误码，并证明非法输入不改变目标或其他分支。命令、退出码和源码哈希见 [XA 协议证据](evidence/xa-standard-contract.md#xa-协议状态机-62)。此项只关闭协议状态机与标准值合同，不代表真实数据库 XA 已接通；完成后停止本批，不自动开始 6.3。（XA-02）
- [x] 6.3 补齐 recover 扫描、资源管理器身份和资源生命周期合同；先写能暴露缺陷的失败测试，再实现并验证以下边界，证据记录到 XA 协议证据：
  - 扫描覆盖 `TMSTARTRSCAN`、`TMNOFLAGS`、`TMENDRSCAN`、标准允许的一次性 `TMSTARTRSCAN | TMENDRSCAN`；非法 flags、无活动扫描时继续/结束均返回 XA 错误，且不改变分支状态或其他扫描。
  - 分页扫描使用稳定快照，仅返回 prepared/启发式完成的可恢复 Xid；验证完整 Xid、分页终止、无重复和遗漏，以及非 prepared 分支不进入恢复结果。
  - `isSameRM` 使用稳定 RM 身份凭证：共享凭证的不同连接句柄为 true；相同显示名称/连接属性但身份不同或未知为 false。身份 API 由外部最小消费者编译验证，默认实现保持既有外部 `XaResource` 实现的源兼容。
  - 所有资源操作在 owner 关闭后明确失败，不能触达已释放连接或改变分支；驱动拒绝超时设置时返回 false，读取值不得伪称新超时生效，并覆盖零值语义。
  - 本项只证明标准合同/示例资源的协议语义；示例、mock 和外部编译测试不能证明 JDBC Agent 或真实数据库恢复，后者只由 6.4 验收。（XA-03、XA-04）
- [x] 6.4 建立并验证真实 JDBC Agent XA 后端闭环：显式 XADataSource 配置、完整 XA RPC、逻辑句柄与 XA owner 分离，以及 PostgreSQL 跨 session prepare/recover/commit 已由真实协议集成测试验证；记录见 [XA Agent 协议证据](evidence/xa-agent-protocol.md) 和 [XA 后端恢复探测](evidence/xa-backend-recovery-probe.md)。（XA-01–04、MC-02）
  - Agent 必须通过明确配置的 `XADataSource` 创建 session，session 同时拥有 JDBC `XAConnection`、逻辑 `Connection` 和驱动 `XAResource`；普通 `DriverManager` 连接不得被包装成 XA。
  - Agent 协议无损传递完整 Xid 字节、flags、prepare 投票、recover 分页和 XA 错误码；XA 操作与逻辑连接 SQL 共用同一 session/资源管理器，关闭逻辑连接不能提前关闭 XA owner，关闭 owner 必须使所有句柄失效。
  - 增加 Java Agent 协议回归和真实 PostgreSQL XA 集成：start→SQL 写入→end→prepare，关闭原 XA session，再创建新 session 执行 recover→commit 或 rollback；最后使用普通 JDBC 会话读取最终数据库状态，并验证完整 Xid 与事务结果。
  - Agent 接入实现前的独立 JDBC 探测命令、H2 不支持结果、PostgreSQL 实际结果和源码哈希见 [XA 后端恢复探测](evidence/xa-backend-recovery-probe.md)。本任务只关闭 JDBC Agent 与 PostgreSQL 的协议闭环，不代表 RDBC `XaDataSource` 的 PostgreSQL 客户端适配器、Druid 事务管理器端到端集成或 6.5 消费者/文档验收已经完成。（XA-01–04、MC-02）
- [x] 6.5.1 为 Rust Agent 客户端增加 XA RPC 合同测试，覆盖 start/end/prepare/commit/rollback/forget/recover、超时、资源身份和结构化错误；普通 session 在发送 RPC 前精确 fail closed 为 `0A000`。捕获并断言既有普通 `open_session` 载荷，运行时握手实现未改动；测试命令、结果和源码身份见 [Rust XA 客户端证据](evidence/xa-rust-client.md)。假传输证据不关闭 6.5.2–6.5.5。（XA-05、MC-03）
- [x] 6.5.2 实现显式 XADataSource 类名/属性配置及标准 `XaDataSource → XaConnection → XaResource` 获取路径；证明普通连接载荷和握手不变、XA 配置不从 URL 推断，配置/反射/协商失败不会降级到普通连接。对每种 XA RPC 验证完整 Xid 三字段字节、flags、一阶段标志、prepare 投票和 XA 错误码往返；未声明或未协商能力在任何 RPC 发出前返回 SQLState `0A000`。使用外部标准消费者验证只依赖 `druid-rdbc` 即可获取并调用资源。实现、命令、TDD 失败与最终源码身份见 [Rust XA 客户端证据](evidence/xa-rust-client.md#652-显式数据源与标准资源入口)。（XA-01、XA-02、XA-05）
- [x] 6.5.3 闭合 XA owner、逻辑租约与标准池事件的生命周期；先以真实 Agent EOF 外部池测试复现非 `SqlException` 未发 owner 错误事件，再修复安全事件投影；外部池 discard、回调重入/panic、sticky 隔离及 Druid 原生池边界均有分层证据，见 [Rust XA 客户端证据](evidence/xa-rust-client.md#653-已完成xa-owner-事件与-sticky-故障隔离)：
  - **Session/租约层**：覆盖首次借出、逻辑关闭后 XA 继续有效、再次借出使旧代次失效、旧代次迟到 SQL/关闭请求不能影响新租约、owner 关闭使所有句柄失效、重复关闭幂等；注入请求中断、逻辑关闭失败、owner 关闭失败和不确定传输结果，验证失败状态 sticky、失败 owner 不可再次借出、主错误不被清理错误覆盖。
  - **标准事件层（PE-02）**：XA owner 通过标准 `PooledConnection` 的监听器注册表发出逻辑关闭与致命连接错误事件；断言事件源为实际稳定 owner、logical close 仅通知一次、SQL 异常完整保留；另注入 Agent EOF/不确定传输这类不含 `SqlException` 的 `RdbcError`，验证错误事件仍能驱动外部池 discard，调用方仍收到原始错误且没有凭据泄露。回调锁外执行，重入/失败不阻断处置。逻辑连接归还不能关闭 XA owner 或使 `XaResource` 失效。
  - **外部标准池与 Druid 边界层**：用一个只依赖 RDBC `XaDataSource`/`XaConnection`/`PooledConnection` 的最小外部池消费者验证借出、逻辑归还、再次借出、owner discard/close 的事件顺序与租约隔离；确认归还逻辑连接时 XA owner 未被提前关闭，致命/不确定状态 owner 不会重新入池。另检查 Druid 原生池入口的真实类型边界：若它只接收普通 `ConnectionFactory`/`Connection`，记录 XA owner 不经普通池代理的限制与示例用法，不伪称已由 Druid `DruidPooledConnection` 管理 XA；只有实际存在 XA 池代理时才要求该代理的正向生命周期回归。
  - 汇总三层命令、退出码、源码哈希和未运行项；环境可用，本项无 `NOT_RUN`。真实 PostgreSQL 跨 session 恢复仍由 6.5.4 独立验收，不能由本项 H2 生命周期证据替代。（XA-04、XA-05、PE-02、MC-02、MC-06）
- [x] 6.5.4 使用已通过 6.4 的 PostgreSQL Testcontainers 配置，从 Rust RDBC 消费入口完成 start→SQL 写入→end→prepare→关闭 session→新 session recover→commit/rollback，并以独立 JDBC 连接核对数据库最终状态；同时运行普通模式和明确不支持 XA 的负向测试。记录精确命令、版本、退出码和源码哈希；环境缺失记 `NOT_RUN`，本任务不得用 Agent Java 协议测试代替 Rust 端到端证据。（XA-03、XA-05、MC-02；证据见 [Rust XA 客户端证据](evidence/xa-rust-client.md#654-rust-rdbc-postgresql-跨-session-xa-恢复)）
- [x] 6.5.5（有限交付 C：XA 对外交付；完成后 XA 批停止）
  - [x] 6.5.5a 编写/更新中文集成指南：`jdbc-agent` feature 与 Agent/JDBC 驱动准备、XADataSource 显式类名和属性、标准入口获取、敏感配置脱敏，以及普通连接维持原配置的迁移说明；不得把 JDBC URL 或普通 Connection 描述为 XA 能力来源。（XA-05、XA-06）
  - [x] 6.5.5b 用 Mermaid 时序图展示事务管理器取得 owner/resource、逻辑连接执行 SQL、逻辑租约归还、prepare、关闭原 owner、新 session recover、核验完整 Xid 后 commit/rollback；标清逻辑 Connection 关闭不关闭 XA owner，owner 关闭后所有句柄失效。（XA-04–06）
  - [x] 6.5.5c 将文档调用片段落实为只依赖 `druid-rdbc` 公开接口的最小外部消费者编译/行为验证；区分 compile-only、合同测试和 PostgreSQL 实库证据，不将它们混称。（XA-01、XA-05–06、MC-03）
  - [x] 6.5.5d 审查当前分支的真实 API 差异：类型/方法、feature gate、外部实现源兼容性、行为变化、迁移要求和 SemVer 结论；标明 API 快照是否与当前源码同步。记录预发布版本依据和发布前版本策略，不虚构版本承诺。（XA-06、MC-03）
  - [x] 6.5.5e 运行 XA 目标测试及直接受影响回归；将 6.5.2–6.5.5 精确命令、feature、Agent/后端/驱动版本、退出码、源码哈希、消费者验证类型、API/SemVer 结论及 `NOT_RUN` 阻断写入 `evidence/xa-rust-client.md`。逐条验收 XA-01–06；严格 OpenSpec 校验通过后停止本批。（XA-01–06、MC-02–04）

## 7. 补齐公共数据源配置

- [x] 7.1 在 `druid-rdbc::CommonDataSource` 增加 `try_login_timeout() -> Result<Option<Duration>, RdbcError>`，保留并弃用旧 getter，默认适配须让只实现旧接口的外部实现保持源码兼容；明确 `None`/零值语义。用外部最小实现验证读/写错误载荷与时长最大值/精度，不允许错误落为默认值。当前标准层没有 Java 秒数转换调用点，已将此范围事实和后续适配约束记录；完整 RDBC 合同及 wrapper 消费者编译通过。证据见 [CD-01 登录超时合同](evidence/common-data-source-timeout.md)。（CD-01、MC-03）
- [x] 7.2 实现不绑定具体日志框架的 `RdbcLogWriter` 与父命名空间入口，并接入普通 `DruidRdbcDataSource`、池化 `DruidDataSource` 和 XA `JdbcAgentXaDataSource`；保留 `druid::sql::RdbcLogWriter` 兼容重导出。以外部消费者验证普通/ConnectionPoolDataSource/XA 三个标准合同均可访问同一 API；验证生产数据源实例初始为空、设置后可读取并写出、设置为空后禁用，以及不支持的父命名空间精确返回 SQLState `0A000`。隔离和凭据脱敏属于 7.3，不在本项提前关闭。证据见 [CD-02/03 日志入口证据](evidence/common-data-source-logging.md)。（CD-02、CD-03）
- [x] 7.3 注入连接/配置 SQL 故障、日志接收器 I/O 故障及两者同时发生；覆盖用户名、密码、token、URL user-info 和敏感查询属性。断言输出不含秘密值且保留安全诊断上下文；同时发生时原 SQLState/主错误载荷不被日志错误覆盖；日志写入失败不伪造连接成功、不回滚已应用配置。测试同时覆盖普通、池化及 XA owner 的诊断边界。证据见 [CD-02/04 安全诊断与故障隔离](evidence/common-data-source-diagnostics.md)。（CD-02、CD-04）
- [x] 7.4 更新普通、池化、XA 三类数据源配置示例与可失败 getter 迁移说明；示例由公开接口消费者测试编译/运行验证。发现并修复已配置池登录超时在标准读取路径丢失的问题；消费者验证 `DruidDataSource` 与 `DruidRdbcDataSource` 保留正值，零值映射为无限等待，负值返回 `InvalidArgument`，读取/配置错误不被静默转换；列明旧 getter 弃用、产品兼容 trait 与标准 trait 的区别、建池后配置不可变和 XA 厂商属性边界。证据见 [CD-01–04 配置迁移](evidence/common-data-source-migration.md)。（CD-01–04、MC-03）

## 8. 建立方法级覆盖与证据门禁

- [x] 8.1 新增 Java 8 Javadoc Doclet 与生成脚本，输入固定 OpenJDK 8u502 归档，按源码直接枚举 `java.sql`/`javax.sql` 公开类型和成员，记录构造器、常量、重载、default 与继承来源；fixture 验证事件监听器完整、重载不合并、显式/隐式构造器、父类/接口继承、非 API 私有成员排除和重复输出确定。完整清单、来源校验、退出码与 SHA-256 见 [Java API 清单证据](evidence/java8-api-method-inventory.md)。（MC-01）
- [x] 8.2 将各批已有证据关联到 Java 签名、Rust 入口与规范场景，分别保留声明/合同/适配器/逻辑代理/实库状态；验证启用能力缺正向证据和精确不支持证据时门禁确实失败。完整台账覆盖 2,001 个唯一 Java API 身份，9 项已有批次证据有条件地标为 PARTIAL，其余保持 UNASSESSED；门禁负向控制与目标批次测试均通过。证据见 [方法证据台账](evidence/method-evidence-ledger.md)。（MC-02）
- [x] 8.3 增加源码哈希及输入版本失效检查，运行故意过期证据的负向验证；明确 NOT_RUN/失败/通过区别并更新证据使用说明。清单版本、Rust 源码和测试源码漂移均会拒绝过期台账；不保存二进制或大型归档。证据见 [证据新鲜度门禁](evidence/method-evidence-freshness.md)。（MC-02、MC-03）
- [x] 8.4 生成本次六类缺口与全局其余缺口分开的报告，逐项核验七份规范 96 个场景均关联任务与现存证据；范围外问题记录位置、影响和证据。生成器只读核验任务清单，测试证明未自动新增任务，也未宣称全 JDBC 完成。证据见 [范围/场景报告](evidence/scope-and-scenario-report.md)。（MC-01–04）
- [x] 8.5 验收 `docs/druid-rust-Architecture.zh_CN.md` 第 4.1 节的 Mermaid 架构图与本变更架构决策及能力矩阵一致。保留五个主节点与合同分组；核对源图和 GitHub Markdown Mermaid 渲染可读，正文分清模块依赖箭头、标准对象链/接口特化、owner/租约回收与后端 I/O 职责，并明确图示不等于后端能力支持。行号、渲染方式和源码 SHA-256 见 [架构图边界审查](evidence/architecture-boundary-review.md)。（MC-05）
- [x] 8.6 对 Array、LOB、SQLXML、SQLData、RowSet、流式结果集和缓冲结果集建立有限方法级清单；核对资源 capability 与实际方法、关闭/释放错误、owner/游标归属及精确不支持结果。每一项归类为合同通过、具体后端通过、精确不支持、`NOT_RUN` 或缺陷；不以类型声明、CodeGraph 符号数或旧测试总数代替行为证据。有限方法矩阵、状态边界及目标合同验证见 [资源与结果集方法矩阵](evidence/resource-resultset-method-matrix.md)。（MC-01、MC-02、MC-06）
- [x] 8.7 为清单中的高风险场景补充或复用最小合同/适配器测试：重复/失败释放、结果集耗尽与显式关闭、owner 关闭时游标清理、关闭错误保真、`known_row_count` 不消费流，以及未声明能力返回精确错误。补充流式计数零消费断言，并使 Array 能力拒绝断言精确 SQLState、Java 重载身份和 driver 零调用；生命周期复用测试及目标运行结果见 [资源与结果集生命周期验收](evidence/resource-resultset-lifecycle.md)。（MC-02、MC-06）
- [x] 8.8 将资源和结果集能力状态、测试命令、后端/版本及源码身份写入证据台账，并与主架构图的职责边界核对；真实数据库资源专项保留 `NOT_RUN`。命令结果、Rust/SQLx/Toasty 版本、源码 SHA-256 和架构责任边界见 [资源与结果集证据台账](evidence/resource-resultset-evidence-ledger.md)；本批完成后停止，不顺延新增任务。（MC-04–06）

## 9. 综合验收与交付关闭

- [ ] 9.1 在最终源码状态运行一次约定的 RDBC 完整合同回归及标准纯度门禁，连同 druid/wrapper 受影响 feature 集成检查保存命令、退出码和源码身份；任何未执行门禁保持未完成。（MC-02、MC-04）
- [ ] 9.2 汇总并审阅公共 API/SemVer 差异与各批迁移记录，运行外部驱动、连接池、事务管理器最小消费验证；检查无盲目刷新快照、扩大 allowlist 或未处置的破坏性差异。（MC-03）
- [ ] 9.3 复核 Java 8 时间差分、真实取消与 XA 恢复记录对应最终受影响源码，检查示例、中文主架构图和能力矩阵一致；只对失效的必要证据重跑，不重复无关全仓覆盖。（MC-02–05）
- [ ] 9.4 交付实际完成/未完成/环境阻断清单；仅在所有本变更场景和任务通过后执行 OpenSpec 等价 verify、增量规格同步与归档，并验证主规格保存七项能力。明确本次关闭不代表 P04–P07、TCK 或全项目生产就绪，随后停止。（MC-04）

以下为后续综合验收的候选命令，本轮规范编写未重新执行它们；准备批需确认参数、feature 和现有门禁入口：

```powershell
cargo test -p druid-rdbc --locked --offline --quiet
python scripts/verify_rdbc_single_standard_purity.py --phase final
openspec validate close-rdbc-jdbc42-contract-gaps --strict --no-interactive
```

`openspec validate` 只验证规范结构，不能替代 Rust、Java、数据库和 API 的实际行为证据。
