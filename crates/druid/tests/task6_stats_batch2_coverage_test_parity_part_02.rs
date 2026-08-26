//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
#![allow(clippy::approx_constant)]
//! Batch 2 coverage tests for stats module deep branches.
//!
//! Targets uncovered branches in:
//! - `stat_filter.rs`: `log_slow_sql` levels, `effective_sql/context_identity` via `StatFilterContext`,
//!   `slow_value/slow_prepared_parameter` all variants, `json_rdbc_string` truncation,
//!   `json_decimal` failure path, `resource_marker` Some path
//! - merge.rs: parameterize edge cases (hex, scientific, negative, bracket identifier,
//!   block comment, escaped quote), `SqlMerger` capacity eviction, `set_max_sql_size` shrink, reset
//! - `druid_stat_manager_facade.rs`: `merge_wall_stat/merge_wall_value/merge_black_list/merge_named_list`
//! - `druid_stat_service.rs`: page nested key, `sql_detail` `MaxTimespanOccurTime`, wall sort

use druid::core::{
    AfterFilter, BatchExecContext, BatchExecKind, BeforeFilter, DruidError, ExecContext,
    ExecOperation, ExecResult, PreparedInputParameter, PreparedTypeNameArgument,
    RdbcCalendarArgument, RdbcCharacterLength, RdbcError, RdbcObject, RdbcStreamLength, ResultSet,
    ResultSetFilter, ResultSetFilterContext, Value,
};
use druid::stats::{RdbcStatManager, StatFilter, StatsCollector};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

// ===========================================================================
// Helpers
// ===========================================================================

fn make_filter() -> (StatFilter, Arc<StatsCollector>) {
    let collector = Arc::new(StatsCollector::new("test", Duration::from_secs(10)));
    let filter = StatFilter::new(Arc::clone(&collector));
    (filter, collector)
}

fn make_exec_context(sql: &str, operation: ExecOperation, in_transaction: bool) -> ExecContext<'_> {
    ExecContext {
        connection_id: 1,
        statement_id: Some(1),
        sql: sql.to_owned(),
        params: &[],
        prepared_parameters: None,
        data_source: "test",
        start: Instant::now(),
        fingerprint: None,
        in_transaction,
        operation,
    }
}

fn make_exec_context_with_params<'a>(
    sql: &'a str,
    operation: ExecOperation,
    params: &'a [Value],
    prepared: Option<&'a [PreparedInputParameter]>,
) -> ExecContext<'a> {
    ExecContext {
        connection_id: 1,
        statement_id: Some(1),
        sql: sql.to_owned(),
        params,
        prepared_parameters: prepared,
        data_source: "test",
        start: Instant::now(),
        fingerprint: None,
        in_transaction: false,
        operation,
    }
}

fn make_batch_context<'a>(
    sql: &'a str,
    statements: &'a [String],
    kind: BatchExecKind,
) -> BatchExecContext<'a> {
    BatchExecContext {
        connection_id: 1,
        statement_id: Some(1),
        sql,
        statements,
        parameter_sets: &[],
        prepared_parameter_sets: None,
        kind,
        data_source: "test",
        start: Instant::now(),
        fingerprint: None,
        in_transaction: false,
    }
}

#[derive(Debug)]
struct EmptyResultSet;
druid_rdbc::impl_standard_wrapper!(EmptyResultSet => ResultSet);
#[async_trait::async_trait]
impl druid::core::ResultSet for EmptyResultSet {
    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }
    async fn next(&mut self) -> Result<bool, RdbcError> {
        Ok(false)
    }
}

// ===========================================================================
// 1. log_slow_sql each level branch
// ===========================================================================

// ===========================================================================
// 2. effective_sql / context_identity via RdbcStatManager::set_stat_context
// ===========================================================================

// ===========================================================================
// 3. slow_value / slow_prepared_parameter all variants
// ===========================================================================

// ===========================================================================
// 4. json_rdbc_string truncation (> 100 UTF-16 code units)
// ===========================================================================

// ===========================================================================
// 5. result_set_close with close_count == 0 and SQL stat association
// ===========================================================================

// ===========================================================================
// 6. after_batch slow SQL with prepared parameters
// ===========================================================================

// ===========================================================================
// 7. after_batch error path with Statement kind
// ===========================================================================

// ===========================================================================
// 8. after Execute with error (row_count None, error path)
// ===========================================================================

// ===========================================================================
// 9. after_batch empty parameter_sets (last() returns None)
// ===========================================================================

// ===========================================================================
// 10. config_from_properties whitespace-padded slowSqlMillis
// ===========================================================================

// ===========================================================================
// 11. slow_sql_millis boundary: exactly at threshold
// ===========================================================================

// ===========================================================================
// 12. after Query with row_count None (streaming)
// ===========================================================================

// ===========================================================================
// 13. slow_prepared_parameter with Some values for Date/Time/Timestamp/Stream
// ===========================================================================

// ===========================================================================
// 14. slow_value Date/Time/Timestamp with Some values
// ===========================================================================

#[tokio::test]
async fn task6_stats_batch2_coverage_test_slow_value_date_time_timestamp() {
    use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
    let (filter, _collector) = make_filter();
    filter.set_slow_sql_millis(0);
    filter.set_log_slow_sql(false);
    let params = vec![
        Value::Date(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap()),
        Value::Time(NaiveTime::from_hms_opt(10, 30, 0).unwrap()),
        Value::Timestamp(
            NaiveDateTime::parse_from_str("2024-01-15 10:30:00", "%Y-%m-%d %H:%M:%S").unwrap(),
        ),
    ];
    let mut ctx = make_exec_context_with_params("SELECT 1", ExecOperation::Query, &params, None);
    filter.before(&mut ctx).await.unwrap();
    let result = Ok(ExecResult {
        rows_affected: 0,
        last_insert_id: None,
        row_count: Some(1),
    });
    filter
        .after(&ctx, &result, Duration::from_millis(50))
        .await
        .unwrap();
}
