//! `WallTableStat` + `WallSqlStat` + `PhysicalDatabaseMetaData` 差分测试
//! （C9 批次：sql + core 0% 文件）。
//!
//! Java 基线：`33824c3dec1612711f9bb4e409319bcab2e4cd0e`。

extern crate druid_core as druid;
use druid::core::PhysicalDatabaseMetaData;
use druid::sql::{WallSqlStat, WallSqlTableStat, WallTableStat, WallViolation};
use std::collections::HashMap;

// ── WallTableStat ─────────────────────────────────────────

#[test]
fn wall_table_stat_add_sql_table_stat() {
    let stat = WallTableStat::default();
    let mut sql_table = WallSqlTableStat::default();
    sql_table.increment_select_count();
    sql_table.increment_select_count();
    sql_table.increment_insert_count();
    stat.add_sql_table_stat(&sql_table);

    let value = stat.stat_value("users".to_owned(), false);
    assert_eq!(value.select_count, 2);
    assert_eq!(value.insert_count, 1);
    assert_eq!(value.delete_count, 0);
}

#[test]
fn wall_table_stat_data_counters() {
    let stat = WallTableStat::default();
    stat.add_fetch_row_count(100);
    stat.add_fetch_row_count(200);
    stat.add_update_data_count(50);
    stat.add_delete_data_count(30);
    stat.add_insert_data_count(20);

    let value = stat.stat_value("orders".to_owned(), false);
    assert_eq!(value.fetch_row_count, 300);
    assert_eq!(value.update_data_count, 50);
    assert_eq!(value.delete_data_count, 30);
    assert_eq!(value.insert_data_count, 20);
}

#[test]
fn wall_table_stat_stat_value_and_reset() {
    let stat = WallTableStat::default();
    stat.add_fetch_row_count(10);

    let value = stat.stat_value("t".to_owned(), false);
    assert_eq!(value.fetch_row_count, 10);

    let value_reset = stat.stat_value("t".to_owned(), true);
    assert_eq!(value_reset.fetch_row_count, 10);
    let value_after = stat.stat_value("t".to_owned(), false);
    assert_eq!(value_after.fetch_row_count, 0);
}

// ── WallSqlStat ──────────────────────────────────────────

#[test]
fn wall_sql_stat_new_and_getters() {
    let violations = vec![WallViolation::DeleteWithoutWhere];
    let stat = WallSqlStat::new("DELETE FROM t".to_owned(), violations, false);
    assert_eq!(stat.sql(), "DELETE FROM t");
    assert_eq!(stat.violations().len(), 1);
    assert!(!stat.is_syntax_error());
    assert!(stat.table_stats().is_empty());
    assert!(stat.function_stats().is_empty());
}

#[test]
fn wall_sql_stat_new_with_stats() {
    let mut table_stats = HashMap::new();
    table_stats.insert("users".to_owned(), WallSqlTableStat::default());
    let mut function_stats = HashMap::new();
    function_stats.insert(
        "count".to_owned(),
        druid::sql::WallSqlFunctionStat::default(),
    );

    let stat = WallSqlStat::new_with_stats(
        "SELECT count(*) FROM users".to_owned(),
        vec![],
        false,
        table_stats,
        function_stats,
    );
    assert_eq!(stat.table_stats().len(), 1);
    assert!(stat.table_stats().contains_key("users"));
    assert_eq!(stat.function_stats().len(), 1);
    assert!(stat.function_stats().contains_key("count"));
}

#[test]
fn wall_sql_stat_execute_counters() {
    let stat = WallSqlStat::new("SELECT 1".to_owned(), vec![], false);
    assert_eq!(stat.increment_execute_count(), 1);
    assert_eq!(stat.increment_execute_count(), 2);
    assert_eq!(stat.increment_execute_error_count(), 1);
    assert_eq!(stat.increment_execute_error_count(), 2);
}

#[test]
fn wall_sql_stat_fetch_and_update_counters() {
    let stat = WallSqlStat::new("SELECT 1".to_owned(), vec![], false);
    stat.add_fetch_row_count(10);
    stat.add_fetch_row_count(20);
    stat.add_update_count(5);
    stat.add_update_count(15);
    let sv = stat.stat_value(false);
    assert_eq!(sv.fetch_row_count, 30);
    assert_eq!(sv.update_count, 20);
}

#[test]
fn wall_sql_stat_stat_value() {
    let stat = WallSqlStat::new("SELECT 1".to_owned(), vec![], false);
    stat.increment_execute_count();
    stat.increment_execute_error_count();
    stat.add_fetch_row_count(5);
    stat.add_update_count(3);

    let sv = stat.stat_value(false);
    assert_eq!(sv.execute_count, 1);
    assert_eq!(sv.execute_error_count, 1);
    assert_eq!(sv.fetch_row_count, 5);
    assert_eq!(sv.update_count, 3);
}

#[test]
fn wall_sql_stat_syntax_error() {
    let stat = WallSqlStat::new(
        "INVALID SQL".to_owned(),
        vec![WallViolation::SyntaxError("parse error".to_owned())],
        true,
    );
    assert!(stat.is_syntax_error());
    assert_eq!(stat.violations().len(), 1);
}

// ── WallSqlTableStat ─────────────────────────────────────

#[test]
fn wall_sql_table_stat_all_counters() {
    let mut stat = WallSqlTableStat::default();
    stat.increment_select_count();
    stat.increment_select_count();
    stat.increment_insert_count();
    stat.increment_update_count();
    stat.increment_delete_count();

    let table_stat = WallTableStat::default();
    table_stat.add_sql_table_stat(&stat);
    let value = table_stat.stat_value("t".to_owned(), false);
    assert_eq!(value.select_count, 2);
    assert_eq!(value.insert_count, 1);
    assert_eq!(value.update_count, 1);
    assert_eq!(value.delete_count, 1);
}

// ── PhysicalDatabaseMetaData ─────────────────────────────

struct MockMetaData;
#[async_trait::async_trait]
impl PhysicalDatabaseMetaData for MockMetaData {}

/// 默认方法返回 UnsupportedOperation（Java `SQLFeatureNotSupportedException` 语义）。
#[tokio::test]
async fn physical_database_meta_data_default_methods() {
    let mut md = MockMetaData;

    let err = md.all_procedures_are_callable().await.unwrap_err();
    assert!(matches!(
        err,
        druid::core::DruidError::UnsupportedOperation { .. }
    ));

    let err = md.get_url().await.unwrap_err();
    assert!(matches!(
        err,
        druid::core::DruidError::UnsupportedOperation { .. }
    ));

    let err = md.get_user_name().await.unwrap_err();
    assert!(matches!(
        err,
        druid::core::DruidError::UnsupportedOperation { .. }
    ));

    let err = md.is_read_only().await.unwrap_err();
    assert!(matches!(
        err,
        druid::core::DruidError::UnsupportedOperation { .. }
    ));

    let err = md.get_database_product_name().await.unwrap_err();
    assert!(matches!(
        err,
        druid::core::DruidError::UnsupportedOperation { .. }
    ));

    let err = md.get_driver_name().await.unwrap_err();
    assert!(matches!(
        err,
        druid::core::DruidError::UnsupportedOperation { .. }
    ));

    let err = md.supports_transactions().await.unwrap_err();
    assert!(matches!(
        err,
        druid::core::DruidError::UnsupportedOperation { .. }
    ));

    let err = md.get_schemas().await.unwrap_err();
    assert!(matches!(
        err,
        druid::core::DruidError::UnsupportedOperation { .. }
    ));

    let err = md.get_schemas().await.unwrap_err();
    assert!(matches!(
        err,
        druid::core::DruidError::UnsupportedOperation { .. }
    ));

    let err = md.supports_batch_updates().await.unwrap_err();
    assert!(matches!(
        err,
        druid::core::DruidError::UnsupportedOperation { .. }
    ));
}
