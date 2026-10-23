# P2 方言保真矩阵（Oracle / ODPS / Hive / Doris）

> 状态：**GOLDEN_DIFFED**。Rust 观测 + Java Druid 1.2.28 jar golden 已生成；parse 匹配 28/32，parameterized 匹配 24/32（空白与大小写不计差异）。
> 本文件由 scripts/dialect_fidelity_report.py 从 rust-observations.tsv 生成，禁止手改统计。

## 方法

| 列 | 含义 | 生成方式 |
|---|---|---|
| parse | SqlUtils::parse_statements 是否接受 | cargo run -p druid-bench --example dialect_fidelity_matrix |
| format | SqlUtils::to_sql_string 输出；parse 失败记为 ERR: | 同上 |
| reparse | 对 format 输出再次 parse 是否成功 | 同上 |
| parameterized | SqlUtils::parameterize 的字面量替换结果 | 同上 |
| wall_violations | 默认 WallConfig 下 WallProvider::check 的违规数 | 同上 |
| Java golden | Java Druid 1.2.28 的同四列输出 | scripts/java_golden/DialectGolden.java（固定 1.2.28 jar） |

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
| oracle-date-literal | ok | ok | SELECT * FROM emp WHERE hiredate > DATE ? | 0 | parse:MATCH, param:DIFF |
| oracle-rownum | ok | ok | SELECT * FROM t WHERE ROWNUM <= ? | 0 | parse:MATCH, param:MATCH |
| oracle-connect-by | ok | ok | SELECT LEVEL, SYS_CONNECT_BY_PATH(name, ?) FROM t START WITH id... | 0 | parse:MATCH, param:MATCH |
| oracle-merge | ok | ok | MERGE INTO t USING s ON (t.id = s.id) WHEN MATCHED THEN UPDATE ... | 1 | parse:MATCH, param:MATCH |
| oracle-returning-into | err | n/a | INSERT INTO t (id) VALUES (?) RETURNING id INTO :out_id | 1 | parse:MATCH, param:MATCH |
| oracle-table-function | ok | ok | SELECT * FROM TABLE(fn(?)) | 0 | parse:MATCH, param:MATCH |
| oracle-decode-nvl | ok | ok | SELECT NVL(a, ?), DECODE(b, ?, ?, ?) FROM dual | 0 | parse:MATCH, param:MATCH |
| oracle-identity | ok | ok | CREATE TABLE t (id NUMBER(?) GENERATED ALWAYS AS IDENTITY, v VA... | 0 | parse:DIFF, param:DIFF |

### Odps

| 用例 | parse | reparse | parameterized | wall | Java golden |
|---|---|---|---|---:|---|
| odps-lateral-view | ok | ok | SELECT * FROM t LATERAL VIEW explode(arr) v AS x | 0 | parse:MATCH, param:MATCH |
| odps-distribute-sort | ok | ok | SELECT * FROM t DISTRIBUTE BY k SORT BY v | 0 | parse:MATCH, param:MATCH |
| odps-insert-overwrite-partition | ok | ok | INSERT OVERWRITE TABLE t PARTITION (ds=?) SELECT * FROM s | 0 | parse:MATCH, param:DIFF |
| odps-create-partitioned | ok | ok | CREATE TABLE t (a STRING) PARTITIONED BY (ds STRING) | 0 | parse:MATCH, param:MATCH |
| odps-mapjoin-hint | ok | ok | SELECT /*+ MAPJOIN(b) */ * FROM a JOIN b ON a.id = b.id | 0 | parse:MATCH, param:MATCH |
| odps-trans-array | ok | ok | SELECT TRANS_ARRAY(?, ?, arr, ?) FROM t | 0 | parse:MATCH, param:MATCH |
| odps-limit | ok | ok | SELECT * FROM t LIMIT ? | 0 | parse:MATCH, param:MATCH |
| odps-cast | ok | ok | SELECT CAST(a AS BIGINT) FROM t | 0 | parse:MATCH, param:MATCH |

### Hive

| 用例 | parse | reparse | parameterized | wall | Java golden |
|---|---|---|---|---:|---|
| hive-lateral-view | ok | ok | SELECT * FROM t LATERAL VIEW explode(arr) v AS x | 0 | parse:MATCH, param:MATCH |
| hive-insert-partition | ok | ok | INSERT OVERWRITE TABLE t PARTITION (ds=?) SELECT * FROM s | 0 | parse:MATCH, param:DIFF |
| hive-mapjoin | ok | ok | SELECT /*+ MAPJOIN(b) */ a.* FROM a JOIN b ON a.id = b.id | 0 | parse:MATCH, param:MATCH |
| hive-stored-parquet | ok | ok | CREATE TABLE t (a STRING) STORED AS PARQUET | 0 | parse:MATCH, param:DIFF |
| hive-cluster-by | ok | ok | SELECT * FROM t CLUSTER BY k | 0 | parse:MATCH, param:MATCH |
| hive-tablesample | err | n/a | SELECT * FROM t TABLESAMPLE (? PERCENT) | 1 | parse:DIFF, param:MATCH |
| hive-named-struct | ok | ok | SELECT named_struct(?, ?) FROM t | 0 | parse:MATCH, param:MATCH |
| hive-partition-filter | ok | ok | SELECT * FROM t WHERE ds = ? | 0 | parse:MATCH, param:MATCH |

### Doris

| 用例 | parse | reparse | parameterized | wall | Java golden |
|---|---|---|---|---:|---|
| doris-select | ok | ok | SELECT * FROM t WHERE k1 = ? | 0 | parse:MATCH, param:MATCH |
| doris-create-unique-key | err | n/a | CREATE TABLE t (k1 INT, v INT) UNIQUE KEY(k1) DISTRIBUTED BY HA... | 1 | parse:DIFF, param:DIFF |
| doris-distribute-buckets | err | n/a | SELECT * FROM t DISTRIBUTE BY HASH(k1) BUCKETS ? | 1 | parse:MATCH, param:DIFF |
| doris-in-subquery | ok | ok | SELECT * FROM t WHERE k1 IN (SELECT k1 FROM s) | 0 | parse:MATCH, param:MATCH |
| doris-insert-select | ok | ok | INSERT INTO t SELECT * FROM s | 0 | parse:MATCH, param:MATCH |
| doris-bitmap-agg | ok | ok | SELECT bitmap_union_count(b) FROM t GROUP BY k | 0 | parse:MATCH, param:MATCH |
| doris-order-limit | ok | ok | SELECT * FROM t ORDER BY k1 LIMIT ? | 0 | parse:MATCH, param:MATCH |
| doris-alter-add-column | ok | ok | ALTER TABLE t ADD COLUMN c INT | 0 | parse:DIFF, param:DIFF |

## 失败用例（Rust 侧）

| 方言 | 用例 | 原始 SQL | 错误 |
|---|---|---|---|
| Oracle | oracle-returning-into | INSERT INTO t (id) VALUES (1) RETURNING id INTO :out_id | ERR:SQL parse error: sql parser error: Expected: end of statement, found: INTO at Line: 1, Column: 44 |
| Hive | hive-tablesample | SELECT * FROM t TABLESAMPLE (10 PERCENT) | ERR:SQL parse error: sql parser error: Expected: identifier, found: 10 at Line: 1, Column: 30 |
| Doris | doris-create-unique-key | CREATE TABLE t (k1 INT, v INT) UNIQUE KEY(k1) DISTRIBUTED BY HASH(k1) BUCKETS 3 PROPERTIE... | ERR:SQL parse error: sql parser error: Expected: end of statement, found: UNIQUE at Line: 1, Column: 32 |
| Doris | doris-distribute-buckets | SELECT * FROM t DISTRIBUTE BY HASH(k1) BUCKETS 10 | ERR:SQL parse error: sql parser error: Expected: end of statement, found: BUCKETS at Line: 1, Column: 40 |

## 差异处置

| 方言 | 用例 | 维度 | 处置 | 理由 | 下一步 |
|---|---|---|---|---|---|
| oracle | oracle-returning-into | parse | FIXED_LOCAL_PATCH | 已通过 vendor/sqlparser 本地补丁支持 RETURNING ... INTO（Insert.returning_into） | 保持补丁；上游支持后删除 |
| hive | hive-tablesample | parse | SQLPARSER_UPSTREAM | Hive 旧式 TABLESAMPLE (n PERCENT) 未被 sqlparser 接受 | P2.2：先升级/验证 sqlparser 上游支持，否则走预处理并记录 |
| doris | doris-create-unique-key | parse | DRUID_EXTENSION | Doris UNIQUE KEY / DISTRIBUTED BY ... BUCKETS DDL 为 Doris 方言扩展 | P2.3：在 Doris 方言层做 DDL 预解析，保留表模型信息 |
| doris | doris-alter-add-column | parse | JAVA_QUIRK | Druid 1.2.28 的 Doris 方言不接受 ALTER TABLE ADD COLUMN；Rust 接受更合理 | 无代码动作；在矩阵 README 标注 Java 侧限制 |
| oracle | oracle-identity | parse | JAVA_QUIRK | Druid 1.2.28 不接受 GENERATED ALWAYS AS IDENTITY；Rust 支持 | 无代码动作；保留 Rust 能力 |
| hive | hive-stored-parquet | parameterized | JAVA_QUIRK | Druid 参数化输出 STORE AS，Rust 保持 STORED AS | 无代码动作；视为 Java 输出瑕疵 |
| oracle | oracle-date-literal | parameterized | JAVA_QUIRK | Druid 不参数化 DATE 字面量，Rust 参数化为 DATE ? | 保留 Rust 行为；差异记入矩阵 |
| odps | odps-insert-overwrite-partition | parameterized | RUST_KEEP | Rust 保留 INSERT OVERWRITE TABLE 的 TABLE 关键字 | 保留；不影响执行语义 |
| hive | hive-insert-partition | parameterized | RUST_KEEP | 同上 | 保留 |
| doris | doris-create-unique-key | parameterized | DRUID_EXTENSION | Druid 把 UNIQUE KEY 重写为 INTO/BUCKETS OPTIONS，丢失表模型 | 随 P2.3 一起修 |

## 结论与下一步

1. **ODPS/Hive 的查询面**已由 sqlparser 覆盖到可解析；缺口集中在 Hive 采样语法。
2. **Doris 的 DDL 扩展**（UNIQUE KEY / BUCKETS）不被 GenericDialect 接受，属 P2 方案 C（Druid 侧扩展）的候选。
3. **Oracle RETURNING INTO** 是 Druid Wall/解析常见语料，需在 P2 决定是补 sqlparser 扩展还是走预处理。
4. Java golden 已由 `druid-1.2.28.jar` 生成；逐用例 Java 列显示 parse/param 的 MATCH/DIFF。
5. 仍有 4 个 parse DIFF、8 个 parameterized DIFF；在差异收敛前，不得把本矩阵当作「方言保真通过」的证据。
