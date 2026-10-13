# 单一 RDBC 标准统一验收报告

核验日期：2026-10-08。**整体结论：INCOMPLETE；Task6 FIX_REQUIRED，Task7/Task8 未完成。** 本报告是正式 Task8 指定的证据交付物，随真实终态更新，不构成新规格或完成批准。所有未完成义务继续保留。

## 1. 权威要求与证据范围

- 规格：[ADR-RDBC-006](../../superpowers/specs/2026-08-24-rdbc-single-standard-trait-tree.md)、[ADR-CRATE-002](../../superpowers/specs/2026-08-21-rdbc-first-five-crate-architecture.md)。
- 计划：[P04](../../superpowers/plans/2026-08-24-rdbc-single-standard-unification.md)、[主路线](../../superpowers/plans/2026-08-21-rdbc-first-master-roadmap.md)。
- 工作树 main，HEAD `f50787031ce64fbc85ebdbe83ab8670a4da05b7b`；大量未提交修改保留，不能仅用 HEAD 表示测试输入。
- Java oracle：Druid1.2.28 / `33824c3dec1612711f9bb4e409319bcab2e4cd0e`；Task6 原行为 BASE：`5afe8609b177544ff7d18461aa84f69efe8ee90e`。
- 方法分母479，删除测试身份分母1283。局部目标的通过数不得相加为全仓通过或扩大为全部数据库认证。

## 2. 最新真实终态

| 门禁 | 结论与范围 | 权威证据 |
| --- | --- | --- |
| DriverAction后最新源码门禁 | PROVED限组件：default normal依赖边界exit0；727生产源码文件Physical/SPI表面及重复trait树检查exit0；final purity exit0，STANDARD_FILES=50。未执行完整no-Physical CLI或认证public API；1099源码+58控制输入变化0 | [机器证据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-driver-action-source-gates1.summary.json) |

| 最新全workspace文档生成 | PROVED：`cargo doc --workspace --all-features --no-deps --locked --offline` 实际exit0，零rustdoc警告；三处文档引用修复后1092源码+58控制输入终态变化0。初次exit0但有三条警告的原日志保留；不构成runtime/MSRV/API或整项通过 | [最新机器证据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-workspace-all-features-doc3.summary.json) |
| 最新完整C1实际运行 | PROVED：原session20187实际exit0/executed=yes，479映射/442选择器，7目标/442函数；完整C2子验收保留，67删除文件/1283原测试身份分母不变。1092源码+58控制输入终态变化0；后续LF自测修复只改一个控制测试脚本，不扩大旧冻结证据 | [原运行终态](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-remaining33-complete-c1-runtime1.summary.json) |
| semantic夹具LF修复后的自测 | PROVED：新增跨平台LF字节回归先RED（两subtest失败），14处write_text显式newline，最终完整自测exit0/19PASS。生产验证器/语义账本/Rust源码未修改；不构成final语义或整体质量通过 | [GREEN证据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-semantic-fixture-lf-green.summary.json) |
| semantic验证器现有自测基线 | CONTRADICTED：实际exit1/18项，14 reported OK/4 ERROR；四个正向控制因Windows夹具CRLF字节被canonical oracle校验拒绝。实际字节诊断确认此差异；负向测试的reported OK不能证明它们到达目标拒绝分支。需修正测试fixture确定LF写入，保持生产严格校验 | [原自测](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-semantic-existing-regression-baseline.summary.json)、[字节诊断](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-semantic-windows-fixture-newline-diagnostic.summary.json) |
| 最新正式workspace Clippy | PROVED：原16070实际exit0/3m41s，all-targets/all-features/locked/offline/-D warnings；1104源码+58控制输入终态变化0，不构成全workspace运行/API/SemVer/final通过 | [最新机器终态](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-standard-constant-exception-regression1.summary.json) |
| ClientInfoStatus canonical修复 | PROVED限对象/回归：RED E0432后两身份测试2/0、完整RDBC all-features474/0，历史路径重导出唯一标准；workspace fmt及严格全workspace Clippy原96459实际exit0；原失败外部探针以新rmeta复验exit0。未修改计划态或关闭final门禁 | [修复证据](task8-client-info-status-recovery-2026-10-08.md) |
| 最新canonical外部编译核验 | CONTRADICTED：当前Cargo JSON产物支持原68探针全量复编译，47PASS/21FAIL，另1项非Rust目标未认证；Types模块与规范授权枚举项通过。原探针SHA未改，1104源码+58控制输入变化0；不认证完整Java行为或精确形态 | [最新机器探针](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-canonical-identity-probes6.summary.json) |
| final来源绑定组件 | PROVED限组件：完整自测35/0，从52+17 oracle与Task3接受映射重建原冻结摘要；拒绝来源身份/分母/摘要漂移，尚未接final CLI，Rust形态和运行证据未认证 | [TDD机器证据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-final-source-binding.summary.json) |
| final状态策略组件 | PROVED限组件：完整自测28/0，真实计划态账本被拒绝；未接final CLI，未替代冻结来源、canonical形态或compiler/runtime门禁 | [TDD机器证据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-final-state-policy.summary.json) |
| Task8 final语义入口 | CONTRADICTED：计划原命令 `verify_jdbc42_semantic_inventory.py --require-final-evidence` 实际exit2，argparse拒绝未实现的参数；未进入语义验收，不能改用planned/logical-live缩小合同代替 | [原命令证据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-final-source-gate-preflight.summary.json) |
| 最新final standard purity | PROVED：当前final源码检查实际exit0，STANDARD_FILES=51，1104源码+58控制输入变化0；仅限脚本覆盖的源码范围，不替代public API/compiler/runtime | [当前机器证据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-standard-constant-exception-purity1.summary.json) |
| 完整 C1 runtime3 | PROVED：exit0、executed=yes；479映射、442唯一选择器；完整C2子验证未跳过 | [机器终态](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-complete-c1-runtime3-2026-10-08.summary.json) |
| 完整 C2 runtime5 | PROVED：exit0、executed=yes；67文件/1283原测试身份，1146EXACT/137PARAM，无批准移除 | [机器终态](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-complete-c2-runtime5-2026-10-08.summary.json) |
| workspace/all-features 最新完整终态 | CONTRADICTED：原15014收集器exit0、实际Cargo exit101；no-fail-fast运行至doctest结束，259结果块/3869PASS/43FAIL/0ignored；唯一失败目标grpc_network_e2e_test为3/43。1099源码+58控制输入终态变化0；旧2886/8保留为历史 | [最新机器终态](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-driver-action-workspace-terminal-audit1.summary.json) |
| 全workspace/all-targets/all-features编译 | PROVED：原session80815实际exit0、4m22s；五处警告保留，不构成Clippy或runtime认证 | [机器终态](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-workspace-check-all-targets-all-features-2026-10-08.summary.json) |
| ClientInfoStatus前正式Clippy | PROVED：原session72889实际exit0；workspace/all-targets/all-features/locked/offline及-Dwarnings，57.55s。Prepared后收集的53项诊断全部消除，1092源码+58控制输入无变化。历史FAIL保留，不扩大为runtime或发布通过 | [机器终态](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-remaining33-formal-clippy1.summary.json) |
| 全workspace格式检查 | PROVED：`cargo fmt --all -- --check`实际exit0；不是Clippy或运行测试证据 | [机器终态](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-workspace-fmt-check-2026-10-08.summary.json) |
| 最新源码C1检查 | PROVED：原session8604实际exit0；479映射/442选择器、executed=no，完整C2子检查保留；1092源码+58控制输入无变化，不构成新的runtime认证 | [机器终态](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-remaining33-source-c1.summary.json) |
| 最新两个crate all-features lib测试 | PROVED：原session15350实际exit0，Druid150/0、Wrapper16/0，共166/0；两条linker stdout警告保留，不等于完整workspace回归 | [机器终态](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-remaining33-libs-test1.summary.json) |
| 修复后的两个目标编译检查 | PROVED：原session2128实际exit0、13m13s；两目标编译通过，保留private unwrap_ref警告；不是runtime或完整workspace门禁 | [机器终态](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-unused-import-check-2026-10-08.summary.json) |

完整C1/C2和全仓回归对应原1092源码/58控制输入；两项import删除后建立新冻结，只有两个测试文件SHA变化。新source检查不扩大原runtime证据到当前输入。文档补齐未改变该源码/控制冻结。

上述段落描述历史运行冻结。本轮Clippy和两crate all-features lib采用 `remaining33-inputs.json` 的1092源码/58控制输入，逐项终态变化0；原479/1283分母和正式矩阵不变。完整C1原session20187已实际exit0/executed=yes，含完整C2子验收，终态冻结无变化。之后仅修复semantic自测脚本的LF写入及增加字节回归，控制输入变化一项，Rust源码、生产验证器及账本均不变；不能把旧58控制冻结描述为修改后的完整新运行。

原C1 runtime2的idle_count断言失败保留；同源码原断言目标重跑10/0不能证明根因已修复。全仓8个gRPC失败的环境候选是实际全features程序Public入站Block规则，当前profile也为Public；未变更安全设置、未获受控对照，不判定全部失败同源。

## 3. 规格第14节逐项审计

| 要求 | 当前判定 | 已有证据与仍缺内容 |
| --- | --- | --- |
| 1. 单一标准树、无Physical公共名 | INCOMPLETE | 标准树与迁移映射存在；最终public API/compiler/no-Physical完整门禁仍未闭合，源码观察不能代替公开面证据 |
| 2. 配置Wrapper直接实现标准 | INCOMPLETE | [实现账本](rdbc-adapter-implementations.tsv)、[真实trace绑定](rdbc-adapter-runtime-traces.tsv)；105过期源码hash对应七family编译覆盖必须真实再生成 |
| 3. Druid代理实现同标准、holder保存Box标准Connection | INCOMPLETE | 实际核心链路和C1行为证据存在；最终源码/编译/API质量门禁仍开放，不能凭结构关闭整项 |
| 4. raw/logical exactly-once recycle/close、无double pool | INCOMPLETE | 外部租约与历史SQLite合同通过；Task7取消、driver release/owner notification失败窗口尚未验收 |
| 5. 52+17语义总账无缺失/未知/重复 | INCOMPLETE | 69条[语义inventory](jdbc42-semantic-inventory.tsv)中61条含计划态，target有49 PLANNED_TARGET/20 DECLARED，adapter有21 PLANNED/8 LIVE/40 NOT_APPLICABLE，logical有17 PLANNED/9 LIVE/43 NOT_APPLICABLE；最终命令exit2且未进入验收。需按§9补齐final入口、完整证据与fail-closed测试，禁止手写LIVE或沿用过期覆盖handoff。见[逐条状态证据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-task8-final-semantic-state-audit.summary.json) |
| 6. 六项error waiver | NOT_VERIFIED | Task1既有接受记录保留；最终phase门禁与当前全部错误传播证据需在完整验收链确认，不由文档认定PASS |
| 7. index/null/default/error/metadata/resource共享conformance | INCOMPLETE | C1/C2固定方法/原测试迁移通过；Task7资源1-based、SQLXML、capability、取消及真实streaming仍有八项义务 |
| 8. public API/SemVer/external implementor/迁移与allowlist一致 | NOT_VERIFIED | external consumer既有运行证据保留；精确breaking allowlist、最终API与真实SemVer报告未交付，工具授权仍待处理 |
| 9. Java差分与真实DB支持每项true capability | INCOMPLETE | `druid-bench` 已新增 Java 8 `JDBCType` 39 项名称、供应方、编号及 Rust 反向查找的同输入差分，本地目标测试 1/1 PASS；Java Druid 1.2.28 的输出/错误/状态/副作用/清理差分及各数据库真实凭据/服务/行为证据仍未齐备 |
| 10. fmt/check/clippy/test/doc/features/MSRV/DAG全部通过 | CONTRADICTED | 最近完整全features回归3869/43、全部后续目标及doctest已执行，该1099冻结早于当前1104输入；当前全workspace runtime未验收。当前fmt与严格Clippy实际exit0；文档生成历史1092冻结exit0，当前输入未复验；MSRV1.95、feature矩阵、DAG及其余最终质量门禁仍未齐备 |

上述PROVED只用于第2节的具体命令范围，不自动升级第3节的复合要求。尚无整个ADR-RDBC-006完成证明。

## 4. Task7固定剩余分母

1. ResourceOperations与ResourceHandleFactory的完整标准操作及产品接线。
2. 资源1-based访问、free后拒绝、SQLXML状态。
3. 独立ResultSet forwarding身份。
4. capability声明与实际driver行为一致。
5. driver release与owner notification各自恰好一次。
6. caller取消后的受监管释放和确定终态。
7. SQLx eager结果集准确buffered命名。
8. 真实有界首行、背压与Stream适配。

提前实现的BufferedRowSet、ResultSetExt和值级alias不关闭这八项。取消窗口的源码定位及尚未执行的行为测试见 [Task7预检](task7-resource-cancellation-source-preflight-2026-10-08.md)。

## 5. Task8交付物盘点

| 交付物 | 当前状态 |
| --- | --- |
| [standard-contract-v1](../../rdbc/standard-contract-v1.md) | 文档已交付，链接/代码块/空白已检查；未宣称标准发布 |
| [integration-author-guide](../../rdbc/integration-author-guide.md) | 文档已交付，涵盖真实接线与验收边界；未替代集成测试 |
| 本报告 | 初始真实残余审计已交付，整体INCOMPLETE |
| README及status/架构文档一致性 | README已有实际拓扑；架构长文已按当前标准Connection、实际external lease及manifest依赖更新，并区分当前依赖与批准目标。局部链接、闭合代码块及manifest事实检查通过，Mermaid未渲染；最终跨文档一致性与Task8整体仍未验收。见[文档检查证据](../../../.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-e-space-architecture-doc-standard-refresh.summary.json) |
| breaking allowlist与两份SemVer报告 | 未完成，不能生成空报告或手写PASS替代工具输出 |
| breaking API验证脚本 | 未完成，需要精确extra/missing finding合同与真实工具输入 |
| Java/Rust JDBC42 differential目标 | 已新增真实 Java 8 oracle 与 Rust 39 项 SQL 类型差分，`cargo test -p druid-bench --locked --test java_rust_jdbc42_differential` 本地 1/1 PASS；其余 Java Druid 1.2.28 行为差分仍未完成 |
| adapter benchmark | 源文件存在，尚无最终真实性能认证 |

## 6. 后续关闭顺序

继续原编译句柄至终态，核对当前冻结；处理网络对照所需授权，恢复真实全仓完整回归；取得编译覆盖、API/SemVer/MSRV/features及审查证据后关闭Task6。再完成Task7八项与Task8实际门禁，随后按主计划完成P05/P06及P07。

全部18顶层任务仍开放。缺少工具、网络策略或真实数据库条件时单独记录，不缩小规格、放宽断言、修改超时或把局部GREEN替代完整FAIL。


2026-10-08 文档门禁终态：完整workspace/all-features/no-deps/locked/offline文档生成实际exit0，零rustdoc警告。首次85500 exit0的三条警告原证据保留；仅修复Admin私有TTL引用及Metrics两处限定链接。新快照doc-link-repair-inputs.json含1092源码+58控制输入，最新运行终态变化0，日志和机器摘要逐字节镜像。旧C1/Clippy冻结不扩大到三处注释及LF测试脚本修改后的新输入。Task6 FIX_REQUIRED/Task8 INCOMPLETE/18顶层任务OPEN。


2026-10-08 final语义CLI防降级RED：扩展真实CLI合同测试到5项，覆盖help、默认final及planned/adapter-live/logical-live三种显式phase均不能绕过final要求。实际5FAIL/0ERROR，四个验收调用仍因参数未实现返回usage exit2；此为入口缺失的真实RED，不是语义验证通过，也尚不证明计划态/伪证据具体拒绝分支。生产验证器、oracle与inventory未修改，61计划态仍保留。证据task6-e-space-task8-final-semantic-cli-phase-bypass-red.summary.json；下一阶段需要完整final状态、来源绑定、canonical形态与实际证据校验，不能只添加参数或转用Task5。Task6 FIX_REQUIRED/Task8 INCOMPLETE/18 OPEN。


2026-10-08 final状态策略组件TDD：新增verify_final_state_policy，首次27测试实际RED（26个失败subtest），边界补充后28测试RED两FAIL/一ERROR，最终完整28/0。独立拒绝四列计划态及PLANNED_V2、十九字段UNKNOWN/MISSING/unresolved、空白canonical、无效状态、实现路径语法与LIVE/evidence不一致；保留既有standard_adapter_matrix引用语法。真实inventory第一行被PLANNED_TARGET拒绝，inventory/oracle SHA未变。仅两个Python控制脚本变化，1092源码变化0；final CLI仍未接入，未构成冻结来源绑定、类型形态或compiler/runtime认证，未生成LIVE账本。证据task6-e-space-task8-final-state-policy.summary.json。Task6 FIX_REQUIRED/Task8 INCOMPLETE/18 OPEN。


2026-10-08 final来源绑定组件TDD：新增verify_final_source_binding，从已校验52+17 Java oracle重建Task2策略并保留Task3接受的canonical/DECLARED（含DriverRegistry），取得原冻结semantic摘要5d99b573...。首次35测试RED含13失败subtest，最终完整35/0；拒绝五个来源身份字段漂移、缺失/额外/重复对象、错误字段数及冻结摘要/来源声明漂移。允许证据晋升而不改oracle，仅证明Java来源绑定；Rust形态、canonical实际公开路径与compiler/runtime仍未核验，final CLI未接入。仅两个Python控制输入改变，1092源码变化0，oracle/inventory SHA不变。证据task6-e-space-task8-final-source-binding.summary.json。Task6 FIX_REQUIRED/Task8 INCOMPLETE/18 OPEN。


2026-10-08 Task8 canonical编译诊断：原4987实际RDBC all-features check exit0，使用该Cargo JSON明确返回的rmeta与Rust1.97.1对69行逐个外部编译，68Rust目标37PASS/31FAIL，host排除1项未认证。6个资源别名TRAIT不符、24个名称无法解析、types模块私有；ENUM_KIND/MODULE通过项只证明import解析，不认证精确形态。源码1092冻结逐项无变化，oracle/inventory不变，原Rust探针/错误/rmeta保留，机器摘要与Cargo日志逐字节镜像。详见task8-canonical-compiler-identity-audit-2026-10-08.md。final CLI仍未接入，Task6 FIX_REQUIRED/Task8 INCOMPLETE/18 OPEN。


2026-10-08 ClientInfoStatus唯一标准归属修复：真实四状态定义迁入RDBC connection，Druid旧模块/门面只重导出同一类型。原33983 RED exit101/E0432；16500 GREEN两项2/0；原25608完整RDBC all-features exit0/474PASS0FAIL含external implementor/public surface/doctest。workspace fmt实际exit0无输出。新冻结1094源码+58控制输入变化0。严格Clippy原96459仍LIVE，观察进程50784→41292→45860→45160，不推定终态。账本/oracle未改，旧37/31/1不扩大到新源码；API/SemVer与其余canonical缺口仍开放。详见task8-client-info-status-recovery-2026-10-08.md。Task6 FIX_REQUIRED/Task8 INCOMPLETE/18 OPEN。


2026-10-08 ClientInfoStatus修复最终终态：原严格workspace Clippy96459实际exit0/3m22s，all-targets/all-features/locked/offline/-Dwarnings通过，旧LIVE描述仅为历史。新Cargo JSON artifact支持原失败probe_04.rs实际rustc exit0，ClientInfoStatus canonical公开路径闭合；原31诊断中其余30仍待处理，不宣称当前完整69 final通过。RDBC474/0、Druid身份2/0及fmt PASS保留，1094源码+58控制终态变化0。原日志/summary逐字节镜像，见task8-client-info-status-recovery-2026-10-08.md。Task6 FIX_REQUIRED/Task8 INCOMPLETE/18 OPEN。


2026-10-08 三个错误对象canonical归属终态：BatchUpdateException、DataTruncation、SqlClientInfoException真实定义迁入RDBC error，compat及Druid门面重导出同一类型，原payload不变。真实RED exit101/E0432；完整RDBC477/0、Druid lib及身份152/0；严格全workspace Clippy复验直接捕获exit0。1097源码+58控制输入变化0。原69行探针完整复编译现为41PASS/27FAIL/1非Rust目标未认证，取代37/31/1作为最新诊断；旧日志保留。oracle/inventory未改，API/SemVer/final CLI/Task7八项仍开放，Task6 FIX_REQUIRED/Task8 INCOMPLETE/18顶层OPEN。详见task8-error-canonical-ownership-recovery-2026-10-08.md。


2026-10-08 DriverAction标准归属恢复：真实trait迁入RDBC driver，Druid历史模块/门面重导出同一类型。标准路径缺失RED实际exit101/E0432；实际DriverManager回调合同GREEN exit0/1PASS，证明重复注册不替换回调、成功注销通知一次、重复注销拒绝。fmt实际exit0，1099源码+58控制输入复核变化0。完整RDBC回归原句柄70019仍LIVE，未宣称终态或更新完整69行诊断。P05注册路径统一/API/SemVer仍开放，Task6 FIX_REQUIRED/Task8 INCOMPLETE/18顶层OPEN。详见task8-driver-action-ownership-recovery-2026-10-08.md。


2026-10-08 DriverAction回归终态更正：原70019完整RDBC all-features实际exit0，477PASS/0FAIL，1099源码+58控制复核变化0，取代上一条LIVE观察。当前完整workspace/all-features/locked/offline/no-fail-fast正式回归已启动，原句柄15014，日志task8-driver-action-workspace-runtime1.log；尚未终态。此命令保留全部测试并运行失败后的目标，不修改断言/超时/系统安全策略。Task6 FIX_REQUIRED/Task8 INCOMPLETE/18顶层OPEN。


2026-10-08 DriverAction后完整workspace回归终态：原15014 Python收集器exit0，实际Cargo exit101，259结果块/3869PASS/43FAIL/0ignored，no-fail-fast到全部后续目标及doctest结束；唯一失败目标Admin grpc_network_e2e_test为3PASS/43FAIL。1099源码+58控制输入终态变化0，原日志SHA4517a79f405b046de13d0903590f450e3ce4b04b8a9f7490869ca33ee37a027b。日志/summary逐字节镜像，旧LIVE及2886/8范围为历史；TCP Public入站Block、Listen/SynSent仅为候选证据，未进行反事实。Task6 FIX_REQUIRED/Task8 INCOMPLETE/18 OPEN。

2026-10-08 三个标准值类型归属恢复终态：JdbcType/RowId/PseudoColumnUsage真实定义迁入canonical模块，旧路径重导出同一类型。Cargo RED exit101/E0432后首次合同3/0；完整RDBC477/0，Druid lib148/2暴露RowId Debug迁名回归。保留原断言并恢复历史Debug展示后，Druid lib150/0和值合同4/0、完整RDBC477/0、fmt及严格workspace Clippy实际exit0；final purity51文件exit0。原68探针全量复验45/23/1；当前JdbcType对固定Java源编译类的39映射/查找及4未知编号拒绝差分通过，仅限值合同。1101源码+58控制终态变化0，oracle/inventory未改。完整workspace3869/43仍为迁移前1099源码证据，未扩大至当前。详见[值类型恢复报告](task8-standard-value-ownership-recovery-2026-10-08.md)。Task6 FIX_REQUIRED/Task8 INCOMPLETE/18 OPEN。

2026-10-08 Types模块与标准异常kind恢复终态：39模块常量唯一数值定义，历史Types/Druid关联常量保持同值；规范InvalidAuthorizationSpec为真实枚举项，旧InvalidAuthorization保留同值常量与原Debug。真实分类RED exit101/General误判，入口RED exit101/E0432/E0603；实现12具体类优先，保留vendor/recoverable/SQLState fallback。当前完整RDBC479/0、Druid核心与消费者167/0、fmt及严格全workspace Clippy exit0/3m41s、final purity51文件exit0。当前Cargo JSON产物全68原探针47/21/1；固定Java14源码/原编译/运行classSHA一致，39常量+24分类实例当前Rust/Java63条一致，非完整differential。旧variant专用导入实际exit1/E0432，迁移指南已记录，最终API/allowlist/SemVer仍待验收。1104源码+58控制输入变化0，oracle/inventory未改，完整全仓3869/43仍属1099历史冻结。Task6 FIX_REQUIRED/Task8 INCOMPLETE/18 OPEN。报告docs/verification/rdbc-first/task8-constant-exception-contract-recovery-2026-10-08.md。无安装、分支、提交、推送或安全策略修改。

2026-10-08 当前完整workspace回归已启动：原句柄54741，命令cargo test --workspace --all-features --locked --offline --no-fail-fast，冻结standard-constant-exception-inputs.json（1104源码+58控制输入）。日志task8-constant-exception-workspace-runtime1.log可持续读取，live收据仅证明启动，最终须以原句柄和实际Cargo退出码/summary为准；当前未取得完整全仓终态。运行期间不修改冻结源码/控制输入，不重启或取消原进程，不放宽断言、超时或安全策略。Task6 FIX_REQUIRED/Task8 INCOMPLETE/18 OPEN。
