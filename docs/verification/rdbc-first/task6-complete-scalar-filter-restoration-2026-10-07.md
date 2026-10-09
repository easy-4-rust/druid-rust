# Task6 完整标量 Filter 合同恢复（2026-10-07）

Task6仍为FIX_REQUIRED；全部18个顶层任务保持开放。本轮只闭合完整标量原合同，不证明其他Filter、Task7资源八义务或发布门禁完成。

## 完整合同

固定BASE 5afe8609b177544ff7d18461aa84f69efe8ee90e，原result_set_filter_semantics_test.rs#pooled_result_set_routes_all_scalar_getters_through_java_filter_overloads。正式替代为core::druid_pooled_result_set::diagnostic_tests::task6_pooled_result_set_routes_all_scalar_getters_through_java_filter_overloads，保留原19个assert_eq、2个assert、18有序事件、九类下标/标签返回值、精确filtered double failure与实际创建Statement异常1。真实Toasty SQLite、标准registry/admission/lease/policy保留原id73和上下文。

新增治理控制遍历九类18入口，隐藏失败均记录一次；关闭与失效租约先拒绝，不运行Filter或原生读取。新增原生双入口探针与两默认Filter，保留原标签find_column顺序及原生返回值，Unicode两次UTF16长度14；A/B进入和反向退出、原生精确错误、实际Statement计数1及A/B/C由B短路全部18入口控制通过。

## 实际RED/GREEN

- 3188初始exit101，两个夹具编译错误：标准Box接口不能调用Druid私有set_hidden_columns。仅治理测试改用现有真实SQLite具体池化游标和Vec隐藏列，未改原断言。日志task6-complete-scalar-filter-initial-2026-10-07.log。
- 88445实际行为exit101：116通过、1失败，隐藏18次只计9。完整原合同及原生/顺序补充已通过，不能伪造这两项RED。日志task6-complete-scalar-filter-behavior-2026-10-07.log。
- 生产read_object把列映射移入同一access_core分类闭包；隐藏下标失败不再提前绕过创建Statement异常记录。不新增API，不重复分类或记错。
- 95010修复后exit0：Druid lib117通过、0失败。日志task6-complete-scalar-filter-green-2026-10-07.log。
- 38877绑定后exit0：117 lib +21 FilterStat +1保留矩阵 +2外部读取Filter，四结果块141通过、0失败。前一117属于重复范围，不相加。日志task6-complete-scalar-filter-binding-regression-2026-10-07.log。

上述运行全部使用已安装默认Rust1.97.1，locked/offline串行。它们是局部真实运行证据，不替代修改后的完整workspace、MSRV1.95或all-features。

## 正式映射与独立验证

仅完整标量条目由PARAMETERIZED改为EXACT；仅其旧case ID退役，其他条目保留。1283原七字段逐项核验固定；1126EXACT/157PARAMETERIZED。方法反向索引及派生迁移文档同步。新鲜全量源码观察审计1263通过/20失败，不作为行为完成率。迁移验证器17自测、final purity48、authority79/pending25/physical73固定SHA7210995f5b1e9ac4b50ec56693cc4a5f95905b0aee923f4048a3d2819d7620c3、派生文档79、五Rust文件格式与git diff --check通过。

完整C2新检查93005终态exit1，首个缺口推进至pooled_result_set_routes_dynamic_statement_identity_through_filter；整体C2仍失败。日志task6-complete-scalar-filter-c2-2026-10-07.log。后续动态Statement/metadata、资源读写、导航/更新/行修改等20项缺口，以及Task7八项、Task8/P05–P07和API/compiler/SemVer/features/MSRV/完整审查全部保持OPEN。

## 先前工作区终态

修改前完整Object workspace6505 exit0/250blocks/3787PASS0FAIL incl2doctests（Rust1.99.0）；默认1.97.1 all-targets check11062 exit0。990冻结输入均未改变，两份summary与日志SHA已保存。它们属于本轮标量生产修复前的源码，不能证明修复后的全仓通过。
