# Task6 完整Metadata Filter合同恢复（2026-10-07）

完整18顶层任务保持开放，Task6仍FIX_REQUIRED。

## 原合同与实现

固定BASE 5afe8609b177544ff7d18461aa84f69efe8ee90e，原result_set_filter_semantics_test.rs#pooled_result_set_routes_metadata_platform_handle_through_filter。正式替代core::druid_pooled_result_set::diagnostic_tests::task6_pooled_result_set_routes_metadata_platform_handle_through_filter。

完整保留原七等值观察：原生默认metadata列0与physical:meta_data原日志；真实Toasty SQLite、id82/sqlite-metadata-filter/SELECT1短路平台句柄列0；精确filtered metadata failure、实际创建Statement异常1与原两Filter事件；同一factory新建id83/sqlite-metadata-pass-through/SELECT1,'two'经默认Filter列2。原column_count返回Result改为唯一标准usize调用，只删除无效unwrap，不改期望。增补直接默认委托Arc::ptr_eq证明原平台句柄，不用相同静态值代替身份。

ResultSetFilter增补公开standard_result_set_metadata(context,next)；外部实现者可以拥有型Arc标准metadata短路，不必命名私有StandardResultSetFilterChain，不暴露raw连接或游标。默认保留既有metadata before/after观察及after对成功/失败结果的替换。旧私有around方法桥接新hook，原注册顺序保留。chain.context明确返回原上下文寿命引用，避免把独立上下文借用错误绑定到可变链借用；未新增第二驱动端口或所有者。

公开默认方法按规格§13追踪；API/compiler/SemVer/external完整门禁仍OPEN。

## 实际RED/GREEN

- 53601接口exit101：缺公开hook1错误，夹具column_count三处无效unwrap分别保留记录；非行为RED。
- 44610实际exit101/120PASS1FAIL：短路列0实际原生SQLite列1，证明接口存在但未路由。日志task6-complete-metadata-filter-behavior-red-2026-10-07.log。
- 6937桥接后exit0/121PASS0FAIL。日志task6-complete-metadata-filter-routing-green-2026-10-07.log。
- 7359补充控制exit0/123PASS0FAIL。原生Arc身份、A/legacy/B正向进入和反向退出、原生精确错误及legacy after错误观察、B短路不进C/raw、legacy after替换原错误结果且返回原replacement Arc通过。真实标准Statement/registry/probe验证原生错误实际异常1、关闭/失效租约在hook/raw前拒绝、统计不追加。日志task6-complete-metadata-filter-controls-green-2026-10-07.log。
- 6229正式绑定后exit0/七结果块152PASS0FAIL：lib123、FilterStat21、MetadataOwner3、保留Filter矩阵1、外部Metadata1、外部Statement1、外部读取Filter2。外部Metadata实现只用public standard接口，经真实DruidPool/SQLite验证默认两列/平台短路0/顺序/错误反向退出/关闭和旧租约拒绝。日志task6-complete-metadata-filter-binding-regression-2026-10-07.log及summary.json。123/121等重复范围不相加。

全部运行使用已安装默认Rust1.97.1 locked/offline串行；上述局部回归不是新鲜完整workspace/MSRV1.95/all-features证明。

## 台账和剩余门禁

只绑定完整原案例并退役它的旧参数case；原1283七字段逐项核验固定。1128EXACT/155PARAMETERIZED，新鲜源码观察1265PASS18FAIL（不作为行为完成率）。反向调用索引和派生文档同步。验证器17自测、final pure48、authority79/pending25/physical73原SHA7210995f5b1e9ac4b50ec56693cc4a5f95905b0aee923f4048a3d2819d7620c3、派生文档79、十Rust文件格式及diff通过。

完整C29435终态exit1，首败完整NString更新Filter案例；2026-10-07架构核验同一源码门禁再次exit1，阻塞一致。其余18源码合同缺口、Task7八义务、Task8/P05–P07与API/compiler/SemVer/MSRV/features/全仓新鲜回归/完整审查均OPEN。未安装、切换分支、提交或发布。
