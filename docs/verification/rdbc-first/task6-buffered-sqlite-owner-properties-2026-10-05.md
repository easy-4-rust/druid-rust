# Task6 真实 SQLite 创建者、生命周期与属性合同恢复（2026-10-05）

## 范围与状态

继续完整 P04 Task6–8、P05–P07，Task6 保持 FIX_REQUIRED。前一轮架构分析重新核实了当前工作区、CodeGraph、计划和失败证据；本轮恢复已有规格要求的 BASE 行为，没有缩减整体目标。

来源：`5afe860:crates/druid/tests/druid_pooled_result_set_differential_test.rs`。此前未完成的10个普通 Statement 创建者/生命周期 helper 和 literal arms，本轮核对后验证通过；再恢复11个完整元数据、警告、列名调用、游标属性和行状态场景。所有场景保留真实 Toasty SQLite factory -> standard holder -> Druid pooled Connection -> Statement -> ResultSet，以及原 SQL、五行数据、原调用和断言。独立入口分别运行10个与11个场景，不能用通用 BufferedRowSet fallback 认证剩余迁移。

## 测试映射修正

`task6-buffered-sqlite-owner-first.log` 原先1 PASS/2 FAIL于 `row().unwrap()` 返回 result_set_row unsupported。BASE仅 `let _ = rs.row(...)`，没有当前位置成功断言；恢复原调用，不增加未经原合同要求的生产能力。`task6-buffered-sqlite-owner-final.log` 3/3 PASS。该失败是迁移测试强化要求，不是生产缺陷 RED。

新增11个场景首次编译失败于旧 metadata.column_count Result 到现代 usize 的机械映射，见 `task6-buffered-sqlite-properties-first.log`。修正签名，不将编译失败记为生产 RED。

## 实际生产 RED/GREEN

`task6-buffered-sqlite-properties-runtime.log` 编译通过，2 PASS/2 FAIL于真实 SQLite `set_fetch_size(100)` 返回 result_set_set_fetch_size unsupported。BASE平台缓冲结果明确支持非负提示状态、初始0、负值InvalidArgument以及关闭错误。

在 wrapper-private StandardAdapterResultSet 恢复 fetch_size 状态和标准 setter/getter；不引入第二个公开驱动接口，不改变或截断缓冲行、不重新执行查询。负值先拒绝，再检查关闭/驱动租约，保留原错误优先级。`task6-buffered-sqlite-properties-fetch-fix.log` 4/4 PASS，涵盖两个原有入口和两个新增独立入口，11个场景中原行更新/插入/删除false断言也通过。

新增真实驱动合同 `buffered_fetch_size_contract_test.rs`，验证0->100->0、负值不修改状态、不定位/截断游标、两行值7/8、关闭后拒绝、关闭后负值优先以及驱动租约失效拒绝。`task6-buffered-fetch-size-contract.log` 2/2 PASS。

## 验证与剩余

最终标准纯净性43文件通过；迁移授权79行、Task7待承接25行、旧接口标准化73行，SHA16b2e8a669dc4ab41c37933a035cb0066f25cb98553386615b409831300f9cbc保持。三个Rust文件fmt check与git diff --check通过。

C2 --no-execute `task6-buffered-sqlite-owner-properties-c2.log` exit1，推进到 close_count_zero_before_close 缺少显式分支；原0->1计数仍需要真实内部状态或已批准的等价观测证据。不能用测试自行计数、统计汇总或静态源码存在替代。1283分母和验证器未更改，通用fallback仍未认证。

完整workspace回归正在执行，日志 `task6-buffered-sqlite-owner-properties-workspace.log`，待终态追加结果；之前3658/2属于更早源代码。Callable prepared5vs6、typedindex owner/cache、普通Prepared创建后拒绝、精确调用形式身份，以及C1/C2/API/compiler/SemVer/features/MSRV/最终review均未关闭。Task7八组件、Task8、P05–P07完整范围保留。没有提交、推送、发布、安装或分支操作。


Task6 real SQLite owner/property fullworkspace terminal 2026-10-05: session7983 exit101 /239 resultblocks /3662PASS /2FAIL /0ignored incl2doctests, task6-buffered-sqlite-owner-properties-workspace.log. Only Callable full error helper/matrix prepared5vs6 fails; new SQLite21 BASE helper recovery/matrix4 and actual fetchhint2 PASS. Supersedes LIVE7983 and previous3658/2 for current source. C2 remainsFAIL at close_count_zero_before_close explicitarm; purity43/authority79 pending25/fmt3files/diffPASS. Full Task6FIX_REQUIRED/C1/C2/API/compiler/SemVer/features/MSRV/review and Task7–8/P05–P07 remainopen; no goal completion. All own launchedCargo/Python handles terminal.
