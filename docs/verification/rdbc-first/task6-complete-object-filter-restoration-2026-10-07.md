# Task6 完整 Object Filter 合同恢复（2026-10-07）

Task6 仍为 FIX_REQUIRED；完整18个顶层任务保持开放。本报告只记录完整原合同恢复及其验证范围，不能证明全部公共API、驱动、资源或发布门禁完成。

## 原合同与实现

固定BASE为 `5afe8609b177544ff7d18461aa84f69efe8ee90e`。原案例为 `crates/druid/tests/result_set_filter_semantics_test.rs#pooled_result_set_routes_all_object_overloads_and_preserves_argument_identity`，原文件SHA256为 `f6a86262f4a1ff10e33af23b12912212f4cc3690cd9102802680a4f988ffca43`。

正式替代为 `crates/druid/src/core/druid_pooled_result_set/diagnostic_tests.rs#core::druid_pooled_result_set::diagnostic_tests::task6_pooled_result_set_routes_all_object_overloads_and_preserves_argument_identity`。保留原十个等值断言、七条成功重载事件、精确DriverError及实际创建Statement异常计数1；新增断言直接观察类型映射和目标类型的借用地址。

六个标准对象读取入口经同一 `ResultSetObjectRead` 描述符与注册顺序的完整around-chain；默认委托保留原生下标/标签重载和完整 `RdbcObject`，不通过标量转换消除资源或Custom对象身份。未配置列治理时不提前find_column；配置隐藏列/映射时先治理；关闭或失效租约先拒绝。资源访问错误分类后只向创建语句记录一次异常。

补充四个测试覆盖全部六入口治理、默认资源共享游标及Custom身份、Filter顺序/短路/错误回退，以及失效租约拒绝且不继续读取原生资源。

## 实际TDD证据

日志均在 `.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/`。

- `task6-complete-object-filter-interface-red-2026-10-07.log`：缺少描述符/hook的4个编译错误，属于接口RED，不作为运行结果。
- `task6-complete-object-filter-behavior-red-2026-10-07.log`：默认委托接口存在、生产路由尚未接入时，111通过、3失败；实际暴露提前游标/列定位拒绝和Filter顺序缺失。
- `task6-complete-object-filter-green-2026-10-07.log`：辅助脚本换行转义失败后运行了未变更路由；仍111通过、3失败。文件名不代表GREEN，明确保留失败记录。
- `task6-complete-object-filter-routing-green-2026-10-07.log`：路由接入后114通过、0失败，包含五个新增合同测试及此前109项单元测试。

## 正式绑定

完整原案例已独立通过 `verify_ledger` 的原分母、断言/调用观察及真实测试目标绑定检查，然后才更新正式账本。全部1283行原始七字段重新从BASE派生核对，保持不变；只变更该案例处置及目标。账本为1125 EXACT、158 PARAMETERIZED；反向调用索引和派生迁移文档同步。旧参数化案例目录仅移除此条已迁移身份，不删除其他义务。

绑定后完整源码观察审计实际结果为1262通过、21失败；新增Object原案例通过，剩余21项仍为ResultSet Filter合同缺口。审计产物为 `task6-complete-object-filter-source-floor-audit-2026-10-07.json`，不是运行测试计数。验证器自测试17通过；迁移处置表仍79行、Task7待处理25行，哈希 `7210995f5b1e9ac4b50ec56693cc4a5f95905b0aee923f4048a3d2819d7620c3` 不变；git diff --check通过。

绑定后session82001终态退出0：Druid单元114与Filter矩阵1，合计115通过、0失败。日志 `task6-complete-object-filter-formal-binding-green-2026-10-07.log`。原始路由114与此114为重复范围，不相加。完整C2 session5755退出1，首个缺口已推进至 `pooled_result_set_routes_all_scalar_getters_through_java_filter_overloads`，日志 `task6-complete-object-filter-c2-2026-10-07.log`；整体C2仍失败。

新的完整workspace回归session6505已启动，实际cargo69032及编译子进程已确认，日志 `task6-complete-object-filter-workspace-2026-10-07.log`，终态取得前不声明全局通过。已保存990项tracked及非忽略untracked的crates/vendor Rust/Cargo输入与根Cargo/toolchain哈希，产物 `task6-complete-object-filter-workspace-inputs-2026-10-07.json`；该范围不包含文档/脚本，也不冒充整个仓库快照。迁移派生文档校验79行和最终standard purity48再次通过。

## 同轮工作区失败修复范围

此前NString源码完整workspace终态为3774通过、8失败，退出101；它不能代表当前源码。归还校验修复在未配置原生checker/query时实际调用标准is_valid，不再把空校验操作视为有效验证；逻辑合同18和完整回收16通过。Wrapper夹具为每个实际新建原生连接保留独立关闭计数，并验证全部连接各关闭一次；原主连接断言保持，Prepared矩阵15通过。合计49项局部复测通过，不代替修复后完整workspace结果。

## 剩余门禁

新增非穷举描述符和默认Filter方法属于规格§13的显式发布前API修正。API diff、external consumer/implementor、compiler identity、SemVer、MSRV、全feature、完整审查继续OPEN。Task7八项义务、Task8与P05–P07范围不变。未执行安装、分支切换、提交、推送或发布。


## 完整工作区终态补充

同一6505执行句柄已返回exit0；完整日志250结果块、3787通过、0失败、0忽略，含2doctests，Rust1.99.0。默认Rust1.97.1独立all-targets workspace check11062同样exit0；属于编译证据，不证明默认工具链运行测试、MSRV或all-features。两份summary.json保存日志SHA与实际句柄终态，990项输入逐项一致。此前LIVE文字为历史执行过程，由此终态替代。Task6 C2仍失败，21项Filter源码合同缺口和完整18顶层任务仍开放。
