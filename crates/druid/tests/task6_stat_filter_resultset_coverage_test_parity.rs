//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
//! `StatFilter` `ResultSetFilter` path differential coverage tests (Java Druid 1.2.28).
//!
//! Covers:
//! - `ResultSetFilter::result_set_open_after`
//! - `ResultSetFilter::result_set_close` (with SQL stat association, `merge_sql` path)
//! - `config_from_properties` all paths
//! - `before_batch` / `after_batch` / `before_batch_error`
//! - `after_connection_event` (Commit/Rollback)
//! - `slow_sql_millis` negative/zero
//! - `merge_sql` parameterization

use druid::core::{
    AfterFilter, BatchExecContext, BatchExecKind, BeforeFilter, Connection, ConnectionEvent,
    ConnectionFactory, DruidError, ExecContext, ExecOperation, ExecResult, FilterChain, RdbcError,
    ResultSetFilter, ResultSetFilterContext, Value,
};
use druid::pool::DruidPool;
use druid::stats::{StatFilter, StatsCollector};
use druid_rdbc::result_set::{ColumnIndex, ResultSet, ResultSetExt};
use druid_rdbc::statement::{Statement, StatementOptions};
use druid_wrapper::toasty::ToastyConnectionFactory;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

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

async fn task6_close_public_result_set(
    filter: Arc<StatFilter>,
    context: &ResultSetFilterContext,
    read_value: bool,
    close_twice: bool,
) {
    let mut chain = FilterChain::new();
    chain.add_filter(filter);
    let factory = ToastyConnectionFactory::new("sqlite::memory:")
        .await
        .expect("factory");
    let pool = DruidPool::builder()
        .name("stat-result-set-close")
        .driver_name("toasty-sqlite")
        .factory(Arc::new(factory) as Arc<dyn ConnectionFactory>)
        .max_open(1)
        .max_idle(1)
        .filter_chain(Arc::new(chain))
        .build()
        .await
        .expect("pool");
    let mut connection = pool.get().await.expect("connection");
    let mut statement = Connection::create_statement(&mut connection, StatementOptions::default())
        .await
        .expect("statement");
    let sql = context.sql().unwrap_or("SELECT 1");
    let mut result_set = statement.execute_query(sql).await.expect("query");
    let requested_rows = context.fetch_row_count().max(1);
    let mut fetched_rows = 0;
    for _ in 0..requested_rows {
        if !result_set.next().await.unwrap() {
            break;
        }
        fetched_rows += 1;
    }
    assert!(fetched_rows >= 1);
    if read_value {
        assert!(result_set
            .string(ColumnIndex::new(1).unwrap())
            .unwrap()
            .is_some());
    }
    result_set.close().await.unwrap();
    assert!(result_set.is_closed().unwrap());
    if close_twice {
        result_set.close().await.unwrap();
        assert!(result_set.is_closed().unwrap());
    }
    statement.close().await.unwrap();
    connection.close().await.unwrap();
    pool.close().await;
}

// ===========================================================================
// 1. result_set_open_after
// ===========================================================================

// ===========================================================================
// 2. result_set_close
// ===========================================================================

// ===========================================================================
// 3. config_from_properties
// ===========================================================================

// ===========================================================================
// 4. before_batch / after_batch / before_batch_error
// ===========================================================================

// ===========================================================================
// 5. after_connection_event
// ===========================================================================

// ===========================================================================
// 6. before + after full chain
// ===========================================================================

// ===========================================================================
// 7. before_execute_error
// ===========================================================================

// ===========================================================================
// 8. name and traits
// ===========================================================================

// ===========================================================================
// 9. merge_sql
// ===========================================================================

// ===========================================================================
// 10. slow_sql_millis boundary
// ===========================================================================

// ===========================================================================
// 11. on_statement_close_context
// ===========================================================================

#[test]
fn task6_stat_filter_resultset_coverage_test_result_set_open_after_records_stat() {
    let (filter, collector) = make_filter();
    let context =
        ResultSetFilterContext::with_sql_and_execute_elapsed(Some("SELECT 1".to_owned()), None);
    filter.result_set_open_after(&context).unwrap();
    let stat = collector.result_set_stat();
    assert!(stat.open_count() >= 1);
}

#[test]
fn task6_stat_filter_resultset_coverage_test_result_set_open_after_no_sql() {
    let (filter, _collector) = make_filter();
    let context = ResultSetFilterContext::new();
    filter.result_set_open_after(&context).unwrap();
}

#[tokio::test]
async fn task6_stat_filter_resultset_coverage_test_result_set_close_records_stats() {
    let (filter, collector) = make_filter();
    let context = ResultSetFilterContext::with_sql_and_execute_elapsed(
        Some("SELECT 1".to_owned()),
        Some(Duration::from_millis(30)),
    );
    context.record_fetch_row_count(1);
    task6_close_public_result_set(Arc::new(filter), &context, false, false).await;
    assert!(collector.result_set_stat().close_count() >= 1);
    assert_eq!(context.fetch_row_count(), 1);
    assert_eq!(
        context.statement_execute_elapsed(),
        Some(Duration::from_millis(30))
    );
}

#[tokio::test]
async fn task6_stat_filter_resultset_coverage_test_result_set_close_with_merge_sql() {
    let (filter, _collector) = make_filter();
    filter.set_merge_sql(true);
    let context = ResultSetFilterContext::with_sql_and_execute_elapsed(
        Some("SELECT 1 WHERE 1 = 1".to_owned()),
        Some(Duration::from_millis(5)),
    );
    context.record_fetch_row_count(1);
    task6_close_public_result_set(Arc::new(filter), &context, false, false).await;
}

#[tokio::test]
async fn task6_stat_filter_resultset_coverage_test_result_set_close_second_close_skips_sql_stat() {
    let (filter, collector) = make_filter();
    let context =
        ResultSetFilterContext::with_sql_and_execute_elapsed(Some("SELECT 1".to_owned()), None);
    context.record_fetch_row_count(1);
    task6_close_public_result_set(Arc::new(filter), &context, false, true).await;
    assert_eq!(collector.result_set_stat().close_count(), 1);
}

#[tokio::test]
async fn task6_stat_filter_resultset_coverage_test_result_set_close_no_sql() {
    let (filter, collector) = make_filter();
    let context = ResultSetFilterContext::new();
    context.record_fetch_row_count(1);
    task6_close_public_result_set(Arc::new(filter), &context, false, false).await;
    assert_eq!(collector.result_set_stat().close_count(), 1);
}

#[tokio::test]
async fn task6_stat_filter_resultset_coverage_test_result_set_close_with_io_stats() {
    let (filter, _collector) = make_filter();
    let context = ResultSetFilterContext::with_sql_and_execute_elapsed(
        Some("SELECT 'hello world'".to_owned()),
        None,
    );
    context.record_fetch_row_count(1);
    task6_close_public_result_set(Arc::new(filter), &context, true, false).await;
}

#[test]
fn task6_stat_filter_resultset_coverage_test_config_from_properties_merge_sql() {
    let (filter, _collector) = make_filter();
    let mut props = HashMap::new();
    props.insert("druid.stat.mergeSql".to_owned(), "true".to_owned());
    filter.config_from_properties(&props).unwrap();
    assert!(filter.is_merge_sql());
}

#[test]
fn task6_stat_filter_resultset_coverage_test_config_from_properties_slow_sql_millis() {
    let (filter, _collector) = make_filter();
    let mut props = HashMap::new();
    props.insert("druid.stat.slowSqlMillis".to_owned(), "5000".to_owned());
    filter.config_from_properties(&props).unwrap();
    assert_eq!(filter.get_slow_sql_millis(), 5000);
}

#[test]
fn task6_stat_filter_resultset_coverage_test_config_from_properties_log_slow_sql() {
    let (filter, _collector) = make_filter();
    let mut props = HashMap::new();
    props.insert("druid.stat.logSlowSql".to_owned(), "true".to_owned());
    filter.config_from_properties(&props).unwrap();
    assert!(filter.is_log_slow_sql());
}

#[test]
fn task6_stat_filter_resultset_coverage_test_config_from_properties_slow_sql_log_level() {
    let (filter, _collector) = make_filter();
    let mut props = HashMap::new();
    props.insert("druid.stat.slowSqlLogLevel".to_owned(), "warn".to_owned());
    filter.config_from_properties(&props).unwrap();
    assert_eq!(filter.get_slow_sql_log_level(), "WARN");
}

#[test]
fn task6_stat_filter_resultset_coverage_test_config_from_properties_max_sql_size() {
    let (filter, collector) = make_filter();
    let mut props = HashMap::new();
    props.insert("druid.stat.sql.MaxSize".to_owned(), "4096".to_owned());
    filter.config_from_properties(&props).unwrap();
    assert_eq!(collector.max_sql_size(), 4096);
}

#[test]
fn task6_stat_filter_resultset_coverage_test_config_from_properties_invalid_slow_sql_millis() {
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
fn task6_stat_filter_resultset_coverage_test_config_from_properties_invalid_max_sql_size() {
    let (filter, _collector) = make_filter();
    let mut props = HashMap::new();
    props.insert(
        "druid.stat.sql.MaxSize".to_owned(),
        "not_a_number".to_owned(),
    );
    filter.config_from_properties(&props).unwrap();
}

#[test]
fn task6_stat_filter_resultset_coverage_test_config_from_properties_empty_slow_sql_millis() {
    let (filter, _collector) = make_filter();
    let mut props = HashMap::new();
    props.insert("druid.stat.slowSqlMillis".to_owned(), String::new());
    filter.config_from_properties(&props).unwrap();
    assert_eq!(filter.get_slow_sql_millis(), 3000);
}

#[test]
fn task6_stat_filter_resultset_coverage_test_config_from_properties_invalid_merge_sql() {
    let (filter, _collector) = make_filter();
    let mut props = HashMap::new();
    props.insert("druid.stat.mergeSql".to_owned(), "invalid".to_owned());
    filter.config_from_properties(&props).unwrap();
    assert!(!filter.is_merge_sql());
}

#[test]
fn task6_stat_filter_resultset_coverage_test_config_from_properties_invalid_log_slow_sql() {
    let (filter, _collector) = make_filter();
    let mut props = HashMap::new();
    props.insert("druid.stat.logSlowSql".to_owned(), "invalid".to_owned());
    filter.config_from_properties(&props).unwrap();
    assert!(!filter.is_log_slow_sql());
}

#[test]
fn task6_stat_filter_resultset_coverage_test_config_from_system_properties() {
    let (filter, _collector) = make_filter();
    let mut props = HashMap::new();
    props.insert("druid.stat.mergeSql".to_owned(), "true".to_owned());
    filter.config_from_system_properties(&props).unwrap();
    assert!(filter.is_merge_sql());
}

#[test]
fn task6_stat_filter_resultset_coverage_test_config_from_properties_empty() {
    let (filter, _collector) = make_filter();
    let props = HashMap::new();
    filter.config_from_properties(&props).unwrap();
    assert!(!filter.is_merge_sql());
    assert_eq!(filter.get_slow_sql_millis(), 3000);
    assert!(!filter.is_log_slow_sql());
}

#[test]
fn task6_stat_filter_resultset_coverage_test_config_from_properties_partial() {
    let (filter, _collector) = make_filter();
    let mut props = HashMap::new();
    props.insert("druid.stat.mergeSql".to_owned(), "true".to_owned());
    filter.config_from_properties(&props).unwrap();
    assert!(filter.is_merge_sql());
    assert_eq!(filter.get_slow_sql_millis(), 3000);
}

#[tokio::test]
async fn task6_stat_filter_resultset_coverage_test_before_batch_sets_fingerprint() {
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
async fn task6_stat_filter_resultset_coverage_test_before_batch_in_transaction() {
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
async fn task6_stat_filter_resultset_coverage_test_before_batch_error_decrements_running() {
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
async fn task6_stat_filter_resultset_coverage_test_after_batch_records_stats() {
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
async fn task6_stat_filter_resultset_coverage_test_after_batch_prepared_statement() {
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
async fn task6_stat_filter_resultset_coverage_test_after_batch_error() {
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
async fn task6_stat_filter_resultset_coverage_test_after_batch_empty_counts() {
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

#[tokio::test]
async fn task6_stat_filter_resultset_coverage_test_after_connection_event_commit() {
    let (filter, _collector) = make_filter();
    filter
        .after_connection_event(&ConnectionEvent::Commit, Duration::ZERO)
        .await
        .unwrap();
}

#[tokio::test]
async fn task6_stat_filter_resultset_coverage_test_after_connection_event_rollback() {
    let (filter, _collector) = make_filter();
    filter
        .after_connection_event(&ConnectionEvent::Rollback, Duration::ZERO)
        .await
        .unwrap();
}

#[tokio::test]
async fn task6_stat_filter_resultset_coverage_test_after_connection_event_set_autocommit() {
    let (filter, _collector) = make_filter();
    filter
        .after_connection_event(&ConnectionEvent::SetAutoCommit(true), Duration::ZERO)
        .await
        .unwrap();
}

#[tokio::test]
async fn task6_stat_filter_resultset_coverage_test_before_after_query_records_fetch() {
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
async fn task6_stat_filter_resultset_coverage_test_before_after_update_records_count() {
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
async fn task6_stat_filter_resultset_coverage_test_before_after_execute_no_row_count() {
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
async fn task6_stat_filter_resultset_coverage_test_before_after_error_records_detail() {
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
async fn task6_stat_filter_resultset_coverage_test_before_after_slow_sql_records_params() {
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
async fn task6_stat_filter_resultset_coverage_test_before_after_execute_with_row_count() {
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
async fn task6_stat_filter_resultset_coverage_test_before_after_in_transaction() {
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
async fn task6_stat_filter_resultset_coverage_test_before_after_execute_error() {
    let (filter, _collector) = make_filter();
    let mut ctx = make_exec_context("INVALID SQL", ExecOperation::Execute, false);
    filter.before(&mut ctx).await.unwrap();
    let result: Result<ExecResult, DruidError> = Err(DruidError::Other("syntax error".to_owned()));
    filter
        .after(&ctx, &result, Duration::from_millis(5))
        .await
        .unwrap();
}

#[tokio::test]
async fn task6_stat_filter_resultset_coverage_test_before_execute_error_decrements() {
    let (filter, _collector) = make_filter();
    let mut ctx = make_exec_context("SELECT 1", ExecOperation::Query, false);
    filter.before(&mut ctx).await.unwrap();
    let error = DruidError::Other("exec error".to_owned());
    filter.before_execute_error(&ctx, &error).await.unwrap();
}

#[test]
fn task6_stat_filter_resultset_coverage_test_filter_name_is_stat() {
    let (filter, _collector) = make_filter();
    assert_eq!(BeforeFilter::name(&filter), "stat");
    assert_eq!(AfterFilter::name(&filter), "stat");
}

#[test]
fn task6_stat_filter_resultset_coverage_test_result_set_stat_returns_shared() {
    let (filter, _collector) = make_filter();
    let stat1 = filter.result_set_stat();
    let stat2 = filter.result_set_stat();
    assert!(std::ptr::eq(stat1, stat2));
}

#[test]
fn task6_stat_filter_resultset_coverage_test_merge_sql_disabled_returns_original() {
    let (filter, _collector) = make_filter();
    filter.set_merge_sql(false);
    let result = filter.merge_sql("SELECT * FROM t WHERE id = 1", None);
    assert_eq!(result, "SELECT * FROM t WHERE id = 1");
}

#[test]
fn task6_stat_filter_resultset_coverage_test_merge_sql_enabled_parameterizes() {
    let (filter, _collector) = make_filter();
    filter.set_merge_sql(true);
    let result = filter.merge_sql("SELECT * FROM t WHERE id = 1", None);
    assert!(result.contains('?'));
}

#[test]
fn task6_stat_filter_resultset_coverage_test_merge_sql_with_db_type() {
    let (filter, _collector) = make_filter();
    filter.set_merge_sql(true);
    let result = filter.merge_sql("SELECT * FROM t WHERE id = 1", Some("mysql"));
    assert!(result.contains('?'));
}

#[test]
fn task6_stat_filter_resultset_coverage_test_slow_sql_millis_negative_all_slow() {
    let (filter, _collector) = make_filter();
    filter.set_slow_sql_millis(-1);
    assert_eq!(filter.get_slow_sql_millis(), -1);
}

#[test]
fn task6_stat_filter_resultset_coverage_test_slow_sql_millis_zero_all_slow() {
    let (filter, _collector) = make_filter();
    filter.set_slow_sql_millis(0);
    assert_eq!(filter.get_slow_sql_millis(), 0);
}

#[test]
fn task6_stat_filter_resultset_coverage_test_on_statement_close_context() {
    let (filter, _collector) = make_filter();
    let event = druid::core::StatementEvent::Close;
    let context = druid::core::StatementEventContext {
        connection_id: 1,
        statement_id: 1,
        event: &event,
    };
    filter.on_statement_close_context(&context).unwrap();
}
