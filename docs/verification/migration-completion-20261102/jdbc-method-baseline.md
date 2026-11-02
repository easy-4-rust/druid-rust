# JDBC 方法证据基线（2026-11-02）

- 来源：`docs/verification/rdbc-first/java8-method-evidence.tsv`
- 冻结副本（3.1 时点）：`java8-method-evidence-baseline.tsv`
- 冻结副本 SHA-256：`cfa065c82c64ab58d794ac247a4debfe23fb193d27d30dc774ce4acf70052d6a`
- 身份分母：**2001**
- 头部 schema：`java8-method-evidence-v1`，来源 schema `jdbc-java-method-inventory-v1`

## 各维度当前状态

| 维度 | PASS | PARTIAL | UNASSESSED |
|---|---:|---:|---:|
| capability | 0 | 133 | 1868 |
| declaration | 117 | 0 | 1884 |
| contract | 119 | 0 | 1882 |
| adapter | 2 | 0 | 1999 |
| logical_proxy | 9 | 0 | 1992 |
| real_db | 14 | 0 | 1987 |

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
