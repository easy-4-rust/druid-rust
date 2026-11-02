# JDBC 方法证据基线（2026-11-02）

- 来源：`docs/verification/rdbc-first/java8-method-evidence.tsv`
- 冻结副本：`java8-method-evidence-baseline.tsv`
- SHA-256：`cfa065c82c64ab58d794ac247a4debfe23fb193d27d30dc774ce4acf70052d6a`
- 身份分母：**2001**
- 头部 schema：`java8-method-evidence-v1`，来源 schema `jdbc-java-method-inventory-v1`

## 各维度当前状态

| 维度 | PASS | PARTIAL | UNASSESSED |
|---|---:|---:|---:|
| capability | 0 | 25 | 1976 |
| declaration | 8 | 0 | 1993 |
| contract | 13 | 0 | 1988 |
| adapter | 3 | 0 | 1998 |
| logical_proxy | 9 | 0 | 1992 |
| real_db | 0 | 0 | **2001** |

## 约束

- 该台账是唯一方法证据事实源，不新建平行清单。
- 未评估项保持 UNASSESSED；不得由同名类型、编译成功或局部测试升级。
- `real_db` 必须在真实数据库运行后填写，不能用 SQLite/mock 代替其他产品。
- 后续每批更新后重新计算 SHA，并记录对应 Java/Rust 基线与命令。

## 2026-11-02 Statement 首批回填

- 当前 ledger SHA-256：`625b13b21cffced6d0b39128dc7a8f9bbac12df02accf97a182e91b77a300b0f`
- 新增：30 个 `java.sql.Statement` 方法 + `CallableStatement#wasNull()` + `ResultSet` 的 `previous/getRow/getFetchSize`，关联 `java_rust_statement_errors_differential.rs#pinned_java_and_rust_compare_all_37_statement_inputs`。
- 结果：capability PARTIAL 58 / UNASSESSED 1943；declaration PASS 42、contract PASS 47、logical_proxy PASS 9、adapter PASS 2、real_db 仍 0/2001。
- 门禁：`python scripts/verify_java8_method_evidence.py` → `EVIDENCE_INTEGRITY_PASS linked=58 unassessed=1943`。
- 边界：只登记 live error differential 可支撑的 declaration/contract；adapter、logical_proxy、real_db 未评估，3.2–3.7 未完成。
