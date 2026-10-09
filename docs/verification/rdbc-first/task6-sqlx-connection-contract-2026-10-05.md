# Task 6 SQLx 完整共享连接合同恢复

规格：ADR-RDBC-006；计划：P04 Task6。
BASE：5afe860:crates/druid-wrapper/tests/sqlx_connection_adapter_test.rs#rdbc_connection_contract 与 common/physical_connection_contract.rs 全部共享合同。

## 范围

原canonical SQLx矩阵只对8个用例名称重复通用insert/query循环。本轮恢复rdbc_connection_contract的完整DruidPool建池/helper/显式literal match arm/独立测试，并迁移已有common/standard_connection_contract.rs全部行为到标准接口。

完整保留：Prepared参数绑定/真实更新数；owned live metadata产品名SQLite；幂等连接close/同池reacquire真实数据；真实SQLx结构化数据库错误与SQLite sorter；fatal租约立即失效，close后新物理数据库不含marker且可执行；显式close回滚dirty事务并恢复auto-commit；dirty Drop更换lease identity、恢复auto-commit并丢弃dirty表。

Ruling: BASE私有holder为空的即时fatal处置观察，迁移为canonical Connection.is_closed=true，重获后物理marker不存在证明真正丢弃。Druid当前保留holder供恰好一次关闭；不重开私有接口。旧exec(conn,params)/get_meta_data映射到owned Prepared set_parameter/execute_prepared_update、owned DatabaseMetaData异步标准方法；结果与所有原行为断言保留。parallel helper收敛唯一标准trait绑定。

## 验证

- C2初始静态RED为缺失rdbc_connection_contract显式arm，无运行时产品RED声称。
- 两次迁移编译失败分别记录metadata异步返回类型和可变borrow缺失；已修正，仅测试源码变化，无SQLx生产修复。
- 最终SQLx独立完整用例+canonical矩阵2/2PASS，task6-sqlx-connection-contract-green.log。
- 原if分支不是verifier要求的literal match arm，已改为显式match并再次2/2PASS，未改verifier。
- C2 no-execute仍FAIL，推进sqlx_adapter_exec_fetch_and_type_mapping显式arm；task6-sqlx-connection-contract-c2-final.log。
- Prepared缓存完整回归5/5PASS，包含新建失败-1和checkout失败0两种计数、LRU、catalog/schema失效。
- 三文件fmt、git diff --check通过。纯净度/authority上一轮已通过，本轮未改production或authority分母。
- 完整workspace默认locked/offline serial --no-fail-fast LIVE session42796，task6-sqlx-connection-contract-workspace.log。待取得终态。

Task6仍FIX_REQUIRED，完整C1/C2/API/compiler/SemVer/features/MSRV/独立复审和Task7-8/P05-P07保持未完成。SQLx其余7个BASE用例仍需各自完整迁移，不以通用循环充数。

Task6 SQLx shared connection fullworkspace terminal 2026-10-05: session42796 exit0 /235blocks /3641PASS /0FAIL /0ignored incl2doctests, task6-sqlx-connection-contract-workspace.log. Supersedes LIVE42796; includes corrected fresh-failed Prepared -1, checkoutfailed no-doubledecrement, complete SQLx sharedconnection2/2. Subsequent SQLx type/reuse/closed/savepoint/transaction migration tests are covered by new target6/6, not yet a new fullworkspace claim.
