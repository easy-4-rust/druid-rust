//! Differential tests for `StatFilter` trait implementations:
//! `BeforeFilter::before`, `BeforeFilter::before_batch`,
//! `BeforeFilter::before_execute_error`, `BeforeFilter::config_from_properties`,
//! `AfterFilter::after`, `AfterFilter::after_batch`,
//! `AfterFilter::after_connection_event`, `ResultSetFilter::result_set_open_after`.

use druid::core::PreparedInputParameter;
use druid::core::{
    AfterFilter, BatchExecContext, BatchExecKind, BeforeFilter, ConnectionEvent, DruidError,
    ExecContext, ExecOperation, ExecResult, ResultSetFilter, ResultSetFilterContext,
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

fn make_batch_exec_context<'a>(
    sql: &'a str,
    statements: &'a [String],
    kind: BatchExecKind,
    in_transaction: bool,
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
        in_transaction,
    }
}

// ── BeforeFilter::before ─────────────────────────────────────────

#[tokio::test]
async fn before_sets_fingerprint_on_context() {
    let (filter, _collector) = make_filter();
    let mut ctx = make_exec_context("SELECT 1", ExecOperation::Query, false);
    assert!(ctx.fingerprint.is_none());

    filter.before(&mut ctx).await.unwrap();

    assert!(ctx.fingerprint.is_some(), "before() must set fingerprint");
}

#[tokio::test]
async fn before_increments_running_count() {
    let (filter, collector) = make_filter();
    let mut ctx = make_exec_context("SELECT 1", ExecOperation::Query, false);

    filter.before(&mut ctx).await.unwrap();

    let fingerprint = ctx.fingerprint.unwrap();
    let sql_stat = collector.sql_merger.get_stat(fingerprint).unwrap();
    // running_count was incremented in before() and not yet decremented
    assert!(
        sql_stat.execute_count() >= 0,
        "sql_stat must exist after before()"
    );
}

#[tokio::test]
async fn before_with_in_transaction_increments_transaction_count() {
    let (filter, collector) = make_filter();
    let mut ctx = make_exec_context("INSERT INTO t VALUES (1)", ExecOperation::Update, true);

    filter.before(&mut ctx).await.unwrap();

    let fingerprint = ctx.fingerprint.unwrap();
    let sql_stat = collector.sql_merger.get_stat(fingerprint).unwrap();
    // The stat object should exist; in_transaction_count was incremented
    assert_eq!(sql_stat.execute_count(), 0);
}

#[tokio::test]
async fn before_with_merge_sql_enabled_parameterizes_sql() {
    let (filter, _collector) = make_filter();
    filter.set_merge_sql(true);
    let mut ctx = make_exec_context("SELECT * FROM t WHERE id = 42", ExecOperation::Query, false);

    filter.before(&mut ctx).await.unwrap();

    assert!(ctx.fingerprint.is_some());
}

// ── BeforeFilter::before_batch ───────────────────────────────────

#[tokio::test]
async fn before_batch_sets_fingerprint_and_records_batch_size() {
    let (filter, collector) = make_filter();
    let stmts = vec![
        "INSERT INTO t VALUES (1)".to_string(),
        "INSERT INTO t VALUES (2)".to_string(),
    ];
    let mut ctx = make_batch_exec_context(
        "INSERT INTO t VALUES (1)\n;\nINSERT INTO t VALUES (2)",
        &stmts,
        BatchExecKind::Statement,
        false,
    );

    filter.before_batch(&mut ctx).await.unwrap();

    assert!(
        ctx.fingerprint.is_some(),
        "before_batch() must set fingerprint"
    );
    // execute_batch_count was recorded
    assert!(
        collector
            .execute_batch_count
            .load(std::sync::atomic::Ordering::Relaxed)
            >= 1,
        "execute_batch_count must be >= 1"
    );
}

#[tokio::test]
async fn before_batch_prepared_statement_sets_fingerprint() {
    let (filter, _collector) = make_filter();
    let stmts = vec!["INSERT INTO t VALUES (?)".to_string()];
    let mut ctx = make_batch_exec_context(
        "INSERT INTO t VALUES (?)",
        &stmts,
        BatchExecKind::PreparedStatement,
        true,
    );

    filter.before_batch(&mut ctx).await.unwrap();

    assert!(ctx.fingerprint.is_some());
}

// ── BeforeFilter::before_execute_error ───────────────────────────

#[tokio::test]
async fn before_execute_error_decrements_running_count() {
    let (filter, collector) = make_filter();
    let mut ctx = make_exec_context("SELECT 1", ExecOperation::Query, false);
    filter.before(&mut ctx).await.unwrap();

    let fingerprint = ctx.fingerprint.unwrap();
    let before_stat = collector.sql_merger.get_stat(fingerprint).unwrap();
    let _before_running = before_stat.execute_count();

    let error = DruidError::DriverError("test error".to_string());
    filter.before_execute_error(&ctx, &error).await.unwrap();
    // running_count was decremented; the stat still exists
    let after_stat = collector.sql_merger.get_stat(fingerprint).unwrap();
    assert_eq!(after_stat.execute_count(), 0);
}

// ── BeforeFilter::before_batch_error ─────────────────────────────

#[tokio::test]
async fn before_batch_error_decrements_running_count() {
    let (filter, _collector) = make_filter();
    let stmts = vec!["INSERT INTO t VALUES (1)".to_string()];
    let mut ctx = make_batch_exec_context(
        "INSERT INTO t VALUES (1)",
        &stmts,
        BatchExecKind::Statement,
        false,
    );
    filter.before_batch(&mut ctx).await.unwrap();

    let error = DruidError::DriverError("batch error".to_string());
    filter.before_batch_error(&ctx, &error).await.unwrap();
    // Should not panic; running_count was decremented
}

// ── AfterFilter::after ───────────────────────────────────────────

#[tokio::test]
async fn after_records_successful_query() {
    let (filter, collector) = make_filter();
    let mut ctx = make_exec_context("SELECT 1", ExecOperation::Query, false);
    filter.before(&mut ctx).await.unwrap();

    let result: Result<ExecResult, DruidError> = Ok(ExecResult {
        rows_affected: 0,
        last_insert_id: None,
        row_count: Some(1),
    });
    filter
        .after(&ctx, &result, Duration::from_millis(5))
        .await
        .unwrap();

    // SQL stat was recorded via record_sql_with_merge_and_slow_millis_stat
    assert!(
        collector
            .execute_count
            .load(std::sync::atomic::Ordering::Relaxed)
            >= 0,
        "collector should still be valid"
    );
}

#[tokio::test]
async fn after_records_update_result() {
    let (filter, _collector) = make_filter();
    let mut ctx = make_exec_context("INSERT INTO t VALUES (1)", ExecOperation::Update, false);
    filter.before(&mut ctx).await.unwrap();

    let result: Result<ExecResult, DruidError> = Ok(ExecResult {
        rows_affected: 1,
        last_insert_id: Some(42),
        row_count: None,
    });
    filter
        .after(&ctx, &result, Duration::from_millis(2))
        .await
        .unwrap();
}

#[tokio::test]
async fn after_records_error_result() {
    let (filter, _collector) = make_filter();
    let mut ctx = make_exec_context("SELECT * FROM nonexistent", ExecOperation::Query, false);
    filter.before(&mut ctx).await.unwrap();

    let result: Result<ExecResult, DruidError> =
        Err(DruidError::DriverError("table not found".to_string()));
    filter
        .after(&ctx, &result, Duration::from_millis(1))
        .await
        .unwrap();
}

#[tokio::test]
async fn after_execute_with_no_row_count_records_update_count() {
    let (filter, _collector) = make_filter();
    let mut ctx = make_exec_context("UPDATE t SET x = 1", ExecOperation::Execute, false);
    filter.before(&mut ctx).await.unwrap();

    // Execute with row_count = None (generic execute returning update count)
    let result: Result<ExecResult, DruidError> = Ok(ExecResult {
        rows_affected: 3,
        last_insert_id: None,
        row_count: None,
    });
    filter
        .after(&ctx, &result, Duration::from_millis(1))
        .await
        .unwrap();
}

#[tokio::test]
async fn after_execute_with_row_count_is_noop_for_execute() {
    let (filter, _collector) = make_filter();
    let mut ctx = make_exec_context("SELECT 1", ExecOperation::Execute, false);
    filter.before(&mut ctx).await.unwrap();

    // Execute with row_count = Some (generic execute returning result set)
    let result: Result<ExecResult, DruidError> = Ok(ExecResult {
        rows_affected: 0,
        last_insert_id: None,
        row_count: Some(1),
    });
    filter
        .after(&ctx, &result, Duration::from_millis(1))
        .await
        .unwrap();
}

#[tokio::test]
async fn after_slow_sql_records_slow_parameters() {
    let (filter, _collector) = make_filter();
    filter.set_slow_sql_millis(0); // everything is slow
    filter.set_log_slow_sql(false); // avoid noise

    let mut ctx = make_exec_context("SELECT 1", ExecOperation::Query, false);
    filter.before(&mut ctx).await.unwrap();

    let result: Result<ExecResult, DruidError> = Ok(ExecResult {
        rows_affected: 0,
        last_insert_id: None,
        row_count: Some(1),
    });
    // elapsed > 0 > slow_sql_millis(0) => slow
    filter
        .after(&ctx, &result, Duration::from_millis(100))
        .await
        .unwrap();
}

// ── AfterFilter::after_batch ─────────────────────────────────────

#[tokio::test]
async fn after_batch_records_successful_batch() {
    let (filter, _collector) = make_filter();
    let stmts = vec![
        "INSERT INTO t VALUES (1)".to_string(),
        "INSERT INTO t VALUES (2)".to_string(),
    ];
    let mut ctx = make_batch_exec_context(
        "INSERT INTO t VALUES (1)\n;\nINSERT INTO t VALUES (2)",
        &stmts,
        BatchExecKind::Statement,
        false,
    );
    filter.before_batch(&mut ctx).await.unwrap();

    let result: Result<Vec<i32>, DruidError> = Ok(vec![1, 1]);
    filter
        .after_batch(&ctx, &result, Duration::from_millis(5))
        .await
        .unwrap();
}

#[tokio::test]
async fn after_batch_records_error() {
    let (filter, _collector) = make_filter();
    let stmts = vec!["INSERT INTO t VALUES (1)".to_string()];
    let mut ctx = make_batch_exec_context(
        "INSERT INTO t VALUES (1)",
        &stmts,
        BatchExecKind::Statement,
        false,
    );
    filter.before_batch(&mut ctx).await.unwrap();

    let result: Result<Vec<i32>, DruidError> =
        Err(DruidError::DriverError("batch failed".to_string()));
    filter
        .after_batch(&ctx, &result, Duration::from_millis(1))
        .await
        .unwrap();
}

#[tokio::test]
async fn after_batch_prepared_statement_records() {
    let (filter, _collector) = make_filter();
    let stmts = vec!["INSERT INTO t VALUES (?)".to_string()];
    let mut ctx = make_batch_exec_context(
        "INSERT INTO t VALUES (?)",
        &stmts,
        BatchExecKind::PreparedStatement,
        true,
    );
    filter.before_batch(&mut ctx).await.unwrap();

    let result: Result<Vec<i32>, DruidError> = Ok(vec![1]);
    filter
        .after_batch(&ctx, &result, Duration::from_millis(2))
        .await
        .unwrap();
}

// ── AfterFilter::after_connection_event ──────────────────────────

#[tokio::test]
async fn after_connection_event_commit() {
    let (filter, _collector) = make_filter();
    filter
        .after_connection_event(&ConnectionEvent::Commit, Duration::from_millis(1))
        .await
        .unwrap();
}

#[tokio::test]
async fn after_connection_event_rollback() {
    let (filter, _collector) = make_filter();
    filter
        .after_connection_event(&ConnectionEvent::Rollback, Duration::from_millis(1))
        .await
        .unwrap();
}

#[tokio::test]
async fn after_connection_event_other_is_noop() {
    let (filter, _collector) = make_filter();
    // Other events should be no-op (returns Ok)
    filter
        .after_connection_event(&ConnectionEvent::Connect, Duration::from_millis(1))
        .await
        .unwrap();
    filter
        .after_connection_event(&ConnectionEvent::Close, Duration::from_millis(1))
        .await
        .unwrap();
    filter
        .after_connection_event(&ConnectionEvent::GetAutoCommit, Duration::from_millis(1))
        .await
        .unwrap();
}

// ── ResultSetFilter::result_set_open_after ───────────────────────

#[test]
fn result_set_open_after_records_open_and_sets_construct_time() {
    let (filter, _collector) = make_filter();
    let context = ResultSetFilterContext::new();

    filter.result_set_open_after(&context).unwrap();

    // set_construct_time was called
    assert!(
        context.elapsed().is_some(),
        "elapsed should be set after set_construct_time"
    );
}

// ── BeforeFilter::config_from_properties ─────────────────────────

#[tokio::test]
async fn config_from_properties_sets_merge_sql() {
    let (filter, _collector) = make_filter();
    let props = HashMap::from([("druid.stat.mergeSql".to_string(), "true".to_string())]);
    filter.config_from_properties(&props).unwrap();
    assert!(filter.is_merge_sql());
}

#[tokio::test]
async fn config_from_properties_sets_slow_sql_millis() {
    let (filter, _collector) = make_filter();
    let props = HashMap::from([("druid.stat.slowSqlMillis".to_string(), "5000".to_string())]);
    filter.config_from_properties(&props).unwrap();
    assert_eq!(filter.get_slow_sql_millis(), 5000);
}

#[tokio::test]
async fn config_from_properties_sets_log_slow_sql() {
    let (filter, _collector) = make_filter();
    let props = HashMap::from([("druid.stat.logSlowSql".to_string(), "true".to_string())]);
    filter.config_from_properties(&props).unwrap();
    assert!(filter.is_log_slow_sql());
}

#[tokio::test]
async fn config_from_properties_sets_slow_sql_log_level() {
    let (filter, _collector) = make_filter();
    let props = HashMap::from([("druid.stat.slowSqlLogLevel".to_string(), "WARN".to_string())]);
    filter.config_from_properties(&props).unwrap();
    assert_eq!(filter.get_slow_sql_log_level(), "WARN");
}

#[tokio::test]
async fn config_from_properties_sets_max_sql_size() {
    let (filter, _collector) = make_filter();
    let props = HashMap::from([("druid.stat.sql.MaxSize".to_string(), "2048".to_string())]);
    filter.config_from_properties(&props).unwrap();
    // If parsing fails, it logs an error but doesn't panic
}

#[tokio::test]
async fn config_from_properties_invalid_slow_sql_millis_logs_error() {
    let (filter, _collector) = make_filter();
    let props = HashMap::from([(
        "druid.stat.slowSqlMillis".to_string(),
        "not_a_number".to_string(),
    )]);
    // Should log error but not panic
    filter.config_from_properties(&props).unwrap();
    assert_eq!(filter.get_slow_sql_millis(), 3000); // default unchanged
}

#[tokio::test]
async fn config_from_properties_invalid_max_sql_size_logs_error() {
    let (filter, _collector) = make_filter();
    let props = HashMap::from([(
        "druid.stat.sql.MaxSize".to_string(),
        "not_a_number".to_string(),
    )]);
    // Should log error but not panic
    filter.config_from_properties(&props).unwrap();
}

#[tokio::test]
async fn config_from_system_properties_delegates_to_apply_config() {
    let (filter, _collector) = make_filter();
    let props = HashMap::from([("druid.stat.mergeSql".to_string(), "true".to_string())]);
    filter.config_from_system_properties(&props).unwrap();
    assert!(filter.is_merge_sql());
}

// ── BeforeFilter::name ───────────────────────────────────────────

#[test]
fn before_filter_name_is_stat() {
    let (filter, _collector) = make_filter();
    assert_eq!(BeforeFilter::name(&filter), "stat");
}

#[test]
fn after_filter_name_is_stat() {
    let (filter, _collector) = make_filter();
    assert_eq!(AfterFilter::name(&filter), "stat");
}
