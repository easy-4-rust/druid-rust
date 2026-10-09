# Task 6 完整连接状态与外部租约恢复

本报告补齐 2026-10-06 的实际终态证据。任务仍为 `FIX_REQUIRED`；P04 Tasks 6–8 与 P05–P07 共 18 个顶层任务保持开放。

## 固定基线与正式绑定

- Rust 原始测试基线：`5afe8609b177544ff7d18461aa84f69efe8ee90e`。
- 1,283 行迁移账本前七个原始字段全部保持固定；当前 1,110 行 `MIGRATED_EXACT`、173 行参数化映射。
- 完整 ConnectionDefaults 与外部租约原始案例已绑定至实际 Cargo 所属的私有单元测试模块，旧的两案例占位租约矩阵已退役。
- `task6-complete-lease-source-floor-audit-2026-10-06.json`：1,247 PASS / 36 FAIL；剩余连接回收 12、ResultSet Filter 24。源码观测检查不证明运行行为等价或整个项目完成率。

## 真实运行证据

日志位于 `.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/`。

| 日志 | 终态 | 范围 |
| --- | --- | --- |
| `task6-connection-state-workspace-final.log` | exit 101，250 个结果块，3,759 PASS / 1 FAIL / 0 ignored | 完整 workspace，包含两个 doctest；唯一失败为 holder 默认状态案例 |
| `task6-holder-original-state-and-lib-regression.log` | exit 0，111 PASS / 0 FAIL | Druid lib 105，加完整 holder 目标 6 |
| `task6-full-lease-wrapper-lib-runtime.log` | exit 0，10 PASS / 0 FAIL | wrapper lib，包含完整租约委托案例 |
| `task6-wrapper-lib-target-routing-red.log` | 15 PASS / 2 FAIL | 验证器包路由缺陷的真实 RED |
| `task6-wrapper-lib-target-routing-green.log` | 17 PASS / 0 FAIL | 修正严格包路由后的 GREEN |

111 与 10 的范围互不重叠，共 121 PASS。完整 workspace 日志早于 holder 修复、完整租约测试接入和占位矩阵退役，不能据局部结果宣布当前全仓通过。

Holder 原始夹具初始 holdability=1、脏值=9、默认值=1；恢复严格五事件：read_only:false、holdability:1、isolation:2、auto_commit:true、clear_warnings。保留真实状态捕获，没有改回硬编码零，也没有删减原断言。

完整租约测试保留原 15 assert / 30 assert_eq，当前 16 / 31；覆盖三对象实际 heap 身份、十入口计数、完整行值、生成键、事务/连接状态、警告/savepoint/LOB 与租约 Drop 一次。discard 管理位在实际 wrapper 私有范围观察，无新增公共 discard 接口。

首次租约编译失败仅为 ColumnIndex import 错误，零测试执行，不计行为 RED。回收完整案例首次编译也仅因 ExecResult import 路径错误，零测试执行，不计行为 RED。

## 验证器修正与派生证据同步

Ruling: 私有 lib 测试必须按真实源码归属路由至所属 Cargo package。原验证器将所有注册 lib 目标路由至 druid，无法真实执行 wrapper 测试。修正仅允许已注册且位于 druid 或 druid-wrapper 明确路径的目标，未知包继续拒绝。两项回归观察 RED→GREEN；未放宽固定基线、断言检查或案例身份规则。

2026-10-07 按正式逐函数账本运行 `render_task6_migration_document.py --write`，仅同步派生 Markdown 表。`verify_task6_migration_authority.py` PASS；79 行权威表、25 行 Task 7 pending、73 行 Physical 标准化表保持不变。权威 SHA-256：`7210995f5b1e9ac4b50ec56693cc4a5f95905b0aee923f4048a3d2819d7620c3`。权威验证器五项回归 PASS。

## 尚未关闭

当前回收完整十二案例正在验证，尚未正式绑定。Task 7 八类资源义务保持独立；API/SemVer/外部消费者、MSRV、features/all-features、真实数据库、全仓新鲜回归与最终审查均未关闭。公开隔离级别类型修正及 DriverValue 的兼容影响仍需正式门禁。

本轮使用 stable Rust 1.99.0 的相关运行结果，不等于规格要求的 MSRV 1.95 或默认工具链 1.97.1 验证。

2026-10-07 完整回收真实RED：15872终态exit101/8PASS4FAIL。失败全名后缀mysql_family_db_type_enables_java_init_schema_restore_policy（夹具缺create_statement）、physical_connection_defaults_are_initialized_in_java_order（实际隔离2vs原8）、rollback_failure_is_swallowed_counted_and_connection_discarded（关闭返回时raw closed仍false）、test_on_return_validates_and_invalid_connection_is_not_recycled（校验计数1vs原2）。固定BASE完整12 sourcefloor独立PASS，未正式绑定。Ruling：恢复原initConnection隔离级别在readOnly与catalog之间的应用，以及非空显式catalog无getter前置门禁；fixedBASE原实现确有两项行为，真实完整RED支持修复，无改断言。生产pool_inner.rs已修复，83376 Druid lib+holder+fullRecycle验证LIVE，输入冻结。其余3项须区分夹具statement适配、native checker绕过和显式关闭完成顺序，不得降低原断言。全部18任务仍OPEN。

2026-10-07 初始化修复终态：83376 exit101，Druid lib105/0、holder6/0、fullRecycle9PASS3FAIL；原完整初始化隔离8与catalog及四事件已GREEN。剩余fullRecycle失败仍为mysql初始化create_statement、rollback失败关闭完成、test_on_return计数1vs2。native归还校验修正已接入私有PoolInner引用，关闭时复用validate_connection（checker/query/timeout），外部或手工holder仍按其驱动is_valid合同；无新增公共API。31171相关lib+holder+fullRecycle新回归LIVE，Rust/Cargo输入冻结。正式1110/173与1247/36未改、12候选floorPASS但未绑定、18任务仍OPEN。

2026-10-07 native checker与MySQL夹具终态：31171 exit101，Druid lib105PASS0FAIL、holder6PASS0FAIL、完整Recycle10PASS2FAIL。原test_on_return首次创建+归还精确2及失败后3校验、recycle/discard/destroy计数全部GREEN。新增单对象TrackingValidationStatement仅用于测试，复用原TrackingConnection fetch/exec和真实BufferedRowSet导航/close；不伪造校验计数、不修改原两个数据库policy断言。随后完整Recycle statement regression终态exit101/11PASS1FAIL，MySQL与Postgres schema policy原完整案例GREEN，剩余唯一rollback_failure_is_swallowed_counted_and_connection_discarded在原raw.closed断言失败。未删除/等待后重解释该断言，后续必须修复显式close的受监管物理关闭完成协议，并保护取消、计数、驱动恰好一次关闭。无本轮Cargo进程LIVE。正式12案例仍未绑定，源观测formal1247/36及1110/173保持，18任务OPEN。
