# 资源与结果集生命周期验收（8.7）

本批仅为 MC-06 既定高风险合同补证并复用既有行为测试；没有扩展可选后端支持范围。目标缺口是流式 `known_row_count()` 缺少直接的非消费断言，另将 Array 能力缺失断言收紧到精确 SQLState 与方法身份。

## 本批测试改动

- `crates/druid-rdbc/tests/result_set_stream_test.rs`：在尚未转换为 Stream 前调用 `known_row_count()`，断言结果为 `None` 且底层 `next()` 计数仍为零；同一用例后续继续验证流式读取惰性和提前 drop 行为。
- `crates/druid-rdbc/tests/resource_standard_support/array_contract.rs`：32 种能力掩码下逐项验证 Array 的 10 个方法族；拒绝时断言异常类型为 `SQLFeatureNotSupportedException`、SQLState 为 `0A000`、消息保留对应 Java 方法/重载身份、底层 driver 未被调用。无 FREE 位时额外验证 `Array#free` 精确身份。
- 没有实现代码偏差需要修复；owner 关闭游标、结果集 EOF/显式关闭、关闭错误保真/重试、Blob/Clob/SQLXML 释放失败或取消重试的既有目标测试继续作为合同证据。

## 行为验证

- `cargo test -p druid-rdbc --locked --offline --test resource_standard_contract_test task7_array_value_all_capabilities_gate_before_driver_and_after_free`：退出码 0，1 passed。
- `cargo test -p druid-rdbc --locked --offline --test result_set_stream_test standard_trait_object_stream_is_lazy_send_and_drops_owned_cursor`：退出码 0，1 passed。
- 8.6 目标合同回归亦已通过：`resource_standard_contract_test` 32 passed，`standard_sql_xml_binding_contract_test` 8 passed，`standard_row_set_complete_contract_test` 14 passed，`result_set_stream_test` 5 passed，`task7_shared_result_set_contract_test` 6 passed，`task7_pooled_async_result_set_test` 5 passed，带 `sqlx toasty-sqlite` feature 的 `generic_execute_row_count_test` 16 passed 与 `async_result_set_test` 15 passed。对应完整命令记录在 [8.6 方法矩阵](resource-resultset-method-matrix.md#本项已运行的合同验证)。
- 真实数据库 Array/LOB/SQLXML/SQLData/RowSet 集成未运行，仍为 `NOT_RUN`；这些合同测试不替代真实后端能力验收。
