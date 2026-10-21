# P2 方言保真矩阵（Oracle / ODPS / Hive / Doris）

> 状态：**STARTED**。Rust 观测已完成；Java golden 列仍为 PENDING（需要固定版本 jar 生成）。
> 本文件由 scripts/dialect_fidelity_report.py 从 rust-observations.tsv 生成，禁止手改统计。

## 方法

| 列 | 含义 | 生成方式 |
|---|---|---|
| parse | SqlUtils::parse_statements 是否接受 | cargo run -p druid-bench --example dialect_fidelity_matrix |
| format | SqlUtils::to_sql_string 输出；parse 失败记为 ERR: | 同上 |
| reparse | 对 format 输出再次 parse 是否成功 | 同上 |
| parameterized | SqlUtils::parameterize 的字面量替换结果 | 同上 |
| wall_violations | 默认 WallConfig 下 WallProvider::check 的违规数 | 同上 |
| Java golden | Java Druid 1.2.28 的同四列输出 | PENDING：需 mvn -pl core -DskipTests package 后用固定 jar 运行 |

## 汇总

| 方言 | 用例 | parse ok | format-reparse ok | wall 违规 | 主要缺口 |
|---|---:|---:|---:|---:|---|
| Oracle | 8 | 7 | 7 | 2 | RETURNING ... INTO 未支持 |
| Odps | 8 | 8 | 8 | 0 | 无 |
| Hive | 8 | 7 | 7 | 1 | TABLESAMPLE (n PERCENT) 未支持 |
| Doris | 8 | 6 | 6 | 2 | UNIQUE KEY / DISTRIBUTED ... BUCKETS DDL 未支持 |

## 逐用例观测

### Oracle

| 用例 | parse | reparse | parameterized | wall | Java golden |
|---|---|---|---|---:|---|
| oracle-date-literal | ok | ok | SELECT * FROM emp WHERE hiredate > DATE ? | 0 | PENDING |
| oracle-rownum | ok | ok | SELECT * FROM t WHERE ROWNUM <= ? | 0 | PENDING |
| oracle-connect-by | ok | ok | SELECT LEVEL, SYS_CONNECT_BY_PATH(name, ?) FROM t START WITH id... | 0 | PENDING |
| oracle-merge | ok | ok | MERGE INTO t USING s ON (t.id = s.id) WHEN MATCHED THEN UPDATE ... | 1 | PENDING |
| oracle-returning-into | err | n/a | INSERT INTO t (id) VALUES (?) RETURNING id INTO :out_id | 1 | PENDING |
| oracle-table-function | ok | ok | SELECT * FROM TABLE(fn(?)) | 0 | PENDING |
| oracle-decode-nvl | ok | ok | SELECT NVL(a, ?), DECODE(b, ?, ?, ?) FROM dual | 0 | PENDING |
| oracle-identity | ok | ok | CREATE TABLE t (id NUMBER(?) GENERATED ALWAYS AS IDENTITY, v VA... | 0 | PENDING |

### Odps

| 用例 | parse | reparse | parameterized | wall | Java golden |
|---|---|---|---|---:|---|
| odps-lateral-view | ok | ok | SELECT * FROM t LATERAL VIEW explode(arr) v AS x | 0 | PENDING |
| odps-distribute-sort | ok | ok | SELECT * FROM t DISTRIBUTE BY k SORT BY v | 0 | PENDING |
| odps-insert-overwrite-partition | ok | ok | INSERT OVERWRITE TABLE t PARTITION (ds=?) SELECT * FROM s | 0 | PENDING |
| odps-create-partitioned | ok | ok | CREATE TABLE t (a STRING) PARTITIONED BY (ds STRING) | 0 | PENDING |
| odps-mapjoin-hint | ok | ok | SELECT /*+ MAPJOIN(b) */ * FROM a JOIN b ON a.id = b.id | 0 | PENDING |
| odps-trans-array | ok | ok | SELECT TRANS_ARRAY(?, ?, arr, ?) FROM t | 0 | PENDING |
| odps-limit | ok | ok | SELECT * FROM t LIMIT ? | 0 | PENDING |
| odps-cast | ok | ok | SELECT CAST(a AS BIGINT) FROM t | 0 | PENDING |

### Hive

| 用例 | parse | reparse | parameterized | wall | Java golden |
|---|---|---|---|---:|---|
| hive-lateral-view | ok | ok | SELECT * FROM t LATERAL VIEW explode(arr) v AS x | 0 | PENDING |
| hive-insert-partition | ok | ok | INSERT OVERWRITE TABLE t PARTITION (ds=?) SELECT * FROM s | 0 | PENDING |
| hive-mapjoin | ok | ok | SELECT /*+ MAPJOIN(b) */ a.* FROM a JOIN b ON a.id = b.id | 0 | PENDING |
| hive-stored-parquet | ok | ok | CREATE TABLE t (a STRING) STORED AS PARQUET | 0 | PENDING |
| hive-cluster-by | ok | ok | SELECT * FROM t CLUSTER BY k | 0 | PENDING |
| hive-tablesample | err | n/a | SELECT * FROM t TABLESAMPLE (? PERCENT) | 1 | PENDING |
| hive-named-struct | ok | ok | SELECT named_struct(?, ?) FROM t | 0 | PENDING |
| hive-partition-filter | ok | ok | SELECT * FROM t WHERE ds = ? | 0 | PENDING |

### Doris

| 用例 | parse | reparse | parameterized | wall | Java golden |
|---|---|---|---|---:|---|
| doris-select | ok | ok | SELECT * FROM t WHERE k1 = ? | 0 | PENDING |
| doris-create-unique-key | err | n/a | CREATE TABLE t (k1 INT, v INT) UNIQUE KEY(k1) DISTRIBUTED BY HA... | 1 | PENDING |
| doris-distribute-buckets | err | n/a | SELECT * FROM t DISTRIBUTE BY HASH(k1) BUCKETS ? | 1 | PENDING |
| doris-in-subquery | ok | ok | SELECT * FROM t WHERE k1 IN (SELECT k1 FROM s) | 0 | PENDING |
| doris-insert-select | ok | ok | INSERT INTO t SELECT * FROM s | 0 | PENDING |
| doris-bitmap-agg | ok | ok | SELECT bitmap_union_count(b) FROM t GROUP BY k | 0 | PENDING |
| doris-order-limit | ok | ok | SELECT * FROM t ORDER BY k1 LIMIT ? | 0 | PENDING |
| doris-alter-add-column | ok | ok | ALTER TABLE t ADD COLUMN c INT | 0 | PENDING |

## 失败用例（Rust 侧）

| 方言 | 用例 | 原始 SQL | 错误 |
|---|---|---|---|
| Oracle | oracle-returning-into | INSERT INTO t (id) VALUES (1) RETURNING id INTO :out_id | ERR:SQL parse error: sql parser error: Expected: end of statement, found: INTO at Line: 1, Column: 44 |
| Hive | hive-tablesample | SELECT * FROM t TABLESAMPLE (10 PERCENT) | ERR:SQL parse error: sql parser error: Expected: identifier, found: 10 at Line: 1, Column: 30 |
| Doris | doris-create-unique-key | CREATE TABLE t (k1 INT, v INT) UNIQUE KEY(k1) DISTRIBUTED BY HASH(k1) BUCKETS 3 PROPERTIE... | ERR:SQL parse error: sql parser error: Expected: end of statement, found: UNIQUE at Line: 1, Column: 32 |
| Doris | doris-distribute-buckets | SELECT * FROM t DISTRIBUTE BY HASH(k1) BUCKETS 10 | ERR:SQL parse error: sql parser error: Expected: end of statement, found: BUCKETS at Line: 1, Column: 40 |

## 结论与下一步

1. **ODPS/Hive 的查询面**已由 sqlparser 覆盖到可解析；缺口集中在 Hive 采样语法。
2. **Doris 的 DDL 扩展**（UNIQUE KEY / BUCKETS）不被 GenericDialect 接受，属 P2 方案 C（Druid 侧扩展）的候选。
3. **Oracle RETURNING INTO** 是 Druid Wall/解析常见语料，需在 P2 决定是补 sqlparser 扩展还是走预处理。
4. Java golden 必须来自固定 commit 的 jar，不得用「同版本号」代替；生成后本文件的 Java 列由 MATCH/MISMATCH 替换 PENDING。
5. 在 golden 完成前，不得把本矩阵当作「方言保真通过」的证据。
