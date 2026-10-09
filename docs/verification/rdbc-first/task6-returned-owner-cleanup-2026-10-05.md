# Task 6 返回语句 owner 的关闭清理失败合同

规格事实源：docs/superpowers/specs/2026-08-24-rdbc-single-standard-trait-tree.md。
计划：P04 Task 6。BASE：5afe860 的 prepared_statement_semantics_test.rs。

恢复 clear_batch / clear_parameters 两个返回语句 owner 关闭失败的完整 helper、显式 canonical arm 和独立运行测试。保留真实 MySQL sorter 判定、1040 错误码、异常类名及原始消息；关闭返回错误时逻辑 is_closed=false，fatal 租约立即失效并拒绝执行。底层 raw 可以先安全关闭，但 registry 保留逻辑关闭完成责任；连接/pool 关闭后逻辑完成，重复 close 不重复 discard 或物理关闭。

生产修复分离逻辑关闭完成与租约有效性：Prepared inherent is_closed 读取逻辑完成状态，ensure_open 单独拒绝失效租约；清理失败不提前发布 finish_logical_close；raw 已取走时 trait is_closed 返回逻辑状态。未增加第二套公共接口。

验证：
- 实际 RED：11 PASS /3 FAIL，失败位于关闭返回错误后的 !handle.is_closed；task6-returned-owner-cleanup-red.log。
- 最终 canonical matrix 14/14 PASS；core cleanup regression 7/7 PASS。日志 task6-returned-owner-cleanup-final.log / task6-returned-owner-cleanup-green.log。
- 完整默认 workspace locked/offline serial --no-fail-fast：session23549 终态 exit0，235结果块，3638 PASS /0 FAIL /0 ignored，含2doctests。日志 task6-returned-owner-cleanup-workspace.log。
- final purity43、authority79/pending25、两文件fmt通过。
- C2 no-execute仍FAIL，推进到 statement_execution_error_removes_handle_instead_of_reusing_it 的原始断言门槛：base2/target1。未修改门槛或1283分母。

Task6仍FIX_REQUIRED；C1/C2/API/compiler/SemVer/features/MSRV/完整复审和 Task7-8/P05-P07未完成。下一步完整恢复执行错误后的缓存排除与Java统计合同。
