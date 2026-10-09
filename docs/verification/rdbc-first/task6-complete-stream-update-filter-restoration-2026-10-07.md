# Task6 完整流更新 Filter 合同恢复（2026-10-07）

原 pooled_result_set_routes_stream_updates_and_preserves_sqlite_capability_errors 完整恢复并正式 EXACT 绑定。Task6仍FIX_REQUIRED，全部18顶层任务仍OPEN。

## 完整原合同

固定BASE 5afe8609b177544ff7d18461aa84f69efe8ee90e，原body SHA52c19b7021c08421083fb9132badc1afc2fca0d753ab6e4fda7efd5c6dd1295f。原9个assert_eq、1个assert、完整22个有序事件及宏展开22次默认能力错误全部保留。

同一真实Toasty SQLite factory、上下文95/96、原数据源名、SELECT X'010203' AS value均保留。ASCII/Binary/Character各下标及标签的未指定/int/long长度，NCharacter未指定/long长度，总计22入口。负长度-7/-11/-17/-23/-31/-41/-47、i32最小/最大、i64最大及正长度13/19/29/37/43完整传递。

新增单对象StreamUpdateShortCircuitFilter，按标准ResultSetUpdate描述符准确恢复原重载事件名。只在无长度下标入口消费一个字节或UTF16单位，带长度入口和标签入口不预读。句柄clone共享原流读位置，原ASCII剩余2,3、Binary剩余5,6、Reader剩余乙、国家Reader剩余丁四断言实际通过。

标签fail的NCharacter长长度入口返回精确filtered stream update failure，实际创建Statement异常1。默认Filter保持原index/label两类UnsupportedOperation操作名，实际异常22、游标仍开放及显式关闭完整验证。没有新生产路由/API或第二接口，未改变原能力合同；现有标准路由已经支持，首次运行完整原合同通过，不伪造生产RED。

## 实际验证

2736默认Rust1.97.1完整Druid lib exit0：139PASS/0FAIL/0ignored；完整流更新主测试实际执行。1071项Rust/Cargo/proto/SQL输入SHA终态全部不变。日志task6-complete-stream-update-filter-green-2026-10-07.log，SHA55a51e03aa0846d996639e839f5726941095e5de9d7d622ef577272cdb0a9920。

91515保护式正式绑定exit0，原1283前七固定字段逐项不变。只移除此原流更新矩阵占位，窄注册真实lib测试入口，验证器规则不变。1136EXACT/147PARAMETERIZED。

70668绑定后三个完整消费者目标23PASS/0FAIL/0ignored：FilterStat21、canonical ResultSet matrix1、外部Resource Filter1；绑定后1071输入SHA不变。日志task6-complete-stream-update-filter-consumers-2026-10-07.log，SHAa437bbf2eec61575faac576224624314a6e6660267aef9c1edc438d2f3190a79。lib139与消费者23分开报告。

98548完整原源码观察审计exit0：1273PASS/10FAIL。83926完整C2 --no-execute终态exit1，下一完整real_sqlite_result_sets_flow_through_stat_filter_and_statement_cascade缺显式case，整体C2仍FAIL。各原日志、终态summary及审计均保存。

验证器自测17/17、final purity48、authority79/pending25/physical73及固定SHA7210995f5b1e9ac4b50ec56693cc4a5f95905b0aee923f4048a3d2819d7620c3、派生文档79、三Rust文件格式检查与实际git diff --check通过。历史不相关警告不作全仓零警告声明。

## 剩余范围

Task6剩余10源码合同；Task7八义务、Task8/P05/P06/P07及全部18顶层任务继续开放。没有当前全仓通过/API/compiler/SemVer/MSRV/features/完整审查通过声明。历史全仓3795PASS/11FAIL及Admin49PASS/11FAIL保留，未调整系统安全策略。

下一真实SQLite统计与Statement级联原文已保存，body SHA5507f632fac9e20bdc0da1430b720f6b2908b97d9fdc2fc70d83007894308160，原17eq/6assert；NOT_COMPILED/NOT_RUN/NOT_BOUND，不计入完成。

本轮所有启动句柄已终态，没有stage/commit/push/分支/安装/发布。
