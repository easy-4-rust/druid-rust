# JDBC 方法证据基线（2026-11-02）

- 来源：`docs/verification/rdbc-first/java8-method-evidence.tsv`
- 冻结副本（3.1 时点）：`java8-method-evidence-baseline.tsv`
- 冻结副本 SHA-256：`cfa065c82c64ab58d794ac247a4debfe23fb193d27d30dc774ce4acf70052d6a`
- 身份分母：**2001**
- 头部 schema：`java8-method-evidence-v1`，来源 schema `jdbc-java-method-inventory-v1`

## 各维度当前状态

| 维度 | PASS | PARTIAL | UNSUPPORTED | UNASSESSED |
|---|---:|---:|---:|---:|
| capability | 78 | 790 | 50 | 1083 |
| declaration | 902 | 0 | 0 | 1099 |
| contract | 807 | 0 | 0 | 1194 |
| adapter | 156 | 0 | 78 N/A | 1767 |
| logical_proxy | 317 | 0 | 78 N/A | 1606 |
| real_db | 59 | 0 | 78 N/A | 1864 |

## 约束

- 该台账是唯一方法证据事实源，不新建平行清单。
- 未评估项保持 UNASSESSED；不得由同名类型、编译成功或局部测试升级。
- `real_db` 必须在真实数据库运行后填写，不能用 SQLite/mock 代替其他产品。
- 后续每批更新后重新计算 SHA，并记录对应 Java/Rust 基线与命令。

## 2026-11-02 Statement 首批回填

- 当前 ledger SHA-256：`fb35378c34d1f2f1e5f71280dcd92236327d21262d65133d83a09e6e6bbdddb1`
- 新增：30 个 `java.sql.Statement` 方法 + `CallableStatement#wasNull()` + `ResultSet` 的 `previous/getRow/getFetchSize/wasNull` + 22 个 `java.sql.PreparedStatement` 属性/生命周期方法，关联 `java_rust_statement_errors_differential.rs#pinned_java_and_rust_compare_all_37_statement_inputs`。
- 结果：capability PARTIAL 81 / UNASSESSED 1920；declaration PASS 65、contract PASS 70、logical_proxy PASS 9、adapter PASS 2、real_db 仍 0/2001。
- 门禁：`python scripts/verify_java8_method_evidence.py` → `EVIDENCE_INTEGRITY_PASS linked=81 unassessed=1920`。
- 边界：只登记 live error differential 可支撑的 declaration/contract；adapter、logical_proxy、real_db 未评估，3.2–3.7 未完成。

## 2026-11-02 Connection 批次回填

- 当前 ledger SHA-256：`2532ee6b186b0ba3c950f9457b8463a057f6402f990509ea34dd5995a384565c`
- 新增：52 个 `java.sql.Connection` 方法，声明层全部 PASS；49 个方法由现有 SQLite/池化/资源合同测试支撑 `contract`；14 个方法由 JDBC Agent H2/HSQLDB 真实运行支撑 `real_db`（driver-matrix run `41410074184`，commit `95be0d6`）。
- 结果：capability PARTIAL 133 / UNASSESSED 1868；declaration PASS 117、contract PASS 119、adapter PASS 2、logical_proxy PASS 9、real_db PASS 14。
- 真实库覆盖：`getAutoCommit/setAutoCommit`、`commit/rollback/rollback(Savepoint)`、`setSavepoint(String)/releaseSavepoint`、`getCatalog/setCatalog`、`getSchema/setSchema`、`getMetaData`、`prepareStatement(String)`、`close`。
- 边界：`isValid` 与两个带结果集参数的 `prepareCall` 重载只有声明证据；`getHoldability/setHoldability`、`getNetworkTimeout/setNetworkTimeout`、`getTypeMap/setTypeMap`、`nativeSQL`、`setReadOnly` 只有默认 trait 合同证据；adapter/logical_proxy 仍未评估，3.2 未完成。
- 门禁：`python scripts/verify_java8_method_evidence.py` → `EVIDENCE_INTEGRITY_PASS linked=133 unassessed=1868`；13 个 verifier 单元测试通过。

## 2026-11-02 PreparedStatement 第二批回填

- 当前 ledger SHA-256：`b83955dad53d175bac2cc476ff44f20861cdf741b8187ae1c47f0d9236112b77`
- 新增：56 个 `java.sql.PreparedStatement` 方法，覆盖 41 个 typed setter、`clearParameters/addBatch/clearBatch/executeBatch/executeLargeBatch`、`executeQuery/executeUpdate/executeLargeUpdate`、`getGeneratedKeys/getMoreResults/getUpdateCount/isClosed`、`isWrapperFor/unwrap`、`getParameterMetaData`。
- 结果：capability PARTIAL 189 / UNASSESSED 1812；declaration PASS 173、contract PASS 175、adapter PASS 2、logical_proxy PASS 9、real_db 14。
- 边界：`setBigDecimal`、`setArray`、`setBlob(Blob)`、`setClob(Clob)`、`setNClob(NClob)`、`setRef`、`setRowId`、`setSQLXML`、`setURL`、`getMetaData/getConnection/getLargeMaxRows/setLargeMaxRows/getLargeUpdateCount` 以及继承的字面 SQL 重载仍 UNASSESSED；本批 setter 通过 `set_input_parameter` 的 typed 变体验证，不宣称 Java 重载逐一经过 adapter。
- 门禁：`python scripts/verify_java8_method_evidence.py` → `EVIDENCE_INTEGRITY_PASS linked=189 unassessed=1812`；13 个 verifier 单元测试通过。

## 2026-11-02 不支持能力负向证据

- 当前 ledger SHA-256：`59b9119660be85b9a9026d5395d7f91c05957be140056122add02fcf744cb236`
- 新增负向测试 `unsupported_interface_negative_test.rs`，登记 4 个 `UNSUPPORTED` 身份：`SQLInput#readObject(Class)`、`SQLOutput#writeObject(Object, SQLType)`、`RowSet#updateBlob(int, InputStream)`、`RowSet#updateBlob(int, InputStream, long)`。
- 精确观测：SQLState `0A000`、class `java.sql.SQLFeatureNotSupportedException`、message 分别包含 `SQLInput#readObject(Class)`、`SQLOutput#writeObject(Object, SQLType)`、`row_set_lob_stream_update`；不是仅以 0A000 泛化声明。
- 结果：linked 193 / unassessed 1808；capability PARTIAL 189 + UNSUPPORTED 4；declaration PASS 177、contract PASS 175、adapter PASS 2、logical_proxy PASS 9、real_db PASS 14。
- 边界：RowSet 其余 319 个身份仍 UNASSESSED，不能因 2 个流更新负例推断整个 RowSet 不支持；SQLInput/SQLOutput 的已实现方法仍 UNASSESSED。
- 门禁：`python scripts/verify_java8_method_evidence.py` → `EVIDENCE_INTEGRITY_PASS linked=193 unassessed=1808`；`unsupported_interface_negative_test` 3 passed。

## 2026-11-02 多智能体批次（Callable/Resource/ResultSet/Statement/Metadata）

- 当前 ledger SHA-256：`6988bbccffabd58589241cf7a3deb14ab9b5e30d911c57f0d9058defc02b871d`
- 新增 **272** 个方法的证据：79 个 `CallableStatement`、87 个 Blob/Clob/NClob/Array/Struct/Ref/SQLXML/RowId/SQLInput/SQLOutput、63 个 ResultSet/Statement、43 个 DatabaseMetaData/ResultSetMetaData/ParameterMetaData；另回填 120 条 adapter/logical_proxy 层证据（46 adapter + 74 logical_proxy）。
- 结果：linked **465** / unassessed **1536**；capability PARTIAL 461 + UNSUPPORTED 4；declaration PASS 449、contract PASS 447、adapter PASS 151、logical_proxy PASS 270、real_db PASS 54。
- 真实库证据：Blob/Clob/NClob/Array 外部消费者测试与 Callable H2/HSQLDB live 测试逐方法回填 40 个 `real_db`，均来自实际 H2/HSQLDB 运行；Array H2 测试本次本机实跑 2 passed。
- 证据边界：metadata 的 27 个 routing-only 与 10 个常量行未回填，Callable 的 `getBoolean(int)` 弱证据行、ResultSet 常量行、资源 `TYPE`/`RowId#toString` 行均被排除，不以路由或 Debug 表示冒充 Java 行为；Toasty SQLite 未被当作资源类真实库证据。
- 门禁：`python scripts/verify_java8_method_evidence.py` → `EVIDENCE_INTEGRITY_PASS linked=465 unassessed=1536`；Callable/Resource/Metadata/ResultSet 目标测试均通过。

## 2026-11-02 Callable getter 追加批次

- 当前 ledger SHA-256：`49717a31799ba0f8b07c6111af0218136a0ffa8c07603095770217a7fd042d4b`
- 新增 36 个 `CallableStatement#getXxx` 方法：标量 index/named getter、BigDecimal（含 scale）、Date/Time/Timestamp（含 Calendar）、Blob/Clob/NClob/CharacterStream/NCharacterStream/NString/URL/Ref/Array/RowId/SQLXML。
- 证据统一为 `task6_callable_canonical_matrix_test.rs#task6_prepare_call_overloads_preserve_keys_delegation_and_cache_lifecycle` 的真实 out 值断言，`contract` 与 `logical_proxy` 同时 PASS。
- 结果：linked **501** / unassessed **1500**；capability PARTIAL 497 + UNSUPPORTED 4；declaration PASS 485、contract PASS 483、adapter PASS 151、logical_proxy PASS 306、real_db PASS 54。
- 仍未回填：`getBoolean(int)`、`getBytes(int)`、`getDouble(int)`、`getFloat(int)`、`getShort(String)`、`getString(int)` 等无值级断言的重载，以及 index/named 之外的继承 Statement 方法。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=501 unassessed=1500`；`task6_callable_canonical_matrix_test` 10 passed。

## 2026-11-03 XA / DataSource / 事件 / RowSet SPI 批次

- 当前 ledger SHA-256：`79fdfa98c93f2d9513579b32bd418523d1ccfc05fa2f9239189747cf57575095`
- 新增 35 个 `javax.sql` 方法：PooledConnection 获取与连接事件监听、DataSource 获取与 Wrapper、四类 DataSource 家族的 log writer/parent logger、XAConnection#getXAResource、ConnectionEvent/StatementEvent 构造与 getter、RowSetInternal/Reader/Writer/Listener。
- 真实库：`jdbc_agent_xa_external_consumer` 的 H2 XA 场景本机实跑通过（含 `external_pool_returns_logical_lease_and_reuses_live_xa_owner`），为 PooledConnection 获取/监听与 XAConnection 获取/资源增加 4 个 `real_db` PASS；同文件 PostgreSQL 恢复用例因缺少 `POSTGRES_*` 环境前置失败，未计入证据。
- 结果：linked **536** / unassessed **1465**；capability PARTIAL 532 + UNSUPPORTED 4；declaration PASS 520、contract PASS 516、adapter PASS 155、logical_proxy 306、real_db PASS 58。
- 边界：`DataSource#getConnection(String,String)`、`XA#getXAConnection(String,String)` 仅默认 0A000，未回填；PooledConnection/XA 的 statement listener 无 trait 级测试；RowSetListener#rowChanged 无正向事件断言；XAConnection/XADataSource 的 close/TYPE 与 ConnectionPoolDataSource 获取仍未评估。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=536 unassessed=1465`；ledger 校验与 OpenSpec `--strict` 通过。

## 2026-11-03 java.sql.Types / JDBCType 常量批次

- 当前 ledger SHA-256：`975b399636f56cea78d71d9e4ed90e9c978bb5bfefed4d5e6c6bf0db1bf34d58`
- 新增 85 个身份：39 个 `java.sql.Types` 常量、39 个 `java.sql.JDBCType` 枚举常量、7 个 JDBCType/SQLType 方法（`getName/getVendor/getVendorTypeNumber` 各自两个继承入口 + `valueOf(int)`）。
- 常量没有 adapter/logical_proxy/real_db 运行时维度，登记为 `NOT_APPLICABLE`；capability 为 `SUPPORTED`，declaration/contract PASS，依据是 `canonical_types_module_preserves_all_java8_constants_and_existing_facades` 与 `canonical_jdbc_type_preserves_every_java8_name_vendor_and_number` 对全部 39 组名称/数值的精确断言。
- 结果：linked **621** / unassessed **1380**；capability SUPPORTED 78 + PARTIAL 539 + UNSUPPORTED 4；declaration PASS 605、contract PASS 601、adapter PASS 155、logical_proxy PASS 306、real_db PASS 58。
- 边界：`JDBCType#values()` 与 `valueOf(String)` 无 Rust 对应入口；两个 `#TYPE` 行保持未评估。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=918 unassessed=1083`；`standard_constant_exception_contract_test` 2 passed、`standard_value_ownership_test` 4 passed。

## 2026-11-03 SQLInput/SQLOutput 剩余标量批次

- 当前 ledger SHA-256：`8e8203a96a99122ee15164e47b7430d1d0fef537cb812fabc58133ca8c195973`
- 新增 34 个方法：17 个 `SQLInput#readXxx` 与 17 个 `SQLOutput#writeXxx`，证据为 `every_standard_input_reader_distinguishes_sql_null` / `every_standard_output_writer_retains_attribute_order_and_nulls`，覆盖 SQL NULL 零值/None、was_null 与单属性消费。
- 结果：linked **655** / unassessed **1346**；capability SUPPORTED 78 + PARTIAL 573 + UNSUPPORTED 4；declaration PASS 639、contract PASS 635。
- 边界：SQLInput/SQLOutput 仅剩 `TYPE` 未评估；两者仍无 adapter/real_db 证据（JDBC Agent 未实现 UDT 通道）。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=918 unassessed=1083`；`standard_sql_mapping_contract_test` 11 passed。

## 2026-11-03 RowSet setter/property 批次

- 当前 ledger SHA-256：`62f593a3c7b381f1e6ceb67d94e98d27863a3dcaa573d7ad4ad0c0f0937221ee`
- 新增 105 个 `javax.sql.RowSet` 方法：73 个固定参数 setter（`every_fixed_java_row_set_setter_preserves_its_family_and_overload`）、28 个属性 setter/getter（`canonical_row_set_properties_forward_all_requests_and_validate_atomically`）、`execute/clearParameters/addRowSetListener/removeRowSetListener`。
- 结果：linked **760** / unassessed **1241**；capability SUPPORTED 78 + PARTIAL 678 + UNSUPPORTED 4；declaration PASS 744、contract PASS 740；RowSet 覆盖 107/321。
- 边界：RowSet 继承的 ResultSet 取值/更新方法（`updateXxx`、`getXxx` 标签重载等）尚无 RowSet 类型上的断言；`RowSetMetaData` 44 个身份仍全部未评估；RowSet 无 adapter/real_db 证据。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=918 unassessed=1083`；`standard_row_set_full_surface_contract_test` 3 passed。

## 2026-11-03 RowSet 继承方法与 RowSetMetaData 批次

- 当前 ledger SHA-256：`679a584d70818f9f231e281a45a05b4ec5305069fd071e1b924edea8b4794942`
- 新增 48 个身份：12 个 RowSet 继承的 ResultSet 方法（next/getObject/getRow/isClosed/getMetaData/close/updateRow/insertRow/deleteRow/afterLast/moveToInsertRow/moveToCurrentRow），17 个 `RowSetMetaData` setter、17 个继承 getter、2 个 Wrapper 方法。
- 结果：linked **808** / unassessed **1193**；capability SUPPORTED 78 + PARTIAL 726 + UNSUPPORTED 4；declaration PASS 792、contract PASS 788；RowSet 覆盖 119/321、RowSetMetaData 36/44。
- 边界：RowSet 的 `absolute/relative/first/last/beforeFirst/isFirst/isLast/wasNull/findColumn` 等未在 RowSet 实例上断言；`getColumnClassName/isReadOnly/isWritable/isDefinitelyWritable` 与 3 个可空性常量仍 UNASSESSED；RowSet 无 adapter/real_db 证据。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=918 unassessed=1083`；`standard_row_set_metadata_contract_test` 5 passed。

## 2026-11-03 ResultSet 资源更新批次

- 当前 ledger SHA-256：`677b0976acd06ca5aa8cb62cabb09ede96b5d8475595a42076065aa5489722c8`
- 新增 7 个 `ResultSet#updateXxx(String, ...)` 方法（Array/Blob/Clob/NClob/Ref/RowId/SQLXML 的 NULL 更新），Java 重载在 Rust 中折叠为 `update_value(ResultSetUpdate::X(None))`，测试断言值变为 SQL NULL 且 `row_updated` 为真。
- 结果：linked **815** / unassessed **1186**；capability SUPPORTED 78 + PARTIAL 733 + UNSUPPORTED 4；declaration PASS 799、contract PASS 795。
- 边界：索引重载与流更新（`updateBlob(int,InputStream)` 等）只在标签/0A000 路径有观测，未登记；`updateNull/updateObject` 与其余标量 `updateXxx` 仍无测试。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=918 unassessed=1083`；`strong_getter_default_contract_test` 4 passed。

## 2026-11-03 Callable 继承方法批次

- 当前 ledger SHA-256：`9911642191c85ab139961ebe1707c1a945efaf1ba507fa3806d8340de6da111e`
- 新增 12 个 Callable 继承方法：`cancel/isClosed/getUpdateCount/getResultSet/setQueryTimeout/getMoreResults(两种)/isWrapperFor/unwrap/executeUpdate()/getWarnings/clearWarnings`。
- 证据：canonical matrix 的有序结果与 wrapper 断言、warning filter 顺序、pooled callable 关闭断言；`cancel()` 由 H2 live 测试断言返回错误，同时登记 adapter/real_db。
- 结果：linked **827** / unassessed **1174**；capability PARTIAL 745；contract PASS 807、adapter PASS 156、logical_proxy PASS 317、real_db PASS 59。
- 边界：Callable 剩余 108 个身份主要是索引 `setXxx(int,...)`、`addBatch/clearBatch/executeBatch`、字面 SQL 重载与若干属性读取，均无 Callable 类型上的断言。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=918 unassessed=1083`；`task6_sql_warning_semantics_test_parity` 12 passed。

## 2026-11-03 DatabaseMetaData 不支持能力批次

- 当前 ledger SHA-256：`7912773e5562f4b7504a24530ed76b00e48f66525e79dab01f635e1ff73d749b`
- 新增 46 个 `UNSUPPORTED`：全部来自 `task6_standard_database_meta_data_differential_test_parity.rs` 的 `*_unsupported` 用例，当前 SQLite 后端对这些必需 JDBC 元数据能力返回 `RdbcError::UnsupportedOperation`。
- 重要边界：这些是**迁移缺口**而非已批准的能力裁剪；测试只断言 `UnsupportedOperation` 变体，未断言 operation 名或 Java SQLState，因此逐行 `unsupported_result` 明确写明这一点。spec 中 UNSUPPORTED 的“精确 0A000”标准尚未满足，3.3 未完成。
- 结果：linked **873** / unassessed **1128**；capability SUPPORTED 78 + PARTIAL 745 + UNSUPPORTED 50；declaration PASS 857、contract PASS 853、adapter 156、logical_proxy 317、real_db 59。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=918 unassessed=1083`；该差分文件 67 passed。

## 2026-11-03 DriverManager 批次

- 当前 ledger SHA-256：`9143c00141f0b9020189d9f3be57ac758ac6a511e5d652cf1c06a4020e54d052`
- 新增 11 个 `java.sql.DriverManager` 方法：register/deregister、getDriver、三种 getConnection 重载、login timeout 与 log writer 读写。
- 证据：`standard_driver_action_test.rs` 的候选链、回调一次性、跨 manager 共享设置断言；全部为合同层，无真实产品数据库依赖。
- 结果：linked **884** / unassessed **1117**；capability PARTIAL 756；contract PASS 864。
- 边界：`getDrivers()`（Rust 有 `get_drivers` 但无测试调用）与弃用的 `getLogStream/setLogStream/println` 仍 UNASSESSED。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=918 unassessed=1083`；`standard_driver_action_test` 8 passed。

## 2026-11-03 新增测试：RowSet typed getters

- 当前 ledger SHA-256：`d14299589dbc6cf544ec7dd636a5008b6b0c87daea8e11cc268d5e0f1786a0b5`
- 新增测试 `crates/druid-rdbc/tests/row_set_typed_getters_contract_test.rs`（3 passed）：真实 `SharedRowSet` reader 发布 14 列 typed 值，断言索引与标签入口读同一值，并覆盖 Date/Time/Timestamp 的 unspecified/specified Calendar。
- 新增 34 个 RowSet 继承 getter 身份：boolean/byte/short/int/long/float/double/string/nString/bytes/bigDecimal 的 int+String 入口，以及三个时间类型的 4 种重载。
- 结果：linked **918** / unassessed **1083**；RowSet 覆盖 153/321；capability PARTIAL 790、SUPPORTED 78、UNSUPPORTED 50；contract PASS 898。
- 边界：RowSet 仍余 168 个身份，主要是 `updateXxx`、`getObject` 重载与导航谓词；本测试只覆盖 typed getter 面。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=918 unassessed=1083`；`row_set_typed_getters_contract_test` 3 passed。
