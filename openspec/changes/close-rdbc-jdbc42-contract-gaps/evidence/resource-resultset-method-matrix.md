# 资源与结果集方法矩阵（8.6）

本矩阵限定于 MC-06 的资源/结果集高风险合同，不代表所有 JDBC 方法或所有后端实现。`合同通过` 表示标准层或 Druid owner 的可观测行为已有代码和测试；`精确不支持` 表示能力未声明时标准层返回对应操作的 `SQLFeatureNotSupportedException` / SQLState `0A000`；`NOT_RUN` 表示本任务没有运行真实数据库适配器专项验证。

| 能力/方法身份 | 标准入口与能力位 | 关闭/所有权语义 | 当前分类与可追溯证据 |
|---|---|---|---|
| `Array#baseTypeName`, `baseType` | `Array`; `READ` | Array 由连接资源 owner 注册；free 后方法拒绝 | 合同通过；`resource_standard_support/array_contract.rs` 的 capability gate 覆盖调用前拒绝、释放后拒绝 |
| `Array#getArray`, `getArray(Map)` | `Array`; `READ` / `TYPE_MAP` | 值读取不释放 Array | 合同通过；同上全能力组合测试 |
| `Array#getResultSet`, `getResultSet(Map)` | `Array`; `RESULT_SET` / `TYPE_MAP` | 返回的游标是独立 owner 子资源；父 Array/owner 关闭会清理游标 | 合同通过；`resource_owner_cancellation_test.rs` 中 `task7_array_cursor_driver_fatal_error_discards_actual_owner_once`、`task7_array_parent_close_during_cursor_creation_drains_rejected_admission` 和 `task7_array_value_identity_maps_ranges_and_raw_cursor_ownership` |
| `Array#free` | `Array`; `FREE` | 幂等；错误/取消允许重试；通过共享 owner coordinator 执行一次底层释放 | 合同通过；`array_contract.rs` 的 failed-free retry / cancelled-free retry / concurrent clone free；`resource_owner_cancellation_test.rs` 的 pooled free/parent close race |
| `Blob#length`, `getBytes`, `position`, `truncate` | `Blob`; `READ`, `SEARCH`, `TRUNCATE` | free 后操作拒绝 | 合同通过；`resource_standard_support/blob_contract.rs` capability 和生命周期测试 |
| `Blob#setBytes`, `getBinaryStream`, `setBinaryStream` | `Blob`; `WRITE`, `STREAM` | 返回流的关闭责任由调用方持有，Blob 仍由 owner 注册 | 合同通过；Blob standard-support 用例；SQLx/具体数据库资源能力 `NOT_RUN` |
| `Blob#free`; `Clob#free`; `NClob#free` | `FREE` | 幂等释放；共享 clone 调用只释放底层一次 | 合同通过；Blob/Clob standard-support；NClob 复用 Clob 合同，`clob_contract.rs` 检查 national 类型能力 |
| `Clob#length`, `getSubString`, `position`, `truncate` | `Clob`; `READ`, `SEARCH`, `TRUNCATE` | free 后操作拒绝 | 合同通过；`resource_standard_support/clob_contract.rs` |
| `Clob#setString`, `getCharacterStream`, `setCharacterStream` | `Clob`; `WRITE`, `STREAM` | 字符流独立交付，Clob 由资源 owner 管理 | 合同通过；Clob standard-support；具体 DB 资源映射 `NOT_RUN` |
| `SQLXML#getString`, `getBinaryStream`, `getCharacterStream`, `getSource` | `SqlXml`; `READ`, `STREAM` | 读取转换状态受标准句柄管理；不支持的表示保留驱动错误 | 合同通过；`resource_standard_support/sql_xml_contract.rs` 全能力/能力掩码合同 |
| `SQLXML#setString`, `setBinaryStream`, `setCharacterStream`, `setResult` | `SqlXml`; `WRITE`, `STREAM` | 写入完成后方可绑定；validate 不隐式消费/关闭调用方句柄 | 合同通过；`standard_sql_xml_binding_contract_test.rs` 与 SQLXML standard-support |
| `SQLXML#free` | `SqlXml`; `FREE` | 幂等；释放失败/取消保留可重试状态 | 合同通过；`sql_xml_contract.rs` 的 error retry / cancellation / concurrent free |
| `SQLData#getSQLTypeName`, `readSQL`, `writeSQL` | `SqlData`; 由类型映射选择，不提供资源 capability 位 | 应用对象按 SQL 属性顺序读写；不归属结果集游标 | 合同通过；`standard_sql_mapping_contract_test.rs` 的成功、失败与流式映射用例；真实数据库 UDT 映射 `NOT_RUN` |
| `RowSet#execute`, `acceptChanges`, `getOriginal`, `getOriginalRow` | `RowSet` + `RowSetInternal`; provider/reader/writer 扩展合同 | 外部传入 Connection 是借用，不取得关闭权；缓冲游标归 RowSet | 合同通过；`standard_row_set_complete_contract_test.rs`、`standard_row_set_external_wrapper_contract_test.rs`；外部 JDBC RowSet provider `NOT_RUN` |
| `ResultSet#next`、EOF、`close`、`isClosed` | `ResultSet` | EOF 与显式关闭均为终态；close 错误保持原错误，owner 关闭清理活动游标 | 合同通过；`result_set_stream_test.rs` EOF/close-error tests，`task7_shared_result_set_contract_test.rs` close error retry，`druid/tests/task7_pooled_async_result_set_test.rs` owner/statement cascade |
| `ResultSet#known_row_count`（Rust 扩展） | 不属于 JDBC update count；仅可报告已知缓冲总行数 | 必须是同步、非消费、不得推进游标；流式未知返回 `None` | 合同定义通过；缓冲路径 `generic_execute_row_count_test.rs`；流式默认 `None` 的显式无消费测试列入 8.7 |
| 未声明的 Array/LOB/SQLXML 可选操作 | capability bit 不存在 | 必须在访问底层 driver 前返回当前操作的精确 unsupported 错误 | 精确不支持；Array capability gate 在 `array_contract.rs` 逐位覆盖；SQLXML 能力掩码测试在 `sql_xml_contract.rs` |
| SQLx/Toasty/其他具体数据库资源能力 | 适配器自己的 `capabilities()` 和真实驱动句柄 | 不从标准 trait 或类型存在推断后端可用；需逐 backend 验证 | `NOT_RUN`：8.6 未运行数据库专项命令；不影响上列标准合同结果 |

## 检查结论

- `ResourceOperations` 是运行时能力来源；`StandardResourceValue` 在下发到底层 trait 前先校验位，不支持的操作使用操作名构造 `0A000`。接口声明不能提升能力位。
- Array/Blob/Clob/SQLXML 的释放错误和取消均由共享资源状态管理，owner 的关闭/回收路径控制实际释放；Array 生成的 ResultSet 作为注册子资源随 owner 清理。
- 缓冲结果集可以报告固定行数；标准流式 ResultSet 的默认值为 `None`。SQLx 当前具体实现名为 `SqlxBufferedResultSet`，查询完整读入内存后提供缓冲行数，不承诺网络流式。
- 本矩阵不把 SQLData 标准映射测试等同于数据库 UDT 集成，不把 RowSet Rust provider 等同于 `javax.sql.RowSet` provider，也不推断任一数据库支持所有 LOB/Array/SQLXML 方法。

来源：当前标准接口/资源 wrapper/owner 源码与矩阵列出的测试；测试命令与退出码在 8.8 证据中汇总。数据库专项未运行项保留 `NOT_RUN`。

## 本项已运行的合同验证

- `cargo test -p druid-rdbc --locked --offline --test resource_standard_contract_test --test standard_sql_xml_binding_contract_test --test standard_row_set_complete_contract_test --test result_set_stream_test --test task7_shared_result_set_contract_test`：退出码 0，65 tests passed。
- `cargo test -p druid --locked --offline --test task7_pooled_async_result_set_test`：退出码 0，5 tests passed。
- `cargo test -p druid-wrapper --locked --offline --features "sqlx toasty-sqlite" --test generic_execute_row_count_test`：退出码 0，16 tests passed。
- `cargo test -p druid-wrapper --locked --offline --features "sqlx toasty-sqlite" --test async_result_set_test`：退出码 0，15 tests passed。
- 未运行真实数据库 Array/LOB/SQLXML/SQLData/RowSet 专项；后端适配状态仍为 `NOT_RUN`。
