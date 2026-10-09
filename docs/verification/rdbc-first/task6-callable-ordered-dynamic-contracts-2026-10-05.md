# Task6 Callable 有序结果与动态创建者恢复（2026-10-05）

状态：两个完整 BASE 场景已恢复并专项验证；Task6 仍为 FIX_REQUIRED。

## 范围与原始合同

来源为 5afe860:crates/druid/tests/callable_statement_semantics_test.rs 的 callable_generic_execute_preserves_inherited_ordered_results 和 callable_result_set_keeps_the_same_dynamic_statement_identity。当前 canonical matrix 为两个名字增加真实 helper 的独立显式分支，并新增两个独立运行时测试；不再用通用 CallableProbe 替代这两个原始场景。未改动生产代码。

有序结果保留原始结果集10 / 更新计数2 / 结果集20、首次无更新计数、两次 current result getter、默认推进关闭第一游标、整数处置1推进到第二游标、Callable 创建者及最终级联关闭。标准 Vec<ExecutionResult> 的首结果种类对应原 boolean execute；None 对应原 JDBC -1。额外双向属性及关闭观察证明返回创建者与原始逻辑 Callable 共享状态。

动态创建者保留 Callable/Prepared/Statement/Wrapper 继承身份、同一返回对象的叶/父接口数据地址、原句柄与返回创建者的双向属性共享、原句柄 Drop 后继续读取值1、通过可变 Callable 视图重复关闭、各返回投影及游标同步关闭。关闭游标仍可取到已关闭创建者，租约结束后 getter 拒绝。退休的 Physical/私有 concrete unwrap 按已批准标准接口迁移，以同对象标准视图和可观察共享行为证明，不恢复第二公共接口；空目标按当前标准返回 InvalidArgument，未知目标查询为false、unwrap错误。

## 验证

- task6-callable-ordered-base-red.log：C2 静态缺少原始 ordered 显式分支。
- task6-callable-ordered-runtime-red.log：初次编译失败（ColumnIndex import、同步 get_statement 错误 await），不作为行为 RED。
- task6-callable-ordered-runtime-validated-red.log：0/1 FAIL，原测试驱动未提供 generic prepared execution；随后恢复 BASE 三结果。
- task6-callable-ordered-runtime-green.log：0/1 FAIL，测试驱动原先忽略 timeout setter；恢复真实属性状态后 task6-callable-ordered-runtime-final.log 为1/1 PASS。
- task6-callable-dynamic-runtime-red.log：0/1 FAIL，测试驱动未提供 query execution；恢复 BASE 一行后 task6-callable-dynamic-runtime-green.log 为1/1 PASS。
- 上述 RED 是测试迁移/fixture 缺口证据，不声称发现新的生产缺陷。
- task6-callable-ordered-dynamic-regression.log：cache6/6、FilterStat21/21、canonical Callable3 PASS/2 FAIL，合计30 PASS/2 FAIL。剩余失败为既有完整错误 helper 和矩阵的 prepare5vs6，原期望6未改变。
- task6-callable-ordered-dynamic-c2.log：exit1，已通过两个恢复场景的静态阶段，推进到 prepare_call_overloads_preserve_keys_delegation_and_cache_lifecycle 显式分支。1283测试分母与 verifier 未修改。
- final purity43、authority79/pending25、authority SHA16b2e8a669dc4ab41c37933a035cb0066f25cb98553386615b409831300f9cbc、单文件fmt和git diff --check PASS。

完整 workspace 终态另见当前进度台账及 task6-callable-ordered-dynamic-workspace.log；专项通过不能替代全量、MSRV、all-features、真实数据库或取消安全认证。

## 剩余工作

完整 Callable overload BASE 为947行、170个原始 assertion，仍需保留全部 OUT、named/resource/calendar/map/target type 委托与缓存键/参数清理/统计合同，不能只补一个健康缓存分支。错误合同的普通Prepared真实创建后拒绝、typed index owner/cache等价证据以及5vs6仍未解决。C1/C2/API/compiler/SemVer/feature/MSRV/最终审查和完整 Task7–8/P05–P07 范围继续开放。

## 完整 workspace 终态

新鲜默认 locked/offline serial --no-fail-fast 回归 session5718 terminal exit101，236结果块，3654 PASS /2 FAIL /0 ignored，包含2doctests。仅既有 Callable 原始错误 helper 和 canonical matrix 在prepare5vs6失败；新增两个独立完整场景均通过，其余目标通过。日志 task6-callable-ordered-dynamic-workspace.log。全部本轮 Cargo/C2 会话已终态；不声明 Task6 或完整计划完成。
