# Task 6：过期空闲连接失败核对与完整 C1 重跑

## 已核实的失败

完整 C1 runtime2 实际 exit1，子 C2 的 `druid-wrapper/task6_pool_deep_branch_coverage_test_parity_part_05` 为9 PASS/1 FAIL。失败发生于第532行 `assert_eq!(pool.state().idle_count, 1)`，实际0；不是物理关闭计数，也不是30ms等待后的借出断言。原始失败日志SHA为83d2c90e7f8e368fc03e6de9ea0658462215e2cba785d55f0dc43a926c32507e，保持原日志不变。

固定BASE `5afe8609b177544ff7d18461aa84f69efe8ee90e` 中的 `crates/druid-wrapper/tests/pool_deep_branch_coverage_test.rs#borrow_skips_lifetime_expired_idle` 与当前迁移函数保留相同10ms寿命、30ms等待和两个计数断言。

CodeGraph核对：holder使用`std::time::Instant`计算物理连接寿命；`PoolInner::return_connection` 在归还时对达到max_lifetime的holder执行discard。首次借出至归还的短窗口受调度影响是候选解释，未记录失败时的连接年龄，不能判定为已证明根因。

## 未修改输入的实际复查

原目标以all-features/locked/offline重新执行，实际exit0，10 PASS/0 FAIL/0 ignored/0 filtered out。日志为 `.superpowers/sdd/2026-08-24-rdbc-single-standard-unification/task6-lifetime-idle-recheck-2026-10-08.log`，同名summary保存SHA和验收范围。未修改Rust源码、原断言、超时或迁移分母；不能把这次通过记为已修复原失败或完成C1。

逐项复核runtime2冻结的1092源码输入和58工具/TSV输入，变化均为0。使用相同输入建立runtime3冻结快照，并启动默认完整 `scripts/verify_task6_method_parity.py`，保留完整C2子验收、479方法映射和全部行为目标，不使用no-execute。

## 完成边界

runtime2为终态FAIL；此前C2 runtime5完整PASS仍保留其独立运行范围。runtime3只有取得真实终态后才能判定。Task6 FIX_REQUIRED、Task7八项义务及全部18顶层任务继续OPEN。compiler/API/SemVer/MSRV/features/新鲜全仓回归及最终审查均未因此关闭。

## 活跃进程确认

2026-10-08T03:21:47：session50617继续运行；C1 Python41380（03:17:44启动）→完整C2 Python45128（03:19:21启动，-X utf8）→Cargo45044（03:21:28启动）。进程枚举确认父子关系，未观察到终态。等待期间1092源码和58工具输入复核变化0，文档diff检查exit0；未启动第二个Cargo。

2026-10-08T03:26:05：继续同一session50617的有界等待，未取得终态。实际观察C1 Python41380→C2 Python45128→external-consumer Cargo45760→test49876，嵌套check26288结束后推进到run32476和rustc44700。1092源码/58工具输入复核变化0；这是活跃进程等待证据，不能记为完整PASS。
