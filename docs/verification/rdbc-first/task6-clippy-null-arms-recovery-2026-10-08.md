# Task6 Clippy NULL 分支修复（2026-10-08）

## 实际修改

合并 BufferedRowSet scalar_update 中显式 Null 和七类资源 None 的等价分支。没有新增 lint 放行，没有修改测试断言、资源更新能力或 SQL NULL 行为。

## 实际证据

- 原正式 workspace/all-targets/all-features Clippy exit101：match_same_arms。
- 修改后 `cargo test -p druid-rdbc --locked --offline` 实际 exit0：18结果块、474 PASS/0 FAIL/0 ignored，包含1 doctest。既有 buffered_resource_null_updates 测试通过。
- `cargo fmt --all -- --check` 实际 exit0。
- 同一正式 Clippy 重跑实际 exit101，原重复分支错误已消除；后续三个错误为 metadata unwrap_ref dead_code、Callable was_null 冗余闭包、Filter 链 collapsible_else_if。
- 输入快照为1092源码/58控制项，在 RDBC 编译期间取得，不能称为命令启动前冻结。相对 unused-import 冻结仅 buffered_row_set.rs 变化；测试终态逐项核对变化0。

机器结果保存在 `.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/` 的 task6-clippy-null-arms-rdbc-test 和 task6-workspace-clippy-null-arms-recheck summary/log 中，原失败日志继续保留。

Task6 保持 FIX_REQUIRED，Task7八项及全部18顶层任务仍开放。上述包测试通过不扩大为全仓通过。

## 后续修复与验证状态

NULL合并后的C1源码验证已实际exit0：479映射/442唯一选择器，executed=no，1092源码/58控制输入变化0；不构成新的完整runtime认证。

随后将Callable was_null冗余闭包替换为同一trait方法引用，并折叠Filter链等价else-if分支。没有改变读取次序、过滤器短路或错误分类。修改后的workspace fmt实际exit0。

两次测试命令误指定不存在的目标，均在执行测试前exit101，机器记录为COMMAND_ERROR，保留原日志；不得计为业务断言失败或通过。已改用Cargo列出的真实目标：druid lib、task6_strong_read_filter_contract_test、task6_result_set_filter_canonical_matrix_test、task6_standard_callable_statement_test_parity。原session71806/Cargo50828正在编译，尚未取得终态。metadata私有unwrap_ref仍待处理，未通过新增lint放行或删除原身份断言掩盖它。

## 最终观察更新

原session71806实际exit0：Druid lib150/0、ResultSet Filter矩阵1/0、Callable标准合同2/0、strong-read Filter合同2/0，共155PASS/0FAIL/0ignored。1092源码/58控制输入自编译期间快照至终态变化0。

正式Clippy再次实际exit101，原三个风格诊断均不再出现，仅报告metadata ResultSetMetaDataProxy::unwrap_ref dead_code，停止在Druid lib；不能推断后续targets无其他问题。metadata身份合同仍保留，无新增lint放行。当前没有本轮仍运行的Cargo或源码验证句柄。Task6仍FIX_REQUIRED/全部18OPEN。
