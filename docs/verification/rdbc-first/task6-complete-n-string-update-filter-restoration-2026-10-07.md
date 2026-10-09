# Task6 完整 NString 更新 Filter 合同恢复（2026-10-07）

完整18顶层任务保持开放，Task6仍FIX_REQUIRED。

## 原合同与本轮范围

固定BASE为5afe8609b177544ff7d18461aa84f69efe8ee90e，原案例result_set_filter_semantics_test.rs#pooled_result_set_routes_n_string_updates_and_preserves_sqlite_capability_errors。正式目标为core::druid_pooled_result_set::diagnostic_tests::task6_pooled_result_set_routes_n_string_updates_and_preserves_sqlite_capability_errors。

原六个等值观察及一个开放状态断言全部保留：同一Toasty SQLite factory；上下文id97/sqlite-n-string-update-short-circuit与id98/sqlite-n-string-update-pass-through；原SELECT '原值' AS value；索引1的Some("过滤-索引")与原Unicode标签"名称"的None；原两条精确事件；标签fail的精确filtered NString update failure；实际创建Statement异常1；默认链两个准确UnsupportedOperation名称、实际创建语句异常2及游标仍开放。标准调用使用ResultSetUpdate::NString，未折叠为String或SQL NULL描述符；增加实际语句和连接清理，不修改原期望。

现有生产更新链已能完成上述合同，因此本轮仅恢复完整测试与映射，不修改生产行为、不新增公开API。首次测试通过，不宣称存在生产行为RED或完成了生产缺陷修复。外部更新Filter扩展及完整API门禁不由本轮私有夹具测试证明。

## 实际运行证据

- 初始53041终态exit0/1PASS0FAIL，原完整案例首次通过。日志task6-complete-n-string-update-filter-initial-2026-10-07.log。
- 补充61811终态exit0/126PASS0FAIL，完整Druid lib通过。两个新增控制验证：双默认链准确转发NString Unicode/None及普通String描述符；A/B/C两重载正向进入、反向退出，B成功/错误短路均不进C或驱动；隐藏索引/标签在hook前拒绝且实际异常各一次；普通String继续原生只读拒绝；成功短路不更改真实SQLite当前值；关闭/旧租约两个入口先拒绝，无追加事件或异常。日志task6-complete-n-string-update-filter-controls-2026-10-07.log。
- 正式绑定后17714终态exit0/七块155PASS0FAIL：lib126、FilterStat21、MetadataOwner3、保留Filter矩阵1、外部Metadata1、外部Statement1、外部读取Filter2。日志task6-complete-n-string-update-filter-binding-regression-2026-10-07.log；summary.json保存终态、scope及日志SHA。重复的初始/控制范围不累加。

所有Cargo使用已安装默认Rust1.97.1，locked/offline串行。本轮155/0不是完整workspace、all-features或MSRV1.95证明。此前全仓3787/0属于后续标量/动态Statement/Metadata修改前源码。

## 正式台账与剩余工作

仅原完整NString更新案例从参数映射改为EXACT，并仅移除旧矩阵对应case id；生成调用索引和派生文档同步。原1283项前七基线字段逐项核验固定，正式1129EXACT/154PARAMETERIZED。新鲜完整源码观察审计1266PASS17FAIL；源码观察不等价于运行测试通过率。

29372完整C2 no-execute终态exit1，推进到pooled_result_set_routes_navigation_properties_and_real_sqlite_state。日志task6-complete-n-string-update-filter-c2-2026-10-07.log；整体C2仍FAIL。

验证器17自测、标准纯净性final48、authority79/pending25/physical73及固定SHA7210995f5b1e9ac4b50ec56693cc4a5f95905b0aee923f4048a3d2819d7620c3、派生文档79、三个Rust文件格式通过。Task7八义务、Task8/P05/P06/P07及API/compiler/SemVer/MSRV/features/新鲜全仓/最终审查均保持开放。下一步完整导航属性/真实SQLite状态合同，不能用缓冲导航单例替代原短路/重载/错误/统计观察。
