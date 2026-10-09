# Task 6 getter 驱动错误迁移（2026-10-05）

状态：PARTIAL / FIX_REQUIRED。总目标仍为完整 Task 6–8 与 P05–P07。

## 完整恢复范围

从 BASE `5afe860` 的 `prepared_statement_semantics_test.rs` 恢复四个 getter 失败分支：成功 query 后 get_result_set；成功 update 后 get_update_count/generated_keys；成功 query/get_result_set 后 get_more_results，且原结果集立即关闭。四个独立测试分别检查真实池化代理的原 SQL 异常、sorter 收到原异常恰好一次、逻辑连接失效、租约最终归还、池 discard_count 恰好一次，以及 shutdown 后 raw connection close 恰好一次。完整 case 已加入 canonical matrix 的显式分支，未降低断言或修改验证器。

## 实际证据

- `task6-getter-errors-red.log`：实际运行时 RED，0 PASS / 4 FAIL，exit101。三个逻辑 getter 跳过 raw 方法；generated_keys 返回原错误但不使连接失效。
- `task6-getter-errors-intermediate.log`：初步 generated_keys 分类已使连接失效，但额外 raw-close 断言置于提交关闭任务后、worker 完成前，测试仍失败。该断言不是 BASE 要求；按既有 close worker 合同移至 pool shutdown 完成后，仍保留两次 connection close 的池计数检查。没有修改生产关闭时序，也没有以延时或放宽计数绕过错误。
- `task6-getter-errors-three-paths.log`：3 PASS / 1 FAIL，exit101。生成键、计数、推进结果的 getter 错误已通过分类与丢弃合同；当前结果 getter 仍失败。
- `task6-getter-wrapper-regression.log`：完整 wrapper 命令实际失败于 cached_auto_statement_has_no_results_from_previous_logical_lease，23/24 generated-keys；新 getter 读取了前一缓存租约的 raw update count。
- 修复逻辑租约的结果可见性后，`task6-getter-result-regressions.log` exit0：generated keys24/24、generic execute16/16、current result2/2，共42 PASS。只证明这些目标合同，不是完整 workspace GREEN。
- `task6-getter-c2.log`：no-execute仍FAIL，静态显式绑定越过 getter arm，下一缺口 prepared_result_set_keeps_the_same_statement_alive_and_can_close_it。getter arm 自身运行尚未全部通过；静态前进不计为完整迁移完成。
- purity final STANDARD_FILES43 PASS；authority79 / Task7 pending25 PASS，SHA16b2e8a669dc4ab41c37933a035cb0066f25cb98553386615b409831300f9cbc；三文件 rustfmt 与 default git diff check PASS。
- `task6-getter-workspace-regression.log`：完整 workspace default locked/offline serial --no-fail-fast 已启动，session84666。尚未终态，不能声称当前源码完整回归通过。此前3626/0属于前一源码。

## 生产变化与剩余问题

Prepared generated_keys 的驱动结果接入现有 classify_statement_mutation，保留原异常并使缓存不可复用。基本 Statement 的 update-count/more-results getter 调用 raw 方法并分类真实错误，只按现有精确 UnsupportedOperation 默认允许逻辑缓存替代；逻辑混合序列仍决定当前结果的类型/计数。Prepared 在新租约未执行时隐藏缓存 raw handle 的旧计数。

当前 get_result_set 尚未修复。它的标准接口返回 owned cursor，而已登记逻辑游标需要稳定的共享身份。下一步必须解决 driver getter 检查与 owned cursor 交接，不能仅丢弃 raw Some、重复登记或覆盖混合序列来让错误 fixture 通过。需要对成功 owned handoff、重复 getter、Filter open 次数、cursor/close 共享和完整 ordered sequence 同时给出证据。

Ruling：保留四个实际失败合同和中间回归证据；三条已验证路径为部分进展，不缩减完整 getter/BASE/Task6 目标。未新增第二个 driver port、未安装工具、未切换分支、未 stage/commit/push/publish。当前 API/SemVer/all-features/MSRV/整体审查仍未验证。

Task 6 getter workspace terminal verification 2026-10-05: session84666 terminalexit101 /235 result blocks /3628 PASS /2 FAIL /0 ignored incl2doctests. Only failing target canonical prepared matrix: prepared_result_getter_driver_error_discards_connection and task6_prepared_statement_canonical_matrix fail at same get_result_set missing raw error. Admin entiregrpc46/46 PASS8.24s. No other observed failure with --no-fail-fast. Prior3626/0 is historical source, current workspace is RED. Afterward test-only fidelity strengthening restores BASE MySQL code1040/message/class and db_type_name mysql; sorter probe delegates actual MySqlExceptionSorter rather than unconditional fatal. Final canonical rerun LIVE session30121/task6-getter-mysql-final.log; no new production edits after workspace run. Current getter owned-handoff/shared-identity remains open. Full Task6–8/P05–P07 active, no completion claim.

Task 6 getter MySQL fidelity final 2026-10-05: session30121 intentionally terminated only verified testPID40900 after CodeGraph proved MySQL validation uses create_statement/execute_query while fixture lacked create_statement, causing acquisition retries. task6-getter-mysql-final.log abnormalexit0xffffffff is a setup failure, not behaviorRED or PASS. Restored BASE one-row query via canonical raw Statement/create_statement (test-only); no production validation/timeouts changed. Fresh task6-getter-mysql-validated.log terminalexit101 /5 PASS /2 FAIL: actual BASE MySQL1040+class+message and actual MySqlExceptionSorter delegation preserve generated-keys/count/more-results GREEN; current-result independent and canonical matrix retain same getterRED. Full workspace3628/2 covers unchanged production before this fixture-only strengthening; no latest test-source fullworkspace rerun claimed. Full scope remains active and Task6FIX_REQUIRED. All launched handles are terminal.
