# Task 6 SQLx 类型、Prepared 复用、关闭与事务合同恢复

规格事实源：ADR-RDBC-006；计划：P04 Task6。
BASE：5afe860:crates/druid-wrapper/tests/sqlx_connection_adapter_test.rs。

## 完整恢复

- exec_fetch_and_type_mapping：driver名、真实int/string/blob/float/bool五列、affected1/insertid1、COUNT运行时类型、两个alias标签、大小写不敏感label查找、当前getter同一游标和关闭状态、零行两个标签。
- executes_and_reuses_real_prepared_statements：两次真实Prepared参数更新、创建/hit/miss/current四项原始统计、按id排序两行完整值。
- exposes_parallel_physical_and_standard_contracts：旧并行trait编译约束收敛唯一standard Connection/Factory，保留原始adapter/factory可实现性，不重开Physical层。
- rejects_a_closed_prepared_statement_handle：真实raw SQLx Prepared close后query拒绝，补充closed和重复close验证。
- rejects_unsafe_savepoint_names：真实池化事务拒绝bad;DROP_TABLE，随后rollback。
- transaction_and_savepoint_semantics：insert1/savepoint/insert2/rollback_to/release/commit只保留id1，再insert3/rollback仍只保留id1。

每项均有独立literal match arm，全部生产路径通过SQLx SQLite或DruidPool。类型、复用、关闭、事务还有独立运行测试；原通用循环仍只用于最后未恢复的资源用例，不计为该用例已完成。

Ruling: BASE current ResultSet raw指针相同，映射为标准接口共享cursor位置/跨视图value/关闭状态：原句柄推进后当前视图直接读到同一行，关闭原句柄后alias closed且拒绝next。保留真实游标身份语义，不暴露raw指针/私有owner接口。新增断言加强行为证据，不以重复assert填充原始门槛。

## 验证

C2实际静态RED依次缺失type_mapping、reuse显式arm；恢复后最终C2 no-executeFAIL推进唯一剩余sqlx_prepared_resources_execute_and_batch_against_real_sqlite。未改verifier或1283分母。

目标runtime依次3/3、4/4、最终6/6PASS，日志task6-sqlx-type-mapping-runtime.log / task6-sqlx-type-reuse-final.log / task6-sqlx-type-reuse-transaction-final.log。无本轮生产代码修改，不声称产品行为RED。完整BASE断言、数据和事务步骤恢复；fmt/diff通过。

此前完整workspace session42796终态exit0 /235blocks /3641PASS /0FAIL /0ignored incl2doctests，属于本轮新测试前源码。新测试已目标验证，不重复声称当前全workspace3641。

下一项SQLx资源完整BASE要求同步setter消费与错误、富描述符Filter、query/generic/batch全部真实数据；当前SQLx延迟绑定可能与原始合同冲突，必须完整恢复测试取得运行证据，不能借用Toasty用例或删减时间要求。

Task6FIX_REQUIRED；C1/C2/API/compiler/SemVer/features/MSRV/完整复审及Task7-8/P05-P07仍未完成。
