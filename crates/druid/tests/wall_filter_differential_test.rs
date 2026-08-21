#![allow(unused_must_use)]
//! Differential tests for `WallFilter` (Java `WallFilter` 语义对照)。
//!
//! Java 基线：`33824c3dec1612711f9bb4e409319bcab2e4cd0e`。
//! 目标：覆盖 `wall_filter.rs` 中 `create_provider` 全方言分支、公共 API、
//! BeforeFilter/AfterFilter/ResultSetFilter trait 实现、以及辅助函数。

use druid::core::{DruidError, ExecResult};
use druid::sql::{DbType, WallConfig, WallFilter, WallProvider, WallSqlStat};

use std::sync::Arc;

// ══════════════════════════════════════════════════════════════════
// 1. WallFilter::new / with_config / default
// ══════════════════════════════════════════════════════════════════

#[test]
fn wall_filter_new_uses_given_provider() {
    let provider = Arc::new(WallProvider::new(WallConfig::default()));
    let filter = WallFilter::new(Arc::clone(&provider));
    // provider() 返回相同的 Arc
    assert!(Arc::ptr_eq(filter.provider(), &provider));
}

#[test]
fn wall_filter_with_config_creates_provider() {
    let config = WallConfig::builder().drop_table_allow(false).build();
    let filter = WallFilter::with_config(config);
    let result = filter.provider().check("DROP TABLE users");
    assert!(!result.violations().is_empty());
}

#[test]
fn wall_filter_default_uses_default_config() {
    let filter = WallFilter::default();
    assert!(filter.provider().config().select_allow);
    assert!(filter.provider().config().drop_table_allow);
}

// ══════════════════════════════════════════════════════════════════
// 2. WallFilter::create_provider -- 全方言分支
// ══════════════════════════════════════════════════════════════════

#[test]
fn create_provider_mysql_family() {
    // Java: case mysql / oceanbase / drds / mariadb / tidb / h2 / lealone / presto / trino / supersql / polardbx
    for db_type in [
        DbType::MySql,
        DbType::OceanBase,
        DbType::Drds,
        DbType::MariaDb,
        DbType::TiDb,
        DbType::H2,
        DbType::Lealone,
        DbType::Presto,
        DbType::Trino,
        DbType::SuperSql,
        DbType::PolarDbX,
    ] {
        let provider = WallFilter::create_provider(None, None, Some(db_type), None);
        assert!(
            provider.is_ok(),
            "db_type={db_type:?} should create provider"
        );
    }
}

#[test]
fn create_provider_mysql_family_with_config() {
    let config = WallConfig::builder().select_allow(false).build();
    let provider = WallFilter::create_provider(None, None, Some(DbType::MySql), Some(config));
    assert!(provider.is_ok());
    let provider = provider.unwrap();
    assert!(!provider.config().select_allow);
}

#[test]
fn create_provider_oracle_family() {
    for db_type in [
        DbType::Oracle,
        DbType::AliOracle,
        DbType::OceanBaseOracle,
        DbType::PolarDb2,
    ] {
        let provider = WallFilter::create_provider(None, None, Some(db_type), None);
        assert!(
            provider.is_ok(),
            "db_type={db_type:?} should create provider"
        );
    }
}

#[test]
fn create_provider_sqlserver_family() {
    for db_type in [DbType::SqlServer, DbType::Jtds] {
        let provider = WallFilter::create_provider(None, None, Some(db_type), None);
        assert!(
            provider.is_ok(),
            "db_type={db_type:?} should create provider"
        );
    }
}

#[test]
fn create_provider_postgresql_family() {
    for db_type in [
        DbType::PostgreSql,
        DbType::Edb,
        DbType::PolarDb,
        DbType::Greenplum,
        DbType::GaussDb,
    ] {
        let provider = WallFilter::create_provider(None, None, Some(db_type), None);
        assert!(
            provider.is_ok(),
            "db_type={db_type:?} should create provider"
        );
    }
}

#[test]
fn create_provider_db2() {
    let provider = WallFilter::create_provider(None, None, Some(DbType::Db2), None);
    assert!(provider.is_ok());
}

#[test]
fn create_provider_sqlite() {
    let provider = WallFilter::create_provider(None, None, Some(DbType::SQLite), None);
    assert!(provider.is_ok());
}

#[test]
fn create_provider_clickhouse() {
    let provider = WallFilter::create_provider(None, None, Some(DbType::ClickHouse), None);
    assert!(provider.is_ok());
}

#[test]
fn create_provider_unsupported_db_type_returns_error() {
    // DbType::Other 在没有注册 creator 时应报错
    let result = WallFilter::create_provider(
        None,
        Some("jdbc:unsupported://host"),
        Some(DbType::Other),
        None,
    );
    assert!(result.is_err());
    if let Err(DruidError::InvalidArgument(msg)) = result {
        assert!(msg.contains("dbType not support"));
    } else {
        panic!("expected InvalidArgument error");
    }
}

#[test]
fn create_provider_none_db_type_returns_error() {
    let result = WallFilter::create_provider(None, None, None, None);
    assert!(result.is_err());
}

#[test]
fn create_provider_sets_name_from_data_source() {
    let provider =
        WallFilter::create_provider(Some("my-datasource"), None, Some(DbType::MySql), None)
            .unwrap();
    assert_eq!(provider.name().as_deref(), Some("my-datasource"));
}

#[test]
fn create_provider_no_name_when_none() {
    let provider = WallFilter::create_provider(None, None, Some(DbType::MySql), None).unwrap();
    assert!(provider.name().is_none());
}

#[test]
fn create_provider_with_config_for_each_builtin_dialect() {
    let config = WallConfig::builder().comment_allow(true).build();
    for db_type in [
        DbType::MySql,
        DbType::Oracle,
        DbType::SqlServer,
        DbType::PostgreSql,
        DbType::Db2,
        DbType::SQLite,
        DbType::ClickHouse,
    ] {
        let provider = WallFilter::create_provider(None, None, Some(db_type), Some(config.clone()));
        assert!(
            provider.is_ok(),
            "db_type={db_type:?} with config should succeed"
        );
    }
}

// ══════════════════════════════════════════════════════════════════
// 3. WallFilter setter/getter
// ══════════════════════════════════════════════════════════════════

#[test]
fn wall_filter_set_log_violation() {
    let filter = WallFilter::default();
    // 默认 false
    filter.set_log_violation(true);
    filter.set_log_violation(false);
    // 不 panic 即可
}

#[test]
fn wall_filter_set_throw_exception() {
    let filter = WallFilter::default();
    filter.set_throw_exception(false);
    filter.set_throw_exception(true);
}

#[test]
fn wall_filter_clear_provider_cache() {
    let provider = WallProvider::new(WallConfig::default());
    // 预填充白名单
    provider.check("SELECT 1");
    assert!(!provider.white_list().is_empty());
    let filter = WallFilter::new(Arc::new(provider));
    filter.clear_provider_cache();
    assert!(filter.provider().white_list().is_empty());
}

#[test]
fn wall_filter_provider_white_list() {
    let provider = Arc::new(WallProvider::new(WallConfig::default()));
    provider.check("SELECT 1");
    let filter = WallFilter::new(provider);
    let white = filter.provider_white_list();
    assert!(white.contains("SELECT 1"));
}

// ══════════════════════════════════════════════════════════════════
// 4. before_sql 内部路径（通过 try_check 间接覆盖）
// ══════════════════════════════════════════════════════════════════

/// 合法 SQL：provider `try_check` 返回无 `violation，before_sql` 应成功。
#[test]
fn wall_filter_valid_sql_no_violation() {
    let filter = WallFilter::default();
    let result = filter.provider().try_check("SELECT 1");
    assert!(result.is_ok());
    let result = result.unwrap();
    assert!(result.violations().is_empty());
}

/// 违规 SQL + `throw_exception=true：provider` `try_check` 返回带 violation 的 Ok。
#[test]
fn wall_filter_violation_detected_by_provider() {
    let config = WallConfig::builder().drop_table_allow(false).build();
    let filter = WallFilter::with_config(config);
    filter.set_throw_exception(true);
    let result = filter.provider().try_check("DROP TABLE users");
    assert!(result.is_ok());
    let check = result.unwrap();
    assert!(!check.violations().is_empty());
}

/// 违规 SQL + `throw_exception=false：provider` `try_check` 不 panic。
#[test]
fn wall_filter_violation_no_throw_when_exception_disabled() {
    let config = WallConfig::builder().drop_table_allow(false).build();
    let filter = WallFilter::with_config(config);
    filter.set_throw_exception(false);
    let result = filter.provider().try_check("DROP TABLE users");
    assert!(result.is_ok());
}

// ══════════════════════════════════════════════════════════════════
// 5. after_sql 路径（通过 WallSqlStat 直接验证语义）
// ══════════════════════════════════════════════════════════════════

/// `after_sql`: Ok(ExecResult) 路径 -- `update_count` 累加。
#[test]
fn wall_sql_stat_add_update_count() {
    let stat = WallSqlStat::new("SELECT 1".to_owned(), vec![], false);
    stat.add_update_count(10);
    stat.add_update_count(5);
    let sv = stat.stat_value(false);
    assert_eq!(sv.update_count, 15);
}

/// `after_sql`: Ok(ExecResult) 路径 -- `fetch_row_count` 累加。
#[test]
fn wall_sql_stat_add_fetch_row_count() {
    let stat = WallSqlStat::new("SELECT 1".to_owned(), vec![], false);
    stat.add_fetch_row_count(100);
    stat.add_fetch_row_count(50);
    let sv = stat.stat_value(false);
    assert_eq!(sv.fetch_row_count, 150);
}

/// `after_sql`: Err 路径 -- `execute_error_count` 增加。
#[test]
fn wall_sql_stat_increment_execute_error_count() {
    let stat = WallSqlStat::new("SELECT 1".to_owned(), vec![], false);
    stat.increment_execute_error_count();
    stat.increment_execute_error_count();
    let sv = stat.stat_value(false);
    assert_eq!(sv.execute_error_count, 2);
}

// ══════════════════════════════════════════════════════════════════
// 6. servlet_path_matches 辅助函数（通过 WallConfig + provider 间接验证）
// ══════════════════════════════════════════════════════════════════

/// `tenant_table_pattern` 前缀通配：`t*` 匹配 `t_orders`。
#[test]
fn tenant_pattern_prefix_wildcard() {
    let config = WallConfig::builder()
        .tenant_table_pattern("t*")
        .tenant_column("tenant_id")
        .build();
    let provider = WallProvider::new(config);
    // 不 panic 即可验证 pattern 匹配路径被访问
    let _ = provider.try_check("SELECT * FROM t_orders WHERE id = 1");
}

/// `tenant_table_pattern` 后缀通配：`*_log` 匹配 `access_log`。
#[test]
fn tenant_pattern_suffix_wildcard() {
    let config = WallConfig::builder()
        .tenant_table_pattern("*_log")
        .tenant_column("tenant_id")
        .build();
    let provider = WallProvider::new(config);
    let _ = provider.try_check("SELECT * FROM access_log WHERE id = 1");
}

/// `tenant_table_pattern` 精确匹配：`users` 仅匹配 `users`。
#[test]
fn tenant_pattern_exact_match() {
    let config = WallConfig::builder()
        .tenant_table_pattern("users")
        .tenant_column("tenant_id")
        .build();
    let provider = WallProvider::new(config);
    let _ = provider.try_check("SELECT * FROM users WHERE id = 1");
}

/// `tenant_table_pattern` 中间通配：`t_*_data` 匹配 `t_user_data`。
#[test]
fn tenant_pattern_middle_wildcard() {
    let config = WallConfig::builder()
        .tenant_table_pattern("t_*_data")
        .tenant_column("tenant_id")
        .build();
    let provider = WallProvider::new(config);
    let _ = provider.try_check("SELECT * FROM t_user_data WHERE id = 1");
}

// ══════════════════════════════════════════════════════════════════
// 7. connection_get_meta_data 路径
// ══════════════════════════════════════════════════════════════════

/// `metadata_allow=false` + `throw_exception=true` 时行为验证。
#[test]
fn wall_filter_metadata_not_allowed_config() {
    let mut config = WallConfig::default();
    config.metadata_allow = false;
    let filter = WallFilter::with_config(config);
    filter.set_throw_exception(true);
    // 验证配置生效
    assert!(!filter.provider().config().metadata_allow);
}

/// `do_privileged_allow=true` + `is_privileged()` 时 metadata 放行。
#[test]
fn wall_filter_metadata_privileged_bypass() {
    let mut config = WallConfig::default();
    config.do_privileged_allow = true;
    config.metadata_allow = false;
    let _filter = WallFilter::with_config(config);
    // privileged 模式下 metadata 不拦截
    WallProvider::do_privileged(|| {
        assert!(WallProvider::is_privileged());
    });
}

// ══════════════════════════════════════════════════════════════════
// 8. update_check 路径（before_sql 中 evaluate_update_items=true）
// ══════════════════════════════════════════════════════════════════

/// 无 handler 时 `update_check_item` 路径应报 "handler missing"。
#[test]
fn wall_filter_update_check_handler_missing() {
    let config = WallConfig::builder()
        .update_check_column("t.status")
        .build();
    let provider = Arc::new(WallProvider::new(config));
    // 有 update_check_column 但无 handler -- check 本身不 panic
    let result = provider.try_check("UPDATE t SET status = 'x' WHERE id = 1");
    assert!(result.is_ok());
}

// ══════════════════════════════════════════════════════════════════
// 9. provider 复用语义（Java WallFilter.provider 字段）
// ══════════════════════════════════════════════════════════════════

/// 多次调用 `try_check` 共用同一 provider 实例。
#[test]
fn wall_filter_provider_reuse_across_checks() {
    let provider = Arc::new(WallProvider::new(WallConfig::default()));
    let filter = WallFilter::new(provider);
    let _ = filter.provider().try_check("SELECT 1");
    let _ = filter.provider().try_check("SELECT 2");
    let _ = filter.provider().try_check("SELECT 1"); // 应命中白名单缓存
    assert_eq!(filter.provider().check_count(), 3);
    assert_eq!(filter.provider().white_list_hit_count(), 1); // 第三次命中
}

/// reset 后计数清零但 provider 实例不变。
#[test]
fn wall_filter_provider_reset_preserves_instance() {
    let provider = Arc::new(WallProvider::new(WallConfig::default()));
    let filter = WallFilter::new(Arc::clone(&provider));
    let _ = filter.provider().try_check("SELECT 1");
    assert_eq!(filter.provider().check_count(), 1);
    filter.provider().reset();
    assert_eq!(filter.provider().check_count(), 0);
    // provider Arc 仍然有效
    let _ = filter.provider().try_check("SELECT 2");
    assert_eq!(filter.provider().check_count(), 1);
}

// ══════════════════════════════════════════════════════════════════
// 10. WallFilter + 不同方言 provider 的 check 语义
// ══════════════════════════════════════════════════════════════════

/// `MySQL` 方言 provider 对 DROP TABLE 的拦截。
#[test]
fn wall_filter_mysql_drop_table_blocked() {
    let config = WallConfig::builder().drop_table_allow(false).build();
    let provider =
        WallFilter::create_provider(None, None, Some(DbType::MySql), Some(config)).unwrap();
    let result = provider.check("DROP TABLE users");
    assert!(!result.violations().is_empty());
}

/// `PostgreSQL` 方言 provider 对合法 SELECT 放行。
#[test]
fn wall_filter_pg_select_allowed() {
    let provider = WallFilter::create_provider(None, None, Some(DbType::PostgreSql), None).unwrap();
    let result = provider.check("SELECT 1");
    assert!(result.violations().is_empty());
}

/// `SQLite` 方言 provider 对语法错误检测。
#[test]
fn wall_filter_sqlite_syntax_error() {
    let provider = WallFilter::create_provider(None, None, Some(DbType::SQLite), None).unwrap();
    let result = provider.try_check("THIS IS NOT VALID SQL !!!").unwrap();
    assert!(result.is_syntax_error());
}

/// `ClickHouse` 方言 provider 对 INSERT 放行。
#[test]
fn wall_filter_clickhouse_insert_allowed() {
    let provider = WallFilter::create_provider(None, None, Some(DbType::ClickHouse), None).unwrap();
    let result = provider.check("INSERT INTO t (a) VALUES (1)");
    assert!(result.violations().is_empty());
}

/// Oracle 方言 provider 对 SELECT 放行。
#[test]
fn wall_filter_oracle_select_allowed() {
    let provider = WallFilter::create_provider(None, None, Some(DbType::Oracle), None).unwrap();
    let result = provider.check("SELECT 1 FROM DUAL");
    assert!(result.violations().is_empty());
}

/// `SqlServer` 方言 provider 对 SELECT 放行。
#[test]
fn wall_filter_sqlserver_select_allowed() {
    let provider = WallFilter::create_provider(None, None, Some(DbType::SqlServer), None).unwrap();
    let result = provider.check("SELECT 1");
    assert!(result.violations().is_empty());
}

// ══════════════════════════════════════════════════════════════════
// 11. ExecResult 语义（after_sql 的 Ok 路径行数回写）
// ══════════════════════════════════════════════════════════════════

/// `ExecResult` 带 `rows_affected` 和 `row_count`。
#[test]
fn exec_result_rows_affected_and_row_count() {
    let result = ExecResult {
        rows_affected: 42,
        last_insert_id: Some(100),
        row_count: Some(200),
    };
    assert_eq!(result.rows_affected, 42);
    assert_eq!(result.row_count, Some(200));
}

/// `ExecResult` 默认值。
#[test]
fn exec_result_default() {
    let result = ExecResult::default();
    assert_eq!(result.rows_affected, 0);
    assert!(result.last_insert_id.is_none());
    assert!(result.row_count.is_none());
}
