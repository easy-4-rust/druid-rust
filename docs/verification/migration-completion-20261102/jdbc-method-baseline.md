# JDBC 方法证据基线（2026-11-02）

- 来源：`docs/verification/rdbc-first/java8-method-evidence.tsv`
- 冻结副本（3.1 时点）：`java8-method-evidence-baseline.tsv`
- 冻结副本 SHA-256：`cfa065c82c64ab58d794ac247a4debfe23fb193d27d30dc774ce4acf70052d6a`
- 身份分母：**2001**
- 头部 schema：`java8-method-evidence-v1`，来源 schema `jdbc-java-method-inventory-v1`

## 各维度当前状态

| 维度 | PASS | PARTIAL | UNSUPPORTED | UNASSESSED |
|---|---:|---:|---:|---:|
| capability | 196 | 1379 | 257 | 169 |
| declaration | 1816 | 0 | 0 | 185 |
| contract | 807 | 0 | 0 | 1194 |
| adapter | 156 | 0 | 142 N/A | 1703 |
| logical_proxy | 441 | 0 | 142 N/A | 1418 |
| real_db | 59 | 0 | 142 N/A | 1700 |

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
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=1832 unassessed=169`；`standard_constant_exception_contract_test` 2 passed、`standard_value_ownership_test` 4 passed。

## 2026-11-03 SQLInput/SQLOutput 剩余标量批次

- 当前 ledger SHA-256：`8e8203a96a99122ee15164e47b7430d1d0fef537cb812fabc58133ca8c195973`
- 新增 34 个方法：17 个 `SQLInput#readXxx` 与 17 个 `SQLOutput#writeXxx`，证据为 `every_standard_input_reader_distinguishes_sql_null` / `every_standard_output_writer_retains_attribute_order_and_nulls`，覆盖 SQL NULL 零值/None、was_null 与单属性消费。
- 结果：linked **655** / unassessed **1346**；capability SUPPORTED 78 + PARTIAL 573 + UNSUPPORTED 4；declaration PASS 639、contract PASS 635。
- 边界：SQLInput/SQLOutput 仅剩 `TYPE` 未评估；两者仍无 adapter/real_db 证据（JDBC Agent 未实现 UDT 通道）。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=1832 unassessed=169`；`standard_sql_mapping_contract_test` 11 passed。

## 2026-11-03 RowSet setter/property 批次

- 当前 ledger SHA-256：`62f593a3c7b381f1e6ceb67d94e98d27863a3dcaa573d7ad4ad0c0f0937221ee`
- 新增 105 个 `javax.sql.RowSet` 方法：73 个固定参数 setter（`every_fixed_java_row_set_setter_preserves_its_family_and_overload`）、28 个属性 setter/getter（`canonical_row_set_properties_forward_all_requests_and_validate_atomically`）、`execute/clearParameters/addRowSetListener/removeRowSetListener`。
- 结果：linked **760** / unassessed **1241**；capability SUPPORTED 78 + PARTIAL 678 + UNSUPPORTED 4；declaration PASS 744、contract PASS 740；RowSet 覆盖 107/321。
- 边界：RowSet 继承的 ResultSet 取值/更新方法（`updateXxx`、`getXxx` 标签重载等）尚无 RowSet 类型上的断言；`RowSetMetaData` 44 个身份仍全部未评估；RowSet 无 adapter/real_db 证据。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=1832 unassessed=169`；`standard_row_set_full_surface_contract_test` 3 passed。

## 2026-11-03 RowSet 继承方法与 RowSetMetaData 批次

- 当前 ledger SHA-256：`679a584d70818f9f231e281a45a05b4ec5305069fd071e1b924edea8b4794942`
- 新增 48 个身份：12 个 RowSet 继承的 ResultSet 方法（next/getObject/getRow/isClosed/getMetaData/close/updateRow/insertRow/deleteRow/afterLast/moveToInsertRow/moveToCurrentRow），17 个 `RowSetMetaData` setter、17 个继承 getter、2 个 Wrapper 方法。
- 结果：linked **808** / unassessed **1193**；capability SUPPORTED 78 + PARTIAL 726 + UNSUPPORTED 4；declaration PASS 792、contract PASS 788；RowSet 覆盖 119/321、RowSetMetaData 36/44。
- 边界：RowSet 的 `absolute/relative/first/last/beforeFirst/isFirst/isLast/wasNull/findColumn` 等未在 RowSet 实例上断言；`getColumnClassName/isReadOnly/isWritable/isDefinitelyWritable` 与 3 个可空性常量仍 UNASSESSED；RowSet 无 adapter/real_db 证据。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=1832 unassessed=169`；`standard_row_set_metadata_contract_test` 5 passed。

## 2026-11-03 ResultSet 资源更新批次

- 当前 ledger SHA-256：`677b0976acd06ca5aa8cb62cabb09ede96b5d8475595a42076065aa5489722c8`
- 新增 7 个 `ResultSet#updateXxx(String, ...)` 方法（Array/Blob/Clob/NClob/Ref/RowId/SQLXML 的 NULL 更新），Java 重载在 Rust 中折叠为 `update_value(ResultSetUpdate::X(None))`，测试断言值变为 SQL NULL 且 `row_updated` 为真。
- 结果：linked **815** / unassessed **1186**；capability SUPPORTED 78 + PARTIAL 733 + UNSUPPORTED 4；declaration PASS 799、contract PASS 795。
- 边界：索引重载与流更新（`updateBlob(int,InputStream)` 等）只在标签/0A000 路径有观测，未登记；`updateNull/updateObject` 与其余标量 `updateXxx` 仍无测试。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=1832 unassessed=169`；`strong_getter_default_contract_test` 4 passed。

## 2026-11-03 Callable 继承方法批次

- 当前 ledger SHA-256：`9911642191c85ab139961ebe1707c1a945efaf1ba507fa3806d8340de6da111e`
- 新增 12 个 Callable 继承方法：`cancel/isClosed/getUpdateCount/getResultSet/setQueryTimeout/getMoreResults(两种)/isWrapperFor/unwrap/executeUpdate()/getWarnings/clearWarnings`。
- 证据：canonical matrix 的有序结果与 wrapper 断言、warning filter 顺序、pooled callable 关闭断言；`cancel()` 由 H2 live 测试断言返回错误，同时登记 adapter/real_db。
- 结果：linked **827** / unassessed **1174**；capability PARTIAL 745；contract PASS 807、adapter PASS 156、logical_proxy PASS 317、real_db PASS 59。
- 边界：Callable 剩余 108 个身份主要是索引 `setXxx(int,...)`、`addBatch/clearBatch/executeBatch`、字面 SQL 重载与若干属性读取，均无 Callable 类型上的断言。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=1832 unassessed=169`；`task6_sql_warning_semantics_test_parity` 12 passed。

## 2026-11-03 DatabaseMetaData 不支持能力批次

- 当前 ledger SHA-256：`7912773e5562f4b7504a24530ed76b00e48f66525e79dab01f635e1ff73d749b`
- 新增 46 个 `UNSUPPORTED`：全部来自 `task6_standard_database_meta_data_differential_test_parity.rs` 的 `*_unsupported` 用例，当前 SQLite 后端对这些必需 JDBC 元数据能力返回 `RdbcError::UnsupportedOperation`。
- 重要边界：这些是**迁移缺口**而非已批准的能力裁剪；测试只断言 `UnsupportedOperation` 变体，未断言 operation 名或 Java SQLState，因此逐行 `unsupported_result` 明确写明这一点。spec 中 UNSUPPORTED 的“精确 0A000”标准尚未满足，3.3 未完成。
- 结果：linked **873** / unassessed **1128**；capability SUPPORTED 78 + PARTIAL 745 + UNSUPPORTED 50；declaration PASS 857、contract PASS 853、adapter 156、logical_proxy 317、real_db 59。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=1832 unassessed=169`；该差分文件 67 passed。

## 2026-11-03 DriverManager 批次

- 当前 ledger SHA-256：`9143c00141f0b9020189d9f3be57ac758ac6a511e5d652cf1c06a4020e54d052`
- 新增 11 个 `java.sql.DriverManager` 方法：register/deregister、getDriver、三种 getConnection 重载、login timeout 与 log writer 读写。
- 证据：`standard_driver_action_test.rs` 的候选链、回调一次性、跨 manager 共享设置断言；全部为合同层，无真实产品数据库依赖。
- 结果：linked **884** / unassessed **1117**；capability PARTIAL 756；contract PASS 864。
- 边界：`getDrivers()`（Rust 有 `get_drivers` 但无测试调用）与弃用的 `getLogStream/setLogStream/println` 仍 UNASSESSED。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=1832 unassessed=169`；`standard_driver_action_test` 8 passed。

## 2026-11-03 新增测试：RowSet typed getters

- 当前 ledger SHA-256：`d14299589dbc6cf544ec7dd636a5008b6b0c87daea8e11cc268d5e0f1786a0b5`
- 新增测试 `crates/druid-rdbc/tests/row_set_typed_getters_contract_test.rs`（3 passed）：真实 `SharedRowSet` reader 发布 14 列 typed 值，断言索引与标签入口读同一值，并覆盖 Date/Time/Timestamp 的 unspecified/specified Calendar。
- 新增 34 个 RowSet 继承 getter 身份：boolean/byte/short/int/long/float/double/string/nString/bytes/bigDecimal 的 int+String 入口，以及三个时间类型的 4 种重载。
- 结果：linked **918** / unassessed **1083**；RowSet 覆盖 153/321；capability PARTIAL 790、SUPPORTED 78、UNSUPPORTED 50；contract PASS 898。
- 边界：RowSet 仍余 168 个身份，主要是 `updateXxx`、`getObject` 重载与导航谓词；本测试只覆盖 typed getter 面。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=1832 unassessed=169`；`row_set_typed_getters_contract_test` 3 passed。

## 2026-11-03 新增测试：ResultSet 标量 updateXxx 族

- 当前 ledger SHA-256：`060a06ac917baf5c1920f865eddde6c9016a485cbcda8eb856e30dcdb6caca7f`
- 新增测试 `crates/druid-rdbc/tests/result_set_update_scalar_contract_test.rs`（2 passed）：在 `BufferedRowSet` 上逐族调用 `update_value`，索引与标签入口各写一次并断言存储值；`updateNull` 另断言 `was_null`。
- 新增 31 个 `ResultSet#updateXxx` 身份（16 个 family，含 int/String 重载；`updateObject` 仅 2 参数重载）。Java 重载在 Rust 中折叠为 `ResultSetUpdate` variant，notes 已写明。
- 结果：linked **949** / unassessed **1052**；ResultSet 覆盖 96/206；capability PARTIAL 821；contract PASS 929。
- 边界：`updateObject` 的 SQLType/scale 重载、流更新与 `updateRef/updateBlob(Some)` 等资源非 NULL 路径仍未覆盖。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=1832 unassessed=169`；新测试 2 passed。

## 2026-11-03 多智能体新增测试波次（导航/资源 setter/Callable 索引）

- 当前 ledger SHA-256：`525d1f0437af99c39b119bbd7d6e2a1ad74a8c1a430e5fe85ea4ffa54fc482bf`
- 新增 4 个测试文件、12 个测试函数、全部实跑通过：
  - `crates/druid-rdbc/tests/row_set_navigation_contract_test.rs`（3 passed，58 断言）：absolute/relative/first/last/beforeFirst/isFirst/isLast/isBeforeFirst/isAfterLast/findColumn；必须 `set_type(1004)` 才能滚动。
  - `crates/druid-rdbc/tests/result_set_label_getter_contract_test.rs`（3 passed）：11 个 `getXxx(String)` + Date/Time/Timestamp 的 String/Calendar 重载 + 未知标签错误。
  - `crates/druid/tests/prepared_resource_setter_contract_test.rs`（2 passed）：8 个资源 setter 的 NULL/非 NULL 载荷，经 DruidPooledPreparedStatement，contract+logical_proxy。
  - `crates/druid/tests/callable_indexed_setter_contract_test.rs`（4 passed）：19 个索引 setter variant、1-based 槽位证明、pooled proxy 证据。
- 新增 **53** 个身份（RowSet 10、ResultSet 16、PreparedStatement 8、CallableStatement 19；`afterLast/getRow` 与 1 个重复 identity 已存在，跳过）。
- 结果：linked **1002** / unassessed **999**；capability PARTIAL 874 + SUPPORTED 78 + UNSUPPORTED 50；declaration PASS 986、contract PASS 982、logical_proxy PASS 344。
- 边界：本轮只覆盖新增测试实际断言的身份；未断言的重载未登记。`real_db` 仍 59/2001。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=1832 unassessed=169`；4 个新测试 12 passed；两包 fmt check exit 0。

## 2026-11-03 RowSet getObject 面

- 当前 ledger SHA-256：`18679e4105c82e08f2282cb63f098fd86564acea07db17fafcc2008198a4751d`
- 新增测试 `crates/druid-rdbc/tests/row_set_object_getter_contract_test.rs`（2 passed）：SharedRowSet 上断言无类型与 typed 的 index/label 入口，以及 type-map 入口的精确默认 `UnsupportedOperation`（operation 名逐字断言）。
- 合入 5 个身份（getObject(int) 已登记）：getObject(String)、getObject(int,Class)、getObject(String,Class) 为 PARTIAL；getObject(int,Map)、getObject(String,Map) 为 UNSUPPORTED。
- 结果：linked **1007** / unassessed **994**；RowSet 覆盖 168/321；capability PARTIAL 877 + UNSUPPORTED 52。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=1832 unassessed=169`；新测试 2 passed。

## 2026-11-04 新增测试：Callable 流/资源 setter

- 当前 ledger SHA-256：`a378ab725f40f5129394f4eecf23c2ac8adebbf6437a8f8f46645726328a533f`
- 新增测试 `crates/druid/tests/callable_stream_setter_contract_test.rs`（2 passed，主会话手工实现，替代配额失败的两个智能体之一）：22 次索引绑定，覆盖 Ascii/Binary/Character/NCharacter 流的三种 length 形式、Blob/Clob/NClob 的 stream/reader/NULL handle、Object 的 SQLType 与 scale 重载。
- 断言 variant 种类、SQL type 与 length 字段（流句柄在代理层会被 clone，比较内容而非句柄 id）；流未被消费或关闭。
- 结果：linked **1029** / unassessed **972**；Callable 覆盖 172/239；capability PARTIAL 899、contract PASS 1009、logical_proxy PASS 366。
- 未能执行的计划：PreparedStatement 的 `getMetaData/getConnection/getLargeMaxRows/setLargeMaxRows/getLargeUpdateCount` 在 Rust 标准 trait 中没有入口；字面 SQL 的 `execute/executeUpdate/executeQuery/executeLargeUpdate` 重载也没有 PreparedStatement 句柄上的现有测试，需生产接口或新 mock 设计，本轮未登记。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=1832 unassessed=169`；新测试 2 passed。

## 2026-11-04 新增测试：RowSet 标量与资源更新

- 当前 ledger SHA-256：`e0f18fe11796f5d6434cbe36250445b51a218debfbea61325f031c8fa6d9128a`
- 新增 `crates/druid-rdbc/tests/row_set_update_contract_test.rs`（2 passed）：16 个标量 update family × 索引/标签入口，经 `update_value` + `update_row` + `accept_changes` 发布后读回，证明更新真正落盘；`after_last` 后更新报错。
- 新增 `crates/druid-rdbc/tests/row_set_resource_update_contract_test.rs`（3 passed）：7 个资源 NULL handle 的索引/标签更新发布 SQL NULL；Character/NCharacter reader 更新保存流值；Ascii/Binary/BlobStream/ClobReader/NClobReader 共 22 个重载逐项断言 `SQLState=0A000`、`java.sql.SQLFeatureNotSupportedException`、message 含 `row_set_lob_stream_update`。
- 结果：linked **1107** / unassessed **894**；RowSet 覆盖 246/321；capability PARTIAL 955 + UNSUPPORTED 74；contract PASS 1087。
- 边界：RowSet 剩余 75 个身份主要是 `updateObject` 的 SQLType/scale 重载与 `getStatement/getHoldability/getFetchDirection` 等未测入口。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=1832 unassessed=169`；两个新测试 5 passed。

## 2026-11-04 RowSet NULL/流 getter 与 ResultSet 常量

- 当前 ledger SHA-256：`b31003f2cfe769b9e3ad8c80c48b8da0f0a558a74cd8f1a5d339e4d29fd2e50d`
- 新增测试 `crates/druid-rdbc/tests/row_set_null_getter_contract_test.rs`（2 passed）：13 类 NULL 资源/流 getter 的索引与标签入口（26）、wasNull、warnings/clearWarnings、holdability、previous、Wrapper isWrapperFor/unwrap、getBigDecimal(scale) 两个重载。
- 新增 20 个 `ResultSet`/`RowSet` 常量身份：以 `statement_compat_constants_test.rs#pooled_statement_retains_public_jdbc_constants` 的精确数值断言为证据，三个运行时层 NOT_APPLICABLE、capability SUPPORTED。
- 结果：linked **1161** / unassessed **840**；capability SUPPORTED 98 + PARTIAL 989 + UNSUPPORTED 74；contract PASS 1141；RowSet 覆盖 291/321、ResultSet 131/206。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=1832 unassessed=169`；两个新测试 3 passed。

## 2026-11-04 RowSet 固定参数 setter 批次

- 当前 ledger SHA-256：`cdf78fd1a00e349765d8e5a86afe35b4ee2678d0db83f721951c2c94af10cf1e`
- 新增测试 `crates/druid-rdbc/tests/row_set_parameter_setter_contract_test.rs`（2 passed）：setNull(typeName) 两种、setArray/setBlob/setClob/setRef、setBytes 两种、setNString(by name)、setNCharacterStream(String,Reader,long)。
- 每个用例 `clear_parameters` 后调用并读取 `params()` 快照，断言 descriptor 家族与 by-name 解析槽位；NCharacterStream 绑定不消费 reader。
- 结果：linked **1173** / unassessed **828**；RowSet 覆盖 302/321；contract PASS 1153。
- 边界：RowSet 仍余 19 个身份（updateObject 6、row-state 5、fetch/holdability/statement 等）。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=1832 unassessed=169`；新测试 2 passed。

## 2026-11-04 多智能体波次：RowSet 状态 / Callable 属性 / Metadata 扫描 / 异常方法

- 当前 ledger SHA-256：`25669e3f1626c6d64aaf5af9bd834c7019472ed2e9263a99f257ec8614936117`
- 新增 4 个测试文件、21 个测试函数，全部实跑通过：
  - `row_set_state_property_contract_test.rs`（9 passed）：row state/fetch/holdability/updateObject SQLType+scale；`getStatement`、`refreshRow` 精确 UnsupportedOperation。
  - `callable_statement_property_contract_test.rs`（8 passed）：maxRows/fetchSize/resultSet 属性/parameterMetaData/addBatch/executeBatch；4 个可选入口记录精确 operation。
  - `database_meta_data_sweep_contract_test.rs`（2 passed）：116 个方法逐项观察，value 2（Wrapper）、unsupported 111（精确 operation 名）、no-entry 3。
  - 异常方法复用 `error_contract_test.rs`（9 passed）：SQLException/SQLWarning/BatchUpdateException 的 getter/setter 共 10 个。
- 结果：linked **1326** / unassessed **675**；capability PARTIAL 1041 + SUPPORTED 98 + UNSUPPORTED 187；declaration PASS 1310、contract PASS 1306。
- 重要边界：新增的 111 个 DatabaseMetaData UNSUPPORTED 是**迁移缺口**，观测为 Rust `UnsupportedOperation`（Java `UnsupportedOperationException`），不是 JDBC `SQLFeatureNotSupportedException`/0A000；`getConnection/getMaxLogicalLobSize/supportsRefCursors` 无 Rust 入口，保持 UNASSESSED。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=1832 unassessed=169`；4 个测试文件 19 passed。

## 2026-11-04 ResultSet 流更新负例批次

- 当前 ledger SHA-256：`a85139e3ebfeeaeff92b4e665165f8d9900e65e71c8a1d3706bb56c4c562b8fb`
- 新增测试 `result_set_stream_update_contract_test.rs`（1 passed，34 个 identity）与 `result_set_character_stream_update_negative_test.rs`（1 passed，6 个 identity）。
- 全部逐项断言 `SQLState=0A000`、`java.sql.SQLFeatureNotSupportedException`、message 含 `buffered_row_set_resource_update`，并确认失败后行值/行号/`row_updated` 不变。
- 关键发现：`BufferedRowSet` 连 `CharacterStream` 也拒绝（与 `SharedRowSet` 行为不同），因此 6 个 `updateCharacterStream` 身份按不支持登记。
- 结果：linked **1366** / unassessed **635**；capability PARTIAL 1041 + SUPPORTED 98 + UNSUPPORTED 227；contract PASS 1346。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=1832 unassessed=169`；两个测试 2 passed。

## 2026-11-04 异常构造器批次

- 当前 ledger SHA-256：`a03eebf625a2959df3e7626131d8bc9b3354ca77a8a6d9e19b117dacfbe822b5`
- 新增测试 `crates/druid-rdbc/tests/sql_exception_constructor_contract_test.rs`（62 passed），覆盖 SQLException/SQLDataException/SQLFeatureNotSupportedException/SQLIntegrityConstraintViolationException 各 8 个构造器、SQLWarning 8 个、BatchUpdateException 10 个。
- 每个用例重建 reason/state/vendorCode/cause/counts 并断言存储字段；回填 **51** 个构造器身份。
- 未登记：DataTruncation 2 个与 SQLClientInfoException 9 个——Rust 侧是 struct literal，没有可引用的构造 fn，不伪造入口。
- 结果：linked **1417** / unassessed **584**；capability PARTIAL 1092 + SUPPORTED 98 + UNSUPPORTED 227；contract PASS 1397。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=1832 unassessed=169`；构造器测试 62 passed。

## 2026-11-04 主会话收口：ResultSet/Prepared/Callable 剩余面

- 当前 ledger SHA-256：`5c8501d38df05af5604c90200ad4cc6b44aa85a51712d6dab06fa063e42a22ea`
- 新增 3 个测试文件、10 个测试函数：
  - `result_set_remaining_contract_test.rs`（4 passed）：标签 typed/NULL getter、before-first/first/last/after-last、fetch/并发/保留、Wrapper、type-map getObject 精确不支持、7 个 NULL 资源更新。
  - `prepared_literal_statement_contract_test.rs`（3 passed）：4 种 executeUpdate keys、4 种 execute keys、executeQuery、addBatch、getResultSet、getMetaData 精确 0A000、setBigDecimal。
  - `callable_literal_property_contract_test.rs`（3 passed）：4 种 executeUpdate/execute keys、executeQuery、addBatch/executeBatch/executeLargeBatch/executeLargeUpdate 折叠、clearParameters、Statement 属性回读、6 个资源 setter、5 个 OUT getter。
- 另回填 14 个 Prepared/Callable 继承常量（SUPPORTED）。
- 结果：linked **1519** / unassessed **482**；capability SUPPORTED 112 + PARTIAL 1177 + UNSUPPORTED 230；ResultSet 204/206、Prepared 109/118、Callable 225/239、RowSet 319/321、DatabaseMetaData 175/240。
- 边界：`executeLargeUpdate(String...)`、`getConnection`、`getLargeMaxRows/setLargeMaxRows/getLargeUpdateCount` 无 Rust 入口；DatabaseMetaData 61 常量与 3 方法无 Rust 入口；Timestamp/Date/Time/DataTruncation/SQLClientInfoException 构造器无独立 Rust 类型。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=1832 unassessed=169`；3 个新测试 10 passed。

## 2026-11-05 八智能体波次 + 主会话收口

- 当前 ledger SHA-256：`7fc502622c5d2b6eb53b8cdc7b6a381a9c203c747b3efa84927f9989c56961a2`
- 八智能体波次（3 完成 / 5 因 Token Plan 配额失败由主会话接手）：
  - SQLState 子类构造器 A（36 passed，48 identity）与 B（45 passed，40 identity）
  - SQLClientInfoException：16 测试 + 主会话新增 `SqlClientInfoException::new`/`failed_properties()` 生产入口，6 个构造器/`getFailedProperties` 身份收口
  - Metadata 常量模块（61 个 JDK8 精确值常量，3 passed）+ trait 3 个默认入口（4 passed，0A000 精确 operation）
  - `TimestampExt/DateExt/TimeExt` 扩展 trait（21 passed，44 identity；deprecated 访问器按 JDK8 抛 IllegalArgumentException）
  - DataTruncation：主会话新增 `new` + 5 个 getter 生产入口（4 passed，6 identity）
  - ResultSet 剩余标签/状态/资源更新（主会话 4 passed，42 identity）
  - Prepared/Callable 字面 SQL 与属性（主会话 6 passed，46 identity）+ 14 个 Statement 继承常量（SUPPORTED）
- 结果：linked **1726** / unassessed **275**；capability SUPPORTED 173 + PARTIAL 1320 + UNSUPPORTED 233；declaration PASS 1710、contract PASS 1706。
- 未登记（无 Rust 入口，诚实保留 UNASSESSED）：DataTruncation cause 构造器与异常/警告链继承方法、`TYPE` 标记、`getConnection`、Timestamp.valueOf(LocalDateTime) 等。子智能体生成的 metadata-sweep 111 个 UNSUPPORTED 均有逐项精确 operation 证据。
- 修复：metadata 常量计数测试原按 `ends_with("()")` 过滤方法，导致带参方法误入常量集合；已改为按 `contains('(')` 过滤。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=1832 unassessed=169`；两包 fmt check exit 0；druid-rdbc 全测试套件 ok。

## 2026-11-05 第二波八智能体 + 主会话收口

- 当前 ledger SHA-256：`0db94bad3a10eeaa672bfcde06ff4fb4453092d0c04eb6b8e8f7bdc1a36584e0`
- 第二波 8 智能体（2 完成/6 因配额失败由主会话接手）+ 主会话批次：
  - SQLState 子类构造器 B（45 passed，40 identity）
  - ClientInfoStatus/PseudoColumnUsage 枚举常量（9 passed，12 行：8 constant + values 等价遍历 2 + TYPE 类型级 2）
  - Callable 剩余重载（5 passed，6 identity）
  - ParameterMetaData 全面（14 passed，13 identity：8 constant + getPrecision/getScale/isSigned/isWrapperFor/unwrap）
  - Statement 7 常量 + 2 个 keys 重载（3 passed，9 identity）
  - RowSetMetaData 8 项（8 passed，7 identity；默认入口返回真实值而非 UnsupportedOperation）
  - DataTruncation 异常面（10 passed，13 identity；生产新增 sql_warning/next_warning 字段与 to_sql_warning/causes，01004/22001 精确）
  - Statement large 默认入口（6 passed，16 identity；生产新增 5 个 trait 默认方法，AbstractMethodError 等价 + api 快照刷新）
  - 主会话：Connection 5 个隔离级别常量（constant）+ RowSet wasNull（1 method）
- 结果：linked **1813** / unassessed **188**；capability SUPPORTED 189 + PARTIAL 1367 + UNSUPPORTED 257；declaration PASS 1797、contract PASS 1793；logical_proxy PASS 424。
- 生产新增：`database_meta_data_constants.rs`（61 常量）、`database_meta_data.rs` 3 个 trait 默认、`jdbc_datetime_ext.rs`（TimestampExt/DateExt/TimeExt）、`statement.rs` 5 个 large 默认、`sql_client_info_exception.rs` new/failed_properties、`data_truncation.rs` sql_warning/next_warning + new/getters。
- 遗留诚实项：各接口 `TYPE` 标记（Java Class 对象，无 Rust 对应）、`SQLPermission`（druid-rdbc 无 dev-dependency 反向依赖）、DataTruncation cause 构造器、Timestamp/Date/Time 的 valueOf(LocalDateTime/LocalDate/LocalTime)、`XaConnection`/`PooledConnection` 部分继承入口。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=1832 unassessed=169`；druid-rdbc 全测试套件 ok；两包 fmt check exit 0；`TASK6_EXTERNAL_PUBLIC_CONTRACT=PASS`（api 快照已刷新）；verifier 16 测试通过。

## 2026-11-05 滞留 TSV 补应用收口

- 当前 ledger SHA-256：`439b3d5cf918ce38c2385f2ef034e534fff6c0759e91595eff0d210edeec9c5a`
- 补应用三份因 `&&` 链中断而滞留的智能体报告：Callable 剩余重载（5）、ParameterMetaData 全面（5 方法 + 7 常量 SUPPORTED）、Statement 7 常量 + 2 keys 重载（9）。
- 结果：linked **1832** / unassessed **169**；capability SUPPORTED 196 + PARTIAL 1379 + UNSUPPORTED 257；declaration PASS 1816、contract PASS 1812；logical_proxy PASS 441。
- ParameterMetaData 到 13/19；ParameterMode/ParameterNullability 常量以枚举变体等值断言登记（verifier/apply 已扩展接受 `const:Name` 匹配枚举变体声明）。
- 门禁：`EVIDENCE_INTEGRITY_PASS linked=1832 unassessed=169`；verifier 16 测试通过；两包 fmt check exit 0。
