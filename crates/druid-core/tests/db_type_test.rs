extern crate druid_core as druid;
use druid::sql::DbType;

// ── of() round-trip ────────────────────────────────────────────

#[test]
fn db_type_of_empty() {
    assert!(DbType::of("").is_none());
}

#[test]
fn db_type_of_unknown() {
    assert!(DbType::of("nonexistent").is_none());
}

#[test]
fn db_type_of_case_sensitive() {
    assert!(DbType::of("MySQL").is_none());
    assert_eq!(DbType::of("mysql"), Some(DbType::MySql));
}

#[test]
fn db_type_of_aliyun_ads_alias() {
    assert_eq!(DbType::of("aliyun_ads"), Some(DbType::Ads));
    assert_eq!(DbType::of("ALIYUN_ADS"), Some(DbType::Ads));
}

#[test]
fn db_type_of_maxcompute_alias() {
    assert_eq!(DbType::of("maxcompute"), Some(DbType::Odps));
    assert_eq!(DbType::of("MAXCOMPUTE"), Some(DbType::Odps));
}

#[test]
fn db_type_of_common_databases() {
    assert_eq!(DbType::of("other"), Some(DbType::Other));
    assert_eq!(DbType::of("postgresql"), Some(DbType::PostgreSql));
    assert_eq!(DbType::of("oracle"), Some(DbType::Oracle));
    assert_eq!(DbType::of("sqlserver"), Some(DbType::SqlServer));
    assert_eq!(DbType::of("db2"), Some(DbType::Db2));
    assert_eq!(DbType::of("sqlite"), Some(DbType::SQLite));
    assert_eq!(DbType::of("h2"), Some(DbType::H2));
    assert_eq!(DbType::of("dm"), Some(DbType::Dm));
    assert_eq!(DbType::of("clickhouse"), Some(DbType::ClickHouse));
    assert_eq!(DbType::of("oceanbase"), Some(DbType::OceanBase));
    assert_eq!(DbType::of("mariadb"), Some(DbType::MariaDb));
}

#[test]
fn db_type_of_case_sensitive_special() {
    assert_eq!(DbType::of("JSQLConnect"), Some(DbType::JsqlConnect));
    assert_eq!(DbType::of("JTurbo"), Some(DbType::JTurbo));
    assert!(DbType::of("jsqlconnect").is_none());
}

// ── as_str() ───────────────────────────────────────────────────

#[test]
fn db_type_as_str_roundtrip() {
    let all = [
        DbType::Other,
        DbType::MySql,
        DbType::PostgreSql,
        DbType::Oracle,
        DbType::SqlServer,
        DbType::Db2,
        DbType::SQLite,
        DbType::H2,
        DbType::ClickHouse,
        DbType::OceanBase,
        DbType::MariaDb,
        DbType::Dm,
        DbType::Hive,
        DbType::Derby,
        DbType::Informix,
        DbType::ElasticSearch,
        DbType::Hbase,
        DbType::Presto,
        DbType::Spark,
        DbType::TiDb,
        DbType::StarRocks,
        DbType::Doris,
    ];
    for db_type in all {
        let name = db_type.as_str();
        assert_eq!(
            DbType::of(name),
            Some(db_type),
            "roundtrip failed for {name}"
        );
    }
}

// ── mask() ─────────────────────────────────────────────────────

#[test]
fn db_type_mask_known() {
    assert_eq!(DbType::MySql.mask(), 1 << 7);
    assert_eq!(DbType::PostgreSql.mask(), 1 << 4);
    assert_eq!(DbType::Oracle.mask(), 1 << 6);
    assert_eq!(DbType::SQLite.mask(), 1 << 22);
}

#[test]
fn db_type_mask_antspark_spark_shared() {
    assert_eq!(DbType::AntSpark.mask(), DbType::Spark.mask());
}

#[test]
fn db_type_mask_unknown_zero() {
    assert_eq!(DbType::Ingres.mask(), 0);
    assert_eq!(DbType::Cloudscape.mask(), 0);
}

// ── is_postgresql_style() ──────────────────────────────────────

#[test]
fn db_type_is_postgresql_style() {
    assert!(DbType::PostgreSql.is_postgresql_style());
    assert!(DbType::Edb.is_postgresql_style());
    assert!(DbType::Greenplum.is_postgresql_style());
    assert!(DbType::Hologres.is_postgresql_style());
    assert!(!DbType::MySql.is_postgresql_style());
    assert!(!DbType::Oracle.is_postgresql_style());
}

// ── hash_code_64() ─────────────────────────────────────────────

#[test]
fn db_type_hash_code_64_nonzero() {
    assert_ne!(DbType::MySql.hash_code_64(), 0);
    assert_ne!(DbType::PostgreSql.hash_code_64(), 0);
}

#[test]
fn db_type_hash_code_64_deterministic() {
    let h1 = DbType::MySql.hash_code_64();
    let h2 = DbType::MySql.hash_code_64();
    assert_eq!(h1, h2);
}

// ── mask_of() ──────────────────────────────────────────────────

#[test]
fn db_type_mask_of_single() {
    assert_eq!(DbType::mask_of([DbType::MySql]), DbType::MySql.mask());
}

#[test]
fn db_type_mask_of_multiple() {
    let mask = DbType::mask_of([DbType::MySql, DbType::PostgreSql]);
    assert_eq!(mask, DbType::MySql.mask() | DbType::PostgreSql.mask());
}

#[test]
fn db_type_mask_of_empty() {
    assert_eq!(DbType::mask_of([]), 0);
}

// ── equals_name() ──────────────────────────────────────────────

#[test]
fn db_type_equals_name() {
    assert!(DbType::MySql.equals_name("mysql"));
    assert!(!DbType::MySql.equals_name("MySQL"));
    assert!(!DbType::MySql.equals_name("postgresql"));
}

// ── Display ────────────────────────────────────────────────────

#[test]
fn db_type_display() {
    assert_eq!(format!("{}", DbType::MySql), "mysql");
    assert_eq!(format!("{}", DbType::PostgreSql), "postgresql");
    assert_eq!(format!("{}", DbType::ClickHouse), "clickhouse");
}
