# Task 6：type-map 与 typed getObject 列名身份恢复（2026-10-06）

沿用 ADR-RDBC-006 与现有 single-standard 规格/计划，固定 BASE 为 5afe8609b177544ff7d18461aa84f69efe8ee90e。完整范围仍为 P04 Tasks 6–8 与 P05–P07，共18个未闭环顶层任务。

## 已实现行为

ResultSet 新增可覆盖的 object_with_type_map_by_label 与 object_as_by_label，ResultSetExt 委托同一标准树。type-map 下标默认恢复准确 result_set_get_object_with_type_map；列名默认直接返回 result_set_get_object_by_label_with_type_map，不先 find_column。typed 列名默认解析一次后委托下标，驱动可覆盖原列名入口。

DruidPooledResultSet 原样转发列名、Some/None type-map 和 target type。配置隐藏列或列映射时先验证可见性，再调用原列名重载；无配置时不额外查列。保留 access_core 锁、治理策略分类和创建语句异常记录，关闭/失效检查位于外层。驱动的返回对象不按请求类型再次转换。

两个完整 BASE 案例经实际 SQLite Statement、资源准入和池化代理接管 StrongGetterProbe；保留原3个断言、31/41下标、mapped/payload列名、APP.MONEY -> BigDecimal map、Custom(com.example.Money)/Bytes目标及原始返回值。调用记录由驱动实际入口产生，未预置计数或缩减原测试。补充 Box<dyn ResultSet> 动态调用、隐藏列拒绝、双向映射、缺列/缺映射及关闭后无额外驱动调用。

对象 Filter 与 LOB 开启统计在原 BASE 四个入口存在。此次两项无 Filter 原始案例通过不关闭这些义务；完整对象 Filter、资源生命周期、错误所有者与取消合同仍继续验收。

## 实际验证

日志均位于 .superpowers/sdd/2026-08-24-rdbc-single-standard-unification/。

- 前轮同一全workspace session3877 终态 exit101 /242结果块 /3697 PASS /2 FAIL /0 ignored，含2doctests。仅 task6_callable_canonical_matrix_test 原prepared5vs6失败；替代旧LIVE记录，该结果早于此次源码。
- task6-object-label-default-red.log：exit101 /0PASS1FAIL，下标type-map默认错误名称不符。
- task6-object-label-default-both-red.log：exit101 /0PASS2FAIL，列名错误为find_column，违反默认直接不支持合同。
- task6-object-label-pooled-red.log：session78110 exit101 /0PASS2FAIL，实际池化列名入口未委托原驱动重载。
- task6-object-label-green.log：session85050 exit0 /lib83 + typed8 =91PASS。
- task6-object-label-final.log：session4218 exit0 /7结果块 /139PASS0FAIL，含动态调用与列治理补充。
- task6-object-label-rdbc-final.log：session51931 exit101，生成探针434PASS1FAIL。旧探针没有覆盖新增canonical列名叶入口，仍依赖查列失败；修复生成器覆盖这两个真实hook，保持原断言和固定435用例分母。不是生产默认合同放宽，也不是C1全生产证据。
- task6-object-label-rdbc-green-final.log：session86343顺序脚本第一步 exit0 /16结果块 /471PASS0FAIL，含1doctest。
- task6-object-label-druid-green-final.log：同session86343终态 exit0 /7结果块 /139PASS0FAIL。lib84、typed8、buffered6、FilterStat21、metadata3、FilterCoverage16、FilterMatrix1。
- task6-object-label-c2-final.log：session5229 exit1，推进至 physical_result_set_metadata_preserves_getter_error_and_wrapper_identity 缺明确分支；完整C2仍FAIL。
- 验证器自测15/15、final purity44文件、authority79行/Task7pending25/Physical disposition73及固定SHA16b2e8a669dc4ab41c37933a035cb0066f25cb98553386615b409831300f9cbc通过；9个Rust文件rustfmt --check及git diff --check通过。

## 账本与剩余范围

两项原始案例绑定准确私有lib入口，仅增加两项显式白名单，移除旧矩阵两项phantom case ID，反向方法索引和派生文档同步。原1283分母、源身份、原摘要及关键观察不变；当前1089 exact /194 parameterized。最低缺明确分支55：buffered17、lease2、recycle12、ResultSetFilter24。数量不是行为完成率。

Task6仍FIX_REQUIRED。C1/C2、Callable错误缓存原计数、完整对象Filter/资源统计、公共API/compiler/SemVer、all-features/MSRV及全范围审查仍未关闭。两项新增standard默认方法需进入新鲜API快照/兼容性验收，旧快照不冒充最终认证。Task7八组件、Task8和P05–P07完整范围保留。未提交、推送、发布、安装或切换分支。

下一项按账本原源路径恢复元数据getter错误和wrapper身份；随后继续完整对象Filter与资源责任。当前源码的新全workspace回归将独立记录，未经终态不得判定全绿。

Fresh current-source fullworkspace confirmed LIVE by handle41083; log task6-object-label-workspace.log. Prior workspace3697/2 is pre-label-restoration evidence. No Rust edits while this Cargo runs; full result remains unverified until terminal.
