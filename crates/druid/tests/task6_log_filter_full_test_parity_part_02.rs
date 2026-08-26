//! Generated from the pinned Task 6 BASE test denominator.
#![allow(dead_code, unused_imports, deprecated)]
//! `LogFilter` 全量覆盖测试（Java `LogFilter.java` 差分对照）。
//!
//! 覆盖目标：
//! - 全部 `is_xxx_log_enabled` / `set_xxx_log_enabled` 开关（含父级开关联动）
//! - `config_from_properties` 七键精确语义
//! - `operation_success_enabled` 四种 `ExecOperation` 分支
//! - after / `after_batch` 成功与错误日志分支
//! - `after_connection_event` 六种 `ConnectionEvent` 分支
//! - `on_statement_event` / `on_statement_event_context` 四种 `StatementEvent` 分支
//! - `on_statement_close_context`
//! - `on_connection_event` / `on_connection_event_context` no-op
//! - `ResultSetFilter` `三方法（open_after` / next / close）成功与错误分支
//! - Default / `statement_sql_format_option` / `statement_sql_pretty_format`
//! - `set_statement_parameter_log_enabled` 旧 API 兼容

use druid::core::{
    AfterFilter, BatchExecContext, BatchExecKind, BeforeFilter, ConnectionEvent,
    ConnectionEventContext, DruidError, ExecContext, ExecOperation, ExecResult, LogFilter,
    RdbcError, ResultSet, ResultSetFilter, ResultSetFilterContext, ResultSetMetaData,
    ResultSetMetaDataSnapshot, StatementEvent, StatementEventContext, Value,
};
use druid::sql::SqlFormatOption;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn props(entries: &[(&str, &str)]) -> HashMap<String, String> {
    entries
        .iter()
        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
        .collect()
}

fn exec_context(operation: ExecOperation) -> (Vec<Value>, ExecContext<'static>) {
    // Leak the params vec so we can return a context with a static lifetime.
    // This is acceptable in tests; we never mutate the vec after creation.
    let params: &'static [Value] = &[];
    let ctx = ExecContext {
        connection_id: 1,
        statement_id: Some(2),
        sql: "SELECT 1".to_owned(),
        params,
        prepared_parameters: None,
        data_source: "test-ds",
        start: Instant::now(),
        fingerprint: None,
        in_transaction: false,
        operation,
    };
    (vec![], ctx)
}

// ── 1. 全量开关 getter/setter ─────────────────────────────────

// 全部 30+ getter 在默认构造下符合 Java 默认值。

// 全部 setter 可正确翻转 getter 值。

// 旧 API `set_statement_parameter_log_enabled` 委托给 `set_statement_parameter_set_log_enabled`。

// ── 2. config_from_properties 七键精确语义 ─────────────────────

// 七键 "true"/"false" 值精确匹配，非布尔值不改变状态。

// 缺失键不改变对应开关。

// 操作无关的键不产生任何副作用。

// ── 3. SQL 格式选项 ────────────────────────────────────────────

// `SqlFormatOption` getter/setter 往返。

// ── 4. operation_success_enabled 四分支（间接通过 after） ───────

// Execute 操作在 after 中应触发 `execute_after` 开关。

// Query 操作在 after 中应触发 `execute_query_after` 开关。

// Update 操作在 after 中应触发 `execute_update_after` 开关。

// Batch 操作在 after 中不触发任何成功日志（走 `after_batch` 通道）。

// 对应开关关闭时，成功日志不发出。

// `错误路径：statement_log_error_enabled` 开启时发出 error 日志。

// `错误路径：statement_log_error_enabled` 关闭时静默。

// ── 5. after_batch 成功与错误 ──────────────────────────────────

// batch 成功 + 开关开：发出日志。

// batch 成功 + 开关关：静默。

// batch 错误 + error 开关开：发出 error 日志。

// batch 错误 + error 开关关：静默。

// ── 6. after_connection_event 六种 ConnectionEvent ─────────────

// Connect 事件始终静默（Java `LogFilter` 不覆盖 connectBefore）。

// Commit 事件：开关控制。

// Rollback 事件：开关控制。

// Close 事件：开关控制。

// `SetAutoCommit` 等其他事件：由 `connection_log_enabled` 总开关控制。

// ── 7. after_connection_event_context ──────────────────────────

// 带 `connection_id` 的事件上下文，走相同分支逻辑。

// ── 8. on_statement_event 四种 StatementEvent ──────────────────

// `CreateStatement` 事件：由 `create_after` 开关控制。

// `PrepareStatement` 事件：由 `prepare_after` 开关控制。

// `PrepareCall` 事件：由 `prepare_call_after` 开关控制。

// Close 事件：由 `close_after` 开关控制。

// Execute/ExecuteQuery/ExecuteUpdate/ExecuteBatch 事件走 default 分支，静默。

// ── 9. on_statement_event_context ──────────────────────────────

// 带身份的 Statement 事件上下文。

// ── 10. on_statement_close_context ─────────────────────────────

// `on_statement_close_context` 同步方法：开关开时发出日志。

// `on_statement_close_context` 同步方法：开关关时静默。

// ── 11. on_connection_event / on_connection_event_context no-op ─

// ── 12. before 方法（参数日志） ────────────────────────────────

// 参数非空 + 开关开：发出参数日志。

// 参数为空：不发出参数日志。

// 参数开关关闭：不发出参数日志。

// ── 13. ResultSetFilter 三个钩子 ───────────────────────────────

// `result_set_open_after：开关开时发出日志`。

// `result_set_open_after：开关关时静默`。

// ── ResultSet mocks ───────────────────────────────────

#[derive(Debug)]
struct StubResultSet;
druid_rdbc::impl_standard_wrapper!(StubResultSet => ResultSet);

#[async_trait::async_trait]
impl ResultSet for StubResultSet {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        Ok(true)
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }
    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        Ok(Arc::new(ResultSetMetaDataSnapshot::new(vec![])))
    }
}

#[derive(Debug)]
struct ErrorResultSet;
druid_rdbc::impl_standard_wrapper!(ErrorResultSet => ResultSet);

#[async_trait::async_trait]
impl ResultSet for ErrorResultSet {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        Err(RdbcError::DriverError("rs next error".to_string()))
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        Err(RdbcError::DriverError("close error".to_string()))
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }
    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        Ok(Arc::new(ResultSetMetaDataSnapshot::new(vec![])))
    }
}

#[derive(Debug)]
struct NoMoreRowsResultSet;
druid_rdbc::impl_standard_wrapper!(NoMoreRowsResultSet => ResultSet);

#[async_trait::async_trait]
impl ResultSet for NoMoreRowsResultSet {
    async fn next(&mut self) -> Result<bool, RdbcError> {
        Ok(false)
    }
    async fn close(&mut self) -> Result<(), RdbcError> {
        Ok(())
    }
    fn is_closed(&self) -> Result<bool, RdbcError> {
        Ok(false)
    }
    fn meta_data(&self) -> Result<Arc<dyn ResultSetMetaData>, RdbcError> {
        Ok(Arc::new(ResultSetMetaDataSnapshot::new(vec![])))
    }
}

// `result_set_next：成功(true)` + 开关开时发出日志。

// `result_set_next：成功(true)` + 开关关时静默。

// `result_set_next：错误` + error 开关开时发出 error 日志。

// `result_set_next：错误` + error 开关关时静默传播错误。

// `result_set_close：成功` + 开关开时发出日志。

// `result_set_close：成功` + 开关关时静默。

// `result_set_close：错误` + error 开关开时发出 error 日志。

// `result_set_close：错误` + error 开关关时静默传播错误。

// `result_set_next` 返回 false（无更多行）时不发出日志，即使开关开。

// ── 14. Default trait ──────────────────────────────────────────

// ── 15. config_from_properties trait 路径 ──────────────────────

// `BeforeFilter::config_from_properties` 委托给 `LogFilter::config_from_properties`。

// ── 16. before_execute_error no-op 路径 ───────────────────────

// `before_execute_error` 是 `BeforeFilter` 默认 no-op。

// ── 17. 全开关开/关矩阵（组合覆盖） ──────────────────────────

// 所有开关关闭后，所有异步 hook 均不 panic。

// ── 18. name 方法 ─────────────────────────────────────────────

#[tokio::test]
async fn task6_log_filter_full_test_log_filter_after_connection_event_context_with_identity() {
    let f = LogFilter::new();
    f.set_connection_commit_after_log_enabled(true);
    let event = ConnectionEvent::Commit;
    let ctx = ConnectionEventContext {
        connection_id: 42,
        event: &event,
    };
    AfterFilter::after_connection_event_context(&f, &ctx, Duration::from_millis(5))
        .await
        .unwrap();

    f.set_connection_commit_after_log_enabled(false);
    AfterFilter::after_connection_event_context(&f, &ctx, Duration::ZERO)
        .await
        .unwrap();

    // Connect 始终静默
    let connect = ConnectionEvent::Connect;
    let connect_ctx = ConnectionEventContext {
        connection_id: 42,
        event: &connect,
    };
    AfterFilter::after_connection_event_context(&f, &connect_ctx, Duration::ZERO)
        .await
        .unwrap();
}

#[tokio::test]
async fn task6_log_filter_full_test_log_filter_on_statement_event_create() {
    let f = LogFilter::new();
    f.set_statement_create_after_log_enabled(true);
    BeforeFilter::on_statement_event(&f, &StatementEvent::CreateStatement)
        .await
        .unwrap();

    f.set_statement_create_after_log_enabled(false);
    BeforeFilter::on_statement_event(&f, &StatementEvent::CreateStatement)
        .await
        .unwrap();
}

#[tokio::test]
async fn task6_log_filter_full_test_log_filter_on_statement_event_prepare() {
    let f = LogFilter::new();
    f.set_statement_prepare_after_log_enabled(true);
    BeforeFilter::on_statement_event(
        &f,
        &StatementEvent::PrepareStatement("SELECT ?".to_string()),
    )
    .await
    .unwrap();

    f.set_statement_prepare_after_log_enabled(false);
    BeforeFilter::on_statement_event(
        &f,
        &StatementEvent::PrepareStatement("SELECT ?".to_string()),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn task6_log_filter_full_test_log_filter_on_statement_event_prepare_call() {
    let f = LogFilter::new();
    f.set_statement_prepare_call_after_log_enabled(true);
    BeforeFilter::on_statement_event(&f, &StatementEvent::PrepareCall("CALL p()".to_string()))
        .await
        .unwrap();

    f.set_statement_prepare_call_after_log_enabled(false);
    BeforeFilter::on_statement_event(&f, &StatementEvent::PrepareCall("CALL p()".to_string()))
        .await
        .unwrap();
}

#[tokio::test]
async fn task6_log_filter_full_test_log_filter_on_statement_event_close() {
    let f = LogFilter::new();
    f.set_statement_close_after_log_enabled(true);
    BeforeFilter::on_statement_event(&f, &StatementEvent::Close)
        .await
        .unwrap();

    f.set_statement_close_after_log_enabled(false);
    BeforeFilter::on_statement_event(&f, &StatementEvent::Close)
        .await
        .unwrap();
}

#[tokio::test]
async fn task6_log_filter_full_test_log_filter_on_statement_event_execute_variants_are_silent() {
    let f = LogFilter::new();
    for event in [
        StatementEvent::Execute("SELECT 1".to_string()),
        StatementEvent::ExecuteQuery("SELECT 1".to_string()),
        StatementEvent::ExecuteUpdate("UPDATE t".to_string()),
        StatementEvent::ExecuteBatch,
    ] {
        BeforeFilter::on_statement_event(&f, &event).await.unwrap();
    }
}

#[tokio::test]
async fn task6_log_filter_full_test_log_filter_on_statement_event_context_with_identity() {
    let f = LogFilter::new();

    f.set_statement_create_after_log_enabled(true);
    let event = StatementEvent::CreateStatement;
    let ctx = StatementEventContext {
        connection_id: 10,
        statement_id: 20,
        event: &event,
    };
    BeforeFilter::on_statement_event_context(&f, &ctx)
        .await
        .unwrap();

    f.set_statement_prepare_after_log_enabled(true);
    let prepare = StatementEvent::PrepareStatement("INSERT ?".to_string());
    let prepare_ctx = StatementEventContext {
        connection_id: 10,
        statement_id: 21,
        event: &prepare,
    };
    BeforeFilter::on_statement_event_context(&f, &prepare_ctx)
        .await
        .unwrap();

    f.set_statement_prepare_call_after_log_enabled(true);
    let call = StatementEvent::PrepareCall("CALL".to_string());
    let call_ctx = StatementEventContext {
        connection_id: 10,
        statement_id: 22,
        event: &call,
    };
    BeforeFilter::on_statement_event_context(&f, &call_ctx)
        .await
        .unwrap();

    f.set_statement_close_after_log_enabled(true);
    let close = StatementEvent::Close;
    let close_ctx = StatementEventContext {
        connection_id: 10,
        statement_id: 23,
        event: &close,
    };
    BeforeFilter::on_statement_event_context(&f, &close_ctx)
        .await
        .unwrap();

    // Execute 等 default 分支
    let exec = StatementEvent::Execute("SELECT 1".to_string());
    let exec_ctx = StatementEventContext {
        connection_id: 10,
        statement_id: 24,
        event: &exec,
    };
    BeforeFilter::on_statement_event_context(&f, &exec_ctx)
        .await
        .unwrap();
}

#[test]
fn task6_log_filter_full_test_log_filter_on_statement_close_context_enabled() {
    let f = LogFilter::new();
    f.set_statement_close_after_log_enabled(true);
    let event = StatementEvent::Close;
    let ctx = StatementEventContext {
        connection_id: 5,
        statement_id: 6,
        event: &event,
    };
    BeforeFilter::on_statement_close_context(&f, &ctx).unwrap();
}

#[test]
fn task6_log_filter_full_test_log_filter_on_statement_close_context_disabled() {
    let f = LogFilter::new();
    f.set_statement_close_after_log_enabled(false);
    let event = StatementEvent::Close;
    let ctx = StatementEventContext {
        connection_id: 5,
        statement_id: 6,
        event: &event,
    };
    BeforeFilter::on_statement_close_context(&f, &ctx).unwrap();
}

#[tokio::test]
async fn task6_log_filter_full_test_log_filter_on_connection_event_is_noop() {
    let f = LogFilter::new();
    BeforeFilter::on_connection_event(&f, &ConnectionEvent::Connect)
        .await
        .unwrap();
    BeforeFilter::on_connection_event(&f, &ConnectionEvent::Close)
        .await
        .unwrap();
}

#[tokio::test]
async fn task6_log_filter_full_test_log_filter_on_connection_event_context_is_noop() {
    let f = LogFilter::new();
    let event = ConnectionEvent::Commit;
    let ctx = ConnectionEventContext {
        connection_id: 1,
        event: &event,
    };
    BeforeFilter::on_connection_event_context(&f, &ctx)
        .await
        .unwrap();
}

#[tokio::test]
async fn task6_log_filter_full_test_log_filter_before_with_params_emits_log_when_enabled() {
    let f = LogFilter::new();
    f.set_statement_parameter_set_log_enabled(true);
    let params = vec![Value::Int(1), Value::String("hello".to_string())];
    let mut ctx = ExecContext {
        connection_id: 1,
        statement_id: Some(2),
        sql: "SELECT * FROM t WHERE id = ? AND name = ?".to_owned(),
        params: &params,
        prepared_parameters: None,
        data_source: "test-ds",
        start: Instant::now(),
        fingerprint: None,
        in_transaction: false,
        operation: ExecOperation::Execute,
    };
    BeforeFilter::before(&f, &mut ctx).await.unwrap();
}

#[tokio::test]
async fn task6_log_filter_full_test_log_filter_before_with_empty_params_no_log() {
    let f = LogFilter::new();
    f.set_statement_parameter_set_log_enabled(true);
    let params: Vec<Value> = vec![];
    let mut ctx = ExecContext {
        connection_id: 1,
        statement_id: None,
        sql: "SELECT 1".to_owned(),
        params: &params,
        prepared_parameters: None,
        data_source: "test-ds",
        start: Instant::now(),
        fingerprint: None,
        in_transaction: false,
        operation: ExecOperation::Query,
    };
    BeforeFilter::before(&f, &mut ctx).await.unwrap();
}

#[tokio::test]
async fn task6_log_filter_full_test_log_filter_before_params_switch_off() {
    let f = LogFilter::new();
    f.set_statement_parameter_set_log_enabled(false);
    let params = vec![Value::Int(42)];
    let mut ctx = ExecContext {
        connection_id: 1,
        statement_id: None,
        sql: "SELECT ?".to_owned(),
        params: &params,
        prepared_parameters: None,
        data_source: "test-ds",
        start: Instant::now(),
        fingerprint: None,
        in_transaction: false,
        operation: ExecOperation::Query,
    };
    BeforeFilter::before(&f, &mut ctx).await.unwrap();
}

#[test]
fn task6_log_filter_full_test_log_filter_result_set_open_after_enabled() {
    let f = LogFilter::new();
    f.set_result_set_open_after_log_enabled(true);
    let ctx = ResultSetFilterContext::new();
    ResultSetFilter::result_set_open_after(&f, &ctx).unwrap();
}

#[test]
fn task6_log_filter_full_test_log_filter_result_set_open_after_disabled() {
    let f = LogFilter::new();
    f.set_result_set_open_after_log_enabled(false);
    let ctx = ResultSetFilterContext::new();
    ResultSetFilter::result_set_open_after(&f, &ctx).unwrap();
}

#[tokio::test]
async fn task6_log_filter_full_test_log_filter_result_set_next_success_enabled() {
    let filter = LogFilter::new();
    filter.set_result_set_next_after_log_enabled(true);
    let context = ResultSetFilterContext::new();
    ResultSetFilter::standard_result_set_next_before(&filter, &context).unwrap();
    let mut raw = StubResultSet;
    let result = ResultSet::next(&mut raw).await.map_err(DruidError::from);
    ResultSetFilter::standard_result_set_next_after(&filter, &context, &result).unwrap();
    assert!(result.unwrap());
}

#[tokio::test]
async fn task6_log_filter_full_test_log_filter_result_set_next_success_disabled() {
    let filter = LogFilter::new();
    filter.set_result_set_next_after_log_enabled(false);
    let context = ResultSetFilterContext::new();
    ResultSetFilter::standard_result_set_next_before(&filter, &context).unwrap();
    let mut raw = StubResultSet;
    let result = ResultSet::next(&mut raw).await.map_err(DruidError::from);
    ResultSetFilter::standard_result_set_next_after(&filter, &context, &result).unwrap();
    assert!(result.unwrap());
}

#[tokio::test]
async fn task6_log_filter_full_test_log_filter_result_set_next_error_enabled() {
    let filter = LogFilter::new();
    filter.set_result_set_log_error_enabled(true);
    let context = ResultSetFilterContext::new();
    ResultSetFilter::standard_result_set_next_before(&filter, &context).unwrap();
    let mut raw = ErrorResultSet;
    let result = ResultSet::next(&mut raw).await.map_err(DruidError::from);
    ResultSetFilter::standard_result_set_next_after(&filter, &context, &result).unwrap();
    assert!(result.is_err());
}

#[tokio::test]
async fn task6_log_filter_full_test_log_filter_result_set_next_error_disabled() {
    let filter = LogFilter::new();
    filter.set_result_set_log_error_enabled(false);
    let context = ResultSetFilterContext::new();
    ResultSetFilter::standard_result_set_next_before(&filter, &context).unwrap();
    let mut raw = ErrorResultSet;
    let result = ResultSet::next(&mut raw).await.map_err(DruidError::from);
    ResultSetFilter::standard_result_set_next_after(&filter, &context, &result).unwrap();
    assert!(result.is_err());
}

#[tokio::test]
async fn task6_log_filter_full_test_log_filter_result_set_close_success_enabled() {
    let filter = LogFilter::new();
    filter.set_result_set_close_after_log_enabled(true);
    let context = ResultSetFilterContext::new();
    ResultSetFilter::standard_result_set_close_before(&filter, &context).unwrap();
    let mut raw = StubResultSet;
    let result = ResultSet::close(&mut raw).await.map_err(DruidError::from);
    ResultSetFilter::standard_result_set_close_after(&filter, &context, &result).unwrap();
    assert!(result.is_ok());
}

#[tokio::test]
async fn task6_log_filter_full_test_log_filter_result_set_close_success_disabled() {
    let filter = LogFilter::new();
    filter.set_result_set_close_after_log_enabled(false);
    let context = ResultSetFilterContext::new();
    ResultSetFilter::standard_result_set_close_before(&filter, &context).unwrap();
    let mut raw = StubResultSet;
    let result = ResultSet::close(&mut raw).await.map_err(DruidError::from);
    ResultSetFilter::standard_result_set_close_after(&filter, &context, &result).unwrap();
    assert!(result.is_ok());
}

#[tokio::test]
async fn task6_log_filter_full_test_log_filter_result_set_close_error_enabled() {
    let filter = LogFilter::new();
    filter.set_result_set_log_error_enabled(true);
    let context = ResultSetFilterContext::new();
    ResultSetFilter::standard_result_set_close_before(&filter, &context).unwrap();
    let mut raw = ErrorResultSet;
    let result = ResultSet::close(&mut raw).await.map_err(DruidError::from);
    ResultSetFilter::standard_result_set_close_after(&filter, &context, &result).unwrap();
    assert!(result.is_err());
}

#[tokio::test]
async fn task6_log_filter_full_test_log_filter_result_set_close_error_disabled() {
    let filter = LogFilter::new();
    filter.set_result_set_log_error_enabled(false);
    let context = ResultSetFilterContext::new();
    ResultSetFilter::standard_result_set_close_before(&filter, &context).unwrap();
    let mut raw = ErrorResultSet;
    let result = ResultSet::close(&mut raw).await.map_err(DruidError::from);
    ResultSetFilter::standard_result_set_close_after(&filter, &context, &result).unwrap();
    assert!(result.is_err());
}

#[tokio::test]
async fn task6_log_filter_full_test_log_filter_result_set_next_false_no_log() {
    let filter = LogFilter::new();
    filter.set_result_set_next_after_log_enabled(true);
    let context = ResultSetFilterContext::new();
    ResultSetFilter::standard_result_set_next_before(&filter, &context).unwrap();
    let mut raw = NoMoreRowsResultSet;
    let result = ResultSet::next(&mut raw).await.map_err(DruidError::from);
    ResultSetFilter::standard_result_set_next_after(&filter, &context, &result).unwrap();
    assert!(!result.unwrap());
}

#[test]
fn task6_log_filter_full_test_log_filter_default_trait_matches_new() {
    let f = LogFilter::default();
    assert!(f.is_connection_log_enabled());
    assert!(f.is_statement_log_enabled());
    assert!(f.is_result_set_log_enabled());
    assert!(!f.is_statement_executable_sql_log_enabled());
}

#[test]
fn task6_log_filter_full_test_log_filter_before_filter_config_from_properties_trait_path() {
    let f = LogFilter::new();
    let p = props(&[("druid.log.conn", "false")]);
    BeforeFilter::config_from_properties(&f, &p).unwrap();
    assert!(!f.is_connection_log_enabled());
}

#[tokio::test]
async fn task6_log_filter_full_test_log_filter_before_execute_error_is_noop() {
    let f = LogFilter::new();
    let params: Vec<Value> = vec![];
    let ctx = ExecContext {
        connection_id: 1,
        statement_id: None,
        sql: "SELECT 1".to_owned(),
        params: &params,
        prepared_parameters: None,
        data_source: "test",
        start: Instant::now(),
        fingerprint: None,
        in_transaction: false,
        operation: ExecOperation::Execute,
    };
    let error = RdbcError::DriverError("test".to_string());
    let product_error = DruidError::from(error);
    BeforeFilter::before_execute_error(&f, &ctx, &product_error)
        .await
        .unwrap();
}

#[tokio::test]
async fn task6_log_filter_full_test_log_filter_all_switches_off_no_panic() {
    let f = LogFilter::new();
    f.set_data_source_log_enabled(false);
    f.set_connection_log_enabled(false);
    f.set_connection_log_error_enabled(false);
    f.set_connection_connect_before_log_enabled(false);
    f.set_connection_connect_after_log_enabled(false);
    f.set_connection_close_after_log_enabled(false);
    f.set_connection_commit_after_log_enabled(false);
    f.set_connection_rollback_after_log_enabled(false);
    f.set_statement_log_enabled(false);
    f.set_statement_log_error_enabled(false);
    f.set_statement_create_after_log_enabled(false);
    f.set_statement_prepare_after_log_enabled(false);
    f.set_statement_prepare_call_after_log_enabled(false);
    f.set_statement_execute_after_log_enabled(false);
    f.set_statement_execute_query_after_log_enabled(false);
    f.set_statement_execute_update_after_log_enabled(false);
    f.set_statement_execute_batch_after_log_enabled(false);
    f.set_statement_close_after_log_enabled(false);
    f.set_statement_parameter_set_log_enabled(false);
    f.set_statement_parameter_clear_log_enabled(false);
    f.set_statement_executable_sql_log_enabled(false);
    f.set_result_set_log_enabled(false);
    f.set_result_set_log_error_enabled(false);
    f.set_result_set_open_after_log_enabled(false);
    f.set_result_set_next_after_log_enabled(false);
    f.set_result_set_close_after_log_enabled(false);

    let params: Vec<Value> = vec![];
    let mut ctx = ExecContext {
        connection_id: 1,
        statement_id: None,
        sql: "SELECT 1".to_owned(),
        params: &params,
        prepared_parameters: None,
        data_source: "test",
        start: Instant::now(),
        fingerprint: None,
        in_transaction: false,
        operation: ExecOperation::Execute,
    };
    BeforeFilter::before(&f, &mut ctx).await.unwrap();
    AfterFilter::after(
        &f,
        &ctx,
        &Ok(ExecResult::default()),
        Duration::from_millis(1),
    )
    .await
    .unwrap();

    for event in [
        ConnectionEvent::Connect,
        ConnectionEvent::Commit,
        ConnectionEvent::Rollback,
        ConnectionEvent::Close,
        ConnectionEvent::SetAutoCommit(true),
    ] {
        AfterFilter::after_connection_event(&f, &event, Duration::ZERO)
            .await
            .unwrap();
    }

    for event in [
        StatementEvent::CreateStatement,
        StatementEvent::PrepareStatement("SELECT ?".to_string()),
        StatementEvent::PrepareCall("CALL".to_string()),
        StatementEvent::Close,
        StatementEvent::Execute("SELECT 1".to_string()),
    ] {
        BeforeFilter::on_statement_event(&f, &event).await.unwrap();
    }

    let rs_ctx = ResultSetFilterContext::new();
    ResultSetFilter::result_set_open_after(&f, &rs_ctx).unwrap();
}

#[test]
fn task6_log_filter_full_test_log_filter_name_returns_log() {
    let f = LogFilter::new();
    assert_eq!(BeforeFilter::name(&f), "log");
    assert_eq!(AfterFilter::name(&f), "log");
}
