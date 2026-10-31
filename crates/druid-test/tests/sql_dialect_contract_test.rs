//! 整仓验收：四方言（Oracle/ODPS/Hive/Doris）解析与格式化往返。
//!
//! 这是 P2 保真矩阵的最小验收子集；完整 32 例观测见
//! `docs/migration/dialect-fidelity/README.md`。

use druid::sql::{DbType, SqlUtils};

fn round_trip(db_type: DbType, sql: &str) {
    let statements = SqlUtils::parse_statements(sql, db_type)
        .unwrap_or_else(|error| panic!("parse failed for {db_type:?}: {error}; sql={sql}"));
    assert!(!statements.is_empty());
    let formatted = SqlUtils::to_sql_string(&statements);
    SqlUtils::parse_statements(&formatted, db_type)
        .unwrap_or_else(|error| panic!("reparse failed for {db_type:?}: {error}; sql={formatted}"));
    assert!(!SqlUtils::parameterize(sql).is_empty());
}

#[test]
fn oracle_select_and_merge_parse_and_round_trip() {
    round_trip(
        DbType::Oracle,
        "SELECT * FROM emp WHERE hiredate > DATE '2020-01-01'",
    );
    round_trip(
        DbType::Oracle,
        "MERGE INTO t USING s ON (t.id = s.id) WHEN MATCHED THEN UPDATE SET t.v = s.v          WHEN NOT MATCHED THEN INSERT (id, v) VALUES (s.id, s.v)",
    );
}

#[test]
fn odps_partitioned_insert_and_lateral_view_round_trip() {
    round_trip(
        DbType::Odps,
        "INSERT OVERWRITE TABLE t PARTITION (ds='2020') SELECT * FROM s",
    );
    round_trip(
        DbType::Odps,
        "SELECT * FROM t LATERAL VIEW explode(arr) v AS x",
    );
}

#[test]
fn hive_partitioned_insert_and_cluster_by_round_trip() {
    round_trip(
        DbType::Hive,
        "INSERT OVERWRITE TABLE t PARTITION (ds='2020') SELECT * FROM s",
    );
    round_trip(DbType::Hive, "SELECT * FROM t CLUSTER BY k");
    round_trip(DbType::Hive, "SELECT * FROM t TABLESAMPLE (10 PERCENT)");
}

#[test]
fn doris_select_and_alter_round_trip() {
    round_trip(
        DbType::Doris,
        "SELECT bitmap_union_count(b) FROM t GROUP BY k",
    );
    round_trip(DbType::Doris, "ALTER TABLE t ADD COLUMN c INT");
}

#[test]
fn oracle_returning_into_preserves_the_bind_clause() {
    let statements = SqlUtils::parse_statements(
        "INSERT INTO t (id) VALUES (1) RETURNING id INTO :out_id",
        DbType::Oracle,
    )
    .expect("RETURNING ... INTO parses");
    let formatted = SqlUtils::to_sql_string(&statements);
    assert!(
        formatted.contains("RETURNING id INTO :out_id"),
        "formatted SQL lost the INTO bind clause: {formatted}"
    );
}

#[test]
fn doris_unique_key_ddl_round_trips() {
    round_trip(
        DbType::Doris,
        "CREATE TABLE t (k1 INT, v INT) UNIQUE KEY(k1) DISTRIBUTED BY HASH(k1) BUCKETS 3 \
         PROPERTIES (\"replication_num\" = \"1\")",
    );
}
