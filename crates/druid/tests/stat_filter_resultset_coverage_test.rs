//! StatFilter ResultSetFilter path differential coverage tests (Java Druid 1.2.28).
//!
//! Covers:
//! - ResultSetFilter::result_set_open_after
//! - ResultSetFilter::result_set_close (with SQL stat association, merge_sql path)
//! - config_from_properties all paths
//! - before_batch / after_batch / before_batch_error
//! - after_connection_event (Commit/Rollback)
//! - slow_sql_millis negative/zero
//! - merge_sql parameterization

use druid::core::{
    AfterFilter, BatchExecContext, BatchExecKind, BeforeFilter, ConnectionEvent, DruidError,
    ExecContext, ExecOperation, ExecResult, PhysicalResultSet, ResultSetFilter,
    ResultSetFilterChain, ResultSetFilterContext, Value,
};
use druid::stats::{StatFilter, StatsCollector};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn make_filter() -> (StatFilter, Arc<StatsCollector>) {
    let collector = Arc::new(StatsCollector::new("test", Duration::from_secs(10)));
    let filter = StatFilter::new(Arc::clone(&collector));
    (filter, collector)
}

fn make_exec_context<'a>(
    sql: &'a str,
    operation: ExecOperation,
    in_transaction: bool,
) -> ExecContext<'a> {
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

/// Empty PhysicalResultSet for ResultSetFilterChain construction.
#[derive(Debug)]
struct EmptyResultSet;

impl PhysicalResultSet for EmptyResultSet {
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
// 1. result_set_open_after
// ===========================================================================

#[test]
fn result_set_open_after_records_stat() {
    let (filter, collector) = make_filter();
    let context =
        ResultSetFilterContext::with_sql_and_execute_elapsed(Some("SELECT 1".to_owned()), None);
    filter.result_set_open_after(&context).unwrap();
    let stat = collector.result_set_stat();
    assert!(stat.open_count() >= 1);
}

#[test]
fn result_set_open_after_no_sql() {
    let (filter, _collector) = make_filter();
    let context = ResultSetFilterContext::new();
    filter.result_set_open_after(&context).unwrap();
}

// ===========================================================================
// 2. result_set_close
// ===========================================================================

#[test]
fn result_set_close_records_stats() {
    let (filter, collector) = make_filter();
    let physical = EmptyResultSet;
    let context = ResultSetFilterContext::with_sql_and_execute_elapsed(
        Some("SELECT 1".to_owned()),
        Some(Duration::from_millis(30)),
    );
    context.record_fetch_row_count(10);
    let filters: Vec<Arc<dyn ResultSetFilter>> = vec![];
    let mut chain = ResultSetFilterChain::new(&filters, &physical, &context);
    filter.result_set_close(&mut chain).unwrap();
    let stat = collector.result_set_stat();
    assert!(stat.close_count() >= 1);
}

#[test]
fn result_set_close_with_merge_sql() {
    let (filter, _collector) = make_filter();
    filter.set_merge_sql(true);
    let physical = EmptyResultSet;
    let context = ResultSetFilterContext::with_sql_and_execute_elapsed(
        Some("SELECT * FROM t WHERE id = 1".to_owned()),
        Some(Duration::from_millis(5)),
    );
    context.record_fetch_row_count(5);
    let filters: Vec<Arc<dyn ResultSetFilter>> = vec![];
    let mut chain = ResultSetFilterChain::new(&filters, &physical, &context);
    filter.result_set_close(&mut chain).unwrap();
}

#[test]
fn result_set_close_second_close_skips_sql_stat() {
    let (filter, _collector) = make_filter();
    let physical = EmptyResultSet;
    let context =
        ResultSetFilterContext::with_sql_and_execute_elapsed(Some("SELECT 1".to_owned()), None);
    context.increment_close_count();
    let filters: Vec<Arc<dyn ResultSetFilter>> = vec![];
    let mut chain = ResultSetFilterChain::new(&filters, &physical, &context);
    filter.result_set_close(&mut chain).unwrap();
}

#[test]
fn result_set_close_no_sql() {
    let (filter, _collector) = make_filter();
    let physical = EmptyResultSet;
    let context = ResultSetFilterContext::new();
    let filters: Vec<Arc<dyn ResultSetFilter>> = vec![];
    let mut chain = ResultSetFilterChain::new(&filters, &physical, &context);
    filter.result_set_close(&mut chain).unwrap();
}

#[test]
fn result_set_close_with_io_stats() {
    let (filter, _collector) = make_filter();
    let physical = EmptyResultSet;
    let context =
        ResultSetFilterContext::with_sql_and_execute_elapsed(Some("SELECT 1".to_owned()), None);
    context.add_read_string_length("hello world");
    context.add_read_bytes_length(1024);
    context.increment_open_input_stream_count();
    context.increment_open_reader_count();
    let filters: Vec<Arc<dyn ResultSetFilter>> = vec![];
    let mut chain = ResultSetFilterChain::new(&filters, &physical, &context);
    filter.result_set_close(&mut chain).unwrap();
}

// ===========================================================================
// 3. config_from_properties
// ===========================================================================

#[test]
fn config_from_properties_merge_sql() {
    let (filter, _collector) = make_filter();
    let mut props = HashMap::new();
    props.insert("druid.stat.mergeSql".to_owned(), "true".to_owned());
    filter.config_from_properties(&props).unwrap();
    assert!(filter.is_merge_sql());
}

#[test]
fn config_from_properties_slow_sql_millis() {
    let (filter, _collector) = make_filter();
    let mut props = HashMap::new();
    props.insert("druid.stat.slowSqlMillis".to_owned(), "5000".to_owned());
    filter.config_from_properties(&props).unwrap();
    assert_eq!(filter.get_slow_sql_millis(), 5000);
}

#[test]
fn config_from_properties_log_slow_sql() {
    let (filter, _collector) = make_filter();
    let mut props = HashMap::new();
    props.insert("druid.stat.logSlowSql".to_owned(), "true".to_owned());
    filter.config_from_properties(&props).unwrap();
    assert!(filter.is_log_slow_sql());
}

#[test]
fn config_from_properties_slow_sql_log_level() {
    let (filter, _collector) = make_filter();
    let mut props = HashMap::new();
    props.insert("druid.stat.slowSqlLogLevel".to_owned(), "warn".to_owned());
    filter.config_from_properties(&props).unwrap();
    assert_eq!(filter.get_slow_sql_log_level(), "WARN");
}

#[test]
fn config_from_properties_max_sql_size() {
    let (filter, collector) = make_filter();
    let mut props = HashMap::new();
    props.insert("druid.stat.sql.MaxSize".to_owned(), "4096".to_owned());
    filter.config_from_properties(&props).unwrap();
    assert_eq!(collector.max_sql_size(), 4096);
}

#[test]
fn config_from_properties_invalid_slow_sql_millis() {
    let (filter, _collector) = make_filter();
    let mut props = HashMap::new();
    props.insert(
        "druid.stat.slowSqlMillis".to_owned(),
        "not_a_number".to_owned(),
    );
    filter.config_from_properties(&props).unwrap();
    assert_eq!(filter.get_slow_sql_millis(), 3000);
}

#[test]
fn config_from_properties_invalid_max_sql_size() {
    let (filter, _collector) = make_filter();
    let mut props = HashMap::new();
    props.insert(
        "druid.stat.sql.MaxSize".to_owned(),
        "not_a_number".to_owned(),
    );
    filter.config_from_properties(&props).unwrap();
}

#[test]
fn config_from_properties_empty_slow_sql_millis() {
    let (filter, _collector) = make_filter();
    let mut props = HashMap::new();
    props.insert("druid.stat.slowSqlMillis".to_owned(), "".to_owned());
    filter.config_from_properties(&props).unwrap();
    assert_eq!(filter.get_slow_sql_millis(), 3000);
}

#[test]
fn config_from_properties_invalid_merge_sql() {
    let (filter, _collector) = make_filter();
    let mut props = HashMap::new();
    props.insert("druid.stat.mergeSql".to_owned(), "invalid".to_owned());
    filter.config_from_properties(&props).unwrap();
    assert!(!filter.is_merge_sql());
}

#[test]
fn config_from_properties_invalid_log_slow_sql() {
    let (filter, _collector) = make_filter();
    let mut props = HashMap::new();
    props.insert("druid.stat.logSlowSql".to_owned(), "invalid".to_owned());
    filter.config_from_properties(&props).unwrap();
    assert!(!filter.is_log_slow_sql());
}

#[test]
fn config_from_system_properties() {
    let (filter, _collector) = make_filter();
    let mut props = HashMap::new();
    props.insert("druid.stat.mergeSql".to_owned(), "true".to_owned());
    filter.config_from_system_properties(&props).unwrap();
    assert!(filter.is_merge_sql());
}

#[test]
fn config_from_properties_empty() {
    let (filter, _collector) = make_filter();
    let props = HashMap::new();
    filter.config_from_properties(&props).unwrap();
    assert!(!filter.is_merge_sql());
    assert_eq!(filter.get_slow_sql_millis(), 3000);
    assert!(!filter.is_log_slow_sql());
}

#[test]
fn config_from_properties_partial() {
    let (filter, _collector) = make_filter();
    let mut props = HashMap::new();
    props.insert("druid.stat.mergeSql".to_owned(), "true".to_owned());
    filter.config_from_properties(&props).unwrap();
    assert!(filter.is_merge_sql());
    assert_eq!(filter.get_slow_sql_millis(), 3000);
}

// ===========================================================================
// 4. before_batch / after_batch / before_batch_error
// ===========================================================================

#[tokio::test]
async fn before_batch_sets_fingerprint() {
    let (filter, _collector) = make_filter();
    let statements = vec!["INSERT INTO t VALUES (1)".to_string()];
    let mut ctx = make_batch_context(
        "INSERT INTO t VALUES (1)",
        &statements,
        BatchExecKind::Statement,
    );
    filter.before_batch(&mut ctx).await.unwrap();
    assert!(ctx.fingerprint.is_some());
}

#[tokio::test]
async fn before_batch_in_transaction() {
    let (filter, _collector) = make_filter();
    let statements = vec!["INSERT INTO t VALUES (1)".to_string()];
    let mut ctx = BatchExecContext {
        in_transaction: true,
        ..make_batch_context(
            "INSERT INTO t VALUES (1)",
            &statements,
            BatchExecKind::Statement,
        )
    };
    filter.before_batch(&mut ctx).await.unwrap();
    assert!(ctx.fingerprint.is_some());
}

#[tokio::test]
async fn before_batch_error_decrements_running() {
    let (filter, _collector) = make_filter();
    let statements = vec!["INSERT INTO t VALUES (1)".to_string()];
    let mut ctx = make_batch_context(
        "INSERT INTO t VALUES (1)",
        &statements,
        BatchExecKind::Statement,
    );
    filter.before_batch(&mut ctx).await.unwrap();
    let error = DruidError::Other("test error".to_owned());
    filter.before_batch_error(&ctx, &error).await.unwrap();
}

#[tokio::test]
async fn after_batch_records_stats() {
    let (filter, _collector) = make_filter();
    let statements = vec!["INSERT INTO t VALUES (1)".to_string()];
    let mut ctx = make_batch_context(
        "INSERT INTO t VALUES (1)",
        &statements,
        BatchExecKind::Statement,
    );
    filter.before_batch(&mut ctx).await.unwrap();
    let result = Ok(vec![1]);
    filter
        .after_batch(&ctx, &result, Duration::from_millis(10))
        .await
        .unwrap();
}

#[tokio::test]
async fn after_batch_prepared_statement() {
    let (filter, _collector) = make_filter();
    let statements = vec!["INSERT INTO t VALUES (?)".to_string()];
    let mut ctx = make_batch_context(
        "INSERT INTO t VALUES (?)",
        &statements,
        BatchExecKind::PreparedStatement,
    );
    filter.before_batch(&mut ctx).await.unwrap();
    let result = Ok(vec![1, 2]);
    filter
        .after_batch(&ctx, &result, Duration::from_millis(5))
        .await
        .unwrap();
}

#[tokio::test]
async fn after_batch_error() {
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

#[tokio::test]
async fn after_batch_empty_counts() {
    let (filter, _collector) = make_filter();
    let statements = vec!["SELECT 1".to_string()];
    let mut ctx = make_batch_context("SELECT 1", &statements, BatchExecKind::Statement);
    filter.before_batch(&mut ctx).await.unwrap();
    let result: Result<Vec<i32>, DruidError> = Ok(vec![]);
    filter
        .after_batch(&ctx, &result, Duration::from_millis(1))
        .await
        .unwrap();
}

// ===========================================================================
// 5. after_connection_event
// ===========================================================================

#[tokio::test]
async fn after_connection_event_commit() {
    let (filter, _collector) = make_filter();
    filter
        .after_connection_event(&ConnectionEvent::Commit, Duration::ZERO)
        .await
        .unwrap();
}

#[tokio::test]
async fn after_connection_event_rollback() {
    let (filter, _collector) = make_filter();
    filter
        .after_connection_event(&ConnectionEvent::Rollback, Duration::ZERO)
        .await
        .unwrap();
}

#[tokio::test]
async fn after_connection_event_set_autocommit() {
    let (filter, _collector) = make_filter();
    filter
        .after_connection_event(&ConnectionEvent::SetAutoCommit(true), Duration::ZERO)
        .await
        .unwrap();
}

// ===========================================================================
// 6. before + after full chain
// ===========================================================================

#[tokio::test]
async fn before_after_query_records_fetch() {
    let (filter, collector) = make_filter();
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
    let sql_stat = collector.sql_merger.get_stat(ctx.fingerprint.unwrap());
    assert!(sql_stat.is_some());
}

#[tokio::test]
async fn before_after_update_records_count() {
    let (filter, _collector) = make_filter();
    let mut ctx = make_exec_context("INSERT INTO t VALUES (1)", ExecOperation::Update, false);
    filter.before(&mut ctx).await.unwrap();
    let result = Ok(ExecResult {
        rows_affected: 1,
        last_insert_id: Some(42),
        row_count: None,
    });
    filter
        .after(&ctx, &result, Duration::from_millis(5))
        .await
        .unwrap();
}

#[tokio::test]
async fn before_after_execute_no_row_count() {
    let (filter, _collector) = make_filter();
    let mut ctx = make_exec_context("CREATE TABLE t (id INT)", ExecOperation::Execute, false);
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

#[tokio::test]
async fn before_after_error_records_detail() {
    let (filter, _collector) = make_filter();
    let mut ctx = make_exec_context("SELECT * FROM nonexistent", ExecOperation::Query, false);
    filter.before(&mut ctx).await.unwrap();
    let result: Result<ExecResult, DruidError> =
        Err(DruidError::Other("table not found".to_owned()));
    filter
        .after(&ctx, &result, Duration::from_millis(5))
        .await
        .unwrap();
}

#[tokio::test]
async fn before_after_slow_sql_records_params() {
    let (filter, _collector) = make_filter();
    filter.set_slow_sql_millis(0);
    filter.set_log_slow_sql(true);
    let params = vec![Value::Int(42), Value::String("test".to_owned())];
    let mut ctx = ExecContext {
        connection_id: 1,
        statement_id: Some(1),
        sql: "SELECT * FROM t WHERE id = ?".to_owned(),
        params: &params,
        prepared_parameters: None,
        data_source: "test",
        start: Instant::now(),
        fingerprint: None,
        in_transaction: false,
        operation: ExecOperation::Query,
    };
    filter.before(&mut ctx).await.unwrap();
    let result = Ok(ExecResult {
        rows_affected: 0,
        last_insert_id: None,
        row_count: Some(1),
    });
    filter
        .after(&ctx, &result, Duration::from_millis(100))
        .await
        .unwrap();
}

#[tokio::test]
async fn before_after_execute_with_row_count() {
    let (filter, _collector) = make_filter();
    let mut ctx = make_exec_context("SELECT 1", ExecOperation::Execute, false);
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

#[tokio::test]
async fn before_after_in_transaction() {
    let (filter, _collector) = make_filter();
    let mut ctx = make_exec_context("INSERT INTO t VALUES (1)", ExecOperation::Update, true);
    filter.before(&mut ctx).await.unwrap();
    let result = Ok(ExecResult {
        rows_affected: 1,
        last_insert_id: None,
        row_count: None,
    });
    filter
        .after(&ctx, &result, Duration::from_millis(5))
        .await
        .unwrap();
}

#[tokio::test]
async fn before_after_execute_error() {
    let (filter, _collector) = make_filter();
    let mut ctx = make_exec_context("INVALID SQL", ExecOperation::Execute, false);
    filter.before(&mut ctx).await.unwrap();
    let result: Result<ExecResult, DruidError> = Err(DruidError::Other("syntax error".to_owned()));
    filter
        .after(&ctx, &result, Duration::from_millis(5))
        .await
        .unwrap();
}

// ===========================================================================
// 7. before_execute_error
// ===========================================================================

#[tokio::test]
async fn before_execute_error_decrements() {
    let (filter, _collector) = make_filter();
    let mut ctx = make_exec_context("SELECT 1", ExecOperation::Query, false);
    filter.before(&mut ctx).await.unwrap();
    let error = DruidError::Other("exec error".to_owned());
    filter.before_execute_error(&ctx, &error).await.unwrap();
}

// ===========================================================================
// 8. name and traits
// ===========================================================================

#[test]
fn filter_name_is_stat() {
    let (filter, _collector) = make_filter();
    assert_eq!(BeforeFilter::name(&filter), "stat");
    assert_eq!(AfterFilter::name(&filter), "stat");
}

#[test]
fn result_set_stat_returns_shared() {
    let (filter, _collector) = make_filter();
    let stat1 = filter.result_set_stat();
    let stat2 = filter.result_set_stat();
    assert!(std::ptr::eq(stat1, stat2));
}

// ===========================================================================
// 9. merge_sql
// ===========================================================================

#[test]
fn merge_sql_disabled_returns_original() {
    let (filter, _collector) = make_filter();
    filter.set_merge_sql(false);
    let result = filter.merge_sql("SELECT * FROM t WHERE id = 1", None);
    assert_eq!(result, "SELECT * FROM t WHERE id = 1");
}

#[test]
fn merge_sql_enabled_parameterizes() {
    let (filter, _collector) = make_filter();
    filter.set_merge_sql(true);
    let result = filter.merge_sql("SELECT * FROM t WHERE id = 1", None);
    assert!(result.contains('?'));
}

#[test]
fn merge_sql_with_db_type() {
    let (filter, _collector) = make_filter();
    filter.set_merge_sql(true);
    let result = filter.merge_sql("SELECT * FROM t WHERE id = 1", Some("mysql"));
    assert!(result.contains('?'));
}

// ===========================================================================
// 10. slow_sql_millis boundary
// ===========================================================================

#[test]
fn slow_sql_millis_negative_all_slow() {
    let (filter, _collector) = make_filter();
    filter.set_slow_sql_millis(-1);
    assert_eq!(filter.get_slow_sql_millis(), -1);
}

#[test]
fn slow_sql_millis_zero_all_slow() {
    let (filter, _collector) = make_filter();
    filter.set_slow_sql_millis(0);
    assert_eq!(filter.get_slow_sql_millis(), 0);
}

// ===========================================================================
// 11. on_statement_close_context
// ===========================================================================

#[test]
fn on_statement_close_context() {
    let (filter, _collector) = make_filter();
    let event = druid::core::StatementEvent::Close;
    let context = druid::core::StatementEventContext {
        connection_id: 1,
        statement_id: 1,
        event: &event,
    };
    filter.on_statement_close_context(&context).unwrap();
}
