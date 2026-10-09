# Task 6 执行失败 Prepared 句柄与 Java 统计恢复

规格：docs/superpowers/specs/2026-08-24-rdbc-single-standard-trait-tree.md。
计划：P04 Task6。BASE：5afe860:crates/druid-wrapper/tests/prepared_statement_semantics_test.rs#statement_execution_error_removes_handle_instead_of_reusing_it。

## 保真要求与实现

原迁移 helper 将 BASE 当前缓存数 -1 改成0，并缺少不池化的可观察证明。完整恢复原来的三个统计：首次未入缓存的失败语句关闭后 cached=-1/delete=1/closed=1。通过标准接口关闭状态、关闭后执行拒绝、重复 close、raw close 一次，以及同 SQL 再次 prepare 原生创建事件两次来替代已删除的私有 holder.is_pooling。再次 prepare 执行成功，pool shutdown 后raw Prepared总关闭次数2。

生产关闭路径区分：已checkout缓存条目删除只计delete/close；新建未入缓存且因执行错误不cacheable的条目使用Java record_cache_delete无条件递减。缓存正常复用和失效租约路径保留既有规则。未修改 verifier、断言门槛或1283分母。

## 验证

- 实际运行RED：13 PASS/2 FAIL，实际cached=0 vsBASE期望-1；task6-execution-error-cache-red.log。
- 修复后canonical matrix15/15PASS，包括独立完整用例；task6-execution-error-cache-green.log。
- C2 no-execute仍FAIL，已越过完整Prepared组，下一缺失显式arm为task6_sqlx_canonical_matrix::rdbc_connection_contract；task6-execution-error-cache-c2.log。
- 两文件fmt、finalpurity43、authority79/pending25及git diff --check通过。
- 最新完整workspace默认locked/offline serial --no-fail-fast仍LIVE session97136；task6-execution-error-cache-workspace.log。不能将此前3638/0用于这次生产修复。

Task6FIX_REQUIRED；C1/C2/API/compiler/SemVer/features/MSRV/独立完整复审和Task7-8/P05-P07仍未完成。下一项BASE SQLx连接合同要求经过DruidPool调用完整共享合同，不允许仅用通用insert/query循环替代。

## 完整回归终态与旧测试纠正

session97136 终态exit101，235结果块，3638 PASS /1 FAIL /0 ignored，含2doctests。唯一失败为 task6_prepared_cache_lru_regression_test::failed_prepared_execution_never_returns_handle_to_cache，其首次失败语句cached=0旧断言与原始Java -1合同冲突。按BASE修正为-1，并新增已checkout缓存语句执行失败测试，证明checkout已递减，关闭不再次递减；重复关闭/rawclose一次。完整LRU/状态失效/失败组5/5PASS，task6-execution-error-cache-regression-final.log。该纠正不放宽原始行为验收。

随后迁移SQLx完整共享连接合同，最终源码完整workspace重跑LIVE42796，task6-sqlx-connection-contract-workspace.log；不能将旧3638/0声明为最终通过。

Task6 SQLx shared connection fullworkspace terminal 2026-10-05: session42796 exit0 /235blocks /3641PASS /0FAIL /0ignored incl2doctests, task6-sqlx-connection-contract-workspace.log. Supersedes LIVE42796; includes corrected fresh-failed Prepared -1, checkoutfailed no-doubledecrement, complete SQLx sharedconnection2/2. Subsequent SQLx type/reuse/closed/savepoint/transaction migration tests are covered by new target6/6, not yet a new fullworkspace claim.
