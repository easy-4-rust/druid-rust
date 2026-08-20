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

extern crate druid_core as druid;
use druid_core::core::{
    AfterFilter, BatchExecContext, BatchExecKind, BeforeFilter, DruidError, ExecContext,
    ExecOperation, ExecResult, PreparedInputParameter, PreparedTypeNameArgument,
    RdbcCalendarArgument, RdbcCharacterLength, RdbcObject, RdbcStreamLength, ResultSetFilter,
    ResultSetFilterChain, ResultSetFilterContext, Value,
};
use druid_core::stats::{RdbcStatManager, StatFilter, StatsCollector};
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

fn make_exec_context(
    sql: &str,
    operation: ExecOperation,
    in_transaction: bool,
) -> ExecContext<'_> {
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
impl druid_core::core::PhysicalResultSet for EmptyResultSet {
    fn close(&self) -> Result<(), DruidError> {
        Ok(())
    }
    fn is_closed(&self) -> bool {
        false
    }
    fn next(&self) -> Result<bool, DruidError> {
        Ok(false)
    }
}

// ===========================================================================
// 1. log_slow_sql each level branch
// ===========================================================================

#[tokio::test]
async fn log_slow_sql_warn_level() {
    let (filter, _collector) = make_filter();
    filter.set_slow_sql_millis(0);
    filter.set_log_slow_sql(true);
    filter.set_slow_sql_log_level("WARN");
    let mut ctx = make_exec_context("SELECT 1", ExecOperation::Query, false);
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

#[tokio::test]
async fn log_slow_sql_info_level() {
    let (filter, _collector) = make_filter();
    filter.set_slow_sql_millis(0);
    filter.set_log_slow_sql(true);
    filter.set_slow_sql_log_level("INFO");
    let mut ctx = make_exec_context("SELECT 1", ExecOperation::Query, false);
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

#[tokio::test]
async fn log_slow_sql_debug_level() {
    let (filter, _collector) = make_filter();
    filter.set_slow_sql_millis(0);
    filter.set_log_slow_sql(true);
    filter.set_slow_sql_log_level("DEBUG");
    let mut ctx = make_exec_context("SELECT 1", ExecOperation::Query, false);
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

#[tokio::test]
async fn log_slow_sql_error_level_default() {
    let (filter, _collector) = make_filter();
    filter.set_slow_sql_millis(0);
    filter.set_log_slow_sql(true);
    let mut ctx = make_exec_context("SELECT 1", ExecOperation::Query, false);
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

// ===========================================================================
// 2. effective_sql / context_identity via RdbcStatManager::set_stat_context
// ===========================================================================

#[tokio::test]
async fn effective_sql_with_stat_context_sql() {
    let (filter, _collector) = make_filter();
    let mut ctx_data = druid_core::stats::RdbcStatContext::new();
    ctx_data.set_sql(Some("SELECT /* overridden */ 1".to_owned()));
    RdbcStatManager::global().set_stat_context(Some(ctx_data));
    let mut ctx = make_exec_context("SELECT original", ExecOperation::Query, false);
    filter.before(&mut ctx).await.unwrap();
    let result = Ok(ExecResult {
        rows_affected: 0,
        last_insert_id: None,
        row_count: Some(1),
    });
    filter
        .after(&ctx, &result, Duration::from_millis(5))
        .await
        .unwrap();
    RdbcStatManager::global().set_stat_context(None);
}

#[tokio::test]
async fn effective_sql_with_empty_context_sql_falls_back() {
    let (filter, _collector) = make_filter();
    let mut ctx_data = druid_core::stats::RdbcStatContext::new();
    ctx_data.set_sql(Some(String::new()));
    RdbcStatManager::global().set_stat_context(Some(ctx_data));
    let mut ctx = make_exec_context("SELECT fallback", ExecOperation::Query, false);
    filter.before(&mut ctx).await.unwrap();
    let result = Ok(ExecResult {
        rows_affected: 0,
        last_insert_id: None,
        row_count: Some(1),
    });
    filter
        .after(&ctx, &result, Duration::from_millis(5))
        .await
        .unwrap();
    RdbcStatManager::global().set_stat_context(None);
}

#[tokio::test]
async fn context_identity_with_name_and_file() {
    let (filter, _collector) = make_filter();
    let mut ctx_data = druid_core::stats::RdbcStatContext::new();
    ctx_data.set_name(Some("my-service".to_owned()));
    ctx_data.set_file(Some("handler.rs".to_owned()));
    RdbcStatManager::global().set_stat_context(Some(ctx_data));
    let mut ctx = make_exec_context("SELECT 1", ExecOperation::Query, false);
    filter.before(&mut ctx).await.unwrap();
    let result = Ok(ExecResult {
        rows_affected: 0,
        last_insert_id: None,
        row_count: Some(1),
    });
    filter
        .after(&ctx, &result, Duration::from_millis(5))
        .await
        .unwrap();
    RdbcStatManager::global().set_stat_context(None);
}

// ===========================================================================
// 3. slow_value / slow_prepared_parameter all variants
// ===========================================================================

#[tokio::test]
async fn slow_parameters_all_value_types() {
    let (filter, _collector) = make_filter();
    filter.set_slow_sql_millis(0);
    filter.set_log_slow_sql(false);
    let params = vec![
        Value::Null,
        Value::Bool(true),
        Value::Int(42),
        Value::Float(3.14),
        Value::Decimal("123.456".parse().unwrap()),
        Value::String("hello".to_owned()),
        Value::Bytes(vec![1, 2, 3]),
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

#[tokio::test]
async fn slow_prepared_parameter_all_types() {
    let (filter, _collector) = make_filter();
    filter.set_slow_sql_millis(0);
    filter.set_log_slow_sql(false);
    let prepared = vec![
        PreparedInputParameter::RustValue(Value::Int(1)),
        PreparedInputParameter::Null {
            sql_type: 0,
            type_name: PreparedTypeNameArgument::Unspecified,
        },
        PreparedInputParameter::Boolean(true),
        PreparedInputParameter::Byte(1),
        PreparedInputParameter::Short(2),
        PreparedInputParameter::Int(3),
        PreparedInputParameter::Long(4),
        PreparedInputParameter::Float(1.5),
        PreparedInputParameter::Double(2.5),
        PreparedInputParameter::BigDecimal(Some("99.9".parse().unwrap())),
        PreparedInputParameter::BigDecimal(None),
        PreparedInputParameter::String(Some("text".to_owned())),
        PreparedInputParameter::String(None),
        PreparedInputParameter::NString(Some("ntext".to_owned())),
        PreparedInputParameter::Bytes(Some(vec![1, 2])),
        PreparedInputParameter::Bytes(None),
        PreparedInputParameter::Date {
            value: None,
            calendar: RdbcCalendarArgument::Unspecified,
        },
        PreparedInputParameter::Time {
            value: None,
            calendar: RdbcCalendarArgument::Unspecified,
        },
        PreparedInputParameter::Timestamp {
            value: None,
            calendar: RdbcCalendarArgument::Unspecified,
        },
        PreparedInputParameter::AsciiStream {
            stream: None,
            length: RdbcStreamLength::Unspecified,
        },
        PreparedInputParameter::CharacterStream {
            reader: None,
            length: RdbcCharacterLength::Unspecified,
        },
        PreparedInputParameter::Object {
            value: None,
            target_sql_type: None,
            scale_or_length: None,
        },
        PreparedInputParameter::Ref(None),
        PreparedInputParameter::Blob(None),
        PreparedInputParameter::Clob(None),
        PreparedInputParameter::NClob(None),
        PreparedInputParameter::Array(None),
        PreparedInputParameter::Url(None),
        PreparedInputParameter::RowId(None),
        PreparedInputParameter::SqlXml(None),
    ];
    let mut ctx =
        make_exec_context_with_params("SELECT 1", ExecOperation::Query, &[], Some(&prepared));
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

#[tokio::test]
async fn slow_rdbc_object_scalar_and_string_variants() {
    let (filter, _collector) = make_filter();
    filter.set_slow_sql_millis(0);
    filter.set_log_slow_sql(false);
    let prepared = vec![
        PreparedInputParameter::Object {
            value: Some(RdbcObject::Scalar(Value::Int(1))),
            target_sql_type: None,
            scale_or_length: None,
        },
        PreparedInputParameter::Object {
            value: Some(RdbcObject::String("s".to_owned())),
            target_sql_type: None,
            scale_or_length: None,
        },
        PreparedInputParameter::Object {
            value: Some(RdbcObject::NString("ns".to_owned())),
            target_sql_type: None,
            scale_or_length: None,
        },
        PreparedInputParameter::Object {
            value: Some(RdbcObject::Boolean(true)),
            target_sql_type: None,
            scale_or_length: None,
        },
        PreparedInputParameter::Object {
            value: Some(RdbcObject::Byte(1)),
            target_sql_type: None,
            scale_or_length: None,
        },
        PreparedInputParameter::Object {
            value: Some(RdbcObject::Short(2)),
            target_sql_type: None,
            scale_or_length: None,
        },
        PreparedInputParameter::Object {
            value: Some(RdbcObject::Integer(3)),
            target_sql_type: None,
            scale_or_length: None,
        },
        PreparedInputParameter::Object {
            value: Some(RdbcObject::Long(4)),
            target_sql_type: None,
            scale_or_length: None,
        },
        PreparedInputParameter::Object {
            value: Some(RdbcObject::Float(1.5)),
            target_sql_type: None,
            scale_or_length: None,
        },
        PreparedInputParameter::Object {
            value: Some(RdbcObject::Double(2.5)),
            target_sql_type: None,
            scale_or_length: None,
        },
        PreparedInputParameter::Object {
            value: Some(RdbcObject::BigDecimal("99.9".parse().unwrap())),
            target_sql_type: None,
            scale_or_length: None,
        },
        PreparedInputParameter::Object {
            value: Some(RdbcObject::Bytes(vec![1, 2])),
            target_sql_type: None,
            scale_or_length: None,
        },
    ];
    let mut ctx =
        make_exec_context_with_params("SELECT 1", ExecOperation::Query, &[], Some(&prepared));
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

// ===========================================================================
// 4. json_rdbc_string truncation (> 100 UTF-16 code units)
// ===========================================================================

#[tokio::test]
async fn slow_string_truncation_over_100_chars() {
    let (filter, _collector) = make_filter();
    filter.set_slow_sql_millis(0);
    filter.set_log_slow_sql(false);
    let long_string = "a".repeat(200);
    let params = vec![Value::String(long_string)];
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

#[tokio::test]
async fn slow_string_exact_100_chars_no_truncation() {
    let (filter, _collector) = make_filter();
    filter.set_slow_sql_millis(0);
    filter.set_log_slow_sql(false);
    let exact_100 = "b".repeat(100);
    let params = vec![Value::String(exact_100)];
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

// ===========================================================================
// 5. result_set_close with close_count == 0 and SQL stat association
// ===========================================================================

#[test]
fn result_set_close_with_sql_stat_association() {
    let (filter, collector) = make_filter();
    collector
        .sql_merger
        .record("SELECT 1", Duration::from_millis(1), true);
    let physical = EmptyResultSet;
    let context = ResultSetFilterContext::with_sql_and_execute_elapsed(
        Some("SELECT 1".to_owned()),
        Some(Duration::from_millis(10)),
    );
    context.record_fetch_row_count(5);
    context.add_read_string_length("test");
    context.add_read_bytes_length(100);
    context.increment_open_input_stream_count();
    context.increment_open_reader_count();
    let filters: Vec<Arc<dyn druid_core::core::ResultSetFilter>> = vec![];
    let mut chain = ResultSetFilterChain::new(&filters, &physical, &context);
    filter.result_set_close(&mut chain).unwrap();
}

#[test]
fn result_set_close_with_merge_sql_and_sql_stat() {
    let (filter, collector) = make_filter();
    filter.set_merge_sql(true);
    collector.sql_merger.record_with_merge(
        "SELECT * FROM t WHERE id = 42",
        Duration::from_millis(1),
        true,
        true,
    );
    let physical = EmptyResultSet;
    let context = ResultSetFilterContext::with_sql_and_execute_elapsed(
        Some("SELECT * FROM t WHERE id = 42".to_owned()),
        Some(Duration::from_millis(10)),
    );
    context.record_fetch_row_count(3);
    let filters: Vec<Arc<dyn druid_core::core::ResultSetFilter>> = vec![];
    let mut chain = ResultSetFilterChain::new(&filters, &physical, &context);
    filter.result_set_close(&mut chain).unwrap();
}

// ===========================================================================
// 6. after_batch slow SQL with prepared parameters
// ===========================================================================

#[tokio::test]
async fn after_batch_slow_with_prepared_params() {
    let (filter, _collector) = make_filter();
    filter.set_slow_sql_millis(0);
    filter.set_log_slow_sql(false);
    let statements = vec!["INSERT INTO t VALUES (?)".to_string()];
    let prepared_sets: Vec<Vec<PreparedInputParameter>> =
        vec![vec![PreparedInputParameter::Int(42)]];
    let mut ctx = BatchExecContext {
        prepared_parameter_sets: Some(&prepared_sets),
        ..make_batch_context(
            "INSERT INTO t VALUES (?)",
            &statements,
            BatchExecKind::PreparedStatement,
        )
    };
    filter.before_batch(&mut ctx).await.unwrap();
    let result = Ok(vec![1]);
    filter
        .after_batch(&ctx, &result, Duration::from_millis(100))
        .await
        .unwrap();
}

#[tokio::test]
async fn after_batch_slow_without_prepared_params() {
    let (filter, _collector) = make_filter();
    filter.set_slow_sql_millis(0);
    filter.set_log_slow_sql(false);
    let statements = vec!["INSERT INTO t VALUES (1)".to_string()];
    let mut ctx = make_batch_context(
        "INSERT INTO t VALUES (1)",
        &statements,
        BatchExecKind::Statement,
    );
    filter.before_batch(&mut ctx).await.unwrap();
    let result = Ok(vec![1]);
    filter
        .after_batch(&ctx, &result, Duration::from_millis(100))
        .await
        .unwrap();
}

// ===========================================================================
// 7. after_batch error path with Statement kind
// ===========================================================================

#[tokio::test]
async fn after_batch_error_statement_kind_sets_sql() {
    let (filter, _collector) = make_filter();
    let statements = vec!["INSERT INTO t VALUES (1)".to_string()];
    let mut ctx = make_batch_context(
        "INSERT INTO t VALUES (1)",
        &statements,
        BatchExecKind::Statement,
    );
    filter.before_batch(&mut ctx).await.unwrap();
    let result: Result<Vec<i32>, DruidError> = Err(DruidError::Other("batch error".to_owned()));
    filter
        .after_batch(&ctx, &result, Duration::from_millis(5))
        .await
        .unwrap();
}

// ===========================================================================
// 8. after Execute with error (row_count None, error path)
// ===========================================================================

#[tokio::test]
async fn after_execute_error_records_hold_time() {
    let (filter, _collector) = make_filter();
    let mut ctx = make_exec_context("INVALID", ExecOperation::Execute, false);
    filter.before(&mut ctx).await.unwrap();
    let result: Result<ExecResult, DruidError> = Err(DruidError::Other("error".to_owned()));
    filter
        .after(&ctx, &result, Duration::from_millis(5))
        .await
        .unwrap();
}

// ===========================================================================
// 9. after_batch empty parameter_sets (last() returns None)
// ===========================================================================

#[tokio::test]
async fn after_batch_slow_empty_parameter_sets() {
    let (filter, _collector) = make_filter();
    filter.set_slow_sql_millis(0);
    filter.set_log_slow_sql(false);
    let statements = vec!["SELECT 1".to_string()];
    let ctx = make_batch_context("SELECT 1", &statements, BatchExecKind::Statement);
    let result: Result<Vec<i32>, DruidError> = Ok(vec![]);
    filter
        .after_batch(&ctx, &result, Duration::from_millis(100))
        .await
        .unwrap();
}

// ===========================================================================
// 10. config_from_properties whitespace-padded slowSqlMillis
// ===========================================================================

#[test]
fn config_from_properties_whitespace_padded_slow_sql_millis() {
    let (filter, _collector) = make_filter();
    let mut props = HashMap::new();
    props.insert("druid.stat.slowSqlMillis".to_owned(), "  5000  ".to_owned());
    filter.config_from_properties(&props).unwrap();
    assert_eq!(filter.get_slow_sql_millis(), 5000);
}

#[test]
fn config_from_properties_whitespace_padded_max_sql_size() {
    let (filter, _collector) = make_filter();
    let mut props = HashMap::new();
    props.insert("druid.stat.sql.MaxSize".to_owned(), "  2048  ".to_owned());
    filter.config_from_properties(&props).unwrap();
}

// ===========================================================================
// 11. slow_sql_millis boundary: exactly at threshold
// ===========================================================================

#[tokio::test]
async fn slow_sql_exactly_at_threshold() {
    let (filter, _collector) = make_filter();
    filter.set_slow_sql_millis(5);
    filter.set_log_slow_sql(true);
    filter.set_slow_sql_log_level("ERROR");
    let mut ctx = make_exec_context("SELECT 1", ExecOperation::Query, false);
    filter.before(&mut ctx).await.unwrap();
    let result = Ok(ExecResult {
        rows_affected: 0,
        last_insert_id: None,
        row_count: Some(1),
    });
    filter
        .after(&ctx, &result, Duration::from_millis(5))
        .await
        .unwrap();
}

// ===========================================================================
// 12. after Query with row_count None (streaming)
// ===========================================================================

#[tokio::test]
async fn after_query_streaming_no_row_count() {
    let (filter, _collector) = make_filter();
    let mut ctx = make_exec_context("SELECT * FROM t", ExecOperation::Query, false);
    filter.before(&mut ctx).await.unwrap();
    let result = Ok(ExecResult {
        rows_affected: 0,
        last_insert_id: None,
        row_count: None,
    });
    filter
        .after(&ctx, &result, Duration::from_millis(5))
        .await
        .unwrap();
}

// ===========================================================================
// 13. slow_prepared_parameter with Some values for Date/Time/Timestamp/Stream
// ===========================================================================

#[tokio::test]
async fn slow_prepared_parameter_with_some_values() {
    let (filter, _collector) = make_filter();
    filter.set_slow_sql_millis(0);
    filter.set_log_slow_sql(false);
    let prepared = vec![
        PreparedInputParameter::Null {
            sql_type: 0,
            type_name: PreparedTypeNameArgument::Specified(Some("VARCHAR".to_owned())),
        },
        PreparedInputParameter::BigDecimal(Some("123.456".parse().unwrap())),
        PreparedInputParameter::String(Some("hello".to_owned())),
        PreparedInputParameter::NString(Some("world".to_owned())),
        PreparedInputParameter::Bytes(Some(vec![1, 2, 3])),
        PreparedInputParameter::Object {
            value: Some(RdbcObject::Scalar(Value::Int(42))),
            target_sql_type: Some(4),
            scale_or_length: Some(0),
        },
    ];
    let mut ctx =
        make_exec_context_with_params("SELECT 1", ExecOperation::Query, &[], Some(&prepared));
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

// ===========================================================================
// 14. slow_value Date/Time/Timestamp with Some values
// ===========================================================================

#[tokio::test]
async fn slow_value_date_time_timestamp() {
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
