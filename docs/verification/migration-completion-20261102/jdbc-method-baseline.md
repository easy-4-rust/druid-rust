# JDBC 方法证据基线（2026-11-02）

- 来源：`docs/verification/rdbc-first/java8-method-evidence.tsv`
- 冻结副本（3.1 时点）：`java8-method-evidence-baseline.tsv`
- 冻结副本 SHA-256：`cfa065c82c64ab58d794ac247a4debfe23fb193d27d30dc774ce4acf70052d6a`
- 身份分母：**2001**
- 头部 schema：`java8-method-evidence-v1`，来源 schema `jdbc-java-method-inventory-v1`

## 各维度当前状态

| 维度 | PASS | PARTIAL | UNSUPPORTED | UNASSESSED |
|---|---:|---:|---:|---:|
| capability | 0 | 497 | 4 | 1500 |
| declaration | 485 | 0 | 0 | 1516 |
| contract | 483 | 0 | 0 | 1518 |
| adapter | 151 | 0 | 0 | 1850 |
| logical_proxy | 306 | 0 | 0 | 1695 |
| real_db | 54 | 0 | 0 | 1947 |

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
