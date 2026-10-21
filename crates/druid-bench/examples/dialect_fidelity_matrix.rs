//! P2 方言保真矩阵的 Rust 观测器。
//!
//! 对 Oracle / ODPS / Hive / Doris 四类方言执行 parse → format → reparse →
//! parameterize → wall check，并输出 TSV 供 golden diff 使用。
//!
//! 运行：`cargo run -p druid-bench --example dialect_fidelity_matrix`

use druid::sql::{DbType, SqlUtils, WallConfig, WallProvider};

struct Case {
    dialect: DbType,
    id: &'static str,
    sql: &'static str,
}

fn one_line(value: &str) -> String {
    value.replace(['\t', '\n', '\r'], " ")
}

fn main() {
    let cases = vec![
        // Oracle
        Case { dialect: DbType::Oracle, id: "oracle-date-literal", sql: "SELECT * FROM emp WHERE hiredate > DATE '2020-01-01'" },
        Case { dialect: DbType::Oracle, id: "oracle-rownum", sql: "SELECT * FROM t WHERE ROWNUM <= 10" },
        Case { dialect: DbType::Oracle, id: "oracle-connect-by", sql: "SELECT LEVEL, SYS_CONNECT_BY_PATH(name, '/') FROM t START WITH id = 1 CONNECT BY PRIOR id = parent_id" },
        Case { dialect: DbType::Oracle, id: "oracle-merge", sql: "MERGE INTO t USING s ON (t.id = s.id) WHEN MATCHED THEN UPDATE SET t.v = s.v WHEN NOT MATCHED THEN INSERT (id, v) VALUES (s.id, s.v)" },
        Case { dialect: DbType::Oracle, id: "oracle-returning-into", sql: "INSERT INTO t (id) VALUES (1) RETURNING id INTO :out_id" },
        Case { dialect: DbType::Oracle, id: "oracle-table-function", sql: "SELECT * FROM TABLE(fn(1))" },
        Case { dialect: DbType::Oracle, id: "oracle-decode-nvl", sql: "SELECT NVL(a, 0), DECODE(b, 1, 'x', 'y') FROM dual" },
        Case { dialect: DbType::Oracle, id: "oracle-identity", sql: "CREATE TABLE t (id NUMBER(10) GENERATED ALWAYS AS IDENTITY, v VARCHAR2(20))" },
        // ODPS
        Case { dialect: DbType::Odps, id: "odps-lateral-view", sql: "SELECT * FROM t LATERAL VIEW explode(arr) v AS x" },
        Case { dialect: DbType::Odps, id: "odps-distribute-sort", sql: "SELECT * FROM t DISTRIBUTE BY k SORT BY v" },
        Case { dialect: DbType::Odps, id: "odps-insert-overwrite-partition", sql: "INSERT OVERWRITE TABLE t PARTITION (ds='2020') SELECT * FROM s" },
        Case { dialect: DbType::Odps, id: "odps-create-partitioned", sql: "CREATE TABLE t (a STRING) PARTITIONED BY (ds STRING)" },
        Case { dialect: DbType::Odps, id: "odps-mapjoin-hint", sql: "SELECT /*+ MAPJOIN(b) */ * FROM a JOIN b ON a.id = b.id" },
        Case { dialect: DbType::Odps, id: "odps-trans-array", sql: "SELECT TRANS_ARRAY(1, ',', arr, ',') FROM t" },
        Case { dialect: DbType::Odps, id: "odps-limit", sql: "SELECT * FROM t LIMIT 10" },
        Case { dialect: DbType::Odps, id: "odps-cast", sql: "SELECT CAST(a AS BIGINT) FROM t" },
        // Hive
        Case { dialect: DbType::Hive, id: "hive-lateral-view", sql: "SELECT * FROM t LATERAL VIEW explode(arr) v AS x" },
        Case { dialect: DbType::Hive, id: "hive-insert-partition", sql: "INSERT OVERWRITE TABLE t PARTITION (ds='2020') SELECT * FROM s" },
        Case { dialect: DbType::Hive, id: "hive-mapjoin", sql: "SELECT /*+ MAPJOIN(b) */ a.* FROM a JOIN b ON a.id = b.id" },
        Case { dialect: DbType::Hive, id: "hive-stored-parquet", sql: "CREATE TABLE t (a STRING) STORED AS PARQUET" },
        Case { dialect: DbType::Hive, id: "hive-cluster-by", sql: "SELECT * FROM t CLUSTER BY k" },
        Case { dialect: DbType::Hive, id: "hive-tablesample", sql: "SELECT * FROM t TABLESAMPLE (10 PERCENT)" },
        Case { dialect: DbType::Hive, id: "hive-named-struct", sql: "SELECT named_struct('a', 1) FROM t" },
        Case { dialect: DbType::Hive, id: "hive-partition-filter", sql: "SELECT * FROM t WHERE ds = '2020'" },
        // Doris
        Case { dialect: DbType::Doris, id: "doris-select", sql: "SELECT * FROM t WHERE k1 = 1" },
        Case { dialect: DbType::Doris, id: "doris-create-unique-key", sql: "CREATE TABLE t (k1 INT, v INT) UNIQUE KEY(k1) DISTRIBUTED BY HASH(k1) BUCKETS 3 PROPERTIES (\"replication_num\" = \"1\")" },
        Case { dialect: DbType::Doris, id: "doris-distribute-buckets", sql: "SELECT * FROM t DISTRIBUTE BY HASH(k1) BUCKETS 10" },
        Case { dialect: DbType::Doris, id: "doris-in-subquery", sql: "SELECT * FROM t WHERE k1 IN (SELECT k1 FROM s)" },
        Case { dialect: DbType::Doris, id: "doris-insert-select", sql: "INSERT INTO t SELECT * FROM s" },
        Case { dialect: DbType::Doris, id: "doris-bitmap-agg", sql: "SELECT bitmap_union_count(b) FROM t GROUP BY k" },
        Case { dialect: DbType::Doris, id: "doris-order-limit", sql: "SELECT * FROM t ORDER BY k1 LIMIT 10" },
        Case { dialect: DbType::Doris, id: "doris-alter-add-column", sql: "ALTER TABLE t ADD COLUMN c INT" },
    ];

    println!("dialect\tcase\tparse\tformat\treparse\tparameterized\twall_violations\tsql");

    for case in cases {
        let parsed = SqlUtils::parse_statements(case.sql, case.dialect);
        let (parse_ok, formatted) = match &parsed {
            Ok(statements) => ("ok", SqlUtils::to_sql_string(statements)),
            Err(error) => ("err", format!("ERR:{error}")),
        };
        let reparse = if parse_ok == "ok" {
            if SqlUtils::parse_statements(&formatted, case.dialect).is_ok() {
                "ok"
            } else {
                "err"
            }
        } else {
            "n/a"
        };
        let parameterized = SqlUtils::parameterize(case.sql);
        let wall = WallProvider::new(WallConfig::default()).check(case.sql);

        println!(
            "{:?}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            case.dialect,
            case.id,
            parse_ok,
            one_line(&formatted),
            reparse,
            one_line(&parameterized),
            wall.violations().len(),
            case.sql
        );
    }
}
