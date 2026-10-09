# Task6 完整 Object 更新 Filter 合同恢复（2026-10-07）

原案例 `result_set_filter_semantics_test.rs#pooled_result_set_routes_object_updates_and_preserves_sqlite_capability_errors` 已恢复为独立完整测试并正式EXACT绑定。Task6仍FIX_REQUIRED，全部18顶层任务及发布门禁仍开放。

## 完整原观察与实现范围

保留主测试原8个assert_eq和1个assert，以及Filter夹具2个对象身份断言。四种独立重载分别观察index15 Custom对象Arc身份/downcast/id99/class_name，label object_value SQL NULL，index16 Integer7/scale-3，label scaled_object Bytes9/scale99；完整四项事件保持原顺序及内容。保留精确filtered updateObject failure及实际原创建Statement异常1。

默认链使用相同真实Toasty SQLite工厂，四种Object更新独立返回原index/label UnsupportedOperation，并保留原创建Statement异常4、结果集仍开放和明确关闭。测试ID89/90、数据源名、SQL均保留。Fixture各为独立对象文件，没有compat/STUB或生产wildcard充数。

本项仅恢复测试与正式迁移映射，没有修改生产路由。首次完整运行已有实现直接通过，因此不宣称生产修复或虚构TDD运行RED。

## 实际验证

- 74226真实终态exit0/default1.97.1：完整Druid lib131PASS/0FAIL/0ignored，新完整Object更新测试实际执行通过；1025项输入终态SHA全部不变。日志task6-complete-object-update-filter-initial-2026-10-07.log，SHA3c814d9a09655d766fb5425d7de363b371d38856a30d5307727e4edcda79dd2b。
- 33684正式绑定后真实终态exit0/default1.97.1：完整FilterStat21与保留ResultSet Filter矩阵1，22PASS/0FAIL/0ignored；1025项绑定后输入SHA全部不变。日志task6-object-update-filter-binding-regression-2026-10-07.log，SHA1f5ddbb42ec67fa26d7d38fa7c0271f3eeec701271344137854446f95375fc07。131和22是不同目标，不把较早168等重复范围累加为当前全仓结果。
- 验证器自测首次调用因错误模块加载入口ImportError未执行；正确unittest discover入口随后17/17实际通过。前者不是产品RED。
- final standard purity48、authority79/pending25/physical73、固定SHA7210995f5b1e9ac4b50ec56693cc4a5f95905b0aee923f4048a3d2819d7620c3、派生文档79实际通过。四文件rustfmt --check与真实默认git diff --check exit0，CRLF警告不是错误。

所有日志和真实终态summary位于 `.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/`，scope及SHA均保存。已有未使用代码/变量警告保留，没有宣称全仓无警告。

## 正式分母与剩余工作

仅这一原完整案例改为EXACT，仅移除旧矩阵对应占位ID，窄注册实际lib测试目标。所有1283项前七冻结字段逐项保持；正式1131EXACT/152PARAMETERIZED。完整源码观察审计1268PASS/15FAIL，不等于运行测试覆盖率或项目完成率。

21701完整C2 --no-execute真实终态exit1，下一原完整案例 `pooled_result_set_routes_resource_getters_and_real_sqlite_streams` 缺显式分支；整体仍FAIL。完整下一案例原文已保存，固定BASE body SHA c93ac073252dd130904a81f09974ba8b42ac4cef38134ebc033f09b530c44949，原11eq/3assert、key_calls及artifact SHA保存于complete_resource_getter_base_manifest_20261007.json；仅准备态，NOT_COMPILED/NOT_RUN/NOT_BOUND。

原导航全workspace3795/11/exit101及独立Admin复现49/11/exit101是真实失败证据。当前已更改测试源码与矩阵，不能把原全仓结果标成最新源码完整通过。防火墙限定程序Public Block规则、监听存在而TCP超时证据见task6-navigation-workspace-network-failures-2026-10-07.md；系统调整审批仍待定，未改规则或提升权限。继续独立合同实施不豁免网络、全仓或发布验收。

Task6剩余15源码合同、Task7八义务、Task8/P05/P06/P07、API/compiler/SemVer/MSRV/features/当前全仓/最终审查及全部18顶层任务仍OPEN。未创建分支/worktree，未stage/commit/push/publish/install。
