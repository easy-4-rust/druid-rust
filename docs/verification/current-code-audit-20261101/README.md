# 当前代码迁移核对（2026-11-01）

后续 Admin 设计见 [浅色监控台评审包](../../functional-design/druid-admin-light-console-20261101/README.md)：基于 `ed161654` 复核真实数据链路，记录剩余工作和可点击原型；本轮 Admin 24 项测试与 scoped 覆盖率独立记录，不改写下方普查历史结果。

后续批次见 [ExceptionSorter 逐方法核验](../method-semantics-20261101/README.md)：补充 30 个显式签名台账、Java 原方法执行与 118 条边界差分，仅修真实配置解析偏差。后续结果不改写下方本次普查的历史基线和统计。

本报告纠正“旧文档等于当前完成状态”的判断。**已有大量真实实现，确有文档滞后；但当前证据仍不足以声明 Java Druid 全量语义迁移完成。** 以下按能力与具体行为分类，不按同名文件数量计算完成率。

## 基线与范围

- Java：`fa8dc9912637a2f729eef9f55356621fec18d40e`。
- Rust：`7b39d0c04d6829493cacb5de125d7e6de50ef5fe` 加工作区修改。并行工作已提交 Admin slow SQL 页面，本次保留该成果，没有重做前端。
- 收尾期间又出现并行 Admin 更新：新增 `src/servlet/monitor_view_servlet/sql_parse.rs`，以及 Admin Cargo.toml / Cargo.lock 修改。它们不在前述测试快照内；[收尾漂移记录](end-of-audit-drift.json) 显示原 922 个产品源码文件哈希未变。后续应对这批新增内容另做增量验收，不能将本轮通过结果套用到它们。本次未编辑或回退这些文件。
- 规格事实源：已有 `openspec/specs/` 与对应 change；架构裁决为 [ADR-CRATE-003](../../migration/adr/ADR-CRATE-003.md)。Superpowers 是执行记录，不覆盖批准规格。
- 当前阶段：Brownfield 验证与文档纠偏；本轮不重建拓扑，不改变公共 API。
- CodeGraph 用于两仓库定位及调用关系核对；落盘源码、测试日志与哈希为快照依据。
- 文件普查覆盖 Java `core`、`druid-admin`、`druid-wrapper` 的全部常规源码根，Rust 五个产品 crate 的源码根和当前 crate 的集成测试根。**普查覆盖不等于逐方法人工审计完成**；未逐篇读完全部历史 docs，未逐对象重跑 Java 全量测试。

| 普查对象 | 数量 | 解释 |
|---|---:|---|
| Java core 源文件 | 1684 | 含 package-info；不是对象/嵌套类分母 |
| Java admin / wrapper 源文件 | 13 / 13 | wrapper 包含 DBCP、DBCP2、c3p0、Proxool，不局限于 com.alibaba 包 |
| Rust 产品源码文件 | 922 | 含源码内单测；不含 vendor 的依赖实现 |
| Rust 集成测试源码文件 | 440 | 含 common/include 支持文件；不是可执行用例数 |
| Java 常规测试源码 / 资源 | 3558 / 823 | 未展开参数化/动态用例，不代表资源已逐一迁移 |
| 同名声明候选 / 引用候选 / 未定位映射 | 163 / 104 / 1443 | 三者都不是完成或缺失统计 |

逐文件路径与 SHA-256 见 [inventory](inventory/baseline.json)；旧 core 对象分母 1875 包含嵌套对象，与本轮 1710 个三模块源文件不可相减或计算趋势。

## 分类规则

1. **已实现但文档滞后**：有当前调用或运行证据反驳旧文档的具体断言。只更正文档，不重建功能。
2. **已实现但缺验证**：有真实实现；现有测试范围、运行环境或 Java 对照不足以证明全部约定。保留实现，补原有验证链。
3. **确实缺失/偏差**：已确定的预期行为或架构要求与当前代码矛盾；记录最小缺口、处理及剩余项。

另设“**待核实**”工作队列：不能确定映射或语义的条目不强行塞进上述三类。默认 `FeatureNotSupported` 不自动算 stub，必须先核对驱动能力和标准的负能力合同。一个能力可以有不同层次的条目：如接口已存在但崩溃恢复未验收。

## 一、已实现但文档滞后

| ID | 已核实事实 | 当前证据 | 文档处置 |
|---|---|---|---|
| D01 | ADR-003 已批准，当前 workspace 为五个产品 crate + bench + test，共七成员 | `Cargo.toml`；ADR-003 ACCEPTED | 撤回“待批准”和“六成员就是当前清单”的描述；不再次征求架构选择 |
| D02 | 淘汰策略已下沉并被池调用；Wall 的十个模块及 hashing 已在 rdbc | `crates/druid-rdbc/src/governance/`；`crates/druid/src/pool/pool_inner/recycle.rs`；Wall 值对象回归 | P3.1/P3.2/P3.3a 不再整体列为待实现；不据此宣布 P3 完成 |
| D03 | Admin 有独立应用、Vue/Vite 静态前端和版本化 HTTP API | `crates/druid-admin/src/druid_admin_application.rs`、`web/src/`、`build.rs`；[admin.log](admin.log)，22 通过 | 不再按旧 Servlet 页面规划重复建设；部署/发现语义另列验证 |
| D04 | sqlx、rbdc、sqlx-bb8/deadpool 等整合已归 wrapper 的模块与 feature | `crates/druid-wrapper/Cargo.toml`、`src/sqlx/`、`src/rbdc/`；源码普查 | 独立旧适配 crate 名称不是待创建目标；需要按当前 feature 验证 |
| D05 | Oracle RETURNING INTO、Hive TABLESAMPLE、Doris UNIQUE KEY 已可解析 | `crates/druid-test/tests/sql_dialect_contract_test.rs`；[acceptance.log](acceptance.log)，该目标 6 通过 | 旧方言表的 ERR 是历史观测，不能重新实现这些语法；Java 输出一致性仍需重新观测 |
| D06 | 旧“10 项未批准差异”已被后续具名差分处置记录取代 | [approved-differences](../rdbc-first/approved-differences-20261021/README.md)：历史 636 条观测，620 MATCH、16 approved | 保留原始失败记录；不冒充本轮重新执行，也不升级为全量 JDBC 合规 |
| D07 | Admin 13 个 Java 源文件均有声明候选；Wrapper 13 个也均有候选 | [source-file-candidates.csv](inventory/source-file-candidates.csv) | 补齐旧 core-only 台账漏掉的两模块入口；声明候选仅是定位信息 |

## 二、已实现但缺验证

| ID | 能力与实现位置 | 本轮/历史证据 | 尚不能宣称的内容与下一步 |
|---|---|---|---|
| V01 | Pool 生命周期、缓存、异常分类、Filter 与 Stat：`crates/druid/src/{pool,core,stats}` | 本轮 druid-test 池契约；前轮 Filter 红绿测试；历史 pool/prepared_cache/statement/result_set 差分 | 全方法和并发路径等价。把 Java 用例身份关联到当前测试/fixture，不用目录或测试名字代替断言复核 |
| V02 | SQLUtils / AST / Wall：`crates/druid/src/sql`、本地 sqlparser patch | 方言最小契约 6 例；dialect_wall_matrix 21 例；Wall 值对象 13 例 | sqlparser 存在不等于 Java SQL 全覆盖。逐方言恢复新观测，严格区分往返成功、参数化一致与安全拦截一致 |
| V03 | HA / Dynamic：`crates/druid/src/dynamic` | dynamic_batch2_ha_pool 43 例通过 | 未证实所有故障、恢复、ZK 宿主行为；复用已有节点/选择器测试和宿主 runner |
| V04 | RDBC Connection、Statement、ResultSet、元数据、LOB、Wrapper 合同 | 当前接口/适配实现；既有 jdbc42-semantic-inventory.tsv 的 69 行 DECLARED；本轮 Wrapper 身份 3 例和标准 XA 4 例 | 69 行是标准对象声明，非方法分母或行为验收；不能用 trait 默认方法、编译成功或生成测试规模宣布 JDBC 全方法完成。复用现有 verify_jdbc42_* 门禁，不另造标准台账 |
| V05 | XA 标准、分支状态机、JDBC Agent XA 资源 | `druid-rdbc/src/data_source/xa_resource.rs` 含 recover；`druid/src/core/xa*` 55 例通过；wrapper 资源/RPC 实现 | XA 已实现，不能重做；资源恢复 API 不等于持久化协调器的崩溃恢复，见 G03 |
| V06 | 驱动目录、工厂、分发、能力协商和安装 | `druid-wrapper/src/driver/{druid_driver_registry,driver_verification_run,druid_database_pool_builder}.rs`；各真实适配目录 | profile/产品名不等于 70+ 数据库验收。逐产品、版本、feature、服务环境留存运行证据；不能把兼容协议一概标作完整支持 |
| V07 | Metrics 有界队列、采样/聚合/发送、监督、序列与 ACK | `druid-metrics/src/{runtime,supervisor,pipeline_ownership,push_worker}.rs`；协议/注册 23 例，队列/监督 7 例通过 | 未证明负载下 SQL p99 无影响、跨进程重连与完整故障矩阵。保留现有管线，补负载和故障验证，不创建第二协议 |
| V08 | Admin 发现、HTTP 代理、DTO、鉴权、监控聚合 | 同名 13 对象候选、实际服务；本轮路由/API/应用 22 例 | 路由通过不是 K8s/鉴权/部署/UI 可用性全验收；本轮没有浏览器截图或真实集群测试 |
| V09 | Java DBCP/DBCP2/c3p0/Proxool 兼容入口 | `druid-wrapper/src/{dbcp,dbcp2,c3p0,proxool}`；例如 c3p0 空派生类通过 canonical 数据源别名表达，managed 工厂委托真实工厂 | 不能把别名一律算完成或一律算 stub；逐对象检查 Java 自有/继承方法、配置映射、生命周期及源测试。JDBC Wrapper 身份测试不证明 DBCP 语义 |
| V10 | 全量源用例、资源和技能验收 | 本轮测试源/资源哈希清单；已有 druid-bench oracle 与 druid-test | 尚无本轮全分母逐例处置闭环；未做全 feature、MSRV、API/SemVer、全量覆盖率和回滚验收。局部覆盖率不授予全仓通过 |

## 三、确实缺失或偏差

| ID | 可复核差距 | 处理 | 状态 |
|---|---|---|---|
| G01 | Java FilterManager.existsFilter 用 String.equals；Rust 原来忽略大小写，合并两个不同注册类 | [已修复记录](../migration-audit-20261101/continuation.md)，新增双工厂测试先失败后通过，复用原链条 | 代码修复、10 例回归和 scoped llvm-cov 通过；严格 Clippy 未通过，change 未归档 |
| G02 | ADR-003 的最终门面目标未达到：druid 的 pool/core/sql/stats 仍包含真实运行逻辑；Filter 参数仍耦合 DruidPooledConnection | 保留已有功能；按 P3.3b/P3.4 逐边界抽取，保持兼容重导出及单一实现 | **未完成的架构迁移**，不是这些业务功能缺失；本轮未搬动运行时 |
| G04 | 旧生成器将名称未匹配写成 MISSING/待实现，且覆盖整份文档；完成数不是语义评估结果 | 新普查仅输出候选；旧 renderer 遇到本次审阅标记在任何写入前拒绝覆盖。回归测试 RED→GREEN | 本轮已修复审计工具误导/覆盖问题；保留旧数据作为历史，不重写原始证据 |

## 待核实的扩展项 G03：持久化 2PC 协调器

批准方向中的持久化 2PC 协调器尚未在当前实现中定位到。已核对的 `XaTransactionState` 使用进程内 `Instant/history`，`XaResource` 提供资源侧 recover；这些不能充当 durable coordinator。需要在既有 ADR 下明确日志持久化、逐参与者决策、未知结果、Mixed 禁止探测提交与崩溃窗口，再完成实现定位和验证。

这是待闭合的能力定位/规格问题，**没有纳入已确认缺失功能计数**，也不是 Java Druid 缺失对象。不能以关键词未命中作全仓不存在证明，更不能以审计名义仓促加入一套不完整的事务协调器。

## 未核实映射的处理

1443 个未定位名称映射主要不能通过简名直接关联；没有逐条断言其实现状况。新清单所有行均为 `REVIEW_REQUIRED`，它只提供源文件分母与候选入口。能力表是人工核对的结论；必须继续补充到嵌套对象/方法级，才可升级对象状态。

下一批按风险复核：公共 RDBC 方法与真实驱动负能力 → SQL/Wall 输入及异常 → 池/Filter 生命周期 → 管理面与兼容配置。每项先查已有实现、已有测试与 approved difference，再决定补测试或补代码。Java 平台专有项不能仅凭名称自动 PLATFORM_NA。

## 本轮命令与运行证据

全部使用现有依赖、`--offline`；测试临时文件在 E:，后续方言构建及临时文件转至 D:，无依赖安装、分支切换或提交。以下为独立范围，不能相加推导覆盖率。

| 命令（省略统一的 cargo test --offline） | 结果 | 原始记录 |
|---|---|---|
| `-p druid-test --tests` | 9 passed，0 failed/ignored | [acceptance.log](acceptance.log) |
| `-p druid-metrics --test protocol_roundtrip_test --test registration_lifecycle_test` | 23 passed，0 failed/ignored | [metrics.log](metrics.log) |
| `-p druid-admin --test admin_console_routes_test --test admin_v1_api_test --test admin_console_app_test` | 22 passed，0 failed/ignored | [admin.log](admin.log) |
| `-p druid --test xa_state_machine_test --test task6_wrapper_semantics_test_parity --test dynamic_batch2_ha_pool_test --test dialect_wall_matrix_test --test wall_table_stat_value_test --test wall_sql_stat_value_test` | 135 passed，0 failed/ignored | [core-domains.log](core-domains.log) |
| `-p druid-rdbc --test xa_standard_external_consumer` | 4 passed，0 failed/ignored | [rdbc-xa.log](rdbc-xa.log) |
| `-p druid-metrics --test queue_semantics_test --test task_supervision_test` | 7 passed，0 failed/ignored | [metrics-backpressure.log](metrics-backpressure.log) |
| `-p druid-wrapper --features jdbc-agent --test jdbc_agent_xa_external_consumer --test task6_driver_registry_test_parity` | exit 101，未执行测试，遗漏测试要求的 sqlx feature | [wrapper.log](wrapper.log) |
| `-p druid-wrapper --features jdbc-agent,sqlx --test jdbc_agent_xa_external_consumer --test task6_driver_registry_test_parity` | exit 0，19 passed、0 failed、3 ignored | [wrapper-correct-features.log](wrapper-correct-features.log) |

Wrapper 重跑仍有三项真实 XA 宿主测试被声明为 ignored，要求 JDBC Agent/驱动 jar、Java 和 PostgreSQL Testcontainers。它们属于未验证，不计通过；17 项驱动注册测试含真实 SQLite 路由，但不证明 PostgreSQL XA 恢复。本轮上述成功运行合计 219 passed、3 ignored；是实际执行次数，不是 Java 对等用例分母。首次缺 feature 属于命令配置错误，不是实现失败。

本轮没有修改业务 Rust 实现。沿用同会话 G01 的 `cargo llvm-cov` 实跑：FilterManager 行 98.21%、函数 100%、区域 97.34%；该命令 TOTAL 行 16.34%。这是两个 Filter 测试目标的覆盖率，**不能宣称满足整仓覆盖率门禁**。见 [原始 coverage](../migration-audit-20261101/filter-coverage.log)。严格 Clippy 被既存 `druid-rdbc/src/extensions/shared_row_set/trait_impls.rs` wildcard import 阻断；本轮不隐藏 lint、不修改该模块。

### 32 例方言重新观测

`cargo run --offline -p druid-bench --example dialect_fidelity_matrix` 首次因 E: 磁盘不足构建失败，exit 101；[原日志](dialect-run.log)。改用 `CARGO_TARGET_DIR=D:/codex-build/druid-audit-20261101`、D: 临时目录、`CARGO_PROFILE_DEV_DEBUG=0`、`CARGO_INCREMENTAL=0`，执行 `cargo run --offline -p druid-bench --no-default-features --example dialect_fidelity_matrix` 成功，exit 0，见 [D: 重跑日志](dialect-run-d-drive.log)。此观测器只调用 druid SQL/Wall，不需要 bench 的可选 wrapper feature。

结果：32 例中 **31 parse ok、31 reparse ok**。逐条确认 SQL 输入与旧 Rust 观测相同；变化的恰好是 Oracle RETURNING INTO、Hive TABLESAMPLE、Doris CREATE UNIQUE KEY 三例，parse/reparse 由失败变为成功，默认 Wall 违规数由 1 变为 0。原始 [TSV](dialect-current.tsv) 与 [差异摘要](dialect-current-summary.json) 已留存，未覆盖历史 oracle。

唯一 parse 失败为 `doris-distribute-buckets` 的 `SELECT * FROM t DISTRIBUTE BY HASH(k1) BUCKETS 10`。旧 `java-golden.tsv` 的相同输入也为 parse err/reparse n/a，且 Java parameterized 返回错误而 Rust 保留参数化字符串。该行不能直接判为“Rust 缺 BUCKETS 语法”；须先核对有效语法和参数化错误合同。CREATE TABLE 内的 BUCKETS 与此 SELECT 输入不可混为一谈。

本次是 **V1 Rust 新观测**；没有重跑 Java，也未重新认证旧 golden 的来源哈希或执行全字段 Java 差分，因此未修改旧 30/32、24/32 的历史匹配数，更未把 31/32 当作语义完成率。

### 工具与证据自检

- 候选分类 4 项单测通过；renderer 保护 1 项先失败后通过。直接运行旧 renderer 返回预期 exit 1，未覆盖四份审阅文档。
- `git diff --check` 通过；未执行/宣称全仓格式、Clippy、所有 feature 或 MSRV 门禁通过。
- [snapshot-validation.json](snapshot-validation.json)：核验 7453 个已普查的源码、测试及资源文件哈希，无变化；该结论只覆盖清单，不包含 vendor、前端及其他未普查目录。Rust HEAD 未改变，Java 仅存在 CodeGraph 未跟踪目录。
- E: 后来耗尽空间；仅将本轮失败编译的 `libdruid-ab330590066fcd7e.rmeta` 和本轮 Wrapper 两个测试的 `.pdb` 移至 `D:/codex-build/druid-audit-20261101-temp` 保留，释放约 350 MiB。未删除共享 target、用户代码或历史证据。D: 原始方言输出在 `D:/codex-build/druid-audit-20261101-evidence`，已复制回本报告目录。

## 文档更新方式

四份迁移主文档的顶部为本次核对摘要；原自动生成区域保留在“历史快照”之后，不再代表当前状态。新脚本只生成独立候选清单，禁止从名称信号自动填三类结论。源码 hash 用于判定证据何时失效；dirty 工作区意味着单独记录 HEAD 不够。

完成的是本轮全模块文件普查、重点行为核对、实跑与文档纠偏。尚未完成逐方法全量语义验收；不提供没有分母与判定依据的“基本完成百分比”。
