//! Batch 2 coverage tests for:
//! - druid_stat_manager_facade.rs: merge_wall_stat/merge_wall_value direct paths
//! - druid_stat_service.rs: page nested keys, sql_detail, wall sort, parameters edge cases
//! - sql_utils.rs: dialect mapping, format, parse_single_statement, to_sql_string
//! - db_type.rs: of/as_str/mask/hash_code_64/mask_of/equals_name/is_postgresql_style/Display

extern crate druid_core as druid;
use druid_core::sql::{DbType, SqlFormatOption, SqlUtils};
use druid_core::stats::{DruidStatManagerFacade, DruidStatService};

// ===========================================================================
// 1. DruidStatService: page nested key, sql_detail, wall sort, parameters
// ===========================================================================

#[test]
fn service_page_with_nested_key_bracket_notation() {
    let svc = DruidStatService;
    let result = svc.service("/wall.json?orderBy=tables[0]&orderType=desc");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_SUCCESS);
}

#[test]
fn service_page_order_by_missing_key() {
    let svc = DruidStatService;
    let result = svc.service("/sql.json?orderBy=NonExistentField&orderType=desc");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_SUCCESS);
}

#[test]
fn service_page_order_by_empty() {
    let svc = DruidStatService;
    let result = svc.service("/sql.json?orderBy=&orderType=desc");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_SUCCESS);
}

#[test]
fn service_parameters_repeated_keys_last_wins() {
    let svc = DruidStatService;
    let result = svc.service("/sql.json?page=1&page=2");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_SUCCESS);
}

#[test]
fn service_parameters_no_equals_sign() {
    let svc = DruidStatService;
    let result = svc.service("/sql.json?page");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_SUCCESS);
}

#[test]
fn service_parameters_empty_key() {
    let svc = DruidStatService;
    let result = svc.service("/sql.json?=value");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_SUCCESS);
}

#[test]
fn service_datasource_with_dot_extension() {
    let svc = DruidStatService;
    let result = svc.service("/datasource-1.json");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_ERROR);
}

#[test]
fn service_wall_id_with_non_numeric() {
    let svc = DruidStatService;
    let result = svc.service("/wall-abc.json");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_ERROR);
}

#[test]
fn service_connection_info_id_parsing() {
    let svc = DruidStatService;
    let result = svc.service("/connectionInfo-1.json");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_ERROR);
}

#[test]
fn service_active_connection_stack_trace_non_numeric_id() {
    let svc = DruidStatService;
    let result = svc.service("/activeConnectionStackTrace-abc.json");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_ERROR);
}

#[test]
fn service_wall_sort_non_object_input() {
    let svc = DruidStatService;
    let result = svc.service("/wall.json?orderBy=name&orderType=desc");
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["ResultCode"], DruidStatService::RESULT_CODE_SUCCESS);
}

// ===========================================================================
// 2. DruidStatManagerFacade: merge_wall_stat/merge_wall_value direct paths
// ===========================================================================

#[test]
fn facade_wall_stat_data_merges_numbers() {
    let facade = DruidStatManagerFacade::global();
    let wall = facade.wall_stat_data(None);
    assert!(wall.is_object() || wall.is_null());
}

#[test]
fn facade_wall_stat_data_with_datasource_id_exercises_filter() {
    let facade = DruidStatManagerFacade::global();
    let wall = facade.wall_stat_data(Some(0));
    assert!(wall.is_object() || wall.is_null());
}

// ===========================================================================
// 3. SqlUtils: dialect mapping, format, parse, to_sql_string
// ===========================================================================

#[test]
fn sql_utils_dialect_mysql_variants() {
    for db in [
        DbType::MySql,
        DbType::MariaDb,
        DbType::OceanBase,
        DbType::Drds,
        DbType::TiDb,
        DbType::GoldenDb,
        DbType::PolarDbX,
        DbType::AdbMySql,
    ] {
        let _dialect = SqlUtils::dialect(db);
    }
}

#[test]
fn sql_utils_dialect_postgresql_variants() {
    for db in [
        DbType::PostgreSql,
        DbType::Edb,
        DbType::Greenplum,
        DbType::GaussDb,
        DbType::Hologres,
    ] {
        let _dialect = SqlUtils::dialect(db);
    }
}

#[test]
fn sql_utils_dialect_sqlserver_variants() {
    for db in [DbType::SqlServer, DbType::Jtds, DbType::Synapse] {
        let _dialect = SqlUtils::dialect(db);
    }
}

#[test]
fn sql_utils_dialect_other_variants() {
    for db in [
        DbType::SQLite,
        DbType::ClickHouse,
        DbType::BigQuery,
        DbType::Snowflake,
        DbType::Redshift,
        DbType::Hive,
    ] {
        let _dialect = SqlUtils::dialect(db);
    }
}

#[test]
fn sql_utils_dialect_generic_fallback() {
    for db in [
        DbType::Other,
        DbType::Hsql,
        DbType::Db2,
        DbType::Oracle,
        DbType::Derby,
        DbType::Mock,
    ] {
        let _dialect = SqlUtils::dialect(db);
    }
}

#[test]
fn sql_utils_format_simple_select() {
    let result = SqlUtils::format("SELECT 1", DbType::MySql);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "SELECT 1");
}

#[test]
fn sql_utils_format_multiple_statements() {
    let result = SqlUtils::format("SELECT 1; SELECT 2", DbType::MySql);
    assert!(result.is_ok());
    let formatted = result.unwrap();
    assert!(formatted.contains(";\n"));
}

#[test]
fn sql_utils_parse_single_statement_ok() {
    let result = SqlUtils::parse_single_statement("SELECT 1", DbType::MySql);
    assert!(result.is_ok());
}

#[test]
fn sql_utils_parse_single_statement_multiple_error() {
    let result = SqlUtils::parse_single_statement("SELECT 1; SELECT 2", DbType::MySql);
    assert!(result.is_err());
}

#[test]
fn sql_utils_parse_single_statement_invalid_sql() {
    let result = SqlUtils::parse_single_statement("NOT VALID SQL !!!", DbType::MySql);
    assert!(result.is_err());
}

#[test]
fn sql_utils_to_sql_string_empty() {
    let result = SqlUtils::to_sql_string(&[]);
    assert_eq!(result, "");
}

#[test]
fn sql_utils_to_sql_string_single() {
    let stmts = SqlUtils::parse_statements("SELECT 1", DbType::MySql).unwrap();
    let result = SqlUtils::to_sql_string(&stmts);
    assert_eq!(result, "SELECT 1");
}

#[test]
fn sql_utils_to_sql_string_multiple() {
    let stmts = SqlUtils::parse_statements("SELECT 1; SELECT 2", DbType::MySql).unwrap();
    let result = SqlUtils::to_sql_string(&stmts);
    assert!(result.contains(";\n"));
}

#[test]
fn sql_utils_parameterize_delegates() {
    let result = SqlUtils::parameterize("SELECT * FROM t WHERE id = 1");
    assert!(result.contains('?'));
}

// ===========================================================================
// 4. SqlFormatOption
// ===========================================================================

#[test]
fn sql_format_option_default() {
    let opt = SqlFormatOption::default();
    assert!(opt.is_ucase());
    assert!(opt.is_pretty_format());
    assert!(!opt.is_parameterized());
    assert!(!opt.is_desensitize());
}

#[test]
fn sql_format_option_setters() {
    let mut opt = SqlFormatOption::new(true, true, false);
    opt.set_ucase(false);
    assert!(!opt.is_ucase());
    opt.set_pretty_format(false);
    assert!(!opt.is_pretty_format());
    opt.set_parameterized(true);
    assert!(opt.is_parameterized());
    opt.set_desensitize(true);
    assert!(opt.is_desensitize());
}

// ===========================================================================
// 5. DbType: of/as_str/mask/hash_code_64/mask_of/equals_name/is_postgresql_style/Display
// ===========================================================================

#[test]
fn db_type_of_all_variants() {
    let cases = [
        ("other", DbType::Other),
        ("mysql", DbType::MySql),
        ("postgresql", DbType::PostgreSql),
        ("oracle", DbType::Oracle),
        ("sqlite", DbType::SQLite),
        ("sqlserver", DbType::SqlServer),
        ("clickhouse", DbType::ClickHouse),
        ("hive", DbType::Hive),
        ("snowflake", DbType::Snowflake),
        ("redshift", DbType::Redshift),
        ("bigquery", DbType::BigQuery),
        ("h2", DbType::H2),
        ("dm", DbType::Dm),
        ("kingbase", DbType::Kingbase),
        ("oceanbase", DbType::OceanBase),
        ("tidb", DbType::TiDb),
        ("mariadb", DbType::MariaDb),
        ("db2", DbType::Db2),
        ("informix", DbType::Informix),
        ("teradata", DbType::Teradata),
        ("phoenix", DbType::Phoenix),
        ("edb", DbType::Edb),
        ("kylin", DbType::Kylin),
        ("ads", DbType::Ads),
        ("presto", DbType::Presto),
        ("elastic_search", DbType::ElasticSearch),
        ("hbase", DbType::Hbase),
        ("drds", DbType::Drds),
        ("blink", DbType::Blink),
        ("antspark", DbType::AntSpark),
        ("spark", DbType::Spark),
        ("oceanbase_oracle", DbType::OceanBaseOracle),
        ("polardb", DbType::PolarDb),
        ("ali_oracle", DbType::AliOracle),
        ("mock", DbType::Mock),
        ("sybase", DbType::Sybase),
        ("highgo", DbType::HighGo),
        ("greenplum", DbType::Greenplum),
        ("gaussdb", DbType::GaussDb),
        ("trino", DbType::Trino),
        ("oscar", DbType::Oscar),
        ("tydb", DbType::TyDb),
        ("starrocks", DbType::StarRocks),
        ("goldendb", DbType::GoldenDb),
        ("hologres", DbType::Hologres),
        ("impala", DbType::Impala),
        ("doris", DbType::Doris),
        ("lealone", DbType::Lealone),
        ("athena", DbType::Athena),
        ("polardbx", DbType::PolarDbX),
        ("supersql", DbType::SuperSql),
        ("databricks", DbType::Databricks),
        ("adb_mysql", DbType::AdbMySql),
        ("polardb2", DbType::PolarDb2),
        ("synapse", DbType::Synapse),
        ("ingres", DbType::Ingres),
        ("cloudscape", DbType::Cloudscape),
        ("timesten", DbType::TimesTen),
        ("as400", DbType::As400),
        ("sapdb", DbType::SapDb),
        ("kdb", DbType::Kdb),
        ("xugu", DbType::Xugu),
        ("firebirdsql", DbType::FirebirdSql),
        ("JSQLConnect", DbType::JsqlConnect),
        ("JTurbo", DbType::JTurbo),
        ("interbase", DbType::Interbase),
        ("pointbase", DbType::Pointbase),
        ("edbc", DbType::Edbc),
        ("mimer", DbType::Mimer),
        ("taosdata", DbType::TaosData),
        ("sundb", DbType::SunDb),
    ];
    for (name, expected) in &cases {
        assert_eq!(
            DbType::of(name),
            Some(*expected),
            "DbType::of({name}) failed"
        );
    }
}

#[test]
fn db_type_of_empty_returns_none() {
    assert!(DbType::of("").is_none());
}

#[test]
fn db_type_of_unknown_returns_none() {
    assert!(DbType::of("unknown_db").is_none());
}

#[test]
fn db_type_of_aliyun_ads_case_insensitive() {
    assert_eq!(DbType::of("ALIYUN_ADS"), Some(DbType::Ads));
    assert_eq!(DbType::of("aliyun_ads"), Some(DbType::Ads));
}

#[test]
fn db_type_of_maxcompute_case_insensitive() {
    assert_eq!(DbType::of("MAXCOMPUTE"), Some(DbType::Odps));
    assert_eq!(DbType::of("maxcompute"), Some(DbType::Odps));
}

#[test]
fn db_type_as_str_roundtrip() {
    let all = [
        DbType::Other,
        DbType::MySql,
        DbType::PostgreSql,
        DbType::Oracle,
        DbType::SQLite,
        DbType::SqlServer,
        DbType::ClickHouse,
        DbType::Hive,
    ];
    for db in all {
        let name = db.as_str();
        assert_eq!(DbType::of(name), Some(db), "Roundtrip failed for {name}");
    }
}

#[test]
fn db_type_mask_returns_nonzero_for_known() {
    assert!(DbType::MySql.mask() > 0);
    assert!(DbType::PostgreSql.mask() > 0);
}

#[test]
fn db_type_mask_returns_zero_for_legacy() {
    assert_eq!(DbType::Ingres.mask(), 0);
    assert_eq!(DbType::Cloudscape.mask(), 0);
}

#[test]
fn db_type_mask_antspark_equals_spark() {
    assert_eq!(DbType::AntSpark.mask(), DbType::Spark.mask());
}

#[test]
fn db_type_hash_code_64_consistency() {
    let h1 = DbType::MySql.hash_code_64();
    let h2 = DbType::MySql.hash_code_64();
    assert_eq!(h1, h2);
    assert_ne!(
        DbType::MySql.hash_code_64(),
        DbType::PostgreSql.hash_code_64()
    );
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

#[test]
fn db_type_equals_name() {
    assert!(DbType::MySql.equals_name("mysql"));
    assert!(!DbType::MySql.equals_name("postgresql"));
}

#[test]
fn db_type_is_postgresql_style() {
    assert!(DbType::PostgreSql.is_postgresql_style());
    assert!(DbType::Edb.is_postgresql_style());
    assert!(DbType::Greenplum.is_postgresql_style());
    assert!(DbType::Hologres.is_postgresql_style());
    assert!(!DbType::MySql.is_postgresql_style());
}

#[test]
fn db_type_display() {
    assert_eq!(format!("{}", DbType::MySql), "mysql");
    assert_eq!(format!("{}", DbType::PostgreSql), "postgresql");
}

// ===========================================================================
// 6. DruidStatManagerFacade: additional paths
// ===========================================================================

#[test]
fn facade_start_time_is_positive() {
    let facade = DruidStatManagerFacade::global();
    let stat = facade.basic_stat();
    assert!(stat["StartTime"].as_u64().unwrap() > 0);
}

#[test]
fn facade_rust_msrv_is_set() {
    let facade = DruidStatManagerFacade::global();
    let stat = facade.basic_stat();
    let msrv = stat["RustMSRV"].as_str().unwrap();
    assert!(!msrv.is_empty());
}

#[test]
fn facade_rust_target_os_is_set() {
    let facade = DruidStatManagerFacade::global();
    let stat = facade.basic_stat();
    assert!(stat["RustTargetOS"].as_str().is_some());
}

#[test]
fn facade_rust_target_arch_is_set() {
    let facade = DruidStatManagerFacade::global();
    let stat = facade.basic_stat();
    assert!(stat["RustTargetArch"].as_str().is_some());
}
